//! Sesión con la máquina: un hilo que recibe órdenes y devuelve avisos. La pausa y la
//! parada van por `Control`, así responden al momento aunque el hilo esté trazando.
//!
//! La usan la interfaz por USB y `vectorio-machine` en la Raspberry Pi por Wi-Fi, así las
//! dos conexiones se comportan igual.

use super::{Control, Event, Link, MachineError, MachineSettings, Outcome, Plotter};
use crate::{output::OutputPlan, PlotSettings};
use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// Cada cuánto se comprueba, sin trazar, que la placa sigue al otro lado del cable.
pub const PING: Duration = Duration::from_secs(2);

pub enum Command {
    Settings(MachineSettings),
    Pen(bool),
    Release,
    Home,
    Plot(Arc<OutputPlan>, PlotSettings, Arc<Control>),
    Disconnect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Notice {
    Connected(String),
    /// La placa ya no responde o no se pudo abrir; el hilo ha terminado.
    Lost(MachineError),
    /// Desconectada a petición; el hilo ha terminado.
    Closed,
    /// Orden terminada; los motores están activados o sueltos.
    Ready { engaged: bool },
    Failed(MachineError),
    Step { index: usize, total: usize },
    PenChange(usize),
    Paused,
    Resumed,
    Button,
    /// Espera de secado de tantos ms.
    Drying(u32),
    Done(Outcome),
}

/// Fallos que dejan la conexión en duda: se da la máquina por desconectada.
pub fn is_lost(error: &MachineError) -> bool {
    matches!(error, MachineError::Io(_) | MachineError::Timeout(_) | MachineError::NotFound | MachineError::Unreachable | MachineError::Busy)
}

pub fn run<L: Link>(open: impl FnOnce() -> Result<L, MachineError>, settings: MachineSettings, cmds: mpsc::Receiver<Command>, send: &dyn Fn(Notice)) {
    let mut plotter = match open().and_then(|link| Plotter::attach(link, settings)) {
        Ok(p) => p,
        Err(e) => return send(Notice::Lost(e)),
    };
    send(Notice::Connected(plotter.version().to_string()));
    loop {
        let cmd = match cmds.recv_timeout(PING) {
            Ok(cmd) => cmd,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Err(e) = plotter.ebb().motion() {
                    if is_lost(&e) { return send(Notice::Lost(e)); }
                }
                continue;
            }
            // Quien mandaba las órdenes se ha ido: se deja la máquina en reposo.
            Err(mpsc::RecvTimeoutError::Disconnected) => { let _ = plotter.release(); return; }
        };
        let result = match cmd {
            Command::Settings(settings) => plotter.update_settings(settings),
            Command::Pen(up) => plotter.move_pen(up, 300).and_then(|()| plotter.wait_idle()),
            Command::Release => plotter.release(),
            Command::Home => {
                let control = Control::default();
                plotter.go_home(PlotSettings::default().travel_speed, &control, &mut |_| {}).map(|_| ())
            }
            Command::Plot(plan, motion, control) => {
                let mut on_event = |event: Event| send(match event {
                    Event::Step { index, total } => Notice::Step { index, total },
                    Event::PenChange { pen } => Notice::PenChange(pen),
                    Event::Button => Notice::Button,
                    Event::Paused => Notice::Paused,
                    Event::Resumed => Notice::Resumed,
                    Event::Drying { ms } => Notice::Drying(ms),
                });
                plotter.execute(&plan, &motion, &control, &mut on_event).map(|outcome| send(Notice::Done(outcome)))
            }
            Command::Disconnect => { let _ = plotter.release(); return send(Notice::Closed); }
        };
        match result {
            // Un trazado a medias no se puede reanudar: se intenta subir la pluma y soltar
            // los motores para que no se queden haciendo fuerza.
            Err(e) if is_lost(&e) => { let _ = plotter.ebb().motors(false); return send(Notice::Lost(e)) }
            Err(e) => {
                // Antes de mover nada (plan fuera del recorrido, ajustes no válidos) no hay
                // nada que soltar.
                if !matches!(e, MachineError::OutOfBounds | MachineError::InvalidSettings) { let _ = plotter.release(); }
                send(Notice::Failed(e))
            }
            Ok(()) => {}
        }
        send(Notice::Ready { engaged: plotter.is_engaged() });
    }
}

/// Órdenes por la entrada de `vectorio-machine` (una línea JSON cada una). Pausa, sigue y
/// para actúan al momento sobre el trazado en curso.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Wire {
    Settings(MachineSettings),
    Pen(bool),
    Release,
    Home,
    Plot { plan: OutputPlan, footprints: Vec<crate::output::Footprint>, motion: PlotSettings },
    Pause,
    Resume,
    Stop,
    Disconnect,
    /// Nadie consulta la sesión desde hace rato: si no traza, suelta los motores y cierra;
    /// si traza, cierra al terminar, y si espera una tinta o está en pausa, para.
    Idle,
}
