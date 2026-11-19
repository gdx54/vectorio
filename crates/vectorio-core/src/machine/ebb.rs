//! Órdenes de la EiBotBoard (firmware 2.x). La placa responde `OK` a cada orden en
//! cuanto la acepta; con la cola de movimiento llena tarda en aceptarla, y esa espera
//! es el control de flujo: el ordenador nunca va más de un par de tramos por delante.

use super::{plan::Move, MachineError};

/// Forma de la respuesta que espera cada orden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    /// Solo `OK`.
    Ok,
    /// Una línea de datos sin `OK` (`V`, `QM`).
    Line,
    /// Líneas de datos seguidas de `OK` (`QS`, `QP`, `QC`…).
    LineOk,
}

/// Canal de órdenes con la placa: el puerto serie o un simulador en las pruebas.
pub trait Link: Send {
    /// Envía `cmd` (sin el retorno de carro) y devuelve los datos de la respuesta.
    fn exchange(&mut self, cmd: &str, reply: Reply) -> Result<String, MachineError>;
}

/// Estado de movimiento (`QM`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Motion {
    pub executing: bool,
    pub motor1: bool,
    pub motor2: bool,
    pub queued: bool,
}

impl Motion {
    pub fn idle(&self) -> bool { !self.executing && !self.motor1 && !self.motor2 && !self.queued }
}

/// Escala del servo de la pluma en unidades de la placa (1/12 µs), la de AxiDraw:
/// 0 % y 100 % de altura.
pub const SERVO_MIN: f64 = 9855.0;
pub const SERVO_MAX: f64 = 27831.0;

pub fn servo_position(pct: f64) -> u32 {
    (SERVO_MIN + (SERVO_MAX - SERVO_MIN) * pct.clamp(0.0, 100.0) / 100.0).round() as u32
}

/// Velocidad del servo para `SC,11` y `SC,12` con la escala de AxiDraw: al 100 %
/// recorre toda la escala en 200 ms. La placa suma esa cantidad en cada refresco del
/// servo, cada 24 ms (8 canales de 3 ms); el 0,03 de AxiDraw es solo para el servo de
/// banda estrecha, con un único canal.
pub fn servo_rate(pct: f64) -> u32 {
    ((SERVO_MAX - SERVO_MIN) * 0.24 / 200.0 * pct.clamp(1.0, 100.0)).round() as u32
}

pub struct Ebb<L> {
    link: L,
}

impl<L: Link> Ebb<L> {
    pub fn new(link: L) -> Self { Ebb { link } }

    pub fn link(&self) -> &L { &self.link }

    pub fn into_link(self) -> L { self.link }

    fn ok(&mut self, cmd: &str) -> Result<(), MachineError> { self.link.exchange(cmd, Reply::Ok).map(|_| ()) }

    /// Texto de versión, p. ej. «EBBv13_and_above EB Firmware Version 2.8.1».
    pub fn version(&mut self) -> Result<String, MachineError> {
        let v = self.link.exchange("V", Reply::Line)?;
        if v.contains("EBB") || v.contains("Firmware") { Ok(v) } else { Err(MachineError::Unexpected(v)) }
    }

    /// Alturas de la pluma arriba y abajo, en % de la escala de AxiDraw, y velocidad del
    /// servo como AxiDraw al subir al 75 % y bajar al 50 %: así la pluma llega en el tiempo
    /// de subir y bajar de serie (0,2 s).
    pub fn configure_pen(&mut self, up_pct: f64, down_pct: f64) -> Result<(), MachineError> {
        self.ok(&format!("SC,4,{}", servo_position(up_pct)))?;
        self.ok(&format!("SC,5,{}", servo_position(down_pct)))?;
        self.ok(&format!("SC,11,{}", servo_rate(75.0)))?;
        self.ok(&format!("SC,12,{}", servo_rate(50.0)))
    }

    /// Sube o baja la pluma; los movimientos siguientes esperan `delay_ms`.
    pub fn pen(&mut self, up: bool, delay_ms: u32) -> Result<(), MachineError> {
        self.ok(&format!("SP,{},{}", u8::from(up), delay_ms))
    }

    /// Activa los dos motores a 1/16 de paso, o los suelta para mover el carro a mano.
    pub fn motors(&mut self, on: bool) -> Result<(), MachineError> {
        self.ok(if on { "EM,1,1" } else { "EM,0,0" })
    }

    /// Pone a cero el contador de pasos de la placa.
    pub fn clear_steps(&mut self) -> Result<(), MachineError> { self.ok("CS") }

    /// Tramo a velocidad constante; sin pasos actúa como espera.
    pub fn step(&mut self, mv: Move) -> Result<(), MachineError> {
        self.ok(&format!("SM,{},{},{}", mv.ms, mv.m1, mv.m2))
    }

    pub fn motion(&mut self) -> Result<Motion, MachineError> {
        let r = self.link.exchange("QM", Reply::Line)?;
        let f: Vec<&str> = r.split(',').map(str::trim).collect();
        let flag = |i: usize| f.get(i).map(|v| *v != "0").ok_or_else(|| MachineError::Unexpected(r.clone()));
        if f.first() != Some(&"QM") { return Err(MachineError::Unexpected(r)); }
        Ok(Motion { executing: flag(1)?, motor1: flag(2)?, motor2: flag(3)?, queued: flag(4)? })
    }

    /// Posición de los motores en pasos desde el último `CS`.
    pub fn steps(&mut self) -> Result<(i64, i64), MachineError> {
        let r = self.link.exchange("QS", Reply::LineOk)?;
        let mut it = r.split(',').map(|v| v.trim().parse::<i64>());
        match (it.next(), it.next()) {
            (Some(Ok(a)), Some(Ok(b))) => Ok((a, b)),
            _ => Err(MachineError::Unexpected(r)),
        }
    }

    /// Si se ha pulsado el botón de la placa desde la última consulta (la placa lo
    /// anota y lo borra al leerlo). No para nada por sí solo.
    pub fn button(&mut self) -> Result<bool, MachineError> {
        let r = self.link.exchange("QB", Reply::LineOk)?;
        match r.trim() { "1" => Ok(true), "0" => Ok(false), _ => Err(MachineError::Unexpected(r)) }
    }

    pub fn pen_is_up(&mut self) -> Result<bool, MachineError> {
        let r = self.link.exchange("QP", Reply::LineOk)?;
        match r.trim() { "1" => Ok(true), "0" => Ok(false), _ => Err(MachineError::Unexpected(r)) }
    }

    /// Lecturas crudas (0–1023) del ajuste de corriente de los motores y de la
    /// tensión de alimentación. Sin alimentador la segunda queda cerca de 0.
    pub fn power(&mut self) -> Result<(u16, u16), MachineError> {
        let r = self.link.exchange("QC", Reply::LineOk)?;
        let mut it = r.split(',').map(|v| v.trim().parse::<u16>());
        match (it.next(), it.next()) {
            (Some(Ok(a)), Some(Ok(b))) => Ok((a, b)),
            _ => Err(MachineError::Unexpected(r)),
        }
    }
}

/// Versión numérica a partir del texto de `V`.
pub fn firmware_version(text: &str) -> Option<(u32, u32, u32)> {
    let v = text.rsplit(' ').next()?;
    let mut it = v.split('.').map(|n| n.trim().parse::<u32>().ok());
    Some((it.next()??, it.next()??, it.next().flatten().unwrap_or(0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_firmware_version() {
        assert_eq!(firmware_version("EBBv13_and_above EB Firmware Version 2.8.1"), Some((2, 8, 1)));
        assert_eq!(firmware_version("basura"), None);
    }

    #[test]
    fn servo_rate_matches_axidraw() {
        // pen_handling.py de AxiDraw 3.9.6 con el servo estándar: 17976 × 0,24 / 200 × %.
        assert_eq!((servo_rate(75.0), servo_rate(50.0)), (1618, 1079));
        // A esa velocidad, bajar del 85 % al 42 % cabe en el tiempo de serie (0,2 s).
        let ms = (SERVO_MAX - SERVO_MIN) * 0.43 / servo_rate(50.0) as f64 * 24.0;
        assert!(ms < 200.0, "{ms} ms");
    }

    #[test]
    fn servo_scale_matches_axidraw() {
        assert_eq!(servo_position(0.0), 9855);
        assert_eq!(servo_position(100.0), 27831);
        assert_eq!(servo_position(60.0), 20641);
        assert_eq!(servo_position(150.0), 27831);
    }
}
