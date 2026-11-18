//! Ejecuta un `OutputPlan` en la máquina: cambios de tinta, esperas de secado, viajes
//! con la pluma arriba y trazos con la pluma abajo. Comprueba el recorrido antes de
//! mover nada, y atiende pausa y parada entre tramos, como mucho 40 ms después.

use super::{
    ebb::{Ebb, Link},
    plan::{self, Kinematics, Limits, Move},
    MachineError,
};
use crate::output::{OutputPlan, OutputStep};
use crate::{p, PlotSettings, P};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineSettings {
    /// Pasos de motor por mm a 1/16 de paso (80 en AxiDraw y MakerQ).
    pub steps_per_mm: f64,
    /// Recorrido útil del carro desde su origen, mm (ancho, alto).
    pub travel_mm: [f64; 2],
    /// Dónde cae la esquina superior izquierda del papel respecto al origen del carro, mm.
    pub paper_origin_mm: [f64; 2],
    /// Alturas de la pluma en % de la escala del servo de AxiDraw. En la MakerQ la palanca
    /// sube poco: 85 % arriba despega bien el boli fijado con el servo al 42 %.
    pub pen_up_pct: f64,
    pub pen_down_pct: f64,
    /// Aceleración, mm/s².
    pub accel: f64,
    /// Desviación de esquina, mm.
    pub junction_mm: f64,
    /// Tope de velocidad, mm/s, por encima de lo que pida el documento.
    pub max_speed: f64,
    /// Volver al origen al terminar.
    pub return_home: bool,
    /// Soltar los motores al volver al origen: parados siguen haciendo fuerza, zumban y
    /// se calientan. La siguiente orden de movimiento los activa otra vez, tomando la
    /// posición del carro como origen.
    pub release_when_done: bool,
}

impl Default for MachineSettings {
    fn default() -> Self {
        // Recorrido de la MakerQ: 317 mm a lo ancho y 440 mm hacia el usuario (cabe un
        // A3 en vertical). En una AxiDraw V3 son 300 × 218.
        MachineSettings { steps_per_mm: 80.0, travel_mm: [317.0, 440.0], paper_origin_mm: [0.0, 0.0],
            pen_up_pct: 85.0, pen_down_pct: 42.0, accel: 1000.0, junction_mm: 0.03, max_speed: 200.0, return_home: true,
            release_when_done: true }
    }
}

impl MachineSettings {
    pub fn valid(&self) -> bool {
        let pos = [self.steps_per_mm, self.travel_mm[0], self.travel_mm[1], self.accel, self.max_speed];
        pos.iter().all(|v| v.is_finite() && *v > 0.0)
            && self.paper_origin_mm.iter().all(|v| v.is_finite())
            && [self.pen_up_pct, self.pen_down_pct].iter().all(|v| (0.0..=100.0).contains(v))
            && self.junction_mm.is_finite() && self.junction_mm >= 0.0
    }

    pub fn kinematics(&self) -> Kinematics { Kinematics { steps_per_mm: self.steps_per_mm } }

    /// Punto del papel en coordenadas de máquina.
    pub fn to_machine(&self, q: P) -> P { p(q.x + self.paper_origin_mm[0], q.y + self.paper_origin_mm[1]) }

    pub fn inside(&self, q: P) -> bool {
        const EPS: f64 = 1e-6;
        q.x >= -EPS && q.y >= -EPS && q.x <= self.travel_mm[0] + EPS && q.y <= self.travel_mm[1] + EPS
    }

    fn limits(&self, speed: f64) -> Limits {
        Limits { speed: speed.min(self.max_speed), accel: self.accel, junction_mm: self.junction_mm }
    }
}

/// Pausa y parada desde otro hilo (la interfaz).
#[derive(Debug, Default)]
pub struct Control {
    stop: AtomicBool,
    pause: AtomicBool,
}

impl Control {
    pub fn stop(&self) { self.stop.store(true, Ordering::Relaxed); }
    pub fn pause(&self) { self.pause.store(true, Ordering::Relaxed); }
    pub fn resume(&self) { self.pause.store(false, Ordering::Relaxed); }
    pub fn is_paused(&self) -> bool { self.pause.load(Ordering::Relaxed) }
    pub fn is_stopped(&self) -> bool { self.stop.load(Ordering::Relaxed) }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Empieza el paso `index` de `total` del plan.
    Step { index: usize, total: usize },
    /// Pluma arriba y quieta: hay que poner la tinta `pen` y reanudar con `Control::resume`.
    PenChange { pen: usize },
    /// Se pulsó el botón de la placa: queda en pausa, pluma arriba. Se sigue con
    /// `Control::resume` o se acaba con `Control::stop`.
    Button,
    Paused,
    Resumed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Finished,
    Stopped,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flow {
    Go,
    Stop,
}

/// Cada cuánto se consulta el botón de la placa durante el movimiento, ms de tramos.
const BUTTON_EVERY_MS: u32 = 200;

/// Máquina conectada. Su origen es donde estaba el carro al conectar o al llamar a
/// `set_origin_here`; desde ahí se lleva la cuenta exacta de pasos.
pub struct Plotter<L> {
    ebb: Ebb<L>,
    settings: MachineSettings,
    kin: Kinematics,
    /// Posición en pasos de motor desde el origen.
    pos: (i64, i64),
    pen_up: bool,
    /// Motores activados; sueltos, la posición del carro ya no es segura.
    engaged: bool,
    /// Ms de tramos enviados desde la última consulta del botón.
    since_button: u32,
    version: String,
}

impl<L: Link> Plotter<L> {
    /// Comprueba la placa, sube la pluma, activa los motores y toma la posición actual
    /// del carro como origen.
    pub fn connect(link: L, settings: MachineSettings) -> Result<Self, MachineError> {
        let mut plotter = Self::attach(link, settings)?;
        plotter.set_origin_here()?;
        Ok(plotter)
    }

    /// Como `connect`, pero deja los motores sueltos para colocar el carro a mano: el
    /// origen se toma al primer movimiento.
    pub fn attach(link: L, settings: MachineSettings) -> Result<Self, MachineError> {
        if !settings.valid() { return Err(MachineError::InvalidSettings); }
        let mut ebb = Ebb::new(link);
        let version = ebb.version()?;
        let kin = settings.kinematics();
        let mut plotter = Plotter { ebb, settings, kin, pos: (0, 0), pen_up: false, engaged: false, since_button: 0, version };
        plotter.apply_pen_settings()?;
        plotter.set_pen(true, 300)?;
        plotter.wait_idle()?;
        plotter.ebb.motors(false)?;
        Ok(plotter)
    }

    pub fn version(&self) -> &str { &self.version }

    pub fn settings(&self) -> &MachineSettings { &self.settings }

    pub fn ebb(&mut self) -> &mut Ebb<L> { &mut self.ebb }

    /// Posición del carro en mm desde el origen.
    pub fn position(&self) -> P { self.kin.point(self.pos) }

    pub fn pen_is_up(&self) -> bool { self.pen_up }

    pub fn update_settings(&mut self, settings: MachineSettings) -> Result<(), MachineError> {
        if !settings.valid() { return Err(MachineError::InvalidSettings); }
        if settings.steps_per_mm != self.settings.steps_per_mm {
            // La posición en mm se conserva aunque cambie la escala.
            let here = self.position();
            self.kin = settings.kinematics();
            self.pos = self.kin.motors(here);
        }
        self.settings = settings;
        self.apply_pen_settings()
    }

    fn apply_pen_settings(&mut self) -> Result<(), MachineError> {
        self.ebb.configure_pen(self.settings.pen_up_pct, self.settings.pen_down_pct)
    }

    /// Activa los motores y toma la posición actual como origen.
    pub fn set_origin_here(&mut self) -> Result<(), MachineError> {
        self.wait_idle()?;
        self.ebb.motors(true)?;
        self.ebb.clear_steps()?;
        self.pos = (0, 0);
        self.engaged = true;
        Ok(())
    }

    /// Activa los motores si estaban sueltos, tomando la posición actual como origen.
    fn engage(&mut self) -> Result<(), MachineError> {
        if self.engaged { Ok(()) } else { self.set_origin_here() }
    }

    pub fn is_engaged(&self) -> bool { self.engaged }

    /// Sube la pluma y suelta los motores para mover el carro a mano. Después hay que
    /// volver a fijar el origen con `set_origin_here`.
    pub fn release(&mut self) -> Result<(), MachineError> {
        self.set_pen(true, 300)?;
        self.wait_idle()?;
        self.ebb.motors(false)?;
        self.engaged = false;
        Ok(())
    }

    pub fn set_pen(&mut self, up: bool, delay_ms: u32) -> Result<(), MachineError> {
        if self.pen_up == up { return Ok(()); }
        self.ebb.pen(up, delay_ms)?;
        self.pen_up = up;
        Ok(())
    }

    /// Lleva la pluma a su altura aunque ya estuviera arriba o abajo: tras cambiar la
    /// altura, `set_pen` no la movería.
    pub fn move_pen(&mut self, up: bool, delay_ms: u32) -> Result<(), MachineError> {
        self.ebb.pen(up, delay_ms)?;
        self.pen_up = up;
        Ok(())
    }

    /// Espera a que la placa termine todos los movimientos encolados.
    pub fn wait_idle(&mut self) -> Result<(), MachineError> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if self.ebb.motion()?.idle() { return Ok(()); }
            if Instant::now() > deadline { return Err(MachineError::Timeout("QM".into())); }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Viaje con la pluma arriba a un punto de máquina dentro del recorrido.
    pub fn travel_to(&mut self, target: P, speed: f64, control: &Control, on_event: &mut dyn FnMut(Event))
        -> Result<Outcome, MachineError> {
        if !self.settings.inside(target) { return Err(MachineError::OutOfBounds); }
        self.engage()?;
        self.clear_button()?;
        let lift_ms = 300;
        let flow = self.travel(target, &self.settings.limits(speed), lift_ms, control, on_event)?;
        self.wait_idle()?;
        Ok(if flow == Flow::Go { Outcome::Finished } else { Outcome::Stopped })
    }

    pub fn go_home(&mut self, speed: f64, control: &Control, on_event: &mut dyn FnMut(Event)) -> Result<Outcome, MachineError> {
        self.travel_to(p(0.0, 0.0), speed, control, on_event)
    }

    /// Comprueba que todo el plan cabe en el recorrido del carro.
    pub fn check_bounds(&self, out: &OutputPlan) -> Result<(), MachineError> {
        let s = &self.settings;
        let strokes = out.strokes.iter().flat_map(|st| st.path.iter().copied());
        let stops = out.steps.iter().filter_map(|st| match *st {
            OutputStep::Travel { to, .. } => Some(to),
            OutputStep::Wait { at, .. } => Some(at),
            _ => None,
        });
        if strokes.chain(stops).all(|q| q.x.is_finite() && q.y.is_finite() && s.inside(s.to_machine(q))) { Ok(()) }
        else { Err(MachineError::OutOfBounds) }
    }

    /// Traza el plan entero. Devuelve `Stopped` si se paró desde `control`; la pluma
    /// queda arriba y el carro donde estaba.
    pub fn execute(&mut self, out: &OutputPlan, motion: &PlotSettings, control: &Control, on_event: &mut dyn FnMut(Event))
        -> Result<Outcome, MachineError> {
        self.check_bounds(out)?;
        self.engage()?;
        self.clear_button()?;
        let draw = self.settings.limits(motion.draw_speed);
        let travel = self.settings.limits(motion.travel_speed);
        if !draw.valid() || !travel.valid() || !motion.lift_time.is_finite() { return Err(MachineError::InvalidSettings); }
        let lift_ms = (motion.lift_time * 1000.0).round().clamp(0.0, 5000.0) as u32;
        let total = out.steps.len();
        for (index, step) in out.steps.iter().enumerate() {
            on_event(Event::Step { index, total });
            let flow = match *step {
                OutputStep::ChangePen { pen, .. } => {
                    self.set_pen(true, lift_ms)?;
                    self.wait_idle()?;
                    // La pausa va antes del aviso: quien lo recibe puede reanudar al momento.
                    control.pause();
                    on_event(Event::PenChange { pen });
                    self.hold(control, on_event, lift_ms)?
                }
                OutputStep::Travel { to, .. } => self.travel(self.settings.to_machine(to), &travel, lift_ms, control, on_event)?,
                OutputStep::Wait { start_s, end_s, .. } => {
                    self.set_pen(true, lift_ms)?;
                    self.dwell(end_s - start_s, control, on_event, lift_ms)?
                }
                OutputStep::Draw { stroke, .. } => {
                    let path: Vec<P> = out.strokes[stroke].path.iter().map(|&q| self.settings.to_machine(q)).collect();
                    self.draw(&path, &draw, &travel, lift_ms, control, on_event)?
                }
            };
            if flow == Flow::Stop { return self.stopped(lift_ms); }
        }
        self.set_pen(true, lift_ms)?;
        if self.settings.return_home && self.travel(p(0.0, 0.0), &travel, lift_ms, control, on_event)? == Flow::Stop {
            return self.stopped(lift_ms);
        }
        self.wait_idle()?;
        if self.settings.return_home && self.settings.release_when_done { self.release()?; }
        Ok(Outcome::Finished)
    }

    /// Olvida una pulsación del botón anterior a esta orden.
    fn clear_button(&mut self) -> Result<(), MachineError> {
        self.ebb.button()?;
        self.since_button = 0;
        Ok(())
    }

    fn stopped(&mut self, lift_ms: u32) -> Result<Outcome, MachineError> {
        self.wait_idle()?;
        self.set_pen(true, lift_ms)?;
        self.wait_idle()?;
        Ok(Outcome::Stopped)
    }

    fn travel(&mut self, target: P, lim: &Limits, lift_ms: u32, control: &Control, on_event: &mut dyn FnMut(Event))
        -> Result<Flow, MachineError> {
        if self.kin.motors(target) == self.pos { return Ok(Flow::Go); }
        self.set_pen(true, lift_ms)?;
        let moves = plan::plan(&[self.position(), target], lim, &self.kin, self.pos);
        self.run(&moves, control, on_event, lift_ms)
    }

    fn draw(&mut self, path: &[P], draw: &Limits, travel: &Limits, lift_ms: u32, control: &Control,
        on_event: &mut dyn FnMut(Event)) -> Result<Flow, MachineError> {
        let Some(&start) = path.first() else { return Ok(Flow::Go) };
        // Un trazo que empieza donde acabó el anterior sigue sin levantar la pluma.
        if self.kin.motors(start) != self.pos && self.travel(start, travel, lift_ms, control, on_event)? == Flow::Stop {
            return Ok(Flow::Stop);
        }
        self.set_pen(false, lift_ms)?;
        let moves = plan::plan(path, draw, &self.kin, self.pos);
        self.run(&moves, control, on_event, lift_ms)
    }

    /// Espera de secado con la pluma arriba, en tramos cortos para poder parar.
    fn dwell(&mut self, seconds: f64, control: &Control, on_event: &mut dyn FnMut(Event), lift_ms: u32)
        -> Result<Flow, MachineError> {
        let mut left = (seconds.max(0.0) * 1000.0).round() as u32;
        while left > 0 {
            let ms = left.min(250);
            if self.run(&[Move { ms, m1: 0, m2: 0 }], control, on_event, lift_ms)? == Flow::Stop { return Ok(Flow::Stop); }
            left -= ms;
        }
        Ok(Flow::Go)
    }

    fn run(&mut self, moves: &[Move], control: &Control, on_event: &mut dyn FnMut(Event), lift_ms: u32)
        -> Result<Flow, MachineError> {
        for &mv in moves {
            if self.since_button >= BUTTON_EVERY_MS {
                self.since_button = 0;
                if self.ebb.button()? {
                    control.pause();
                    on_event(Event::Button);
                }
            }
            if control.is_stopped() { return Ok(Flow::Stop); }
            if control.is_paused() && self.hold(control, on_event, lift_ms)? == Flow::Stop { return Ok(Flow::Stop); }
            self.ebb.step(mv)?;
            self.since_button = self.since_button.saturating_add(mv.ms);
            self.pos = (self.pos.0 + mv.m1, self.pos.1 + mv.m2);
        }
        Ok(Flow::Go)
    }

    /// Para con la pluma arriba hasta que se reanude o se pare.
    fn hold(&mut self, control: &Control, on_event: &mut dyn FnMut(Event), lift_ms: u32) -> Result<Flow, MachineError> {
        let was_down = !self.pen_up;
        self.wait_idle()?;
        self.set_pen(true, lift_ms)?;
        on_event(Event::Paused);
        while control.is_paused() {
            if control.is_stopped() { return Ok(Flow::Stop); }
            std::thread::sleep(Duration::from_millis(30));
        }
        if control.is_stopped() { return Ok(Flow::Stop); }
        if was_down { self.set_pen(false, lift_ms)?; }
        on_event(Event::Resumed);
        Ok(Flow::Go)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Reply;
    use crate::output::{prepare, OutputSettings};
    use crate::{Paper, Pen, PlotLayer};
    use std::sync::{Arc, Mutex};

    /// Placa simulada: integra los pasos y registra las órdenes.
    #[derive(Default)]
    struct Sim {
        log: Vec<String>,
        pos: (i64, i64),
        pen_up: bool,
        /// Posiciones (pasos) con la pluma abajo, para comprobar qué se dibuja.
        inked: Vec<(i64, i64)>,
        stop_after_moves: Option<(usize, Arc<Control>)>,
        moves: usize,
        /// Se pulsa el botón al llegar a este número de tramos.
        press_at_move: Option<usize>,
    }

    #[derive(Clone, Default)]
    struct SimLink(Arc<Mutex<Sim>>);

    impl Link for SimLink {
        fn exchange(&mut self, cmd: &str, _reply: Reply) -> Result<String, MachineError> {
            let mut s = self.0.lock().unwrap();
            s.log.push(cmd.to_string());
            let f: Vec<&str> = cmd.split(',').collect();
            Ok(match f[0] {
                "V" => "EBBv13_and_above EB Firmware Version 2.8.1".into(),
                "QM" => "QM,0,0,0,0".into(),
                "QS" => format!("{},{}", s.pos.0, s.pos.1),
                "QP" => if s.pen_up { "1" } else { "0" }.into(),
                "CS" => { s.pos = (0, 0); String::new() }
                "SP" => { s.pen_up = f[1] == "1"; String::new() }
                "SM" => {
                    let (a, b): (i64, i64) = (f[2].parse().unwrap(), f[3].parse().unwrap());
                    assert!(f[1].parse::<u32>().unwrap() >= 1);
                    s.pos = (s.pos.0 + a, s.pos.1 + b);
                    if !s.pen_up { let at = s.pos; s.inked.push(at); }
                    s.moves += 1;
                    if let Some((n, c)) = &s.stop_after_moves { if s.moves == *n { c.stop(); } }
                    String::new()
                }
                "QB" => {
                    let pressed = s.press_at_move.is_some_and(|n| s.moves >= n);
                    if pressed { s.press_at_move = None; }
                    if pressed { "1" } else { "0" }.into()
                }
                "SC" | "EM" => String::new(),
                other => return Err(MachineError::Device(format!("!8 Err: {other}"))),
            })
        }
    }

    fn sample_plan() -> (OutputPlan, PlotSettings) {
        let paper = Paper { name: "Prueba".into(), width_mm: 100.0, height_mm: 80.0, margin_mm: 5.0 };
        let pens = vec![Pen::new("Negro", [0, 0, 0], 0.5), Pen::new("Rojo", [200, 0, 0], 0.5)];
        let layers = vec![
            PlotLayer { pen: 0, paths: vec![vec![p(10.0, 10.0), p(40.0, 10.0), p(40.0, 30.0)], vec![p(50.0, 50.0), p(60.0, 55.0)]] },
            PlotLayer { pen: 1, paths: vec![vec![p(20.0, 60.0), p(30.0, 70.0)]] },
        ];
        let motion = PlotSettings::default();
        let cfg = OutputSettings { drying: false, ..Default::default() };
        (prepare(&layers, &paper, &pens, &motion, &cfg).unwrap(), motion)
    }

    #[test]
    fn executes_plan_and_returns_home() {
        let link = SimLink::default();
        let mut plotter = Plotter::connect(link.clone(), MachineSettings::default()).unwrap();
        let (out, motion) = sample_plan();
        let control = Control::default();
        let mut events = Vec::new();
        let mut on_event = |e: Event| {
            if let Event::PenChange { .. } = e { control.resume(); }
            events.push(e);
        };
        let outcome = plotter.execute(&out, &motion, &control, &mut on_event).unwrap();
        assert_eq!(outcome, Outcome::Finished);
        let s = link.0.lock().unwrap();
        assert_eq!(s.pos, (0, 0), "vuelve al origen");
        assert!(s.pen_up);
        assert_eq!(s.log.last().map(String::as_str), Some("EM,0,0"), "suelta los motores al acabar");
        assert!(!plotter.is_engaged());
        // Tres trazos separados: la pluma baja tres veces.
        assert_eq!(s.log.iter().filter(|c| c.starts_with("SP,0")).count(), 3);
        let kin = plotter.settings().kinematics();
        for target in [p(40.0, 10.0), p(40.0, 30.0), p(60.0, 55.0), p(30.0, 70.0)] {
            assert!(s.inked.contains(&kin.motors(target)), "no llega a dibujar {target:?}");
        }
        // Nada se dibuja fuera de los trazos.
        for &at in &s.inked {
            let q = kin.point(at);
            assert!(q.x >= 10.0 - 0.05 && q.y >= 10.0 - 0.05, "tinta en {q:?}");
        }
        let changes: Vec<usize> = events.iter().filter_map(|e| if let Event::PenChange { pen } = e { Some(*pen) } else { None }).collect();
        assert_eq!(changes.last(), Some(&1));
    }

    #[test]
    fn second_run_engages_motors_again() {
        let link = SimLink::default();
        let mut plotter = Plotter::connect(link.clone(), MachineSettings::default()).unwrap();
        let (out, motion) = sample_plan();
        let control = Control::default();
        let mut on_event = |e: Event| if let Event::PenChange { .. } = e { control.resume() };
        plotter.execute(&out, &motion, &control, &mut on_event).unwrap();
        let before = link.0.lock().unwrap().log.len();
        plotter.execute(&out, &motion, &control, &mut on_event).unwrap();
        let s = link.0.lock().unwrap();
        let second = &s.log[before..];
        let first_move = second.iter().position(|c| c.starts_with("SM")).unwrap();
        assert!(second[..first_move].iter().any(|c| c == "EM,1,1"), "activa los motores antes de moverse");
        assert_eq!(s.pos, (0, 0));
    }

    #[test]
    fn board_button_pauses_and_can_stop() {
        let link = SimLink::default();
        link.0.lock().unwrap().press_at_move = Some(30);
        let mut plotter = Plotter::connect(link.clone(), MachineSettings::default()).unwrap();
        let (out, motion) = sample_plan();
        let control = Control::default();
        let mut buttons = 0;
        let r = plotter.execute(&out, &motion, &control, &mut |e| match e {
            Event::PenChange { .. } => control.resume(),
            Event::Button => { buttons += 1; control.stop() }
            _ => {}
        }).unwrap();
        assert_eq!(r, Outcome::Stopped);
        assert_eq!(buttons, 1);
        let s = link.0.lock().unwrap();
        assert!(s.pen_up);
        // Se consulta cada 200 ms de tramos de 20 ms: reacciona en menos de 12 tramos.
        assert!(s.moves < 30 + 12, "{} tramos", s.moves);
    }

    #[test]
    fn earlier_button_press_is_ignored() {
        let link = SimLink::default();
        link.0.lock().unwrap().press_at_move = Some(0);
        let mut plotter = Plotter::connect(link.clone(), MachineSettings::default()).unwrap();
        let (out, motion) = sample_plan();
        let control = Control::default();
        let r = plotter.execute(&out, &motion, &control, &mut |e| match e {
            Event::PenChange { .. } => control.resume(),
            Event::Button => control.stop(),
            _ => {}
        });
        assert_eq!(r, Ok(Outcome::Finished));
    }

    #[test]
    fn attach_leaves_motors_free_until_the_first_move() {
        let link = SimLink::default();
        let mut plotter = Plotter::attach(link.clone(), MachineSettings::default()).unwrap();
        assert!(!plotter.is_engaged());
        assert_eq!(link.0.lock().unwrap().log.last().map(String::as_str), Some("EM,0,0"));
        // El carro se ha movido a mano: el origen se toma al empezar.
        link.0.lock().unwrap().pos = (5000, -300);
        let (out, motion) = sample_plan();
        let control = Control::default();
        let outcome = plotter.execute(&out, &motion, &control, &mut |e| if let Event::PenChange { .. } = e { control.resume() }).unwrap();
        assert_eq!(outcome, Outcome::Finished);
        let log = link.0.lock().unwrap().log.clone();
        let cs = log.iter().position(|c| c == "CS").expect("origen al empezar");
        assert_eq!(log[cs - 1], "EM,1,1");
        assert_eq!(link.0.lock().unwrap().pos, (0, 0));
    }

    #[test]
    fn rejects_plan_outside_travel() {
        let link = SimLink::default();
        let settings = MachineSettings { travel_mm: [50.0, 50.0], ..Default::default() };
        let mut plotter = Plotter::connect(link.clone(), settings).unwrap();
        let (out, motion) = sample_plan();
        let r = plotter.execute(&out, &motion, &Control::default(), &mut |_| {});
        assert_eq!(r, Err(MachineError::OutOfBounds));
        assert!(!link.0.lock().unwrap().log.iter().any(|c| c.starts_with("SM")), "no se mueve");
    }

    #[test]
    fn stop_leaves_pen_up_and_position_known() {
        let link = SimLink::default();
        let control = Arc::new(Control::default());
        link.0.lock().unwrap().stop_after_moves = Some((40, control.clone()));
        let mut plotter = Plotter::connect(link.clone(), MachineSettings::default()).unwrap();
        let (out, motion) = sample_plan();
        let r = plotter.execute(&out, &motion, &control, &mut |e| if let Event::PenChange { .. } = e { control.resume() }).unwrap();
        assert_eq!(r, Outcome::Stopped);
        let s = link.0.lock().unwrap();
        assert!(s.pen_up);
        assert_eq!(s.moves, 40, "ninguna orden de movimiento tras la parada");
        assert_eq!(plotter.kin.motors(plotter.position()), s.pos);
    }

    #[test]
    fn origin_offset_shifts_the_plot() {
        let link = SimLink::default();
        let settings = MachineSettings { paper_origin_mm: [5.0, 7.0], return_home: false, ..Default::default() };
        let mut plotter = Plotter::connect(link.clone(), settings).unwrap();
        let (out, motion) = sample_plan();
        let control = Control::default();
        let r = plotter.execute(&out, &motion, &control, &mut |e| if let Event::PenChange { .. } = e { control.resume() });
        assert_eq!(r, Ok(Outcome::Finished));
        let kin = plotter.settings().kinematics();
        assert!(link.0.lock().unwrap().inked.contains(&kin.motors(p(45.0, 17.0))));
    }

    #[test]
    fn settings_validation() {
        assert!(MachineSettings::default().valid());
        assert!(!MachineSettings { pen_up_pct: 120.0, ..Default::default() }.valid());
        assert!(matches!(Plotter::connect(SimLink::default(), MachineSettings { accel: 0.0, ..Default::default() }),
            Err(MachineError::InvalidSettings)));
    }
}
