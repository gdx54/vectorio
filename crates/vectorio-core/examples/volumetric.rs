//! Flujo local reproducible, sin iniciar egui ni conectar un plotter.
//! cargo run -p vectorio-core --release --example volumetric -- target/volumetric [modelo.stl]
use std::{path::PathBuf, sync::Arc};
use vectorio_core::{svg_out, volumetric::*, Paper, Pen};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let directory = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/volumetric"));
    std::fs::create_dir_all(&directory)?;
    let import_start = std::time::Instant::now();
    let mesh = if let Some(path) = args.next() {
        let (m, r) = load_mesh(&PathBuf::from(path))?;
        println!(
            "Importación: {} degenerados, {} fronteras",
            r.removed, r.boundary_edges
        );
        m
    } else {
        Primitive::Torus.mesh()
    };
    println!(
        "Carga y validación: {:.2} s",
        import_start.elapsed().as_secs_f64()
    );
    let save_recipes = !args.any(|s| s == "--sin-recetas");
    let mesh = Arc::new(mesh);
    let paper = Paper::default();
    let inks = vec![
        Pen::new("Índigo", [53, 33, 108], 0.1764),
        Pen::new("Óxido", [182, 73, 48], 0.1764),
        Pen::new("Azul mineral", [50, 105, 150], 0.1764),
    ];
    let mut engine = Engine::default();
    for (name, kind) in [
        ("atlas", RecipeKind::Atlas),
        ("strata", RecipeKind::Strata),
        ("x-ray", RecipeKind::XRay),
    ] {
        let settings = Settings {
            recipe: kind,
            ..Default::default()
        };
        let drawing = engine.draw(mesh.clone(), &settings, &paper, &|| false)?;
        let doc = drawing.document(paper.clone(), inks.clone(), [0, 2, 1]);
        std::fs::write(
            directory.join(format!("{name}.svg")),
            svg_out::export(&doc, &doc.layers()),
        )?;
        if save_recipes {
            let mut recipe = Recipe::new(mesh.as_ref().clone(), settings);
            recipe.inks = inks.clone();
            recipe.set_layer_pens([0, 2, 1]);
            std::fs::write(directory.join(format!("{name}.json")), recipe.to_json()?)?;
        }
        println!(
            "{name}: {} triángulos originales, {} de trabajo, {} curvas, {:.1} ms",
            mesh.triangles.len(),
            drawing.mesh.triangles.len(),
            drawing.curves.len(),
            drawing.seconds * 1000.
        );
    }
    Ok(())
}
