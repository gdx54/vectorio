//! La máquina por Wi-Fi: una Raspberry Pi conectada a la placa por USB ejecuta la misma
//! sesión (`vectorio-machine`) detrás del servicio del plóter que también usa Linear
//! (`bridge.py`, puerto 8765). Aquí se reenvían las órdenes y se recogen los avisos.
//!
//! El trazado corre en la Raspberry: si se corta el Wi-Fi, sigue hasta terminar.
//!
//!   POST /session              abre la sesión con los ajustes; responde {"id": …}
//!   POST /session/<id>/send    una orden (`Wire`)
//!   GET  /session/<id>?after=N avisos nuevos (`Notice`), si sigue abierta y el siguiente N

use super::session::{Command, Notice, Wire};
use super::{Control, MachineError, MachineSettings};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::collections::VecDeque;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

pub const SERVICE_PORT: u16 = 8765;
/// Sin respuesta de la Raspberry durante este tiempo, se da la conexión por perdida.
const PATIENCE: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(120);

/// Nombre de la Raspberry: `pizero.local`, otro nombre de la red o una IP.
pub fn valid_host(host: &str) -> bool {
    let host = host.trim();
    !host.is_empty() && host.len() <= 253 && !host.starts_with(['-', '.'])
        && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

/// Cliente HTTP mínimo del servicio: una petición por conexión, como responde el servicio.
pub struct Bridge {
    host: String,
    port: u16,
    addr: Option<SocketAddr>,
}

impl Bridge {
    pub fn new(host: &str) -> Self { Bridge { host: host.trim().to_string(), port: SERVICE_PORT, addr: None } }

    pub fn with_port(host: &str, port: u16) -> Self { Bridge { port, ..Self::new(host) } }

    /// La dirección se resuelve una vez (`.local` tarda) y se reutiliza.
    fn connect(&mut self, timeout: Duration) -> Result<TcpStream, MachineError> {
        if let Some(addr) = self.addr {
            if let Ok(stream) = TcpStream::connect_timeout(&addr, timeout) { return Ok(stream); }
        }
        let addrs = (self.host.as_str(), self.port).to_socket_addrs().map_err(|_| MachineError::Unreachable)?;
        for addr in addrs {
            if let Ok(stream) = TcpStream::connect_timeout(&addr, timeout) { self.addr = Some(addr); return Ok(stream); }
        }
        Err(MachineError::Unreachable)
    }

    pub fn call(&mut self, method: &str, path: &str, body: Option<&[u8]>, timeout: Duration) -> Result<(u16, Value), MachineError> {
        let mut stream = self.connect(Duration::from_secs(5))?;
        let lost = |_| MachineError::Unreachable;
        stream.set_read_timeout(Some(timeout)).map_err(lost)?;
        stream.set_write_timeout(Some(timeout)).map_err(lost)?;
        let body = body.unwrap_or_default();
        let head = format!("{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            self.host, body.len());
        stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(body)).map_err(lost)?;
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).map_err(lost)?;
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or(MachineError::Unreachable)?;
        let head = String::from_utf8_lossy(&raw[..split]);
        let code = head.split_whitespace().nth(1).and_then(|c| c.parse::<u16>().ok()).ok_or(MachineError::Unreachable)?;
        let value = serde_json::from_slice(&raw[split + 4..]).unwrap_or(Value::Null);
        Ok((code, value))
    }

    /// Abre la sesión de la máquina en la Raspberry.
    pub fn open(&mut self, settings: &MachineSettings) -> Result<String, MachineError> {
        let body = json!({ "app": "vectorio", "settings": settings }).to_string();
        match self.call("POST", "/session", Some(body.as_bytes()), Duration::from_secs(15))? {
            (200, reply) => reply["id"].as_str().filter(|id| id.len() == 36).map(str::to_string)
                .ok_or_else(|| MachineError::Unexpected(reply.to_string())),
            (409, _) => Err(MachineError::Busy),
            (503, _) => Err(MachineError::NotFound),
            (_, reply) => Err(MachineError::Unexpected(reply["error"].as_str().unwrap_or("").chars().take(160).collect())),
        }
    }

    pub fn send(&mut self, id: &str, order: &Value) -> Result<(), MachineError> {
        let body = order.to_string();
        let timeout = Duration::from_secs(15 + body.len() as u64 / 200_000);
        match self.call("POST", &format!("/session/{id}/send"), Some(body.as_bytes()), timeout)? {
            (200, _) => Ok(()),
            (404, _) => Err(MachineError::Unreachable),
            (_, reply) => Err(MachineError::Unexpected(reply.to_string())),
        }
    }

    /// Avisos posteriores a `after`, si la sesión sigue abierta y el número siguiente.
    pub fn poll(&mut self, id: &str, after: u64) -> Result<(Vec<Notice>, bool, u64), MachineError> {
        match self.call("GET", &format!("/session/{id}?after={after}"), None, Duration::from_secs(5))? {
            (200, reply) => {
                let notes = reply["notes"].as_array().map(|list| list.iter().filter_map(|n| serde_json::from_value(n.clone()).ok()).collect()).unwrap_or_default();
                Ok((notes, reply["running"].as_bool().unwrap_or(false), reply["next"].as_u64().unwrap_or(after)))
            }
            // El servicio se reinició: la sesión ya no existe.
            (404, _) => Err(MachineError::Io("session".into())),
            (_, reply) => Err(MachineError::Unexpected(reply.to_string())),
        }
    }
}

/// Orden de la interfaz en el formato de la Raspberry.
fn wire(cmd: &Command) -> Value {
    match cmd {
        // El plan va por referencia: puede ocupar varios MB.
        Command::Plot(plan, motion, _) => json!({ "Plot": { "plan": &**plan, "footprints": plan.footprints(), "motion": motion } }),
        Command::Settings(s) => serde_json::to_value(Wire::Settings(s.clone())).unwrap_or(Value::Null),
        Command::Pen(up) => serde_json::to_value(Wire::Pen(*up)).unwrap_or(Value::Null),
        Command::Release => json!("Release"),
        Command::Home => json!("Home"),
        Command::Disconnect => json!("Disconnect"),
    }
}

/// Como `session::run`, pero con la sesión en la Raspberry. Pausa, sigue y para se leen del
/// `Control` del trazado en curso y se reenvían; las pausas que decide la máquina (cambio
/// de tinta, botón) se reflejan en ese mismo `Control`, como si fuera la sesión local.
pub fn run(host: &str, settings: MachineSettings, cmds: mpsc::Receiver<Command>, send: &dyn Fn(Notice)) {
    run_with(Bridge::new(host), settings, cmds, send)
}

pub fn run_with(mut bridge: Bridge, settings: MachineSettings, cmds: mpsc::Receiver<Command>, send: &dyn Fn(Notice)) {
    let id = match bridge.open(&settings) {
        Ok(id) => id,
        Err(e) => return send(Notice::Lost(e)),
    };
    let mut after = 0;
    let mut control: Option<Arc<Control>> = None;
    // Lo que la Raspberry ya sabe del trazado en curso: en pausa y parado.
    let (mut paused, mut stopped) = (false, false);
    let mut heard = Instant::now();
    // Órdenes enviadas que aún no han dado su `Ready` (la sesión las atiende en orden):
    // el `Ready` de una orden anterior no debe olvidar el trazado que va detrás.
    let mut queue: VecDeque<bool> = VecDeque::new();
    loop {
        loop {
            let cmd = match cmds.try_recv() {
                Ok(cmd) => cmd,
                Err(mpsc::TryRecvError::Empty) => break,
                // La interfaz se ha ido: la Raspberry deja la máquina en reposo.
                Err(mpsc::TryRecvError::Disconnected) => { let _ = bridge.send(&id, &json!("Disconnect")); return; }
            };
            let order = wire(&cmd);
            if !matches!(cmd, Command::Disconnect) { queue.push_back(matches!(cmd, Command::Plot(..))); }
            if let Command::Plot(_, _, c) = cmd { control = Some(c); (paused, stopped) = (false, false); }
            if let Err(e) = retry(&mut bridge, &id, &order) { return send(Notice::Lost(e)); }
        }
        if let Some(c) = &control {
            let order = if c.is_stopped() && !stopped { stopped = true; Some("Stop") }
                else if !stopped && c.is_paused() != paused { paused = c.is_paused(); Some(if paused { "Pause" } else { "Resume" }) }
                else { None };
            if let Some(order) = order {
                if let Err(e) = retry(&mut bridge, &id, &json!(order)) { return send(Notice::Lost(e)); }
            }
        }
        match bridge.poll(&id, after) {
            Ok((notes, running, next)) => {
                heard = Instant::now();
                after = next;
                let mut ended = false;
                for note in notes {
                    match &note {
                        Notice::Paused | Notice::PenChange(_) | Notice::Button => { if let Some(c) = &control { c.pause(); } paused = true; }
                        Notice::Resumed => { if let Some(c) = &control { c.resume(); } paused = false; }
                        Notice::Ready { .. } => if queue.pop_front() == Some(true) && !queue.contains(&true) { control = None },
                        Notice::Lost(_) | Notice::Closed => ended = true,
                        _ => {}
                    }
                    send(note);
                }
                if ended { return; }
                if !running { return send(Notice::Lost(MachineError::Io("session".into()))); }
            }
            Err(MachineError::Io(e)) => return send(Notice::Lost(MachineError::Io(e))),
            Err(_) if heard.elapsed() > PATIENCE => return send(Notice::Lost(MachineError::Unreachable)),
            Err(_) => {}
        }
        std::thread::sleep(POLL);
    }
}

/// Una orden no se pierde por un corte breve del Wi-Fi.
fn retry(bridge: &mut Bridge, id: &str, order: &Value) -> Result<(), MachineError> {
    let start = Instant::now();
    loop {
        match bridge.send(id, order) {
            Err(MachineError::Unreachable) if start.elapsed() < PATIENCE => std::thread::sleep(Duration::from_millis(500)),
            other => return other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_names() {
        assert!(valid_host("pizero.local") && valid_host("192.168.1.139") && valid_host("plotter-pi"));
        assert!(!valid_host("") && !valid_host("pi zero") && !valid_host("-x") && !valid_host("a/b") && !valid_host("x:80"));
    }

    /// Servicio simulado: abre la sesión, guarda las órdenes y entrega los avisos de `script`
    /// de uno en uno (uno por consulta), como si la máquina tardara.
    fn fake_bridge(script: Vec<Notice>) -> (u16, Arc<std::sync::Mutex<Vec<Value>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let orders: Arc<std::sync::Mutex<Vec<Value>>> = Arc::default();
        let seen = orders.clone();
        std::thread::spawn(move || {
            let id = "123e4567-e89b-12d3-a456-426614174000";
            let mut sent = 0usize;
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut raw = Vec::new();
                let mut buf = [0u8; 65536];
                let body = loop {
                    let n = stream.read(&mut buf).unwrap();
                    raw.extend_from_slice(&buf[..n]);
                    if let Some(split) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&raw[..split]).to_lowercase();
                        let len: usize = head.lines().find_map(|l| l.strip_prefix("content-length:")).map(|v| v.trim().parse().unwrap()).unwrap_or(0);
                        if raw.len() >= split + 4 + len { break raw[split + 4..split + 4 + len].to_vec(); }
                    }
                };
                let line = String::from_utf8_lossy(&raw).lines().next().unwrap_or("").to_string();
                let reply = if line.starts_with("POST /session ") {
                    json!({ "id": id })
                } else if line.starts_with("POST /session/") {
                    seen.lock().unwrap().push(serde_json::from_slice(&body).unwrap());
                    json!({ "ok": true })
                } else {
                    // Un aviso por consulta, desde el que pide el cliente.
                    let after: usize = line.split("after=").nth(1).and_then(|v| v.split_whitespace().next()).and_then(|v| v.parse().ok()).unwrap_or(0);
                    // Los avisos de trazado esperan a que llegue el plan.
                    let ready = seen.lock().unwrap().iter().any(|o| o.get("Plot").is_some());
                    let limit = if ready { script.len() } else { 2.min(script.len()) };
                    if after < limit { sent = after + 1; }
                    let notes: Vec<&Notice> = script[after.min(sent)..sent.max(after)].iter().collect();
                    json!({ "id": id, "running": true, "notes": notes, "next": sent.max(after) })
                };
                let body = reply.to_string();
                let _ = write!(stream, "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            }
        });
        (port, orders)
    }

    #[test]
    fn relays_orders_and_mirrors_pauses_from_the_machine() {
        use crate::{output::{prepare, OutputSettings}, p, Paper, Pen, PlotLayer, PlotSettings};
        let script = vec![Notice::Connected("2.8.1".into()), Notice::Ready { engaged: false },
            Notice::Step { index: 0, total: 2 }, Notice::PenChange(1), Notice::Resumed,
            Notice::Done(super::super::Outcome::Finished), Notice::Ready { engaged: true }, Notice::Closed];
        let (port, orders) = fake_bridge(script);
        let plan = prepare(&[PlotLayer { pen: 0, paths: vec![vec![p(2.0, 2.0), p(9.0, 4.0)]] }],
            &Paper { name: "Prueba".into(), width_mm: 30.0, height_mm: 20.0, margin_mm: 0.0 },
            &[Pen::new("Negro", [0, 0, 0], 0.5)], &PlotSettings::default(), &OutputSettings::default()).unwrap();
        let (tx, rx) = mpsc::channel();
        let notes: Arc<std::sync::Mutex<Vec<Notice>>> = Arc::default();
        let got = notes.clone();
        let client = std::thread::spawn(move || {
            run_with(Bridge::with_port("127.0.0.1", port), MachineSettings::default(), rx, &|n| got.lock().unwrap().push(n));
        });
        let control = Arc::new(Control::default());
        let wait = |what: &dyn Fn(&[Notice]) -> bool| {
            let start = Instant::now();
            while !what(&notes.lock().unwrap()) { assert!(start.elapsed() < Duration::from_secs(10), "{:?}", notes.lock().unwrap()); std::thread::sleep(Duration::from_millis(20)); }
        };
        // Pluma arriba y trazado seguidos: el `Ready` de la pluma llega con el trazado ya
        // enviado y no debe soltar su control.
        tx.send(Command::Pen(true)).unwrap();
        tx.send(Command::Plot(Arc::new(plan.clone()), PlotSettings::default(), control.clone())).unwrap();
        // El cambio de tinta lo decide la máquina: el Control local queda en pausa…
        wait(&|n| n.contains(&Notice::PenChange(1)));
        assert!(control.is_paused() || notes.lock().unwrap().contains(&Notice::Resumed));
        wait(&|n| n.contains(&Notice::Closed));
        client.join().unwrap();
        // …y al reanudarse allí, también aquí, sin reenviar pausas ni reanudaciones.
        assert!(!control.is_paused());
        let orders = orders.lock().unwrap();
        assert_eq!(orders.len(), 2, "{orders:?}");
        assert_eq!(orders[0], json!({ "Pen": true }));
        let sent = &orders[1]["Plot"];
        assert_eq!(sent["footprints"].as_array().unwrap().len(), plan.strokes.len());
        let mut back: crate::output::OutputPlan = serde_json::from_value(sent["plan"].clone()).unwrap();
        assert!(back.set_footprints(serde_json::from_value(sent["footprints"].clone()).unwrap()));
        assert_eq!(back.footprints(), plan.footprints());
    }

    #[test]
    fn local_pause_and_stop_reach_the_machine_once() {
        use crate::{output::{prepare, OutputSettings}, p, Paper, Pen, PlotLayer, PlotSettings};
        let (port, orders) = fake_bridge(vec![Notice::Connected("2.8.1".into())]);
        let plan = prepare(&[PlotLayer { pen: 0, paths: vec![vec![p(2.0, 2.0), p(9.0, 4.0)]] }],
            &Paper { name: "Prueba".into(), width_mm: 30.0, height_mm: 20.0, margin_mm: 0.0 },
            &[Pen::new("Negro", [0, 0, 0], 0.5)], &PlotSettings::default(), &OutputSettings::default()).unwrap();
        let (tx, rx) = mpsc::channel();
        let client = std::thread::spawn(move || run_with(Bridge::with_port("127.0.0.1", port), MachineSettings::default(), rx, &|_| {}));
        let control = Arc::new(Control::default());
        tx.send(Command::Plot(Arc::new(plan), PlotSettings::default(), control.clone())).unwrap();
        let count = |name: &str| orders.lock().unwrap().iter().filter(|o| o.as_str() == Some(name)).count();
        let wait = |name: &str, n: usize| { let start = Instant::now(); while count(name) < n { assert!(start.elapsed() < Duration::from_secs(10)); std::thread::sleep(Duration::from_millis(20)); } };
        control.pause();
        wait("Pause", 1);
        control.resume();
        wait("Resume", 1);
        control.stop();
        wait("Stop", 1);
        drop(tx);
        client.join().unwrap();
        assert_eq!((count("Pause"), count("Resume"), count("Stop"), count("Disconnect")), (1, 1, 1, 1));
    }

    #[test]
    fn notices_round_trip() {
        for note in [Notice::Lost(MachineError::Busy), Notice::Step { index: 3, total: 9 }, Notice::Paused, Notice::Done(super::super::Outcome::Stopped)] {
            let text = serde_json::to_string(&note).unwrap();
            assert_eq!(serde_json::from_str::<Notice>(&text).unwrap(), note);
        }
    }
}
