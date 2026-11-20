//! Enlace con el plóter: EiBotBoard (AxiDraw, MakerQ y compatibles) por USB.
//!
//! - `ebb`: órdenes de la placa sobre un `Link` intercambiable (serie real o simulado).
//! - `serial`: busca la placa por su identificador USB y abre el puerto.
//! - `plan`: convierte polilíneas en mm en tramos de velocidad constante con aceleración.
//! - `run`: ejecuta un `OutputPlan` (cambios de tinta, esperas, viajes y trazos).
//! - `session`: hilo de la máquina que recibe órdenes y devuelve avisos, igual por USB y por Wi-Fi.
//! - `remote`: la misma sesión en una Raspberry Pi junto al plóter, por Wi-Fi (servicio de Linear).
//!
//! Las coordenadas de la máquina son mm desde el origen del carro (donde estaba al
//! conectar), con X a la derecha e Y hacia el usuario, igual que el papel (Y hacia abajo).

pub mod ebb;
pub mod plan;
pub mod remote;
pub mod run;
pub mod serial;
pub mod session;

pub use ebb::{Ebb, Link, Motion, Reply};
pub use plan::{Kinematics, Limits, Move};
pub use run::{Control, Event, MachineSettings, Outcome, Plotter};
pub use serial::{find_ports, PortInfo, SerialLink};
pub use session::{Command, Notice};

/// Motivo por el que falló el enlace. El texto para el usuario lo pone la interfaz.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MachineError {
    /// No hay ninguna placa conectada.
    NotFound,
    /// El puerto falló o se desconectó.
    Io(String),
    /// La placa no respondió a tiempo a esta orden.
    Timeout(String),
    /// La placa rechazó una orden (respuesta que empieza por `!`).
    Device(String),
    /// La respuesta no tiene el formato esperado.
    Unexpected(String),
    /// Algún punto del plan queda fuera del recorrido del carro.
    OutOfBounds,
    InvalidSettings,
    /// No se llega a la Raspberry por Wi-Fi (apagada, otra red o sin servicio).
    Unreachable,
    /// La Raspberry está ocupada con otra aplicación o con un trazado anterior.
    Busy,
}
