//! Salida SVG en mm con una capa de Inkscape por plumilla (compatible con AxiDraw y la
//! importación de capas de Linear).

use crate::doc::{Document, PlotLayer};
use crate::geom::Polyline;
use std::fmt::Write;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn path_d(l: &Polyline) -> String {
    let mut d = String::with_capacity(l.len() * 16);
    for (i, q) in l.iter().enumerate() {
        let _ = write!(d, "{}{:.3} {:.3}", if i == 0 { "M" } else { " L" }, q.x, q.y);
    }
    d
}

fn header(doc: &Document) -> String {
    let (w, h) = (doc.paper.width_mm, doc.paper.height_mm);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" \
         xmlns:inkscape=\"http://www.inkscape.org/namespaces/inkscape\" \
         width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">\n<!-- Vectorio -->\n"
    )
}

fn layer_group(doc: &Document, layer: &PlotLayer, n: usize) -> String {
    let pen = &doc.pens[layer.pen];
    let [r, g, b] = pen.color;
    let mut s = format!(
        "<g inkscape:groupmode=\"layer\" id=\"layer{n}\" inkscape:label=\"{n} {}\" data-vectorio-pen=\"{}\" \
         fill=\"none\" stroke=\"#{r:02x}{g:02x}{b:02x}\" stroke-width=\"{:.3}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\n",
        esc(&pen.name),
        layer.pen,
        pen.width_mm
    );
    for l in &layer.paths {
        let _ = writeln!(s, "<path d=\"{}\"/>", path_d(l));
    }
    s.push_str("</g>\n");
    s
}

/// Todas las plumillas en un SVG, una capa por plumilla en orden de dibujo.
pub fn export(doc: &Document, layers: &[PlotLayer]) -> String {
    let mut s = header(doc);
    for (i, layer) in layers.iter().enumerate() {
        s.push_str(&layer_group(doc, layer, i + 1));
    }
    s.push_str("</svg>\n");
    s
}

/// Un SVG por plumilla: (nombre de archivo sugerido, contenido).
pub fn export_per_pen(doc: &Document, layers: &[PlotLayer]) -> Vec<(String, String)> {
    layers
        .iter()
        .enumerate()
        .map(|(i, layer)| {
            let name = doc.pens[layer.pen].name.to_lowercase().replace(|c: char| !c.is_alphanumeric(), "-");
            let mut s = header(doc);
            s.push_str(&layer_group(doc, layer, i + 1));
            s.push_str("</svg>\n");
            (format!("{:02}-{name}.svg", i + 1), s)
        })
        .collect()
}
