//! Puerto serie USB de la EiBotBoard. macOS la reconoce con su driver CDC, sin
//! instalar nada, como `/dev/cu.usbmodem…`.

use super::{ebb::Link, MachineError, Reply};
use serialport::{ClearBuffer, SerialPort, SerialPortType};
use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

/// Identificador USB de la EiBotBoard (Microchip / SchmalzHaus).
pub const EBB_VID: u16 = 0x04D8;
pub const EBB_PID: u16 = 0xFD92;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortInfo {
    pub path: String,
    pub serial: Option<String>,
    pub product: Option<String>,
}

/// Placas conectadas. El nombre del puerto cambia según el conector USB, así que se
/// buscan por su identificador.
pub fn find_ports() -> Vec<PortInfo> {
    let Ok(ports) = serialport::available_ports() else { return vec![] };
    let mut out: Vec<PortInfo> = Vec::new();
    for port in ports {
        let SerialPortType::UsbPort(usb) = port.port_type else { continue };
        if usb.vid != EBB_VID || usb.pid != EBB_PID { continue; }
        // macOS ofrece cada placa como /dev/tty.* y /dev/cu.*; cu no espera a la portadora.
        let path = port.port_name.replacen("/dev/tty.", "/dev/cu.", 1);
        if out.iter().all(|p| p.path != path) {
            out.push(PortInfo { path, serial: usb.serial_number, product: usb.product });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

pub struct SerialLink {
    port: Box<dyn SerialPort>,
    buf: Vec<u8>,
    /// Tiempo máximo de respuesta. Una orden de movimiento puede esperar a que se
    /// vacíe la cola, y una pluma con retardo largo la retiene varios segundos.
    pub timeout: Duration,
}

impl SerialLink {
    pub fn open(path: &str) -> Result<Self, MachineError> {
        let port = serialport::new(path, 115_200)
            .timeout(Duration::from_millis(20))
            .open()
            .map_err(|e| MachineError::Io(e.to_string()))?;
        port.clear(ClearBuffer::All).map_err(|e| MachineError::Io(e.to_string()))?;
        Ok(SerialLink { port, buf: Vec::new(), timeout: Duration::from_secs(10) })
    }

    /// Abre la primera placa conectada.
    pub fn open_first() -> Result<(Self, PortInfo), MachineError> {
        let info = find_ports().into_iter().next().ok_or(MachineError::NotFound)?;
        Ok((Self::open(&info.path)?, info))
    }

    /// Siguiente línea no vacía. La placa termina unas respuestas en `\r\n` y otras en
    /// `\n\r`, así que se corta por cualquiera de los dos.
    fn read_line(&mut self, deadline: Instant, cmd: &str) -> Result<String, MachineError> {
        loop {
            while let Some(i) = self.buf.iter().position(|&b| b == b'\r' || b == b'\n') {
                let raw: Vec<u8> = self.buf.drain(..=i).collect();
                let line = String::from_utf8_lossy(&raw[..raw.len() - 1]).trim().to_string();
                if !line.is_empty() { return Ok(line); }
            }
            if Instant::now() > deadline { return Err(MachineError::Timeout(cmd.into())); }
            let mut chunk = [0u8; 256];
            match self.port.read(&mut chunk) {
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e) if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
                Err(e) => return Err(MachineError::Io(e.to_string())),
            }
        }
    }
}

impl Link for SerialLink {
    fn exchange(&mut self, cmd: &str, reply: Reply) -> Result<String, MachineError> {
        self.port.write_all(format!("{cmd}\r").as_bytes()).map_err(|e| MachineError::Io(e.to_string()))?;
        let deadline = Instant::now() + self.timeout;
        let mut data: Vec<String> = Vec::new();
        loop {
            let line = self.read_line(deadline, cmd)?;
            if line.starts_with('!') { return Err(MachineError::Device(line)); }
            match reply {
                Reply::Ok if line == "OK" => return Ok(String::new()),
                Reply::Ok => {}
                Reply::Line => return Ok(line),
                Reply::LineOk if line == "OK" => return Ok(data.join("\n")),
                Reply::LineOk => data.push(line),
            }
        }
    }
}
