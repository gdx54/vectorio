//! Muestra nativa: `cargo run -p vectorio-core --release --example muestra_topografia -- foto.png salida.svg [finura] [negro] [separación]`.
//! SVG y PNG de los mismos trazos, con 200 mm de lado mayor.
use vectorio_core::vectorize::{self, VecMode, VecParams};
use vectorio_core::{p, Document, Geometry, Item, Pen};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 {
        return Err(
            "Uso: muestra_topografia foto.png salida.svg [finura] [negro] [separación]".into(),
        );
    }
    let img = image::open(&args[1])?;
    let mut prm = VecParams {
        mode: VecMode::Topography,
        blur: 0.0,
        ..Default::default()
    };
    if let Some(n) = args.get(3) {
        prm.topo.fineness = n.parse()?;
    }
    if let Some(n) = args.get(4) {
        prm.topo.black = n.parse()?;
    }
    if let Some(n) = args.get(5) {
        prm.topo.gap = n.parse()?;
    }
    let start = std::time::Instant::now();
    let result = vectorize::vectorize(&img, &prm);
    eprintln!(
        "Motor: {:.3} s · {} × {} px",
        start.elapsed().as_secs_f64(),
        result.width,
        result.height
    );
    let scale = 200.0 / result.width.max(result.height).max(1.0);
    let mut doc = Document::default();
    doc.paper.width_mm = result.width * scale + 20.0;
    doc.paper.height_mm = result.height * scale + 20.0;
    doc.pens = vec![Pen::new("Negro", [30, 30, 34], prm.topo.pen_width_mm)];
    for layer in result.layers {
        eprintln!(
            "{} trazos · {} puntos",
            layer.lines.len(),
            layer.lines.iter().map(|l| l.len()).sum::<usize>()
        );
        let lines = layer
            .lines
            .into_iter()
            .map(|l| l.into_iter().map(|q| q * scale).collect())
            .collect();
        doc.add(Item::new(
            "Topografía",
            Geometry::Path {
                lines,
                closed: false,
            },
            p(10.0, 10.0),
        ));
    }
    doc.plot.optimize = false;
    let layers = doc.layers();
    std::fs::write(&args[2], vectorio_core::svg_out::export(&doc, &layers))?;
    raster(&doc, &layers).save(std::path::Path::new(&args[2]).with_extension("png"))?;
    Ok(())
}

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
