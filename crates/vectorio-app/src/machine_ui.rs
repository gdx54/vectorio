//! Máquina: enlace con el plóter por USB o por Wi-Fi (una Raspberry Pi junto a la placa).
//! La placa vive en un hilo propio que recibe órdenes y devuelve avisos; la pausa y la
//! parada van por `Control`, así responden al momento aunque el hilo esté trazando. Por
//! Wi-Fi, la misma sesión corre en la Raspberry y el hilo solo reenvía (`machine::remote`).

use crate::{app::VectorioApp, theme};
use egui::RichText;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use vectorio_core::machine::{remote, session, Control, Link, MachineError, MachineSettings, Notice, Outcome, SerialLink};
use vectorio_core::{output::{OutputPlan, OutputStep}, PlotSettings};

type Cmd = session::Command;

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
    /// Espera de secado de tantos ms.
    Drying(u32),
    Done(Outcome),
}

impl Note {
    fn from(notice: Notice, wifi: bool) -> Note {
        match notice {
            Notice::Connected(version) => Note::Connected(version),
            Notice::Lost(e) => Note::Lost(describe(&e, wifi)),
            Notice::Closed => Note::Closed,
            Notice::Ready { engaged } => Note::Ready { engaged },
            Notice::Failed(e) => Note::Failed(describe(&e, wifi)),
            Notice::Step { index, total } => Note::Step { index, total },
            Notice::PenChange(pen) => Note::PenChange(pen),
            Notice::Paused => Note::Paused,
            Notice::Resumed => Note::Resumed,
            Notice::Button => Note::Button,
            Notice::Drying(ms) => Note::Drying(ms),
            Notice::Done(outcome) => Note::Done(outcome),
        }
    }
}

struct Worker { tx: mpsc::Sender<Cmd>, rx: mpsc::Receiver<Note>, thread: std::thread::JoinHandle<()> }

/// Hilo de la máquina: `run` es la sesión local (`session::run`) o la de la Raspberry
/// (`remote::run`); las dos atienden las mismas órdenes y dan los mismos avisos.
fn spawn(run: impl FnOnce(mpsc::Receiver<Cmd>, &dyn Fn(Notice)) + Send + 'static, wifi: bool, ctx: Option<egui::Context>) -> Worker {
    let (tx, cmds) = mpsc::channel();
    let (notes, rx) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let send = |notice: Notice| { let _ = notes.send(Note::from(notice, wifi)); if let Some(ctx) = &ctx { ctx.request_repaint(); } };
        run(cmds, &send);
    });
    Worker { tx, rx, thread }
}

fn describe(error: &MachineError, wifi: bool) -> String {
    match error {
        MachineError::NotFound if wifi => tr!("La Raspberry no encuentra la máquina. Comprueba que está encendida y conectada a la Raspberry por el cable USB de datos."),
        MachineError::NotFound => tr!("No se encuentra la máquina. Comprueba que está encendida y que el cable USB pasa datos, no solo carga."),
        MachineError::Io(_) | MachineError::Timeout(_) if wifi => tr!("Se ha perdido la conexión entre la Raspberry y la máquina. Comprueba el cable y vuelve a conectar; antes de trazar, lleva el carro al origen."),
        MachineError::Io(_) | MachineError::Timeout(_) => tr!("Se ha perdido la conexión USB con la máquina. Comprueba el cable y vuelve a conectar; antes de trazar, lleva el carro al origen."),
        MachineError::Unreachable => tr!("No se puede conectar con la Raspberry. Comprueba que está encendida, en la misma red Wi-Fi y con el servicio del plóter instalado, y que Vectorio tiene permiso de red local (Ajustes del Sistema → Privacidad y seguridad → Red local). Si estaba trazando, la Raspberry sigue hasta terminar."),
        MachineError::Busy => tr!("La Raspberry está ocupada con otra acción de la máquina, quizá de Linear. Espera a que termine y vuelve a conectar."),
        MachineError::OutOfBounds => tr!("El dibujo no cabe en el recorrido de la máquina. Revisa el recorrido y la posición del papel."),
        MachineError::InvalidSettings => tr!("Revisa los ajustes de la máquina: hay valores fuera de rango."),
        MachineError::Device(text) | MachineError::Unexpected(text) => tr!("La máquina ha respondido algo inesperado: %{respuesta}", respuesta = text),
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
    /// Hasta cuándo espera a que se seque la tinta.
    drying_until: Option<Instant>,
    /// Cuándo empezó el trazado, tiempo total en pausa (también los cambios de tinta) y
    /// desde cuándo está en pausa, para decir cuánto lleva y cuánto le queda.
    started: Option<Instant>,
    paused_total: Duration,
    paused_since: Option<Instant>,
    /// Segundos trazando (sin pausas) al reanudar por última vez: de ahí cuenta la pausa
    /// programada.
    since_break: f64,
    /// La pausa programada está esperando a que se pulse Aceptar.
    break_due: bool,
    /// Ya se ha avisado en el Dock de esta pausa programada.
    called_break: bool,
    /// Se ha pulsado Trazar: en cuanto la placa esté conectada y la salida calculada,
    /// pasa al paso de Empezar.
    want_plot: bool,
    /// Último fallo, a la vista hasta la siguiente orden.
    error: Option<String>,
    /// Aviso para la barra de mensajes de la aplicación.
    notice: Option<String>,
    /// Ajustes enviados a la placa por última vez.
    sent: Option<MachineSettings>,
    /// Cerrar la conexión en cuanto termine la orden en curso, con este aviso.
    close_after: Option<String>,
    /// El carro se quedó fuera del origen (trazado parado o fallido): hay que volver
    /// antes de empezar otra vez.
    away: bool,
    /// Volviendo al origen por «Volver a empezar» o antes de desconectar.
    homing: bool,
    /// Tinta por la que ya se ha avisado en el Dock.
    called_for: Option<usize>,
    awake: KeepAwake,
}

impl Drop for MachineUi {
    fn drop(&mut self) { self.shutdown(); }
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
        if settings.wifi {
            let (host, start) = (settings.wifi_host.trim().to_string(), settings.clone());
            self.worker = Some(spawn(move |cmds, send| remote::run(&host, start, cmds, send), true, Some(ctx.clone())));
            self.connecting(settings);
        } else {
            self.connect_with(|| SerialLink::open_first().map(|(link, _)| link), settings, Some(ctx.clone()));
        }
    }

    fn connect_with<L: Link + 'static>(&mut self, open: impl FnOnce() -> Result<L, MachineError> + Send + 'static,
        settings: &MachineSettings, ctx: Option<egui::Context>) {
        let start = settings.clone();
        self.worker = Some(spawn(move |cmds, send| session::run(open, start, cmds, send), false, ctx));
        self.connecting(settings);
    }

    /// Al salir del programa: para el trazado y cierra la sesión antes de que termine el
    /// proceso. Por USB sube la pluma y suelta los motores; por Wi-Fi, además, la Raspberry
    /// deja la placa libre al momento (para Linear, por ejemplo).
    pub fn shutdown(&mut self) {
        if let Some(control) = self.control.take() { control.stop(); control.resume(); }
        if let Some(Worker { tx, thread, .. }) = self.worker.take() {
            drop(tx);
            let start = Instant::now();
            while !thread.is_finished() && start.elapsed() < Duration::from_secs(3) { std::thread::sleep(Duration::from_millis(20)); }
        }
    }

    fn connecting(&mut self, settings: &MachineSettings) {
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
                    if self.status == Status::Plotting || self.homing { self.away = true; }
                    self.homing = false;
                    self.reset();
                    if was_connected { self.notice = Some(message.clone()); }
                    self.error = Some(message);
                }
                Note::Closed => self.reset(),
                Note::Ready { engaged } => {
                    self.engaged = engaged;
                    if self.status != Status::Off { self.status = Status::Idle; }
                    self.control = None; self.waiting_pen = None; self.paused = false; self.running = None;
                    if std::mem::take(&mut self.homing) && self.error.is_none() { self.away = false; }
                    if let Some(notice) = self.close_after.take() {
                        self.send(Cmd::Disconnect);
                        if !notice.is_empty() { self.notice = Some(notice); }
                    }
                }
                Note::Failed(message) => {
                    if self.status == Status::Plotting { self.away = true; }
                    self.error = Some(message);
                }
                Note::Step { index, total } => {
                    self.progress = (index, total); self.waiting_pen = None;
                    self.step_elapsed = 0.0; self.drying_until = None;
                    self.step_clock = (!self.paused).then(Instant::now);
                }
                Note::PenChange(pen) => { self.waiting_pen = Some(pen); self.hold(); }
                Note::Drying(ms) => self.drying_until = Some(Instant::now() + Duration::from_millis(ms as u64)),
                Note::Paused => self.hold(),
                Note::Resumed => self.unhold(),
                Note::Button => { self.hold(); self.notice = Some(tr!("Botón de la máquina pulsado: trazado en pausa.")); }
                Note::Done(Outcome::Finished) => {
                    self.progress.0 = self.progress.1;
                    self.notice = Some(tr!("Trazado terminado."));
                    self.away = !settings.return_home;
                    if settings.disconnect_when_done {
                        self.close_after = Some(tr!("Trazado terminado. Motores sueltos y máquina desconectada: el Mac ya puede entrar en reposo."));
                    }
                }
                Note::Done(Outcome::Stopped) => {
                    self.away = true;
                    self.notice = Some(tr!("Trazado parado. La pluma queda arriba. «Volver a empezar» lleva el carro al origen."));
                }
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
        self.started = Some(Instant::now()); self.paused_total = Duration::ZERO; self.paused_since = None;
        self.since_break = 0.0; self.break_due = false;
        self.running = Some(plan.clone());
        self.away = false;
        self.send(Cmd::Plot(plan, motion, control));
    }

    /// Lleva el carro al origen con la pluma arriba.
    fn go_home(&mut self) {
        self.homing = true;
        self.send(Cmd::Home);
    }

    /// Desconecta; si el carro quedó fuera del origen y se sabe dónde está, vuelve antes.
    fn disconnect(&mut self) {
        if self.away && self.engaged {
            self.close_after = Some(String::new());
            self.go_home();
        } else {
            self.send(Cmd::Disconnect);
        }
    }

    /// Detiene el reloj del paso: en pausa la línea de tiempo no avanza.
    fn hold(&mut self) {
        self.paused = true;
        if let Some(start) = self.step_clock.take() { self.step_elapsed += start.elapsed().as_secs_f64(); }
        self.paused_since.get_or_insert_with(Instant::now);
    }

    /// Vuelve a poner en marcha el reloj del paso y el del trazado.
    fn unhold(&mut self) {
        self.paused = false; self.waiting_pen = None; self.break_due = false;
        self.step_clock.get_or_insert_with(Instant::now);
        if let Some(since) = self.paused_since.take() { self.paused_total += since.elapsed(); }
        // Cualquier pausa sirve para sacar mina: la programada vuelve a contar desde aquí.
        if let Some(active) = self.active_at(Instant::now()) { self.since_break = active; }
    }

    /// Segundos desde que empezó el trazado y segundos trazando, sin pausas ni cambios de tinta.
    fn clocks_at(&self, now: Instant) -> Option<(f64, f64)> {
        let elapsed = now.saturating_duration_since(self.started?).as_secs_f64();
        let paused = self.paused_total + self.paused_since.map_or(Duration::ZERO, |s| now.saturating_duration_since(s));
        Some((elapsed, (elapsed - paused.as_secs_f64()).max(0.0)))
    }

    fn active_at(&self, now: Instant) -> Option<f64> { self.clocks_at(now).map(|(_, active)| active) }

    /// Segundos que faltan para la pausa programada, si la hay.
    fn break_in(&self, minutes: u32, now: Instant) -> Option<f64> {
        if minutes == 0 || self.status != Status::Plotting { return None; }
        Some(self.since_break + minutes as f64 * 60.0 - self.active_at(now)?)
    }

    /// Lleva adelante «Trazar en la máquina»: con la placa conectada y libre y la salida
    /// calculada (`plan`), pasa al paso de Empezar. Si no se pudo conectar, se olvida.
    fn follow_plot_request(&mut self, plan: Option<Arc<OutputPlan>>, motion: &PlotSettings) {
        if !self.want_plot { return; }
        match self.status {
            Status::Off if self.error.is_some() => self.want_plot = false,
            Status::Idle if self.pending.is_some() => self.want_plot = false,
            Status::Idle => if let Some(plan) = plan {
                self.pending = Some((plan, motion.clone()));
                self.want_plot = false;
            },
            _ => {}
        }
    }

    /// Hace la pausa programada cuando toca.
    fn check_break(&mut self, minutes: u32, now: Instant) {
        if self.paused || self.break_due { return; }
        if self.break_in(minutes, now).is_some_and(|left| left <= 0.0) {
            self.pause();
            self.break_due = true;
        }
    }

    /// Cuánto lleva el trazado (con pausas) y cuánto le queda, s, y los cambios de tinta
    /// pendientes. Lo que queda sale del plan corregido por lo que la máquina ha tardado
    /// de verdad en lo ya hecho, sin contar pausas ni cambios de tinta, que dependen de
    /// quien está delante.
    fn timing_at(&self, now: Instant) -> Option<(f64, f64, usize)> {
        let (plan, t) = self.live_at(now)?;
        let (elapsed, active) = self.clocks_at(now)?;
        let current = self.progress.0;
        let (mut changes_done, mut changes_left, mut count_left) = (0.0, 0.0, 0);
        for (i, step) in plan.steps.iter().enumerate() {
            if let OutputStep::ChangePen { start_s, end_s, .. } = *step {
                if i < current { changes_done += end_s - start_s; } else {
                    changes_left += end_s - start_s;
                    // El cambio que está esperando ahora ya se ve en pantalla.
                    if !(i == current && self.waiting_pen.is_some()) { count_left += 1; }
                }
            }
        }
        let done = (t - changes_done).max(0.0);
        let left = (plan.stats.total_seconds - t - changes_left).max(0.0);
        // Con poco hecho, la proporción todavía no dice nada: se usa el plan.
        let ratio = if done > 20.0 { (active / done).clamp(0.5, 4.0) } else { 1.0 };
        Some((elapsed, left * ratio, count_left))
    }

    /// «Lleva 12 min 30 s · quedan unos 25 min 10 s + 2 cambios de tinta».
    pub fn time_text(&self) -> Option<String> {
        let (elapsed, left, changes) = self.timing_at(Instant::now())?;
        let mut text = tr!("Lleva %{elapsed} · quedan unos %{left}", elapsed = crate::fmt::duration(elapsed), left = crate::fmt::duration(left));
        match changes {
            0 => {}
            1 => text += &tr!(" + 1 cambio de tinta"),
            n => text += &tr!(" + %{n} cambios de tinta", n = n),
        }
        Some(text)
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
    /// Reanuda sin esperar al aviso del hilo: si se pulsa Continuar antes de que la
    /// máquina llegue a pararse, ese aviso no llega nunca y el botón se quedaría en
    /// Continuar sin hacer nada.
    fn resume(&mut self) {
        if let Some(c) = &self.control { c.resume(); self.unhold(); }
    }
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
                None => match self.drying_until.map(|t| t.saturating_duration_since(Instant::now())) {
                    Some(left) if !left.is_zero() => tr!("Esperando a que se seque la tinta · faltan %{s} s", s = left.as_secs() + 1),
                    _ => tr!("Trazando · paso %{i} de %{n}", i = self.progress.0 + 1, n = self.progress.1),
                },
            },
        }
    }
}

/// Recoge los avisos de la máquina en cada fotograma, en cualquier vista: el trazado
/// sigue aunque se cambie de pestaña.
pub fn update(app: &mut VectorioApp, ctx: &egui::Context) {
    let m = &mut app.machine_ui;
    m.update(&app.prefs.machine);
    m.check_break(app.prefs.machine.break_minutes, Instant::now());
    m.awake.set(m.status == Status::Plotting);
    if m.break_due && !m.called_break {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
    }
    m.called_break = m.break_due;
    // Al pedir una tinta, el Dock rebota hasta que se vuelve a Vectorio.
    if m.waiting_pen.is_some() && m.called_for != m.waiting_pen {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
    }
    m.called_for = m.waiting_pen;
    if m.status == Status::Plotting { ctx.request_repaint_after(Duration::from_millis(500)); }
    if let Some(message) = m.notice.take() { app.notify(ctx, message); }
}

/// Diálogo del cambio de tinta o de la pausa programada: el trazado espera, con la
/// pluma arriba, hasta Aceptar.
pub fn pen_dialog(app: &mut VectorioApp, ctx: &egui::Context) {
    let m = &mut app.machine_ui;
    if m.status != Status::Plotting { return; }
    let pen = m.waiting_pen;
    if pen.is_none() && !m.break_due { return; }
    let pal = theme::current(ctx);
    let screen = ctx.screen_rect();
    let veil = egui::Area::new(egui::Id::new("pen-change-veil")).fixed_pos(screen.min).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.allocate_rect(screen, egui::Sense::click());
        ui.painter().rect_filled(screen, 0.0, theme::veil(ui.visuals().dark_mode));
    });
    ctx.move_to_top(veil.response.layer_id);
    let ink = pen.and_then(|pen| app.doc.pens.get(pen));
    let (title, text) = match pen {
        Some(_) => (tr!("Cambio de tinta"), tr!("Pon %{pen} en la máquina y pulsa Aceptar para seguir. La pluma está arriba y el carro no se mueve.",
            pen = ink.map_or_else(|| "?".into(), crate::i18n::pen_name))),
        None => (tr!("Pausa programada"), tr!("Saca un poco de mina (o revisa la punta) y pulsa Aceptar para seguir. La pluma está arriba y el carro no se mueve.")),
    };
    let sheet = egui::Area::new(egui::Id::new("pen-change-sheet")).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .order(egui::Order::Foreground).show(ctx, |ui| {
        theme::glass_sheet(ui, &pal, |ui| {
            ui.set_max_width(340.0);
            ui.horizontal(|ui| {
                if let Some(ink) = ink {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 8.0, theme::pen_color(ink.color));
                }
                ui.label(RichText::new(title).size(15.0).strong().color(pal.text));
            });
            ui.add_space(8.0);
            ui.label(RichText::new(text).color(pal.text));
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
    let mut status = m.status_text(&app.doc.pens);
    if app.prefs.machine.wifi && matches!(m.status, Status::Off | Status::Connecting) {
        status = format!("{status} · Wi-Fi ({})", app.prefs.machine.wifi_host.trim());
    }
    ui.label(RichText::new(status).color(pal.text));
    let link_ok = !app.prefs.machine.wifi || remote::valid_host(&app.prefs.machine.wifi_host);
    if m.status == Status::Plotting {
        let (i, n) = m.progress;
        ui.add(egui::ProgressBar::new(if n == 0 { 0.0 } else { i as f32 / n as f32 }).desired_height(6.0));
    }
    // Trazar pedido: se conecta si hace falta y espera a la salida calculada.
    let plan = app.output_ui.plan.clone().filter(|p| p.stats.strokes > 0);
    m.follow_plot_request(plan.clone().filter(|_| ready), &app.doc.plot);
    ui.horizontal_wrapped(|ui| match m.status {
        Status::Idle if m.pending.is_some() => {
            if theme::accent_button(ui, &tr!("Empezar")).clicked() {
                if let Some((plan, motion)) = m.pending.take() { m.plot(plan, motion); }
            }
            if ui.button(tr!("Cancelar")).clicked() { m.pending = None; }
        }
        Status::Off | Status::Connecting | Status::Idle | Status::Busy => {
            let off = matches!(m.status, Status::Off | Status::Connecting);
            let idle = m.status == Status::Idle;
            if m.status == Status::Off && ui.add_enabled(link_ok, egui::Button::new(tr!("Conectar"))).on_hover_text(tr!("Conecta la placa para mover la pluma o calibrar. «Trazar» también conecta si hace falta.")).clicked() {
                m.connect(&app.prefs.machine, ui.ctx());
            }
            if m.away && m.engaged && ui.add_enabled_ui(idle, |ui| ui.button(tr!("Volver a empezar"))).inner
                .on_hover_text(tr!("Lleva el carro al origen con la pluma arriba y pone la línea de tiempo al principio."))
                .clicked() {
                m.go_home();
                restart_timeline(&mut app.sim);
            }
            if m.away && !m.engaged && m.status == Status::Off && ui.button(tr!("Volver a empezar")).on_hover_text(tr!("Pone la línea de tiempo al principio. Lleva el carro a mano al origen antes de trazar.")).clicked() {
                restart_timeline(&mut app.sim);
            }
            let can = plan.is_some() && !m.want_plot && m.status != Status::Connecting && (link_ok || !off);
            let trazar = ui.add_enabled_ui(can, |ui| theme::accent_button(ui, &tr!("Trazar en la máquina"))).inner
                .on_hover_text(tr!("Conecta la placa si hace falta y, en cuanto la salida esté calculada, pide la primera tinta. Toma como origen el punto donde esté el carro si los motores están sueltos."))
                .on_disabled_hover_text(if plan.is_none() { tr!("No hay nada que trazar todavía.") } else { tr!("Preparando el trazado…") });
            if trazar.clicked() {
                m.want_plot = true;
                if m.status == Status::Off { m.connect(&app.prefs.machine, ui.ctx()); }
            }
            if !off && ui.add_enabled(idle, egui::Button::new(tr!("Desconectar")))
                .on_hover_text(tr!("Suelta los motores y cierra la conexión USB. Si el carro quedó a medio dibujo, antes vuelve al origen."))
                .clicked() { m.disconnect(); }
        }
        Status::Plotting => {
            if m.paused {
                if theme::accent_button(ui, &tr!("Continuar")).clicked() { m.resume(); }
            } else if ui.button(tr!("Pausar")).clicked() { m.pause(); }
            if ui.button(tr!("Parar")).on_hover_text(tr!("Sube la pluma y deja el carro donde está.")).clicked() { m.stop(); }
        }
    });
    if m.want_plot {
        ui.horizontal(|ui| {
            ui.spinner();
            let why = match m.status {
                Status::Off | Status::Connecting => tr!("Conectando la placa…"),
                Status::Busy => tr!("Preparando la máquina…"),
                _ => tr!("Calculando la salida…"),
            };
            ui.label(RichText::new(why).small().color(pal.text));
            if ui.small_button(tr!("Cancelar")).clicked() { m.want_plot = false; }
        });
    } else if m.status == Status::Connecting || (m.status == Status::Busy && m.pending.is_none()) {
        ui.spinner();
    }
    if let Some(text) = m.time_text() {
        ui.label(RichText::new(text).strong().color(pal.text))
            .on_hover_text(tr!("Lo que queda se calcula con lo que ha tardado la máquina hasta ahora, sin contar pausas ni cambios de tinta."));
    }
    let before = app.prefs.machine.break_minutes;
    let minutes = &mut app.prefs.machine.break_minutes;
    let plotting = m.status == Status::Plotting;
    let mut drying = app.doc.output.drying;
    let mut toggled = false;
    ui.horizontal(|ui| {
        // Secado por zonas: con lápiz o portaminas no hace falta esperar. Mientras traza no se cambia.
        if !plotting {
            toggled = ui.checkbox(&mut drying, RichText::new(tr!("Esperar secado")).small())
                .on_hover_text(tr!("Con rotuladores, no vuelve a pisar una zona hasta que se seca. Con lápiz o portaminas, quítalo: traza sin esperas. Los documentos nuevos recuerdan lo último que elijas."))
                .changed();
            ui.add_space(4.0);
        }
        ui.label(RichText::new(tr!("Pausa")).small().color(pal.tertiary))
            .on_hover_text(tr!("Para portaminas: cada tantos minutos trazando, sube la pluma, se para y te pide sacar mina. Cuenta desde la última pausa; las pausas y los cambios de tinta no suman."));
        let label = |n: u32| if n == 0 { tr!("Nunca") } else { tr!("Cada %{n} min", n = n) };
        egui::ComboBox::from_id_salt("machine-break").icon(crate::theme::combo_icon).selected_text(label(*minutes)).width(96.0).show_ui(ui, |ui| {
            for n in [0, 5, 10, 15, 20, 30, 45, 60] { ui.selectable_value(minutes, n, label(n)); }
        });
        if let Some(left) = m.break_in(*minutes, Instant::now()).filter(|_| !m.paused) {
            ui.label(RichText::new(tr!("próxima en %{time}", time = crate::fmt::duration(left.max(0.0)))).small().color(pal.text));
        }
    });
    let mut changed = app.prefs.machine.break_minutes != before;
    if toggled {
        app.doc.output.drying = drying;
        app.prefs.new_document.output.drying = drying;
        changed = true;
        // El plan pendiente ya no vale: se recalcula y se vuelve a pedir.
        if app.machine_ui.pending.take().is_some() { app.machine_ui.want_plot = true; }
    }
    let m = &mut app.machine_ui;
    if m.away && !m.engaged && m.status != Status::Plotting {
        ui.label(RichText::new(tr!("El carro no está en el origen: con los motores sueltos, llévalo a mano a la esquina superior izquierda antes de volver a trazar.")).small().color(pal.tertiary));
    }
    if let Some(error) = &m.error { ui.label(RichText::new(error).small().color(pal.text)); }
    ui.add_space(4.0);
    if changed { app.prefs_changed(); }
}

/// Línea de tiempo de Salida al principio, parada.
fn restart_timeline(sim: &mut crate::canvas::Sim) {
    sim.playing = false;
    sim.progress = 0.0;
}

/// Calibración de la pluma, origen y recorrido, dentro de los ajustes de Salida.
pub fn settings(app: &mut VectorioApp, ui: &mut egui::Ui) {
    let pal = theme::current(ui.ctx());
    let before = app.prefs.machine.clone();
    let m = &mut app.machine_ui;
    let s = &mut app.prefs.machine;
    let idle = m.status == Status::Idle;
    egui::CollapsingHeader::new(tr!("Máquina: pluma, origen y recorrido")).id_salt("machine-settings").show(ui, |ui| {
        // USB o Wi-Fi (Raspberry Pi junto a la placa): se elige sin conexión y vale al conectar.
        ui.add_enabled_ui(m.status == Status::Off, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(tr!("Conexión"));
                egui::ComboBox::from_id_salt("machine-link")
                    .selected_text(if s.wifi { tr!("Wi-Fi (Raspberry Pi)") } else { tr!("USB") })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.wifi, false, tr!("USB"));
                        ui.selectable_value(&mut s.wifi, true, tr!("Wi-Fi (Raspberry Pi)"));
                    });
                if s.wifi {
                    ui.add(egui::TextEdit::singleline(&mut s.wifi_host).desired_width(120.0).hint_text("pizero.local"))
                        .on_hover_text(tr!("Nombre o IP de la Raspberry Pi conectada a la placa por USB, con el servicio del plóter instalado."));
                }
            });
        });
        if s.wifi && !remote::valid_host(&s.wifi_host) {
            ui.label(RichText::new(tr!("Escribe el nombre de la Raspberry (por ejemplo, pizero.local) o su IP.")).small().color(pal.text));
        }
        ui.add_space(6.0);
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
    struct FakeLink { log: Arc<Mutex<Vec<String>>>, unplugged: Arc<std::sync::atomic::AtomicBool>, reject_moves: Arc<std::sync::atomic::AtomicBool>, slow_moves: Arc<std::sync::atomic::AtomicBool> }

    impl Link for FakeLink {
        fn exchange(&mut self, cmd: &str, _reply: Reply) -> Result<String, MachineError> {
            if self.unplugged.load(std::sync::atomic::Ordering::Relaxed) { return Err(MachineError::Io("Broken pipe".into())); }
            if cmd.starts_with("SM") && self.slow_moves.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(2));
            }
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
        let _ = ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| pen_dialog(&mut app, ctx));
        let out = ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| pen_dialog(&mut app, ctx));
        let pen = crate::i18n::pen_name(&app.doc.pens[1]);
        assert!(out.shapes.iter().any(|s| has_text(&s.shape, "Cambio de tinta")));
        assert!(out.shapes.iter().any(|s| has_text(&s.shape, &format!("Pon {pen} en la máquina"))));
        assert!(control.is_paused());
        let enter = egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
        let _ = ctx.run(egui::RawInput { screen_rect: Some(screen), events: vec![enter], ..Default::default() }, |ctx| pen_dialog(&mut app, ctx));
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
    fn continue_works_even_before_the_machine_has_stopped() {
        let mut ui = MachineUi::default();
        let control = Arc::new(Control::default());
        ui.status = Status::Plotting; ui.control = Some(control.clone()); ui.running = Some(sample_plan());
        ui.started = Some(Instant::now());
        ui.pause();
        assert!(ui.paused && control.is_paused());
        // Continuar antes de que llegue el aviso de pausa del hilo, que ya no llegará.
        ui.resume();
        assert!(!ui.paused && !control.is_paused(), "el botón vuelve a Pausar");
        assert!(ui.paused_since.is_none());
    }

    #[test]
    fn scheduled_break_pauses_after_the_chosen_minutes_of_plotting() {
        let mut ui = MachineUi::default();
        let control = Arc::new(Control::default());
        let start = Instant::now();
        ui.status = Status::Plotting; ui.control = Some(control.clone()); ui.running = Some(sample_plan());
        ui.started = Some(start - Duration::from_secs(9 * 60));
        ui.check_break(10, start);
        assert!(!ui.break_due && !control.is_paused(), "a los 9 min todavía no");
        assert!((ui.break_in(10, start).unwrap() - 60.0).abs() < 1.0);
        ui.check_break(0, start + Duration::from_secs(120));
        assert!(!ui.break_due, "sin pausa programada no para nunca");
        ui.check_break(10, start + Duration::from_secs(61));
        assert!(ui.break_due && ui.paused && control.is_paused(), "a los 10 min se para");
        // Aceptar: sigue y vuelve a contar desde aquí, sin contar lo que estuvo en pausa.
        ui.resume();
        assert!(!ui.break_due && !control.is_paused());
        let left = ui.break_in(10, Instant::now()).unwrap();
        assert!((left - 600.0).abs() < 2.0, "{left}");
    }

    #[test]
    fn plot_request_waits_for_the_board_and_the_output() {
        let plan = sample_plan();
        let motion = PlotSettings::default();
        let mut ui = MachineUi::default();
        ui.want_plot = true;
        ui.status = Status::Connecting;
        ui.follow_plot_request(Some(plan.clone()), &motion);
        assert!(ui.want_plot && ui.pending.is_none(), "espera a la placa");
        ui.status = Status::Idle;
        ui.follow_plot_request(None, &motion);
        assert!(ui.want_plot && ui.pending.is_none(), "espera a la salida");
        ui.follow_plot_request(Some(plan.clone()), &motion);
        assert!(!ui.want_plot && ui.pending.is_some(), "pasa a Empezar");
        // Si la conexión falla, se olvida la petición.
        let mut ui = MachineUi::default();
        ui.want_plot = true; ui.error = Some("sin placa".into());
        ui.follow_plot_request(Some(plan), &motion);
        assert!(!ui.want_plot);
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

    /// Posición del carro en pasos sumando los `SM` enviados.
    fn carriage(log: &[String]) -> (i64, i64) {
        log.iter().filter(|c| c.starts_with("SM,")).fold((0, 0), |(a, b), c| {
            let v: Vec<i64> = c.split(',').skip(2).map(|n| n.parse().unwrap()).collect();
            (a + v[0], b + v[1])
        })
    }

    #[test]
    fn after_a_stop_disconnect_returns_home_first_and_start_over_too() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let link = FakeLink::default();
        link.slow_moves.store(true, std::sync::atomic::Ordering::Relaxed);
        let settings = MachineSettings::default();
        let mut ui = MachineUi::default();
        let l = link.clone();
        ui.connect_with(move || Ok(l), &settings, None);
        wait(&mut ui, &settings, |u| u.status == Status::Idle);
        let stop_midway = |ui: &mut MachineUi| {
            ui.plot(sample_plan(), PlotSettings::default());
            let before = link.log.lock().unwrap().iter().filter(|c| c.starts_with("SM")).count();
            for _ in 0..1000 {
                ui.update(&settings);
                if ui.waiting_pen.is_some() { ui.resume(); ui.waiting_pen = None; }
                if link.log.lock().unwrap().iter().filter(|c| c.starts_with("SM")).count() > before + 20 { break; }
                std::thread::sleep(Duration::from_millis(1));
            }
            ui.stop();
            wait(ui, &settings, |u| u.status == Status::Idle);
            assert!(ui.away && ui.engaged);
            let log = link.log.lock().unwrap().clone();
            assert_ne!(carriage(&log), (0, 0), "se para a medio dibujo: {log:?}");
        };
        // «Volver a empezar»: vuelve al origen y sigue conectada.
        stop_midway(&mut ui);
        ui.go_home();
        wait(&mut ui, &settings, |u| u.status == Status::Idle);
        assert!(!ui.away);
        assert_eq!(carriage(&link.log.lock().unwrap()), (0, 0));
        // Desconectar tras parar: primero al origen y luego suelta los motores.
        stop_midway(&mut ui);
        ui.disconnect();
        wait(&mut ui, &settings, |u| u.status == Status::Off);
        let log = link.log.lock().unwrap().clone();
        assert_eq!(carriage(&log), (0, 0), "vuelve al origen antes de soltar");
        assert_eq!(log.last().map(String::as_str), Some("EM,0,0"));
        assert!(!ui.away);
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
