//! Prueba del enlace con la máquina real.
//!
//!   cargo run -p vectorio-core --example maquina_prueba --release -- info
//!   cargo run -p vectorio-core --example maquina_prueba --release -- aire
//!   cargo run -p vectorio-core --example maquina_prueba --release -- papel
//!
//! `info` solo pregunta (no mueve nada). `boton [s]` escucha el botón de la placa
//! (20 s si no se indica). `soltar` sube la pluma y suelta los motores.
//! `pluma <%>` mueve solo el servo a esa altura. `aire` recorre la muestra con la pluma
//! siempre arriba. `papel` la dibuja. La muestra ocupa de 10 a 50 mm a la derecha y
//! hacia el usuario desde donde esté el carro, que se toma como origen.

use std::time::Instant;
use vectorio_core::machine::{ebb::firmware_version, Control, Ebb, Event, MachineSettings, Plotter, SerialLink};
use vectorio_core::output::{prepare, OutputSettings};
use vectorio_core::{p, Paper, Pen, PlotLayer, PlotSettings, P};

fn sample() -> Vec<PlotLayer> {
    let square = vec![p(10.0, 10.0), p(50.0, 10.0), p(50.0, 50.0), p(10.0, 50.0), p(10.0, 10.0)];
    let circle: Vec<P> = (0..=180).map(|i| {
        let a = i as f64 / 180.0 * std::f64::consts::TAU;
        p(30.0 + 14.0 * a.cos(), 30.0 + 14.0 * a.sin())
    }).collect();
    let zigzag: Vec<P> = (0..=8).map(|i| p(14.0 + i as f64 * 4.0, if i % 2 == 0 { 46.0 } else { 42.0 })).collect();
    let paths = vec![square, circle, vec![p(10.0, 10.0), p(50.0, 50.0)], vec![p(50.0, 10.0), p(10.0, 50.0)], zigzag];
    vec![PlotLayer { pen: 0, paths }]
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "info".into());
    let (link, info) = match SerialLink::open_first() {
        Ok(v) => v,
        Err(e) => { eprintln!("No se encuentra la máquina: {e:?}"); std::process::exit(1); }
    };
    println!("Puerto: {}", info.path);
    if mode == "info" {
        let mut ebb = Ebb::new(link);
        let version = ebb.version().expect("versión");
        println!("Firmware: {version} → {:?}", firmware_version(&version));
        println!("Alimentación (corriente, tensión): {:?}", ebb.power().expect("QC"));
        println!("Pluma arriba: {:?}", ebb.pen_is_up().expect("QP"));
        println!("Movimiento: {:?}", ebb.motion().expect("QM"));
        println!("Pasos: {:?}", ebb.steps().expect("QS"));
        println!("Botón pulsado desde la última consulta: {:?}", ebb.button().expect("QB"));
        return;
    }
    if mode == "boton" {
        let mut ebb = Ebb::new(link);
        ebb.button().expect("QB");
        let secs: u64 = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(20);
        println!("Pulsa el botón de la máquina ({secs} s)…");
        let start = Instant::now();
        while start.elapsed().as_secs() < secs {
            if ebb.button().expect("QB") {
                println!("Botón pulsado a los {:.1} s.", start.elapsed().as_secs_f64());
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        println!("No ha llegado ninguna pulsación.");
        return;
    }
    if mode == "pluma" {
        // `pluma 70`: lleva solo el servo a esa altura (%), sin mover el carro.
        let pct: f64 = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(-1.0);
        if !(0.0..=100.0).contains(&pct) {
            eprintln!("Uso: pluma <0–100>"); std::process::exit(2);
        }
        let mut ebb = Ebb::new(link);
        ebb.configure_pen(pct, pct).expect("SC");
        ebb.pen(true, 500).expect("SP");
        let pos = vectorio_core::machine::ebb::servo_position(pct);
        println!("Pluma al {pct} % (servo {pos}).");
        return;
    }
    if mode == "soltar" {
        // Sube la pluma y deja los motores sin corriente: el carro se mueve a mano.
        let mut ebb = Ebb::new(link);
        ebb.pen(true, 300).expect("SP");
        ebb.motors(false).expect("EM");
        println!("Motores sueltos y pluma arriba.");
        return;
    }
    let mut settings = MachineSettings::default();
    match mode.as_str() {
        "aire" => settings.pen_down_pct = settings.pen_up_pct,
        "papel" => {
            // `papel 42 85`: alturas de la pluma abajo y arriba, en % (por defecto, las de la MakerQ).
            if let Some(pct) = std::env::args().nth(2).and_then(|v| v.parse::<f64>().ok()) {
                settings.pen_down_pct = pct;
            }
            if let Some(pct) = std::env::args().nth(3).and_then(|v| v.parse::<f64>().ok()) {
                settings.pen_up_pct = pct;
            }
            println!("Pluma arriba {} %, abajo {} %.", settings.pen_up_pct, settings.pen_down_pct);
        }
        _ => { eprintln!("Modo desconocido: {mode} (info, aire o papel)"); std::process::exit(2); }
    }
    let paper = Paper { name: "Muestra".into(), width_mm: 60.0, height_mm: 60.0, margin_mm: 0.0 };
    let pens = vec![Pen::new("Negro", [0, 0, 0], 0.4)];
    let motion = PlotSettings::default();
    let out = prepare(&sample(), &paper, &pens, &motion, &OutputSettings { drying: false, ..Default::default() })
        .expect("plan de salida");
    println!("Plan: {} pasos, {:.0} mm de trazo, ~{:.0} s", out.steps.len(), out.stats.draw_mm, out.stats.total_seconds);
    let mut plotter = Plotter::connect(link, settings).expect("conectar");
    println!("Conectada: {}", plotter.version());
    let control = Control::default();
    let start = Instant::now();
    let mut last = 0;
    let outcome = plotter.execute(&out, &motion, &control, &mut |e| match e {
        Event::PenChange { .. } => control.resume(),
        Event::Button => { println!("  Botón pulsado: paro."); control.stop() }
        Event::Step { index, total } if index * 10 / total.max(1) != last => {
            last = index * 10 / total.max(1);
            println!("  {}0 %", last);
        }
        _ => {}
    });
    println!("Resultado: {outcome:?} en {:.1} s; posición final {:?}", start.elapsed().as_secs_f64(), plotter.position());
    let steps = plotter.ebb().steps();
    println!("Pasos según la placa: {steps:?}");
}
