//! Muestra de la mezcla óptica: `cargo run -p vectorio-core --example muestra_mezcla -- foto.png salida.svg [puntos]`.
use vectorio_core::mix::{self, Screen, ScreenParams};
use vectorio_core::vectorize::{self, VecMode, VecParams};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let img = image::open(&args[1]).expect("imagen");
    let dots = args.get(3).is_some();
    let res = vectorize::vectorize(&img, &VecParams { mode: VecMode::Mix, max_px: 600, ..Default::default() });
    let k = 200.0 / res.width;
    let prm = ScreenParams { screen: if dots { Screen::Dots } else { Screen::Hatch }, spacing_mm: if dots { 0.9 } else { 0.8 }, angle_deg: 15.0, pen_width_mm: 0.5 };
    let mut svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{0}mm" height="{1}mm" viewBox="0 0 {0} {1}"><rect width="100%" height="100%" fill="white"/>"#, res.width * k, res.height * k);
    let n = res.layers.len();
    for (i, l) in res.layers.iter().enumerate() {
        let lines = mix::render(l.coverage.as_ref().unwrap(), k, &prm, i, n);
        let c = l.color;
        svg += &format!(r#"<g fill="none" stroke="rgb({},{},{})" stroke-width="0.5" stroke-linecap="round" opacity="0.85">"#, c[0], c[1], c[2]);
        for ln in &lines {
            svg += "<polyline points=\"";
            for q in ln {
                svg += &format!("{:.2},{:.2} ", q.x, q.y);
            }
            svg += "\"/>";
        }
        svg += "</g>";
        eprintln!("{:?}: {} trazos", l.name, lines.len());
    }
    svg += "</svg>";
    std::fs::write(&args[2], svg).unwrap();
}
