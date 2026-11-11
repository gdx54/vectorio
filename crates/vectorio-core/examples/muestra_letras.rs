//! Escribe un SVG con una muestra de cada tipografía: `cargo run -p vectorio-core --example muestra_letras -- salida.svg`
use vectorio_core::{font, FontStyle, TextSpec};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "muestra_letras.svg".into());
    let mut body = String::new();
    let mut y = 10.0;
    for style in FontStyle::ALL {
        for (jitter, x0) in [(0.0, 10.0), (1.0, 150.0)] {
            let spec = TextSpec { text: format!("Querida mañana, ¿nunca minimum? {}", if jitter > 0.0 { "pulso 1" } else { "" }), style, jitter, size_mm: 6.0, ..Default::default() };
            for l in font::layout(&spec) {
                let pts: Vec<String> = l.iter().map(|q| format!("{:.2},{:.2}", q.x + x0, q.y + y)).collect();
                body.push_str(&format!("<polyline points=\"{}\"/>\n", pts.join(" ")));
            }
        }
        y += 14.0;
    }
    let svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"300mm\" height=\"{y}mm\" viewBox=\"0 0 300 {y}\"><rect width=\"300\" height=\"{y}\" fill=\"white\"/><g fill=\"none\" stroke=\"#1d3557\" stroke-width=\"0.35\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{body}</g></svg>");
    std::fs::write(&path, svg).unwrap();
}
