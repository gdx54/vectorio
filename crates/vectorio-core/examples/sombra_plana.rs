//! Exportación reproducible del modo sombra plana, sin abrir la aplicación.
//! cargo run -p vectorio-core --release --example sombra_plana -- target/volumetric [modelo.stl]
use std::{path::PathBuf, sync::Arc};
use vectorio_core::{svg_out, volumetric::*, Paper, Pen};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| "target/volumetric".into());
    std::fs::create_dir_all(&dir)?;
    let start = std::time::Instant::now();
    let mesh = Arc::new(if let Some(path) = args.next() {
        load_mesh(&PathBuf::from(path))?.0
    } else {
        Primitive::Torus.mesh()
    });
    println!(
        "Carga: {:.2}s, {} triángulos",
        start.elapsed().as_secs_f64(),
        mesh.triangles.len()
    );
    let mut engine = Engine::default();
    let mut s = Settings {
        recipe: RecipeKind::FlatShadow,
        ..Default::default()
    };
    let paper = Paper::default();
    let inks = vec![
        Pen::new("Índigo", [53, 33, 108], 0.22),
        Pen::new("Óxido", [182, 73, 48], 0.22),
    ];
    for (name, az, middle, outline) in [
        ("sombra-plana", 135., false, false),
        ("sombra-lateral", 20., false, false),
        ("sombra-dos-tonos", 135., true, true),
    ] {
        s.shadow.azimuth = az;
        s.shadow.middle_tone = middle;
        s.shadow.outline = outline;
        let d = engine.draw(mesh.clone(), &s, &paper, &|| false)?;
        let doc = d.document(paper.clone(), inks.clone(), [0, 1, 0]);
        std::fs::write(
            dir.join(format!("{name}.svg")),
            svg_out::export(&doc, &doc.layers()),
        )?;
        println!(
            "{name}: {} facetas, {} trazos, {:.1}ms",
            d.mesh.triangles.len(),
            d.parts.iter().map(|p| p.lines.len()).sum::<usize>(),
            d.seconds * 1000.
        );
    }
    Ok(())
}
