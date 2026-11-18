//! Máquina: enlace con el plóter por USB. La placa vive en un hilo propio que recibe
//! órdenes y devuelve avisos; la pausa y la parada van por `Control`, así responden
//! al momento aunque el hilo esté trazando.

use crate::{app::VectorioApp, theme};
use egui::RichText;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use vectorio_core::machine::{Control, Event, Link, MachineError, MachineSettings, Outcome, Plotter, SerialLink};
use vectorio_core::{output::OutputPlan, PlotSettings};

/// Cada cuánto se comprueba, sin trazar, que la placa sigue al otro lado del cable.
const PING: Duration = Duration::from_secs(2);

enum Cmd {
    Settings(MachineSettings),
    Pen(bool),
    Release,
    Home,
    Plot(Arc<OutputPlan>, PlotSettings, Arc<Control>),
    Disconnect,
}

#[derive(Clone, Debug, PartialEq)]
enum Note {
    Connected(String),
    /// La placa ya no responde o no se pudo abrir; el hilo ha terminado.
    Lost(String),
    /// Desconectada a petición; el hilo ha terminado.
    Closed,
    /// Orden terminada; los motores están activados o sueltos.
    Ready { engaged: bool },
    Failed(String),
    Step { index: usize, total: usize },
    PenChange(usize),
    Paused,
    Resumed,
    Button,
    Done(Outcome),
}

struct Worker { tx: mpsc::Sender<Cmd>, rx: mpsc::Receiver<Note> }

fn spawn<L: Link + 'static>(open: impl FnOnce() -> Result<L, MachineError> + Send + 'static, settings: MachineSettings,
    ctx: Option<egui::Context>) -> Worker {
    let (tx, cmds) = mpsc::channel();
    let (notes, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let send = |note: Note| { let _ = notes.send(note); if let Some(ctx) = &ctx { ctx.request_repaint(); } };
        run_worker(open, settings, cmds, &send);
    });
    Worker { tx, rx }
}

/// Fallos que dejan la conexión en duda: se da la máquina por desconectada.
fn is_lost(error: &MachineError) -> bool { matches!(error, MachineError::Io(_) | MachineError::Timeout(_) | MachineError::NotFound) }

fn describe(error: &MachineError) -> String {
    match error {
        MachineError::NotFound => tr!("No se encuentra la máquina. Comprueba que está encendida y que el cable USB pasa datos, no solo carga."),
        MachineError::Io(_) | MachineError::Timeout(_) => tr!("Se ha perdido la conexión USB con la máquina. Comprueba el cable y vuelve a conectar; antes de trazar, lleva el carro al origen."),
        MachineError::OutOfBounds => tr!("El dibujo no cabe en el recorrido de la máquina. Revisa el recorrido y la posición del papel."),
        MachineError::InvalidSettings => tr!("Revisa los ajustes de la máquina: hay valores fuera de rango."),
        MachineError::Device(text) | MachineError::Unexpected(text) => tr!("La máquina ha respondido algo inesperado: %{respuesta}", respuesta = text),
    }
}

fn run_worker<L: Link>(open: impl FnOnce() -> Result<L, MachineError>, settings: MachineSettings, cmds: mpsc::Receiver<Cmd>, send: &dyn Fn(Note)) {
    let mut plotter = match open().and_then(|link| Plotter::attach(link, settings)) {
        Ok(p) => p,
        Err(e) => return send(Note::Lost(describe(&e))),
    };
    send(Note::Connected(plotter.version().to_string()));
    loop {
        let cmd = match cmds.recv_timeout(PING) {
            Ok(cmd) => cmd,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Err(e) = plotter.ebb().motion() {
                    if is_lost(&e) { return send(Note::Lost(describe(&e))); }
                }
                continue;
            }
            // La interfaz se ha ido: se deja la máquina en reposo.
            Err(mpsc::RecvTimeoutError::Disconnected) => { let _ = plotter.release(); return; }
        };
        let result = match cmd {
            Cmd::Settings(settings) => plotter.update_settings(settings),
            Cmd::Pen(up) => plotter.move_pen(up, 300).and_then(|()| plotter.wait_idle()),
            Cmd::Release => plotter.release(),
            Cmd::Home => {
                let control = Control::default();
                plotter.go_home(PlotSettings::default().travel_speed, &control, &mut |_| {}).map(|_| ())
            }
            Cmd::Plot(plan, motion, control) => {
                let mut on_event = |event: Event| send(match event {
                    Event::Step { index, total } => Note::Step { index, total },
                    Event::PenChange { pen } => Note::PenChange(pen),
                    Event::Button => Note::Button,
                    Event::Paused => Note::Paused,
                    Event::Resumed => Note::Resumed,
                });
                plotter.execute(&plan, &motion, &control, &mut on_event).map(|outcome| send(Note::Done(outcome)))
            }
            Cmd::Disconnect => { let _ = plotter.release(); return send(Note::Closed); }
        };
        match result {
            // Un trazado a medias no se puede reanudar: se intenta subir la pluma y soltar
            // los motores para que no se queden haciendo fuerza.
            Err(e) if is_lost(&e) => { let _ = plotter.ebb().motors(false); return send(Note::Lost(describe(&e))) }
            Err(e) => {
                // Antes de mover nada (plan fuera del recorrido, ajustes no válidos) no hay
                // nada que soltar.
                if !matches!(e, MachineError::OutOfBounds | MachineError::InvalidSettings) { let _ = plotter.release(); }
                send(Note::Failed(describe(&e)))
            }
            Ok(()) => {}
        }
        send(Note::Ready { engaged: plotter.is_engaged() });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Status {
    #[default]
    Off,
    Connecting,
    Idle,
    /// Orden corta en curso (pluma, soltar, volver al origen).
    Busy,
    Plotting,
}

#[derive(Default)]
pub struct MachineUi {
    worker: Option<Worker>,
    status: Status,
    version: String,
    engaged: bool,
    control: Option<Arc<Control>>,
    progress: (usize, usize),
    /// Tinta que hay que poner antes de reanudar.
    waiting_pen: Option<usize>,
    /// Trazado listo para empezar en cuanto se ponga la primera tinta.
    pending: Option<(Arc<OutputPlan>, PlotSettings)>,
    paused: bool,
    /// Plan que está trazando la máquina, para que la línea de tiempo la siga.
    running: Option<Arc<OutputPlan>>,
    /// Tiempo ya transcurrido en el paso actual y desde cuándo corre, si no está en pausa.
    step_elapsed: f64,
    step_clock: Option<Instant>,
    /// Último fallo, a la vista hasta la siguiente orden.
    error: Option<String>,
    /// Aviso para la barra de mensajes de la aplicación.
    notice: Option<String>,
    /// Ajustes enviados a la placa por última vez.
    sent: Option<MachineSettings>,
    /// Cerrar la conexión en cuanto termine la orden en curso.
    close_after: bool,
    /// Tinta por la que ya se ha avisado en el Dock.
    called_for: Option<usize>,
    awake: KeepAwake,
}

impl Drop for MachineUi {
    fn drop(&mut self) { if let Some(control) = &self.control { control.stop(); } }
}

/// Mientras traza, el Mac no entra en reposo por inactividad: al dormirse cortaría el
/// USB a mitad de dibujo. La pantalla sí puede apagarse. `caffeinate -w` termina solo
/// si Vectorio se cierra de golpe.
#[derive(Default)]
struct KeepAwake(Option<std::process::Child>);

impl KeepAwake {
    fn set(&mut self, on: bool) {
        if on == self.0.is_some() { return; }
        if on {
            if cfg!(target_os = "macos") {
                self.0 = std::process::Command::new("/usr/bin/caffeinate")
                    .args(["-i", "-w", &std::process::id().to_string()])
                    .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
                    .spawn().ok();
            }
        } else if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) { self.set(false); }
}

impl MachineUi {
    fn connect(&mut self, settings: &MachineSettings, ctx: &egui::Context) {
        self.connect_with(|| SerialLink::open_first().map(|(link, _)| link), settings, Some(ctx.clone()));
    }

    fn connect_with<L: Link + 'static>(&mut self, open: impl FnOnce() -> Result<L, MachineError> + Send + 'static,
        settings: &MachineSettings, ctx: Option<egui::Context>) {
        self.worker = Some(spawn(open, settings.clone(), ctx));
        self.sent = Some(settings.clone());
        self.status = Status::Connecting;
        self.error = None;
    }

    fn send(&mut self, cmd: Cmd) {
        let busy = if matches!(cmd, Cmd::Plot(..)) { Status::Plotting } else { Status::Busy };
        if let Some(worker) = &self.worker {
            if worker.tx.send(cmd).is_ok() { self.status = busy; self.error = None; }
        }
    }

    fn connected(&self) -> bool { !matches!(self.status, Status::Off | Status::Connecting) }

    /// Recoge los avisos del hilo y manda los ajustes que hayan cambiado.
    fn update(&mut self, settings: &MachineSettings) {
        while let Some(next) = self.worker.as_ref().map(|w| w.rx.try_recv()) {
            let note = match next {
                Ok(note) => note,
                Err(mpsc::TryRecvError::Empty) => break,
                // Un hilo que ya no existe deja la máquina desconectada.
                Err(mpsc::TryRecvError::Disconnected) => { self.reset(); break; }
            };
            match note {
                Note::Connected(version) => { self.version = version; self.status = Status::Idle; self.engaged = false; }
                Note::Lost(message) => {
                    let was_connected = self.connected();
                    self.reset();
                    if was_connected { self.notice = Some(message.clone()); }
                    self.error = Some(message);
                }
                Note::Closed => self.reset(),
                Note::Ready { engaged } => {
                    self.engaged = engaged;
                    if self.status != Status::Off { self.status = Status::Idle; }
                    self.control = None; self.waiting_pen = None; self.paused = false; self.running = None;
                    if std::mem::take(&mut self.close_after) {
                        self.send(Cmd::Disconnect);
                        self.notice = Some(tr!("Trazado terminado. Motores sueltos y máquina desconectada: el Mac ya puede entrar en reposo."));
                    }
                }
                Note::Failed(message) => self.error = Some(message),
                Note::Step { index, total } => {
                    self.progress = (index, total); self.waiting_pen = None;
                    self.step_elapsed = 0.0;
                    self.step_clock = (!self.paused).then(Instant::now);
                }
                Note::PenChange(pen) => { self.waiting_pen = Some(pen); self.hold(); }
                Note::Paused => self.hold(),
                Note::Resumed => {
                    self.paused = false; self.waiting_pen = None;
                    self.step_clock.get_or_insert_with(Instant::now);
                }
                Note::Button => { self.hold(); self.notice = Some(tr!("Botón de la máquina pulsado: trazado en pausa.")); }
                Note::Done(Outcome::Finished) => {
                    self.progress.0 = self.progress.1;
                    self.notice = Some(tr!("Trazado terminado."));
                    self.close_after = settings.disconnect_when_done;
                }
                Note::Done(Outcome::Stopped) => self.notice = Some(tr!("Trazado parado. La pluma queda arriba.")),
            }
        }
        if self.status == Status::Idle && self.sent.as_ref() != Some(settings) && settings.valid() {
            self.sent = Some(settings.clone());
            self.send(Cmd::Settings(settings.clone()));
        }
    }

    fn reset(&mut self) {
        if let Some(control) = self.control.take() { control.stop(); }
        self.worker = None; self.status = Status::Off; self.waiting_pen = None; self.pending = None; self.paused = false; self.engaged = false; self.sent = None;
        self.running = None;
    }

    fn plot(&mut self, plan: Arc<OutputPlan>, motion: PlotSettings) {
        let control = Arc::new(Control::default());
        self.control = Some(control.clone());
        self.progress = (0, plan.steps.len());
        self.paused = false; self.waiting_pen = None;
        self.step_elapsed = 0.0; self.step_clock = None;
        self.running = Some(plan.clone());
        self.send(Cmd::Plot(plan, motion, control));
    }

    /// Detiene el reloj del paso: en pausa la línea de tiempo no avanza.
    fn hold(&mut self) {
        self.paused = true;
        if let Some(start) = self.step_clock.take() { self.step_elapsed += start.elapsed().as_secs_f64(); }
    }

    /// Plan que se está trazando y punto de su línea de tiempo, en segundos, por el que
    /// va la máquina. Dentro de cada paso avanza al ritmo previsto y espera al final si
    /// la máquina va más lenta; el siguiente paso lo vuelve a poner en hora.
    pub fn live(&self) -> Option<(Arc<OutputPlan>, f64)> { self.live_at(Instant::now()) }

    fn live_at(&self, now: Instant) -> Option<(Arc<OutputPlan>, f64)> {
        if self.status != Status::Plotting { return None; }
        let plan = self.running.clone()?;
        let Some(step) = plan.steps.get(self.progress.0) else { return Some((plan.clone(), plan.stats.total_seconds)); };
        let (a, b) = step.times();
        let running = self.step_clock.map_or(0.0, |start| now.saturating_duration_since(start).as_secs_f64());
        let t = a + (self.step_elapsed + running).min((b - a).max(0.0));
        Some((plan, t))
    }

    fn pause(&mut self) { if let Some(c) = &self.control { c.pause(); self.hold(); } }
    fn resume(&mut self) { if let Some(c) = &self.control { c.resume(); } }
    fn stop(&mut self) { if let Some(c) = &self.control { c.stop(); c.resume(); } }

    fn status_text(&self, pens: &[vectorio_core::Pen]) -> String {
        match self.status {
            Status::Off => tr!("Sin conectar"),
            Status::Connecting => tr!("Conectando…"),
            Status::Idle if self.pending.is_some() => {
                let pen = self.pending.as_ref().and_then(|(plan, _)| plan.strokes.first()).map(|s| s.pen);
                tr!("Pon %{pen} y pulsa Empezar", pen = pen.and_then(|p| pens.get(p)).map_or_else(|| "?".into(), crate::i18n::pen_name))
            }
            Status::Idle | Status::Busy => {
                let version = vectorio_core::machine::ebb::firmware_version(&self.version)
                    .map_or_else(String::new, |(a, b, c)| format!(" · EBB {a}.{b}.{c}"));
                if self.engaged { tr!("Conectada%{version} · motores activados", version = version) }
                else { tr!("Conectada%{version} · carro libre", version = version) }
            }
            Status::Plotting => match self.waiting_pen {
                Some(pen) => tr!("Pon %{pen} y pulsa Continuar", pen = pens.get(pen).map_or_else(|| "?".into(), crate::i18n::pen_name)),
                None if self.paused => tr!("En pausa · pluma arriba"),
                None => tr!("Trazando · paso %{i} de %{n}", i = self.progress.0 + 1, n = self.progress.1),
            },
        }
    }
}

/// Recoge los avisos de la máquina en cada fotograma, en cualquier vista: el trazado
/// sigue aunque se cambie de pestaña.
pub fn update(app: &mut VectorioApp, ctx: &egui::Context) {
    let m = &mut app.machine_ui;
    m.update(&app.prefs.machine);
    m.awake.set(m.status == Status::Plotting);
    // Al pedir una tinta, el Dock rebota hasta que se vuelve a Vectorio.
    if m.waiting_pen.is_some() && m.called_for != m.waiting_pen {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
    }
    m.called_for = m.waiting_pen;
    if m.status == Status::Plotting { ctx.request_repaint_after(Duration::from_millis(500)); }
    if let Some(message) = m.notice.take() { app.notify(ctx, message); }
}

/// Diálogo del cambio de tinta: el trazado espera, con la pluma arriba, hasta Aceptar.
pub fn pen_dialog(app: &mut VectorioApp, ctx: &egui::Context) {
    let m = &mut app.machine_ui;
    let Some(pen) = m.waiting_pen.filter(|_| m.status == Status::Plotting) else { return };
    let pal = theme::current(ctx);
    let screen = ctx.screen_rect();
    let veil = egui::Area::new(egui::Id::new("pen-change-veil")).fixed_pos(screen.min).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.allocate_rect(screen, egui::Sense::click());
        ui.painter().rect_filled(screen, 0.0, theme::veil(ui.visuals().dark_mode));
    });
    ctx.move_to_top(veil.response.layer_id);
    let ink = app.doc.pens.get(pen);
    let name = ink.map_or_else(|| "?".into(), crate::i18n::pen_name);
    let sheet = egui::Area::new(egui::Id::new("pen-change-sheet")).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .order(egui::Order::Foreground).show(ctx, |ui| {
        theme::glass_sheet(ui, &pal, |ui| {
            ui.set_max_width(340.0);
            ui.horizontal(|ui| {
                if let Some(ink) = ink {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 8.0, theme::pen_color(ink.color));
                }
                ui.label(RichText::new(tr!("Cambio de tinta")).size(15.0).strong().color(pal.text));
            });
            ui.add_space(8.0);
            ui.label(RichText::new(tr!("Pon %{pen} en la máquina y pulsa Aceptar para seguir. La pluma está arriba y el carro no se mueve.", pen = name)).color(pal.text));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let accept = theme::accent_button(ui, &tr!("Aceptar")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter));
                let stop = ui.button(tr!("Parar")).on_hover_text(tr!("Termina el trazado aquí, con la pluma arriba.")).clicked();
                if accept { m.resume(); }
                if stop { m.stop(); }
            });
        });
    });
    ctx.move_to_top(sheet.response.layer_id);
}

/// Estado, conexión y trazado: abajo del inspector de Salida.
pub fn actions(app: &mut VectorioApp, ui: &mut egui::Ui, ready: bool) {
    let pal = theme::current(ui.ctx());
    let m = &mut app.machine_ui;
    theme::section(ui, &tr!("MÁQUINA"));
    ui.label(RichText::new(m.status_text(&app.doc.pens)).color(pal.text));
    if m.status == Status::Plotting {
        let (i, n) = m.progress;
        ui.add(egui::ProgressBar::new(if n == 0 { 0.0 } else { i as f32 / n as f32 }).desired_height(6.0));
    }
    ui.horizontal_wrapped(|ui| match m.status {
        Status::Off => {
            if theme::accent_button(ui, &tr!("Conectar")).clicked() { m.connect(&app.prefs.machine, ui.ctx()); }
        }
        Status::Connecting => { ui.spinner(); }
        Status::Idle if m.pending.is_some() => {
            if theme::accent_button(ui, &tr!("Empezar")).clicked() {
                if let Some((plan, motion)) = m.pending.take() { m.plot(plan, motion); }
            }
            if ui.button(tr!("Cancelar")).clicked() { m.pending = None; }
        }
        Status::Idle | Status::Busy => {
            let plan = app.output_ui.plan.clone().filter(|p| ready && p.stats.strokes > 0);
            let idle = m.status == Status::Idle;
            if ui.add_enabled_ui(idle && plan.is_some(), |ui| theme::accent_button(ui, &tr!("Trazar en la máquina"))).inner
                .on_hover_text(tr!("Toma como origen el punto donde esté el carro si los motores están sueltos. Antes de cada tinta, pausa para que la pongas."))
                .clicked() {
                if let Some(plan) = plan { m.pending = Some((plan, app.doc.plot.clone())); }
            }
            if ui.add_enabled(idle, egui::Button::new(tr!("Desconectar"))).clicked() { m.send(Cmd::Disconnect); }
            if !idle { ui.spinner(); }
        }
        Status::Plotting => {
            if m.paused {
                if theme::accent_button(ui, &tr!("Continuar")).clicked() { m.resume(); }
            } else if ui.button(tr!("Pausar")).clicked() { m.pause(); }
            if ui.button(tr!("Parar")).on_hover_text(tr!("Sube la pluma y deja el carro donde está.")).clicked() { m.stop(); }
        }
    });
    if let Some(error) = &m.error { ui.label(RichText::new(error).small().color(pal.text)); }
    ui.add_space(4.0);
}

/// Calibración de la pluma, origen y recorrido, dentro de los ajustes de Salida.
pub fn settings(app: &mut VectorioApp, ui: &mut egui::Ui) {
    let pal = theme::current(ui.ctx());
    let before = app.prefs.machine.clone();
    let m = &mut app.machine_ui;
    let s = &mut app.prefs.machine;
    let idle = m.status == Status::Idle;
    egui::CollapsingHeader::new(tr!("Máquina: pluma, origen y recorrido")).id_salt("machine-settings").show(ui, |ui| {
        ui.label(RichText::new(tr!("Fija el boli en el soporte con la pluma abajo, apoyado en el papel sin forzar.")).small().color(pal.tertiary));
        for (label, value, up) in [(tr!("Pluma arriba"), &mut s.pen_up_pct, true), (tr!("Pluma abajo"), &mut s.pen_down_pct, false)] {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(egui::DragValue::new(value).range(0.0..=100.0).speed(0.5).suffix(" %"));
                if ui.add_enabled(idle, egui::Button::new(tr!("Probar"))).on_hover_text(tr!("Mueve solo la pluma a esta altura.")).clicked() {
                    m.send(Cmd::Pen(up));
                }
            });
        }
        if s.pen_down_pct >= s.pen_up_pct {
            ui.label(RichText::new(tr!("La pluma abajo debe quedar por debajo de la de arriba.")).small().color(pal.text));
        }
        ui.add_space(6.0);
        ui.label(RichText::new(tr!("El origen es donde esté el carro al empezar a trazar: suelta los motores y llévalo a mano a la esquina superior izquierda.")).small().color(pal.tertiary));
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(idle, egui::Button::new(tr!("Soltar motores"))).clicked() { m.send(Cmd::Release); }
            if ui.add_enabled(idle && m.engaged, egui::Button::new(tr!("Volver al origen"))).clicked() { m.send(Cmd::Home); }
        });
        ui.add_space(6.0);
        egui::Grid::new("machine-grid").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label(tr!("Papel desde el origen"));
            ui.add(egui::DragValue::new(&mut s.paper_origin_mm[0]).range(-500.0..=500.0).speed(0.5).prefix("X ").suffix(" mm"));
            ui.add(egui::DragValue::new(&mut s.paper_origin_mm[1]).range(-500.0..=500.0).speed(0.5).prefix("Y ").suffix(" mm"));
            ui.end_row();
            ui.label(tr!("Recorrido del carro"));
            ui.add(egui::DragValue::new(&mut s.travel_mm[0]).range(10.0..=2000.0).speed(1.0).suffix(" mm"));
            ui.add(egui::DragValue::new(&mut s.travel_mm[1]).range(10.0..=2000.0).speed(1.0).suffix(" mm"));
            ui.end_row();
            ui.label(tr!("Velocidad máxima"));
            ui.add(egui::DragValue::new(&mut s.max_speed).range(5.0..=400.0).speed(1.0).suffix(" mm/s"));
            ui.end_row();
            ui.label(tr!("Aceleración")).on_hover_text(tr!("Con la pluma abajo y en los viajes con la pluma arriba."));
            ui.add(egui::DragValue::new(&mut s.accel).range(100.0..=5000.0).speed(10.0).suffix(" mm/s²"));
            ui.add(egui::DragValue::new(&mut s.travel_accel).range(100.0..=5000.0).speed(10.0).suffix(" mm/s²"));
            ui.end_row();
        });
        ui.checkbox(&mut s.return_home, tr!("Volver al origen al terminar"));
        ui.checkbox(&mut s.release_when_done, tr!("Soltar los motores al terminar"))
            .on_hover_text(tr!("Parados siguen haciendo fuerza: zumban y se calientan."));
        ui.checkbox(&mut s.disconnect_when_done, tr!("Desconectar al terminar"))
            .on_hover_text(tr!("Suelta los motores y cierra la conexión USB. Mientras traza, el Mac no entra en reposo; al acabar, ya puede."));
    });
    if app.prefs.machine != before { app.prefs_changed(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use vectorio_core::machine::Reply;

    /// Placa simulada mínima: responde a todo y puede desconectarse.
    #[derive(Clone, Default)]
    struct FakeLink { log: Arc<Mutex<Vec<String>>>, unplugged: Arc<std::sync::atomic::AtomicBool>, reject_moves: Arc<std::sync::atomic::AtomicBool> }

    impl Link for FakeLink {
        fn exchange(&mut self, cmd: &str, _reply: Reply) -> Result<String, MachineError> {
            if self.unplugged.load(std::sync::atomic::Ordering::Relaxed) { return Err(MachineError::Io("Broken pipe".into())); }
            if cmd.starts_with("SM") && self.reject_moves.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(MachineError::Device("!8 Err: Unknown command".into()));
            }
            self.log.lock().unwrap().push(cmd.to_string());
            Ok(match cmd.split(',').next().unwrap() {
                "V" => "EBBv13_and_above EB Firmware Version 2.8.1".into(),
                "QM" => "QM,0,0,0,0".into(),
                "QB" | "QP" => "0".into(),
                "QS" => "0,0".into(),
                _ => String::new(),
            })
        }
    }

    fn wait(ui: &mut MachineUi, settings: &MachineSettings, until: impl Fn(&MachineUi) -> bool) {
        for _ in 0..500 {
            ui.update(settings);
            if until(ui) { return; }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("la máquina no llega al estado esperado: {:?}", ui.status);
    }

    fn sample_plan() -> Arc<OutputPlan> {
        use vectorio_core::{p, output::{prepare, OutputSettings}, Paper, Pen, PlotLayer};
        let paper = Paper { name: "Prueba".into(), width_mm: 100.0, height_mm: 80.0, margin_mm: 5.0 };
        let pens = vec![Pen::new("Negro", [0, 0, 0], 0.5), Pen::new("Rojo", [200, 0, 0], 0.5)];
        let layers = vec![
            PlotLayer { pen: 0, paths: vec![vec![p(10.0, 10.0), p(40.0, 10.0)]] },
            PlotLayer { pen: 1, paths: vec![vec![p(20.0, 60.0), p(30.0, 70.0)]] },
        ];
        Arc::new(prepare(&layers, &paper, &pens, &PlotSettings::default(), &OutputSettings { drying: false, ..Default::default() }).unwrap())
    }

    #[test]
    fn pen_change_asks_in_a_dialog_and_enter_resumes() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let control = Arc::new(Control::default());
        control.pause();
        app.machine_ui.status = Status::Plotting;
        app.machine_ui.control = Some(control.clone());
        app.machine_ui.waiting_pen = Some(1);
        app.machine_ui.paused = true;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
        fn has_text(shape: &egui::Shape, text: &str) -> bool {
            match shape {
                egui::Shape::Text(t) => t.galley.text().contains(text),
                egui::Shape::Vec(v) => v.iter().any(|s| has_text(s, text)),
                _ => false,
            }
        }
        // Un área nueva se mide en el primer fotograma y se dibuja en el segundo.
        ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| pen_dialog(&mut app, ctx));
        let out = ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| pen_dialog(&mut app, ctx));
        let pen = crate::i18n::pen_name(&app.doc.pens[1]);
        assert!(out.shapes.iter().any(|s| has_text(&s.shape, "Cambio de tinta")));
        assert!(out.shapes.iter().any(|s| has_text(&s.shape, &format!("Pon {pen} en la máquina"))));
        assert!(control.is_paused());
        let enter = egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
        ctx.run(egui::RawInput { screen_rect: Some(screen), events: vec![enter], ..Default::default() }, |ctx| pen_dialog(&mut app, ctx));
        assert!(!control.is_paused(), "Intro acepta el cambio de tinta");
        // Sin tinta pendiente no hay diálogo.
        app.machine_ui.waiting_pen = None;
        let out = ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| pen_dialog(&mut app, ctx));
        assert!(!out.shapes.iter().any(|s| has_text(&s.shape, "Cambio de tinta")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn keeps_the_mac_awake_only_while_plotting() {
        let mut awake = KeepAwake::default();
        awake.set(true);
        let child = awake.0.as_mut().expect("caffeinate");
        assert!(child.try_wait().unwrap().is_none(), "caffeinate sigue en marcha");
        awake.set(true);
        awake.set(false);
        assert!(awake.0.is_none());
    }

    #[test]
    fn timeline_follows_the_machine_and_stops_while_paused() {
        let plan = sample_plan();
        let mut ui = MachineUi::default();
        ui.status = Status::Plotting; ui.running = Some(plan.clone());
        let index = plan.steps.iter().position(|s| matches!(s, vectorio_core::output::OutputStep::Draw { .. })).unwrap();
        let (a, b) = plan.steps[index].times();
        assert!(b - a > 0.2, "el trazo de prueba dura {} s", b - a);
        let start = Instant::now();
        ui.progress = (index, plan.steps.len());
        ui.step_clock = Some(start);
        let at = |ui: &MachineUi, s: f64| ui.live_at(start + Duration::from_secs_f64(s)).unwrap().1;
        assert!((at(&ui, 0.0) - a).abs() < 1e-9);
        assert!((at(&ui, 0.1) - (a + 0.1)).abs() < 1e-6);
        // Si la máquina va más lenta que lo previsto, espera al final del paso.
        assert!((at(&ui, 1e4) - b).abs() < 1e-9);
        // En pausa no avanza.
        ui.step_clock = None; ui.step_elapsed = 0.1; ui.paused = true;
        assert!((at(&ui, 50.0) - (a + 0.1)).abs() < 1e-6);
        // Terminado: al final; sin trazar, nada que seguir.
        ui.progress.0 = plan.steps.len();
        assert_eq!(at(&ui, 0.0), plan.stats.total_seconds);
        ui.status = Status::Idle;
        assert!(ui.live().is_none());
    }

    #[test]
    fn connects_plots_with_pen_changes_and_stays_connected() {
        let _lock = crate::i18n::locale_lock();
        let link = FakeLink::default();
        let settings = MachineSettings { disconnect_when_done: false, ..Default::default() };
        let mut ui = MachineUi::default();
        let l = link.clone();
        ui.connect_with(move || Ok(l), &settings, None);
        wait(&mut ui, &settings, |u| u.status == Status::Idle);
        assert!(ui.version.contains("2.8.1") && !ui.engaged);
        ui.plot(sample_plan(), PlotSettings::default());
        assert_eq!(ui.status, Status::Plotting);
        let mut changes = 0;
        for _ in 0..1000 {
            ui.update(&settings);
            if ui.waiting_pen.is_some() { changes += 1; ui.resume(); ui.waiting_pen = None; }
            if ui.status == Status::Idle { break; }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(ui.status, Status::Idle);
        assert_eq!(changes, 1, "la segunda tinta pausa; la primera se pide antes de empezar");
        assert!(ui.notice.as_deref().is_some_and(|n| n.contains("terminado")));
        assert!(!ui.engaged, "suelta los motores al volver");
        // Los ajustes nuevos llegan a la placa.
        let changed = MachineSettings { pen_up_pct: 90.0, ..settings.clone() };
        wait(&mut ui, &changed, |u| u.status == Status::Idle && u.sent.as_ref() == Some(&changed));
        assert!(link.log.lock().unwrap().iter().any(|c| c == &format!("SC,4,{}", vectorio_core::machine::ebb::servo_position(90.0))));
    }

    #[test]
    fn an_error_mid_plot_lifts_the_pen_and_releases_the_motors() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let link = FakeLink::default();
        let settings = MachineSettings::default();
        let mut ui = MachineUi::default();
        let l = link.clone();
        ui.connect_with(move || Ok(l), &settings, None);
        wait(&mut ui, &settings, |u| u.status == Status::Idle);
        ui.plot(sample_plan(), PlotSettings::default());
        wait(&mut ui, &settings, |_| link.log.lock().unwrap().iter().any(|c| c.starts_with("SM")));
        link.reject_moves.store(true, std::sync::atomic::Ordering::Relaxed);
        for _ in 0..1000 {
            ui.update(&settings);
            if ui.waiting_pen.is_some() { ui.resume(); ui.waiting_pen = None; }
            if ui.error.is_some() && ui.status == Status::Idle { break; }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(ui.error.is_some(), "avisa del error");
        assert!(!ui.engaged, "los motores quedan sueltos");
        let log = link.log.lock().unwrap().clone();
        assert_eq!(log.last().map(String::as_str), Some("EM,0,0"));
        let pen = log.iter().rev().find(|c| c.starts_with("SP,")).map(String::as_str);
        assert!(pen.is_some_and(|c| c.starts_with("SP,1")), "la pluma queda arriba: {pen:?}");
    }

    #[test]
    fn a_finished_plot_releases_the_motors_and_disconnects() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let link = FakeLink::default();
        let settings = MachineSettings::default();
        assert!(settings.disconnect_when_done);
        let mut ui = MachineUi::default();
        let l = link.clone();
        ui.connect_with(move || Ok(l), &settings, None);
        wait(&mut ui, &settings, |u| u.status == Status::Idle);
        ui.plot(sample_plan(), PlotSettings::default());
        ui.awake.set(true);
        for _ in 0..1000 {
            ui.update(&settings);
            if ui.waiting_pen.is_some() { ui.resume(); ui.waiting_pen = None; }
            if ui.status == Status::Off { break; }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(ui.status, Status::Off, "se desconecta al terminar");
        assert!(ui.error.is_none());
        assert!(ui.notice.as_deref().is_some_and(|n| n.contains("reposo")));
        let log = link.log.lock().unwrap();
        assert_eq!(log.iter().rev().find(|c| c.starts_with("EM")).map(String::as_str), Some("EM,0,0"));
        drop(log);
        // Sin trazar, el Mac puede dormir.
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        app.machine_ui = ui;
        let _ = ctx.run(egui::RawInput::default(), |ctx| update(&mut app, ctx));
        assert!(app.machine_ui.awake.0.is_none());
    }

    #[test]
    fn stop_while_waiting_for_a_pen_ends_the_plot() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let settings = MachineSettings::default();
        let mut ui = MachineUi::default();
        ui.connect_with(|| Ok(FakeLink::default()), &settings, None);
        wait(&mut ui, &settings, |u| u.status == Status::Idle);
        ui.plot(sample_plan(), PlotSettings::default());
        wait(&mut ui, &settings, |u| u.waiting_pen.is_some());
        ui.stop();
        wait(&mut ui, &settings, |u| u.status == Status::Idle);
        assert!(ui.notice.as_deref().is_some_and(|n| n.contains("parado")));
    }

    #[test]
    fn unplugging_the_cable_disconnects_with_a_clear_message() {
        let _lock = crate::i18n::locale_lock();
        let link = FakeLink::default();
        let settings = MachineSettings::default();
        let mut ui = MachineUi::default();
        let l = link.clone();
        ui.connect_with(move || Ok(l), &settings, None);
        wait(&mut ui, &settings, |u| u.status == Status::Idle);
        link.unplugged.store(true, std::sync::atomic::Ordering::Relaxed);
        ui.send(Cmd::Pen(false));
        wait(&mut ui, &settings, |u| u.status == Status::Off);
        assert!(ui.error.as_deref().is_some_and(|e| e.contains("USB")));
        assert!(ui.notice.is_some());
    }

    #[test]
    fn a_missing_machine_reports_why() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let settings = MachineSettings::default();
        let mut ui = MachineUi::default();
        ui.connect_with(|| Err::<FakeLink, _>(MachineError::NotFound), &settings, None);
        wait(&mut ui, &settings, |u| u.status == Status::Off);
        assert!(ui.error.as_deref().is_some_and(|e| e.contains("cable USB")));
        assert!(ui.notice.is_none(), "no estaba conectada: basta con el texto del panel");
    }
}
