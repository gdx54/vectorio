//! Genera el icono de macOS a partir del dibujo maestro `assets/branding/vectorio-icon.png`.
//!
//! El maestro es una imagen generada cuya placa ocupa casi todo el lienzo y está algo
//! descentrada; en el Dock y en ⌘⇥ quedaba pegada al recuadro de selección. Aquí se
//! extrae solo la V (cuánto se aparta cada píxel del fondo hacia el índigo), se centra
//! y se compone sobre una placa nueva con la retícula de Apple: 824 px de 1024,
//! centrada, esquinas continuas y una sombra suave.
//!
//! Uso: `cargo run -p vectorio-core --example icono_macos --release`
//! Escribe `assets/branding/vectorio-icon-macos.png` y `packaging/macos/Vectorio.icns`.

use image::{imageops, Rgba, RgbaImage};
use std::path::Path;

const FONDO: [f32; 3] = [246.0, 240.0, 149.0]; // #F6F095
const TINTA: [f32; 3] = [53.0, 33.0, 108.0]; // #35216C
const LIENZO: u32 = 1024;
const PLACA: f32 = 824.0;
/// Ancho de la V respecto a la placa (el del dibujo maestro).
const V_RELATIVA: f32 = 0.75;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = image::open(root.join("assets/branding/vectorio-icon.png")).expect("falta el icono maestro").to_rgba8();

    // Cantidad de tinta por píxel, 0 (fondo) … 1 (índigo).
    let span: f32 = (0..3).map(|i| (FONDO[i] - TINTA[i]).powi(2)).sum::<f32>().sqrt();
    let ink = |p: &Rgba<u8>| -> f32 {
        if p[3] < 200 { return 0.0; }
        let along: f32 = (0..3).map(|i| (FONDO[i] - p[i] as f32) * (FONDO[i] - TINTA[i])).sum::<f32>() / span / span;
        ((along - 0.08) / 0.84).clamp(0.0, 1.0)
    };
    let (sw, sh) = src.dimensions();
    let field: Vec<f32> = src.pixels().map(ink).collect();
    let (mut x0, mut y0, mut x1, mut y1) = (sw, sh, 0, 0);
    for y in 0..sh { for x in 0..sw { if field[(y * sw + x) as usize] > 0.5 { x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x); y1 = y1.max(y); } } }
    let (mw, mh) = ((x1 - x0) as f32, (y1 - y0) as f32);
    let (mcx, mcy) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
    let scale = mw / (PLACA * V_RELATIVA);
    let sample = |x: f32, y: f32| -> f32 {
        if x < 0.0 || y < 0.0 || x >= (sw - 1) as f32 || y >= (sh - 1) as f32 { return 0.0; }
        let (ix, iy) = (x as u32, y as u32);
        let (fx, fy) = (x - ix as f32, y - iy as f32);
        let g = |a: u32, b: u32| field[(b * sw + a) as usize];
        let top = g(ix, iy) * (1.0 - fx) + g(ix + 1, iy) * fx;
        let bot = g(ix, iy + 1) * (1.0 - fx) + g(ix + 1, iy + 1) * fx;
        top * (1.0 - fy) + bot * fy
    };

    // Placa: superelipse (esquinas continuas, como las de macOS) con 4×4 muestras por píxel.
    let c = LIENZO as f32 / 2.0;
    let half = PLACA / 2.0;
    let inside = |x: f32, y: f32| ((x - c).abs() / half).powf(5.0) + ((y - c).abs() / half).powf(5.0) <= 1.0;
    let mut tile = vec![0f32; (LIENZO * LIENZO) as usize];
    for y in 0..LIENZO { for x in 0..LIENZO {
        let mut n = 0;
        for sy in 0..4 { for sx in 0..4 { if inside(x as f32 + (sx as f32 + 0.5) / 4.0, y as f32 + (sy as f32 + 0.5) / 4.0) { n += 1; } } }
        tile[(y * LIENZO + x) as usize] = n as f32 / 16.0;
    } }

    // Sombra: la silueta desplazada hacia abajo y difuminada.
    let mut shadow = RgbaImage::new(LIENZO, LIENZO);
    for y in 0..LIENZO { for x in 0..LIENZO {
        let a = if y >= 10 { tile[((y - 10) * LIENZO + x) as usize] } else { 0.0 };
        shadow.put_pixel(x, y, Rgba([0, 0, 0, (a * 255.0 * 0.28) as u8]));
    } }
    let shadow = imageops::blur(&shadow, 12.0);

    let mut out = RgbaImage::new(LIENZO, LIENZO);
    for y in 0..LIENZO { for x in 0..LIENZO {
        let t = tile[(y * LIENZO + x) as usize];
        let sa = shadow.get_pixel(x, y)[3] as f32 / 255.0;
        // Centro visual de la V en el centro de la placa (un pelo más alto que el geométrico).
        let k = sample(mcx + (x as f32 + 0.5 - c) * scale, mcy + (y as f32 + 0.5 - c + mh / scale * 0.015) * scale);
        let rgb: [f32; 3] = std::array::from_fn(|i| FONDO[i] + (TINTA[i] - FONDO[i]) * k);
        // Placa sobre sombra (operador «sobre» con fondo transparente).
        let a = t + sa * (1.0 - t);
        let px = if a > 0.0 { std::array::from_fn(|i| (rgb[i] * t / a) as u8) } else { [0; 3] };
        out.put_pixel(x, y, Rgba([px[0], px[1], px[2], (a * 255.0).round() as u8]));
    } }
    out.save(root.join("assets/branding/vectorio-icon-macos.png")).unwrap();

    let set = std::env::temp_dir().join("Vectorio.iconset");
    let _ = std::fs::remove_dir_all(&set);
    std::fs::create_dir_all(&set).unwrap();
    for base in [16u32, 32, 128, 256, 512] {
        for (mul, suffix) in [(1, ""), (2, "@2x")] {
            let s = base * mul;
            let img = if s == LIENZO { out.clone() } else { imageops::resize(&out, s, s, imageops::FilterType::Lanczos3) };
            img.save(set.join(format!("icon_{base}x{base}{suffix}.png"))).unwrap();
        }
    }
    let ok = std::process::Command::new("iconutil").arg("-c").arg("icns").arg(&set).arg("-o").arg(root.join("packaging/macos/Vectorio.icns")).status().map(|s| s.success()).unwrap_or(false);
    assert!(ok, "iconutil no pudo crear el .icns");
    println!("Icono listo: placa {PLACA} px de {LIENZO}, V centrada (escala {:.3}).", 1.0 / scale);
}
