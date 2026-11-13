//! Bodegón técnico A4 del motor de escenas:
//! `cargo run -p vectorio-core --example muestra_escena --release -- muestra_escena.svg --dimetric`.
//! `--dimetric` (o VECTORIO_DIMETRIC=1) añade una segunda hoja con elevación de 26,57°.
use std::fmt::Write;
use vectorio_core::geom::{p, BBox};
use vectorio_core::scene::{Camera, PartId, Scene, Tone, V3};

fn v(x: f64, y: f64, z: f64) -> V3 { V3::new(x, y, z) }

fn shade(scene: &mut Scene, faces: Vec<Vec<V3>>, parts: [PartId; 3]) {
    for face in faces {
        let n = (face[1] - face[0]).cross(face[2] - face[0]).normalized();
        let (part, spacing) = match scene.tone(n) {
            Tone::Lit => (parts[0], 2.4), Tone::Mid => (parts[1], 1.2), Tone::Dark => (parts[2], 0.65),
        };
        scene.set_part(part);
        // Una diagonal del plano evita que la trama coincida con sus aristas.
        let direction = (face[1] - face[0]).normalized() + (face[face.len() - 1] - face[0]).normalized() * 0.37;
        scene.hatch_face(&face, direction, spacing);
    }
    scene.set_part(PartId(0));
}

fn shade_surface(scene: &mut Scene, faces: Vec<Vec<V3>>, parts: [PartId; 3]) {
    let mut groups = [Vec::new(), Vec::new(), Vec::new()];
    for face in faces {
        let n = (face[1]-face[0]).cross(face[2]-face[0]).normalized();
        let i = match scene.tone(n) { Tone::Lit=>0, Tone::Mid=>1, Tone::Dark=>2 }; groups[i].push(face);
    }
    for (i, faces) in groups.iter().enumerate() { scene.set_part(parts[i]); scene.hatch_surface(faces, 0.0, [2.4,1.2,0.65][i]); }
    scene.set_part(PartId(0));
}

fn build(elevation: f64, fitted: Option<Camera>) -> Scene {
    let paper = BBox { min: p(0.0, 0.0), max: p(210.0, 297.0) };
    let mut camera = Camera { elev_deg: elevation, ..Camera::default() };
    camera.fit([v(-5.7, -4.5, 0.0), v(9.0, 6.7, 5.6)],
        BBox { min: p(4.0, 24.0), max: p(206.0, 273.0) }, 8.0);
    let mut scene = Scene::new(fitted.unwrap_or(camera), paper);
    scene.set_sun(Some(v(-2.0, 1.0, 3.0)));
    let roof = scene.part("tejados");
    let parts = [scene.part("trama clara"), scene.part("trama media"), scene.part("trama oscura")];
    let details = scene.part("ventanas enteras");
    let shadows = scene.part("sombra");
    scene.begin_shadows(0.0, Some([-7.0, -6.0, 10.0, 10.0]), 18.0);

    // Casa: la base del tejado es un triángulo extruido, sin dependencia de ejes de cámara.
    let faces = scene.box3(v(-4.0, -3.0, 0.0), v(-1.0, 0.0, 2.4));
    shade(&mut scene, faces, parts);
    scene.set_part(roof);
    let faces = scene.extrude(&[v(-4.2, -3.2, 2.4), v(-0.8, -3.2, 2.4), v(-2.5, -3.2, 4.0)], v(0.0, 3.4, 0.0));
    shade(&mut scene, faces, parts);
    scene.set_part(details);
    // Libre, sobre la fachada que mira hacia +y.
    scene.whole_line(&[v(-3.2, 0.0, 0.8), v(-2.4, 0.0, 0.8), v(-2.4, 0.0, 1.6), v(-3.2, 0.0, 1.6), v(-3.2, 0.0, 0.8)]);
    // Oculta en la fachada posterior; se rechaza como una sola pieza.
    scene.whole_line(&[v(-3.2, -3.0, 0.8), v(-2.4, -3.0, 0.8), v(-2.4, -3.0, 1.6), v(-3.2, -3.0, 1.6)]);
    scene.set_part(PartId(0));

    // Torre y cono; se suprimen las uniones entre facetas suaves.
    let faces = scene.lathe(v(2.0, -2.6, 0.0), &[(1.0, 0.0), (1.0, 3.8)], true);
    shade_surface(&mut scene, faces, parts);
    scene.set_part(roof);
    let faces = scene.lathe(v(2.0, -2.6, 0.0), &[(1.2, 3.8), (0.0, 5.6)], true);
    shade_surface(&mut scene, faces, parts);

    // Cúpula convexa: el perfil termina en un único vértice.
    let profile: Vec<_> = (0..=12).map(|i| {
        let angle = std::f64::consts::FRAC_PI_2 * i as f64 / 12.0;
        (if i == 12 { 0.0 } else { 1.6 * angle.cos() }, 0.4 + 1.6 * angle.sin())
    }).collect();
    let faces = scene.lathe(v(6.0, -0.8, 0.0), &profile, true);
    shade_surface(&mut scene, faces, parts);

    // Cajas solapadas en proyección, de distintas alturas.
    for (min, max) in [
        (v(-4.0, 3.0, 0.0), v(-2.0, 5.0, 2.8)),
        (v(-2.7, 4.5, 0.0), v(-0.7, 6.5, 1.6)),
        (v(-5.7, 5.4, 0.0), v(-4.4, 6.7, 1.0)),
    ] {
        let faces = scene.box3(min, max); shade(&mut scene, faces, parts);
    }

    // Muralla. Las almenas empiezan donde acaba el muro, sin caras superpuestas.
    let faces = scene.box3(v(2.0, 4.8, 0.0), v(8.0, 5.6, 2.0));
    shade(&mut scene, faces, parts);
    for i in 0..6 {
        let x = 2.1 + i as f64;
        let faces = scene.box3(v(x, 4.8, 2.0), v(x + 0.65, 5.6, 2.7));
        shade(&mut scene, faces, parts);
    }
    scene.set_part(shadows); scene.hatch_shadows(1.1);
    scene
}

fn save(path: &std::path::Path, elevation: f64) -> std::io::Result<()> {
    let start = std::time::Instant::now();
    // El ajuste por caja incluye esquinas vacías del mundo. La segunda pasada
    // encuadra los trazos reales y vuelve a tramar a su separación física en mm.
    let initial = build(elevation, None);
    let bounds = BBox::of(initial.render().parts.iter().flat_map(|part| &part.lines));
    let target = BBox { min: p(12.0, 28.0), max: p(198.0, 267.0) };
    let k = (target.width()/bounds.width()).min(target.height()/bounds.height());
    let mut camera = *initial.camera();
    camera.paper_center = target.center() + (camera.paper_center - bounds.center()) * k;
    camera.scale *= k;
    let scene = build(elevation, Some(camera));
    let built = start.elapsed();
    let start = std::time::Instant::now();
    let output = scene.render();
    eprintln!("{}: construcción {built:?}, ocultas {:?}", path.display(), start.elapsed());
    let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="210mm" height="297mm" viewBox="0 0 210 297"><rect width="210" height="297" fill="white"/>"#);
    write!(svg, r##"<g fill="#35216C" font-family="Helvetica"><text x="12" y="13" font-size="4.5">Vectorio · motor de escenas</text><text x="12" y="19" font-size="2.8">Bodegón de pruebas · elevación {elevation:.3}°</text></g>"##).unwrap();
    let colors = ["#35216C", "#B85B21", "#8E9227", "#3A9290", "#695397", "#A42C71", "#A6A6B0"];
    for (i, part) in output.parts.iter().enumerate() {
        eprintln!("  {}: {} trazos, {:.3} m", part.name, part.lines.len(), output.length_m(part.id));
        write!(svg, r#"<g id="parte-{}" data-name="{}" fill="none" stroke="{}" stroke-width="0.2" stroke-linecap="round" stroke-linejoin="round">"#,
            part.id.0, part.name, colors[i % colors.len()]).unwrap();
        for line in &part.lines {
            svg.push_str("<polyline points=\"");
            for point in line { write!(svg, "{:.6},{:.6} ", point.x, point.y).unwrap(); }
            svg.push_str("\"/>");
        }
        svg.push_str("</g>");
        write!(svg, r#"<text x="12" y="{}" fill="{}" font-family="Helvetica" font-size="2.5">{} · {:.2} m</text>"#,
            277.0 + i as f64 * 2.4, colors[i % colors.len()], part.name, output.length_m(part.id)).unwrap();
    }
    svg.push_str("</svg>");
    std::fs::write(path, svg)
}

fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let out = args.iter().find(|arg| !arg.starts_with("--")).map(String::as_str).unwrap_or("muestra_escena.svg");
    let path = std::path::Path::new(out);
    save(path, Camera::default().elev_deg)?;
    if args.iter().any(|arg| arg == "--dimetric") || std::env::var("VECTORIO_DIMETRIC").is_ok_and(|v| v == "1") {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        save(&path.with_file_name(format!("{stem}_dimetrica.svg")), 26.57)?;
    }
    Ok(())
}
