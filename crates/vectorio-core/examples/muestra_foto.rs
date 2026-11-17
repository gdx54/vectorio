//! Hoja de prueba de un estilo de foto, simulando la tinta en el papel.
//! `cargo run -p vectorio-core --example muestra_foto --release -- Portrait salida.png foto1.png foto2.png…`
//! `VECTORIO_PEN=0.35` grosor de la punta en mm sobre un lado mayor de 200 mm.
//! `VECTORIO_SET='{"sketch":{"inks":"Two"}}'` mezcla ajustes en los parámetros (JSON).
use image::{Rgb, RgbImage};
use vectorio_core::vectorize::{vectorize, VecMode, VecParams};

const SIDE_MM: f64 = 200.0;
const PX_PER_MM: f64 = 5.0;

fn merge(a: &mut serde_json::Value, b: serde_json::Value) {
    match (a, b) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (k, v) in b { merge(a.entry(k).or_insert(serde_json::Value::Null), v); }
        }
        (a, b) => *a = b,
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode: VecMode = serde_json::from_str(&format!("\"{}\"", args.next().expect("modo"))).expect("modo válido");
    let out = args.next().expect("salida");
    let photos: Vec<String> = args.collect();
    let pen_mm: f64 = std::env::var("VECTORIO_PEN").ok().and_then(|v| v.parse().ok()).unwrap_or(0.35);
    let mut prm = VecParams { mode, ..Default::default() };
    if let Ok(set) = std::env::var("VECTORIO_SET") {
        let mut v = serde_json::to_value(&prm).unwrap();
        merge(&mut v, serde_json::from_str(&set).expect("JSON"));
        prm = serde_json::from_value(v).expect("parámetros");
    }
    let cell = (SIDE_MM * PX_PER_MM) as u32;
    let mut sheet = RgbImage::from_pixel(cell * 2, cell * photos.len() as u32, Rgb([255, 255, 255]));
    for (row, path) in photos.iter().enumerate() {
        let img = image::open(path).expect("foto");
        let started = std::time::Instant::now();
        let res = vectorize(&img, &prm);
        let elapsed = started.elapsed().as_secs_f64();
        let k = SIDE_MM * PX_PER_MM / res.width.max(res.height);
        let original = image::imageops::resize(&img.to_rgb8(), (res.width * k) as u32, (res.height * k) as u32, image::imageops::FilterType::Triangle);
        image::imageops::overlay(&mut sheet, &original, 0, (row as u32 * cell) as i64);
        let (mut segments, mut strokes, mut ink_mm) = (0usize, 0usize, 0.0);
        let half = pen_mm * PX_PER_MM / 2.0;
        let oy = (row as u32 * cell) as f64;
        let ox = cell as f64;
        for layer in &res.layers {
            let [r, g, b] = layer.color;
            for line in &layer.lines {
                strokes += 1;
                for pair in line.windows(2) {
                    segments += 1;
                    let a = (ox + pair[0].x * k, oy + pair[0].y * k);
                    let c = (ox + pair[1].x * k, oy + pair[1].y * k);
                    ink_mm += ((a.0 - c.0).hypot(a.1 - c.1)) / PX_PER_MM;
                    draw(&mut sheet, a, c, half, [r, g, b], ox, oy, cell as f64);
                }
            }
        }
        eprintln!("{path}: {strokes} trazos, {segments} segmentos, {:.1} m de tinta, {elapsed:.2} s", ink_mm / 1000.0);
    }
    sheet.save(out).unwrap();
}

#[allow(clippy::too_many_arguments)]
fn draw(img: &mut RgbImage, a: (f64, f64), b: (f64, f64), half: f64, color: [u8; 3], ox: f64, oy: f64, cell: f64) {
    let (x0, x1) = ((a.0.min(b.0) - half - 1.0).max(ox).floor() as i64, (a.0.max(b.0) + half + 1.0).min(ox + cell - 1.0).ceil() as i64);
    let (y0, y1) = ((a.1.min(b.1) - half - 1.0).max(oy).floor() as i64, (a.1.max(b.1) + half + 1.0).min(oy + cell - 1.0).ceil() as i64);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = (dx * dx + dy * dy).max(1e-12);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let t = (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0);
            let d = (px - a.0 - t * dx).hypot(py - a.1 - t * dy);
            let cover = (half + 0.5 - d).clamp(0.0, 1.0);
            if cover <= 0.0 { continue; }
            let p = img.get_pixel_mut(x as u32, y as u32);
            for i in 0..3 {
                // Tinta transparente: multiplica, como un rotulador sobre otro.
                let ink = 1.0 - cover * (1.0 - color[i] as f64 / 255.0) * 0.9;
                p[i] = (p[i] as f64 * ink).round() as u8;
            }
        }
    }
}
