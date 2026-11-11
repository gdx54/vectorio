//! Genera un proyecto de ejemplo paginado y sus SVG, sin necesitar la interfaz.
use std::path::PathBuf;
use vectorio_core::{font, svg_out, p, Align, Document, Geometry, Item, Paper, TextSpec};

fn main() {
    let folder = std::env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("target/texto-demo"));
    std::fs::create_dir_all(&folder).unwrap();
    let mut doc = Document::default();
    doc.paper = Paper::presets().into_iter().find(|paper| paper.name == "A4 vertical").unwrap();
    let paragraph = "La escritura dibuja un recorrido sobre el papel. Cada palabra mantiene su carácter, mientras las líneas se adaptan al espacio disponible. Una caja de texto permite componer una carta completa, separar sus párrafos y continuar en otra hoja sin perder el contenido.";
    let spec = TextSpec {
        text: std::iter::repeat_n(paragraph, 16).collect::<Vec<_>>().join("\n"),
        size_mm: 4.0, wrap_mm: 180.0, frame_height_mm: 267.0, padding_mm: 3.0,
        paragraph_spacing_mm: 3.0, first_indent_mm: 5.0, align: Align::Justify,
        hyphenate: true, ..Default::default()
    };
    let count = font::compose(&spec).page_count;
    let id = doc.add(Item::new("Carta manuscrita", Geometry::Text(spec), p(15.0, 15.0)));
    std::fs::write(folder.join("carta.vectorio"), doc.to_json()).unwrap();
    for page in 0..count {
        if let Geometry::Text(spec) = &mut doc.get_mut(id).unwrap().geometry { spec.page = page; }
        std::fs::write(folder.join(format!("carta-{:03}.svg", page + 1)), svg_out::export(&doc, &doc.layers())).unwrap();
    }
    println!("{} páginas generadas en {}", count, folder.display());
}
