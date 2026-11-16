//! Pruebas de las piezas densas (fase 5): multitud, cintas, vehículos, fachadas,
//! cúpulas y criaturas. Deterministas, sin NaN, sin costuras ni caras solapadas,
//! con el interior de la cúpula recortado y dentro del presupuesto de la lámina.
use super::*;
use crate::scene::{Camera, FACE_BUDGET, SEGMENT_BUDGET};
use crate::{p, BBox};
use crate::generate::{GenContext, GenOutput};

type Piece = fn(&mut Scene, Style, &mut Rng);

#[test]
fn curved_creatures_keep_a_complete_visible_eye_in_every_view() {
    for yaw in [45.0, 135.0, 225.0, 315.0] {
        let (mut s, style) = scene(3.0, Detail::Final);
        let mut camera = *s.camera();
        camera.yaw_deg = yaw;
        s = Scene::new(camera, A4);
        style.prepare(&mut s);
        let heading = camera.axes().0;
        creature(&mut s, style, Species::Cetacean, heading * 15.0, heading, 30.0, 0.65, &mut Rng::new(1));
        let output = style.finish(&s);
        let details = output.parts.iter().find(|part| part.name == "detalle").unwrap();
        assert!(details.lines.iter().any(|line| line.len() >= 12 && line.first() == line.last()), "vista {yaw}: ojo cortado por la piel");
    }
}

const A4: BBox = BBox { min: crate::geom::P { x: 0.0, y: 0.0 }, max: crate::geom::P { x: 210.0, y: 297.0 } };

/// Escena A4 con la cámara a escala fija (mm por metro) y la vista habitual.
fn scene(scale: f64, detail: Detail) -> (Scene, Style) {
    let ctx = GenContext { area: A4, seed: 1, pen_mm: 0.35, detail };
    let style = Style::new(&ctx, 0.8);
    let camera = Camera { scale, yaw_deg: 45.0, ..Default::default() };
    let mut scene = Scene::new(camera, A4);
    style.prepare(&mut scene);
    scene.set_sun(Some(v(-1.0, 1.0, 1.0)));
    (scene, style)
}

/// Escena A4 encuadrada en la caja `world`.
fn framed(world: [V3; 2], detail: Detail) -> (Scene, Style) {
    let (mut scene, style) = scene(1.0, detail);
    let mut camera = *scene.camera();
    camera.fit(world, A4, 5.0);
    scene = Scene::new(camera, A4);
    style.prepare(&mut scene);
    scene.set_sun(Some(v(-1.0, 1.0, 1.0)));
    (scene, style)
}

fn band(points: &[V3], width: f64) -> Band {
    Band::new(&smooth(points, 3), width).expect("banda válida")
}

fn crowd_piece(s: &mut Scene, style: Style, rng: &mut Rng) {
    let mut crowd = Crowd::new(style);
    crowd.queue(s, rng, v(-6.0, 4.0, 0.0), v(2.0, 7.0, 0.0));
    crowd.crossing(s, rng, v(-3.0, -4.0, 0.0), v(-3.0, 4.0, 0.0), 2.4, 0.9);
    crowd.stands(s, rng, v(-7.0, 2.0, 0.0), v(14.0, 0.0, 0.0), v(0.0, 0.9, 0.0), 0.45, 4, 0.85);
    crowd.cluster(s, rng, v(4.0, 4.0, 0.0), 1.6, 7);
    crowd.dance(s, rng, v(2.0, -3.0, 0.0), 2);
}

fn street_piece(s: &mut Scene, _: Style, rng: &mut Rng) {
    let road = band(&[v(-20.0, -10.0, 0.0), v(-5.0, -8.0, 0.0), v(5.0, 8.0, 0.0), v(20.0, 10.0, 0.0)], 9.0);
    ribbon(s, &road, 0.0, Marking::Dashed);
    // Fuera de una estación de la banda: si no, el canto de las franjas cae sobre
    // la arista (invisible) entre dos triángulos de la calle y el detector de
    // costuras lo toma por una junta.
    zebra(s, &road, road.length() * 0.5 + 0.3, 3.0);
    convoy(s, &road, &[Vehicle::Car, Vehicle::Bus, Vehicle::Moto], 2.2, 5.0, rng);
}

fn circuit_piece(s: &mut Scene, _: Style, rng: &mut Rng) {
    let path: Vec<V3> = (0..=40)
        .map(|k| {
            let a = std::f64::consts::TAU * k as f64 / 40.0 * 0.8;
            v(a.cos() * 15.0, a.sin() * 10.0, 3.0 + 2.0 * (a * 2.0).sin())
        })
        .collect();
    let track = Band::new(&path, 8.0).expect("circuito");
    ribbon(s, &track, 0.8, Marking::Kerbs);
    convoy(s, &track, &[Vehicle::Car], 0.0, 9.0, rng);
}

fn rail_piece(s: &mut Scene, _: Style, rng: &mut Rng) {
    let rail = band(&[v(-20.0, 6.0, 0.0), v(0.0, 2.0, 0.0), v(20.0, 12.0, 0.0)], 3.0);
    ribbon(s, &rail, 0.0, Marking::Rails);
    convoy(s, &rail, &[Vehicle::Train], 0.0, 0.0, rng);
    vehicle(s, Vehicle::Boat, v(-12.0, -10.0, 0.0), v(1.0, 0.3, 0.0));
}

fn facade_piece(s: &mut Scene, style: Style, rng: &mut Rng) {
    let (min, max) = (v(0.0, 0.0, 0.0), v(14.0, 10.0, 16.0));
    house(s, style, min, max, Roof::Flat, rng);
    let mut crowd = Crowd::new(style);
    for wall in walls(min, max) {
        if !wall.faces_camera(s) {
            continue;
        }
        window_grid(s, style, wall, 4.0, 3.0, Rhythm::Paired);
        balconies(s, style, wall, 7.0, Rhythm::Paired, 1.0, &mut crowd, rng);
        shopfront(s, style, wall, 0.5, wall.width * 0.5 - 0.7, 3.8, Some("PAN"), true);
    }
}

fn tower_piece(s: &mut Scene, style: Style, rng: &mut Rng) {
    glass_tower(s, style, v(0.0, 0.0, 0.0), v(8.0, 8.0, 30.0), 3.4, rng);
    house(s, style, v(9.5, 0.0, 0.0), v(16.0, 10.0, 8.0), Roof::Flat, rng);
    let mut crowd = Crowd::new(style);
    roof_terrace(s, v(9.5, 0.0, 8.0), v(16.0, 10.0, 8.0), &mut crowd, rng);
}

fn dome_piece(s: &mut Scene, style: Style, rng: &mut Rng) {
    let seed = rng.next_u64();
    let glass = Dome { center: V3::default(), radius: 10.0, plinth: 1.5, ribs: 12 };
    dome(s, glass, |s, floor| {
        let mut rng = Rng::new(seed);
        let c = floor.center;
        house(s, style, c + v(-5.0, -2.0, 0.0), c + v(-1.0, 2.0, 3.0), Roof::Gable, &mut rng);
        vegetation(s, style, c + v(3.0, 3.0, 0.0), 5.0, Plant::Cypress, &mut rng);
    });
}

fn creature_piece(s: &mut Scene, style: Style, rng: &mut Rng) {
    let species = SPECIES[(rng.f() * 3.0) as usize % 3];
    creature(s, style, species, v(20.0, 0.0, 0.0), v(1.0, 0.0, 0.0), 20.0, 0.8, rng);
}

fn space_piece(s: &mut Scene, style: Style, _: &mut Rng) {
    rocket(s, style, v(10.0, 0.0, -3.0), v(0.3, 0.0, 1.0), 9.0);
    station(s, style, v(-10.0, 0.0, 0.0), v(0.3, 0.2, 1.0), 8.0);
}

const PIECES: [(&str, Piece); 10] = [
    ("multitud", crowd_piece),
    ("calle", street_piece),
    ("circuito", circuit_piece),
    ("vía", rail_piece),
    ("fachada", facade_piece),
    ("torre", tower_piece),
    ("cúpula", dome_piece),
    ("criatura", creature_piece),
    ("espacio", space_piece),
    ("rótulos", |s, _, _| {
        sign(s, v(0.0, 0.0, 0.5), v(1.0, 0.0, 0.0), "MERCADO", 1.4);
        sign(s, v(0.0, 0.0, 2.3), v(0.0, 1.0, 0.0), "Café del puerto", 1.0);
    }),
];

fn build(piece: Piece, scale: f64, seed: u64) -> (Scene, GenOutput) {
    let (mut s, style) = scene(scale, Detail::Final);
    piece(&mut s, style, &mut Rng::new(seed));
    let output = style.finish(&s);
    (s, output)
}

fn finite(output: &GenOutput) -> bool {
    output.parts.iter().flat_map(|part| &part.lines).flatten().all(|q| q.x.is_finite() && q.y.is_finite())
}

#[test]
fn dense_pieces_are_deterministic_finite_and_visible() {
    for (name, piece) in PIECES {
        let (_, first) = build(piece, 4.0, 5);
        let (_, second) = build(piece, 4.0, 5);
        assert_eq!(first, second, "{name}");
        assert!(finite(&first), "{name}");
        assert!(first.total_m() > 0.0, "{name}");
    }
}

#[test]
fn dense_pieces_draw_nothing_with_impossible_inputs() {
    let (mut s, style) = scene(6.0, Detail::Final);
    let mut rng = Rng::new(1);
    let mut crowd = Crowd::new(style);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let at = v(bad, 0.0, 0.0);
        figure(&mut s, at, 1.7, Pose::Stand, false);
        crowd.queue(&mut s, &mut rng, at, V3::default());
        crowd.cluster(&mut s, &mut rng, at, 2.0, 5);
        vehicle(&mut s, Vehicle::Bus, at, v(1.0, 0.0, 0.0));
        vehicle(&mut s, Vehicle::Car, V3::default(), v(bad, 0.0, 0.0));
        sign(&mut s, at, v(1.0, 0.0, 0.0), "BAR", 1.0);
        creature(&mut s, style, Species::Cetacean, at, v(1.0, 0.0, 0.0), 10.0, 0.5, &mut rng);
        rocket(&mut s, style, at, v(0.0, 0.0, 1.0), 8.0);
    }
    for bad in [f64::NAN, f64::INFINITY, -1.0, 0.0, 1e20] {
        figure(&mut s, V3::default(), bad, Pose::Wave, true);
        assert!(Band::new(&[V3::default(), v(5.0, 0.0, 0.0)], bad).is_none());
        sign(&mut s, V3::default(), v(1.0, 0.0, 0.0), "BAR", bad);
        glass_tower(&mut s, style, V3::default(), v(bad, bad, bad), 3.0, &mut rng);
        dome(&mut s, Dome { center: V3::default(), radius: bad, plinth: 1.0, ribs: 8 }, |_, _| {});
        creature(&mut s, style, Species::Saurian, V3::default(), v(1.0, 0.0, 0.0), bad, 0.5, &mut rng);
        rocket(&mut s, style, V3::default(), v(0.0, 0.0, 1.0), bad);
        station(&mut s, style, V3::default(), v(0.0, 0.0, 1.0), bad);
    }
    assert!(tube(&mut s, style, &[], &[], false).is_empty());
    assert_eq!(style.finish(&s).total_m(), 0.0);
}

#[test]
fn dense_pieces_draw_no_seams_and_no_overlapping_faces() {
    for (name, piece) in PIECES {
        for scale in [3.0, 8.0] {
            for seed in 0..3 {
                let (s, _) = build(piece, scale, seed);
                let seams = s.seams(&s.render());
                assert!(seams.is_empty(), "{name} a escala {scale}, semilla {seed}: {seams:?}");
                let issues = s.construction_issues(1.5);
                assert!(issues.is_empty(), "{name} a escala {scale}, semilla {seed}: {issues:?}");
            }
        }
    }
}

#[test]
fn the_dome_keeps_its_interior_inside_the_glass() {
    let (mut s, style) = scene(6.0, Detail::Final);
    let radius = 8.0;
    let glass = Dome { center: V3::default(), radius, plinth: 0.0, ribs: 10 };
    dome(&mut s, glass, |s, floor| {
        // Una torre tres veces más alta que la cúpula: sin recorte se saldría.
        let c = floor.center;
        let mut rng = Rng::new(2);
        house(s, style, c + v(-2.0, -2.0, 0.0), c + v(2.0, 2.0, radius * 3.0), Roof::Flat, &mut rng);
        let mut crowd = Crowd::new(style);
        crowd.field(s, &mut rng, c + v(-12.0, -12.0, 0.0), c + v(12.0, 12.0, 0.0), 0.3);
    });
    let (center, _) = s.camera().project(V3::default());
    let limit = radius * s.camera().scale + 0.05;
    let output = style.finish(&s);
    let tower = output.parts.iter().find(|part| part.name == "contorno").expect("contorno");
    assert!(tower.lines.len() > 2, "el interior se ha perdido entero");
    for part in &output.parts {
        for q in part.lines.iter().flatten() {
            let distance = ((q.x - center.x).powi(2) + (q.y - center.y).powi(2)).sqrt();
            assert!(distance <= limit, "{}: {distance} mm > {limit} mm", part.name);
        }
    }
}

#[test]
fn preview_thins_a_crowd_without_moving_anyone() {
    let place = |detail: Detail| {
        let (mut s, style) = scene(8.0, detail);
        let mut crowd = Crowd::new(style);
        let mut rng = Rng::new(9);
        crowd.field(&mut s, &mut rng, v(-6.0, -6.0, 0.0), v(6.0, 6.0, 0.0), 0.8);
        (crowd.drawn, rng.next_u64())
    };
    let (full, after_full) = place(Detail::Final);
    let (thin, after_thin) = place(Detail::Preview);
    assert!(thin * 3 < full * 2 && thin * 3 > full, "{thin} de {full}");
    assert_eq!(after_full, after_thin, "la vista previa consume otro sorteo");
}

#[test]
fn three_thousand_people_fit_the_sheet_budget_and_time() {
    let world = [v(-45.0, -45.0, 0.0), v(45.0, 45.0, 2.0)];
    let (mut s, style) = framed(world, Detail::Final);
    let mut crowd = Crowd::new(style);
    let mut rng = Rng::new(4);
    crowd.field(&mut s, &mut rng, world[0], world[1], 0.6);
    assert_eq!(crowd.drawn, crowd.limit, "escala {}", s.camera().scale);
    let load = s.load();
    assert!(load.within_budget(), "{load:?}");
    let start = std::time::Instant::now();
    let output = style.finish(&s);
    let elapsed = start.elapsed();
    let measured = density(&output, A4, crowd.drawn);
    assert_eq!(measured.people, 3_000);
    assert!((measured.per_dm2 - measured.meters / 6.237).abs() < 1e-9);
    eprintln!("multitud: {load:?}, {measured:?} en {elapsed:?}");
    assert!(elapsed.as_millis() < 1_500, "{elapsed:?} para {load:?}");
}

#[test]
fn a_sheet_at_the_budget_renders_in_time() {
    let world = [v(-60.0, -60.0, 0.0), v(60.0, 60.0, 14.0)];
    let (mut s, style) = framed(world, Detail::Final);
    let mut rng = Rng::new(8);
    let mut crowd = Crowd::new(style);
    // Una manzana por celda de una rejilla de 20 × 20 hasta rozar uno de los topes;
    // con gente, el de segmentos llega antes que el de caras.
    for k in 0..400 {
        let load = s.load();
        if load.faces >= FACE_BUDGET - 100 || load.segments >= SEGMENT_BUDGET - 800 {
            break;
        }
        let x = -58.0 + (k % 20) as f64 * 5.8;
        let y = -58.0 + (k / 20) as f64 * 5.8;
        let height = 3.0 + rng.f() * 9.0;
        house(&mut s, style, v(x, y, 0.0), v(x + 4.2, y + 4.2, height), Roof::Gable, &mut rng);
        crowd.cluster(&mut s, &mut rng, v(x + 5.0, y + 5.0, 0.0), 0.8, 20);
    }
    let load = s.load();
    assert!(load.within_budget(), "{load:?}");
    assert!(load.faces * 4 >= FACE_BUDGET * 3 || load.segments * 4 >= SEGMENT_BUDGET * 3, "{load:?}");
    let start = std::time::Instant::now();
    let output = style.finish(&s);
    let elapsed = start.elapsed();
    assert!(output.total_m() > 0.0);
    eprintln!("presupuesto: {load:?} de {FACE_BUDGET}/{SEGMENT_BUDGET} en {elapsed:?}");
    assert!(elapsed.as_millis() < 3_000, "{elapsed:?} para {load:?}");
}

#[test]
fn density_divides_meters_by_paper_area() {
    let line = vec![p(0.0, 0.0), p(1000.0, 0.0)];
    let output = GenOutput {
        parts: vec![crate::generate::GenPart { name: "contorno".into(), lines: vec![line], pen_order: None }],
        warning: None,
    };
    let square = BBox { min: p(0.0, 0.0), max: p(100.0, 100.0) };
    assert_eq!(density(&output, square, 12), Density { people: 12, meters: 1.0, per_dm2: 1.0 });
    let empty = BBox { min: p(0.0, 0.0), max: p(0.0, 0.0) };
    assert_eq!(density(&output, empty, 0).per_dm2, 0.0);
}
