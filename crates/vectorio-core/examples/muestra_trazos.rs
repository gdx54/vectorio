//! Hoja de muestra de los estilos y controles de trazo:
//! `cargo run -p vectorio-core --example muestra_trazos --release -- salida.svg`.
use vectorio_core::raster::Grid;
use vectorio_core::trace::{self, DotShape, TraceParams, TraceStyle, Vibe};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "muestra_trazos.svg".into());
    let (tw, th, k) = (60.0, 42.0, 0.25);
    let (w, h) = ((tw / k) as usize, (th / k) as usize);
    // Óvalo con un degradado de claro (izquierda) a oscuro (derecha).
    let mut map = Grid::<f32>::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = ((x as f64 + 0.5) / w as f64 * 2.0 - 1.0, (y as f64 + 0.5) / h as f64 * 2.0 - 1.0);
            if u * u + v * v <= 0.92 {
                map.set(x, y, (0.08 + 0.92 * (u + 1.0) / 2.0) as f32);
            }
        }
    }
    let b = TraceParams::default();
    let tiles: Vec<(&str, TraceParams)> = vec![
        ("Rayado", TraceParams { dash_mm: 8.0, ..b.clone() }),
        ("Rayado · 3 pasadas", TraceParams { dash_mm: 30.0, passes: 3, ..b.clone() }),
        ("Rayado · a mano", TraceParams { dash_mm: 14.0, dash_vary: 0.7, jitter: 0.7, angle_jitter_deg: 6.0, max_len_mm: 12.0, ..b.clone() }),
        ("Ondas", TraceParams { style: TraceStyle::Waves, spacing_mm: 1.1, wave_mm: 3.0, ..b.clone() }),
        ("Ondas · deformadas", TraceParams { style: TraceStyle::Waves, spacing_mm: 1.1, wave_mm: 3.0, warp_mm: 3.0, warp_scale_mm: 18.0, ..b.clone() }),
        ("Flujo · desorden", TraceParams { style: TraceStyle::Flow, dash_mm: 14.0, jitter: 0.8, ..b.clone() }),
        ("Círculos", TraceParams { style: TraceStyle::Circles, dash_mm: 10.0, center: [0.3, 0.4], ..b.clone() }),
        ("Rayos", TraceParams { style: TraceStyle::Radial, spacing_mm: 1.0, dash_mm: 10.0, dash_vary: 0.5, ..b.clone() }),
        ("Teselas", TraceParams { style: TraceStyle::Truchet, spacing_mm: 0.9, ..b.clone() }),
        ("Laberinto", TraceParams { style: TraceStyle::Hilbert, spacing_mm: 1.0, dash_mm: 20.0, jitter: 0.3, ..b.clone() }),
        ("Puntos · cruces", TraceParams { style: TraceStyle::Dots, spacing_mm: 1.4, dot_shape: DotShape::Cross, angle_deg: 45.0, angle_jitter_deg: 12.0, ..b.clone() }),
        ("Puntos · rayas", TraceParams { style: TraceStyle::Dots, spacing_mm: 1.2, dot_shape: DotShape::Dash, angle_jitter_deg: 40.0, jitter: 0.6, ..b.clone() }),
        ("Contornos · umbral 40 %", TraceParams { style: TraceStyle::Contour, dash_mm: 25.0, tone_floor: 0.4, ..b.clone() }),
        ("Rayado · curva +0,8", TraceParams { dash_mm: 8.0, tone_curve: 0.8, ..b.clone() }),
        ("Garabato · zigzag", TraceParams { style: TraceStyle::Scribble, vibe: Vibe::Zigzag, vibe_mm: 0.3, wave_mm: 1.0, ..b.clone() }),
        ("Espiral descentrada", TraceParams { style: TraceStyle::Spiral, dash_mm: 6.0, center: [0.75, 0.3], ..b }),
    ];
    let cols = 4;
    let (gap, label) = (6.0, 6.0);
    let rows = tiles.len().div_ceil(cols);
    let (sw, sh) = (cols as f64 * (tw + gap) + gap, rows as f64 * (th + gap + label) + gap);
    let mut svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{sw}mm" height="{sh}mm" viewBox="0 0 {sw} {sh}"><rect width="100%" height="100%" fill="white"/>"#);
    for (i, (name, prm)) in tiles.iter().enumerate() {
        let (x0, y0) = (gap + (i % cols) as f64 * (tw + gap), gap + (i / cols) as f64 * (th + gap + label));
        let lines = trace::render(&map, k, prm, 0.4, 0);
        svg += &format!(r##"<text x="{x0}" y="{}" font-family="Helvetica" font-size="3.4" fill="#35216C">{name}</text>"##, y0 + 3.6);
        svg += &format!(r##"<g transform="translate({x0},{})" fill="none" stroke="#35216C" stroke-width="0.4" stroke-linecap="round" stroke-linejoin="round">"##, y0 + label);
        for ln in &lines {
            svg += "<polyline points=\"";
            for q in ln {
                svg += &format!("{:.2},{:.2} ", q.x, q.y);
            }
            svg += "\"/>";
        }
        svg += "</g>";
        eprintln!("{name}: {} trazos", lines.len());
    }
    svg += "</svg>";
    std::fs::write(&out, svg).unwrap();
}
