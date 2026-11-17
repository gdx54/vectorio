//! Reproducible import/stack/orbit benchmark. No files are modified.
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use vectorio_core::{volumetric::*, Paper};

fn main() -> Result<(), String> {
    let path = std::env::args().nth(1).ok_or("Pass an OBJ/STL path")?;
    let start = Instant::now();
    let (mesh, _) = load_mesh(Path::new(&path))?;
    println!(
        "import_ms={} triangles={} vertices={}",
        start.elapsed().as_millis(),
        mesh.triangles.len(),
        mesh.vertices.len()
    );
    let mesh = Arc::new(mesh);
    let mut settings = Settings::default();
    let mode = std::env::args().nth(2).unwrap_or_default();
    if mode.contains("original") {
        settings.mesh_detail = MeshDetail::Original;
    }
    if mode.contains("four") {
        settings.sheet = SheetViews::Orthographic;
    }
    println!("mode={mode}");
    let mut outline = Pass::new(
        "Outline",
        PassKind::Outline(OutlineParams::Surface { facets: 1800 }),
        0,
    );
    outline.enabled = true;
    let mut map = ReadingStyle::Curvature.passes(&settings).remove(0);
    if let PassKind::Reading(p) = &mut map.kind {
        p.facets = 3000;
    }
    let mut passes = vec![outline, map];
    let mut engine = Engine::default();
    let paper = Paper::default();
    for stage in ["first_stack", "orbit", "model_rotation", "ink_only"] {
        if stage == "orbit" {
            settings.yaw += 15.;
        }
        if stage == "model_rotation" {
            settings.model_rotation[2] += 15.;
        }
        if stage == "ink_only" {
            passes[0].pen = 1;
        }
        let start = Instant::now();
        let deadline = start + Duration::from_secs(60);
        let drawing = engine.draw_passes(mesh.clone(), &settings, &passes, &paper, &|| {
            Instant::now() > deadline
        })?;
        let doc = drawing.document(paper.clone(), vectorio_core::Document::default().pens, []);
        let svg = vectorio_core::svg_out::export(&doc, &doc.layers());
        let digest = svg.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        println!("svg_hash={digest:016x}");
        let points: usize = drawing
            .parts
            .iter()
            .flat_map(|p| &p.lines)
            .map(Vec::len)
            .sum();
        println!(
            "stage={stage} ms={} strokes={} points={points}",
            start.elapsed().as_millis(),
            drawing.parts.iter().map(|p| p.lines.len()).sum::<usize>()
        );
    }
    Ok(())
}
