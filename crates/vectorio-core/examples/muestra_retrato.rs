//! `cargo run -p vectorio-core --release --example muestra_retrato -- foto.jpg salida.svg [mono]`
//! Escribe un SVG por tintas de 200 mm de lado mayor, con los ajustes iniciales.
use vectorio_core::vectorize::{self, VecMode, VecParams};
use vectorio_core::{p, Document, Geometry, Item, Pen};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 {
        return Err("Uso: muestra_retrato foto.jpg salida.svg [mono]".into());
    }
    let img = image::open(&args[1])?;
    let mut prm = VecParams {
        mode: VecMode::Portrait,
        ..Default::default()
    };
    prm.portrait.color = args.get(3).is_none_or(|a| a != "mono");
    let start = std::time::Instant::now();
    let result = vectorize::vectorize(&img, &prm);
    let scale = 200.0 / result.width.max(result.height);
    let mut doc = Document::default();
    doc.paper.width_mm = result.width * scale + 20.0;
    doc.paper.height_mm = result.height * scale + 20.0;
    doc.pens.clear();
    for layer in result.layers {
        let index = doc.pens.len();
        let name = layer.name.unwrap_or("Tinta");
        doc.pens
            .push(Pen::new(name, layer.color, prm.portrait.pen_width_mm));
        eprintln!("{name}: {} trazos", layer.lines.len());
        let lines = layer
            .lines
            .into_iter()
            .map(|l| l.into_iter().map(|q| q * scale).collect())
            .collect();
        let mut item = Item::new(
            name,
            Geometry::Path {
                lines,
                closed: false,
            },
            p(10.0, 10.0),
        );
        item.stroke = Some(index);
        doc.add(item);
    }
    doc.plot.optimize = false;
    let layers = doc.layers();
    std::fs::write(&args[2], vectorio_core::svg_out::export(&doc, &layers))?;
    raster(&doc, &layers).save(std::path::Path::new(&args[2]).with_extension("png"))?;
    eprintln!("Generado en {:.2} s", start.elapsed().as_secs_f64());
    Ok(())
}

/// Prueba visual de las mismas polilíneas y grosores, sobre papel blanco.
fn raster(doc: &Document, layers: &[vectorio_core::doc::PlotLayer]) -> image::RgbImage {
    let px = 5.0;
    let (w, h) = (
        (doc.paper.width_mm * px).ceil() as u32,
        (doc.paper.height_mm * px).ceil() as u32,
    );
    let mut image = image::RgbImage::from_pixel(w, h, image::Rgb([255; 3]));
    for layer in layers {
        let pen = &doc.pens[layer.pen];
        let radius = pen.width_mm * px * 0.5;
        let mut coverage = vec![0.0_f64; (w * h) as usize];
        for line in &layer.paths {
            for segment in line.windows(2) {
                let (a, b) = (segment[0] * px, segment[1] * px);
                let d = b - a;
                let lo = p(
                    (a.x.min(b.x) - radius - 1.0).max(0.0),
                    (a.y.min(b.y) - radius - 1.0).max(0.0),
                );
                let hi = p(
                    (a.x.max(b.x) + radius + 1.0).min(w as f64),
                    (a.y.max(b.y) + radius + 1.0).min(h as f64),
                );
                for y in lo.y as u32..hi.y.ceil() as u32 {
                    for x in lo.x as u32..hi.x.ceil() as u32 {
                        let q = p(x as f64 + 0.5, y as f64 + 0.5);
                        let t = ((q - a).dot(d) / d.dot(d).max(1e-12)).clamp(0.0, 1.0);
                        let alpha = (radius + 0.5 - q.dist(a + d * t)).clamp(0.0, 1.0);
                        let value = &mut coverage[(y * w + x) as usize];
                        *value = value.max(alpha);
                    }
                }
            }
        }
        for (out, alpha) in image.pixels_mut().zip(coverage) {
            for c in 0..3 {
                out[c] = (out[c] as f64 * (1.0 - alpha + alpha * pen.color[c] as f64 / 255.0))
                    .round() as u8;
            }
        }
    }
    image
}
