//! Lenguajes artísticos reproducibles, sin abrir ventanas ni conectar el plotter.
//! cargo run -p vectorio-core --release --example volumetric_arte -- target/volumetric-arte [modelo.stl]
use std::{path::PathBuf, sync::Arc};
use vectorio_core::{svg_out, volumetric::*, Paper, Pen};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let directory = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| "target/volumetric-arte".into());
    std::fs::create_dir_all(&directory)?;
    let start = std::time::Instant::now();
    let mesh = Arc::new(if let Some(path) = args.next() {
        load_mesh(&PathBuf::from(path))?.0
    } else {
        Primitive::Torus.mesh()
    });
    println!(
        "Carga {:.2}s · {} triángulos originales",
        start.elapsed().as_secs_f64(),
        mesh.triangles.len()
    );
    let inks = vec![
        Pen::new("Índigo", [53, 33, 108], 0.1764),
        Pen::new("Óxido", [182, 73, 48], 0.1764),
        Pen::new("Azul mineral", [50, 105, 150], 0.1764),
    ];
    let paper = Paper::default();
    let mut engine = Engine::default();
    for (name, kind, pattern, colors) in [
        ("ondas", RecipeKind::SurfaceWaves, WavePattern::Waves, 3),
        ("malla", RecipeKind::TriMesh, WavePattern::Waves, 3),
        ("combinado", RecipeKind::Hybrid, WavePattern::Waves, 2),
        (
            "interferencias",
            RecipeKind::SurfaceWaves,
            WavePattern::Interference,
            3,
        ),
        (
            "malla-una-tinta",
            RecipeKind::TriMesh,
            WavePattern::Waves,
            1,
        ),
    ] {
        let settings = Settings {
            recipe: kind,
            art: ArtSettings {
                pattern,
                mesh_inks: colors,
                ..Default::default()
            },
            ..Default::default()
        };
        let d = engine.draw(mesh.clone(), &settings, &paper, &|| false)?;
        let doc = d.document(paper.clone(), inks.clone(), [0, 2, 1]);
        std::fs::write(
            directory.join(format!("{name}.svg")),
            svg_out::export(&doc, &doc.layers()),
        )?;
        println!(
            "{name}: {} caras de trabajo, {} curvas, {} aristas, {} capas, {:.1}ms",
            d.mesh.triangles.len(),
            d.curves.len(),
            d.art.as_ref().unwrap().edges.len(),
            doc.layers().len(),
            d.seconds * 1000.
        );
    }
    Ok(())
}
