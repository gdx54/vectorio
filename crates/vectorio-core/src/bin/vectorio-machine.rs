//! Sesión de la máquina en la Raspberry Pi junto al plóter (Vectorio por Wi-Fi).
//!
//! La lanza el servicio del plóter (`bridge.py`): lee órdenes `Wire` por la entrada, una
//! línea JSON cada una, y escribe avisos `Notice` por la salida. Es la misma sesión que la
//! interfaz usa por USB, así que traza, pausa y cambia de tinta igual.
//!
//!   vectorio-machine --port /dev/ttyACM0 --settings ajustes.json

use std::collections::VecDeque;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use vectorio_core::machine::session::{self, Command, Notice, Wire};
use vectorio_core::machine::{Control, MachineSettings, SerialLink};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let (Some(port), Some(settings)) = (value("--port"), value("--settings")) else {
        eprintln!("uso: vectorio-machine --port /dev/ttyACM0 --settings ajustes.json");
        std::process::exit(2);
    };
    let settings: MachineSettings = match std::fs::read(&settings).ok().and_then(|data| serde_json::from_slice(&data).ok()) {
        Some(s) => s,
        None => { eprintln!("ajustes no válidos"); std::process::exit(2); }
    };

    let (tx, cmds) = mpsc::channel::<Command>();
    // Trazado en curso: pausa, sigue y para actúan sobre él sin esperar a la cola.
    let current: Arc<Mutex<Option<Arc<Control>>>> = Arc::default();
    // Órdenes pendientes de su `Ready`, en orden (con el control si son un trazado): el
    // `Ready` de una orden anterior no debe olvidar el trazado que va detrás.
    let queue: Arc<Mutex<VecDeque<Option<Arc<Control>>>>> = Arc::default();
    // Sin nadie al otro lado: cerrar en cuanto termine lo que esté haciendo.
    let orphan = Arc::new(AtomicBool::new(false));

    let worker = {
        let (tx, current, orphan, queue) = (tx.clone(), current.clone(), orphan.clone(), queue.clone());
        std::thread::spawn(move || {
            let out = Mutex::new(std::io::stdout());
            let send = |note: Notice| {
                if matches!(note, Notice::Ready { .. }) {
                    let mut queue = queue.lock().unwrap();
                    let mut current = current.lock().unwrap();
                    if let (Some(Some(done)), Some(now)) = (queue.pop_front(), current.as_ref()) {
                        if Arc::ptr_eq(&done, now) { *current = None; }
                    }
                    if orphan.load(Ordering::Relaxed) && queue.is_empty() { let _ = tx.send(Command::Disconnect); }
                }
                if let Ok(line) = serde_json::to_string(&note) {
                    let mut out = out.lock().unwrap();
                    let _ = writeln!(out, "{line}");
                    let _ = out.flush();
                }
            };
            session::run(|| SerialLink::open(&port), settings, cmds, &send);
            // La sesión ha terminado (cerrada, placa perdida): no se espera a la entrada,
            // que el servicio mantiene abierta, para soltar la placa en el acto.
            let _ = std::io::stdout().flush();
            std::process::exit(0);
        })
    };

    let idle = |current: &Mutex<Option<Arc<Control>>>, tx: &mpsc::Sender<Command>| {
        orphan.store(true, Ordering::Relaxed);
        match current.lock().unwrap().as_ref() {
            // Esperando una tinta o en pausa: la pluma ya está arriba; se para.
            Some(control) if control.is_paused() => control.stop(),
            Some(_) => {}
            None => { let _ = tx.send(Command::Disconnect); }
        }
    };

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(order) = serde_json::from_str::<Wire>(&line) else { continue };
        let control = current.lock().unwrap().clone();
        let cmd = match order {
            Wire::Settings(s) => Command::Settings(s),
            Wire::Pen(up) => Command::Pen(up),
            Wire::Release => Command::Release,
            Wire::Home => Command::Home,
            Wire::Plot { mut plan, footprints, motion } => {
                if !plan.set_footprints(footprints) { continue; }
                let control = Arc::new(Control::default());
                *current.lock().unwrap() = Some(control.clone());
                Command::Plot(Arc::new(plan), motion, control)
            }
            Wire::Pause => { if let Some(c) = control { c.pause(); } continue; }
            Wire::Resume => { if let Some(c) = control { c.resume(); } continue; }
            Wire::Stop => { if let Some(c) = control { c.stop(); } continue; }
            Wire::Disconnect => Command::Disconnect,
            Wire::Idle => { idle(&current, &tx); continue; }
        };
        if !matches!(cmd, Command::Disconnect) {
            let plot = match &cmd { Command::Plot(_, _, c) => Some(c.clone()), _ => None };
            queue.lock().unwrap().push_back(plot);
        }
        if tx.send(cmd).is_err() { break; }
    }
    // Entrada cerrada (el servicio se ha ido): como sin nadie al otro lado.
    idle(&current, &tx);
    drop(tx);
    let _ = worker.join();
}
