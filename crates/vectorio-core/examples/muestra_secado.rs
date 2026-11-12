//! Hoja sintética para comprobar acumulación, cruces y secado sin conectar el plotter.
//! cargo run -p vectorio-core --example muestra_secado --release
use vectorio_core::{output, p, Document, Geometry, Item, P};

fn main() {
    let folder = std::path::Path::new("target/salida-qa");
    std::fs::create_dir_all(folder).unwrap();
    let mut doc = Document::default();
    doc.plot.join_tolerance = 0.0;
    doc.plot.optimize = false;
    // Tres zonas alejadas: el plan puede alternarlas en vez de esperar siempre.
    for (cx, cy, repeats) in [(90.0, 115.0, 3), (215.0, 115.0, 6), (330.0, 115.0, 10)] {
        let mut lines = Vec::new();
        for y in 0..22 {
            for _ in 0..repeats {
                lines.push(vec![p(cx - 32.0, cy - 32.0 + y as f64 * 3.0), p(cx + 32.0, cy - 32.0 + y as f64 * 3.0)]);
            }
        }
        let mut it = Item::new(format!("{repeats} pasadas"), Geometry::Path { lines, closed: false }, P::default());
        it.stroke = Some(0); doc.add(it);
    }
    let mut crossing = Item::new("Cruces de otra tinta", Geometry::Path {
        lines: (0..20).map(|i| vec![p(182.0 + i as f64 * 3.0, 74.0), p(182.0 + i as f64 * 3.0, 156.0)]).collect(), closed: false }, P::default());
    crossing.stroke = Some(1); doc.add(crossing);
    let layers = doc.layers();
    let start = std::time::Instant::now();
    let plan = output::prepare(&layers, &doc.paper, &doc.pens, &doc.plot, &doc.output).unwrap();
    std::fs::write(folder.join("prueba-secado.vectorio"), doc.to_json()).unwrap();
    std::fs::write(folder.join("prueba-secado-plan.json"), serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    println!("{} trazos → {} tramos; pico húmedo {} → {}; {} esperas, {:.1} s; cálculo {:?}",
        layers.iter().map(|l| l.paths.len()).sum::<usize>(), plan.stats.strokes,
        plan.wet_before.max(), plan.wet_after.max(), plan.stats.waits, plan.stats.wait_seconds, start.elapsed());
}
