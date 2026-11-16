//! Muestrario A4 de las piezas densas de la fase 5 (multitud, cintas, vehículos,
//! fachadas, cúpulas y criaturas):
//! `cargo run -p vectorio-core --example kit_denso --release -- kit_denso.svg`.
mod support;
use std::fmt::Write;
use vectorio_core::{p, BBox, geom::Rng, scene::{Camera, Scene, V3}};
use vectorio_core::generate::{fit_scene, GenContext, Detail, style::Style, kit::{self, v, Band, Marking, Vehicle, Crowd, Rhythm, Roof, Species}};

const LABELS: [&str; 16] = [
    "Cola y paso de cebra", "Grada y explanada", "Corrillos y baile", "Calle curva con tráfico",
    "Circuito elevado", "Vía, tren y barcas", "Fachada comercial", "Torre de cristal y azotea",
    "Cúpula con pueblo", "Cetáceo", "Cefalópodo", "Saurio",
    "Cohete", "Estación", "Rótulos", "Cúpula y criatura",
];

fn world(i: usize) -> [V3; 2] {
    match i {
        0..=2 => [v(-8.0, -8.0, 0.0), v(8.0, 8.0, 2.0)],
        3..=5 => [v(-20.0, -20.0, 0.0), v(20.0, 20.0, 6.0)],
        6 => [v(-1.0, -1.0, 0.0), v(15.0, 11.0, 17.0)],
        7 => [v(-2.0, -2.0, 0.0), v(16.0, 12.0, 30.0)],
        8 | 15 => [v(-14.0, -14.0, -2.0), v(14.0, 14.0, 12.0)],
        13 => [v(-12.0, -12.0, -12.0), v(12.0, 12.0, 12.0)],
        14 => [v(-1.0, -1.0, 0.0), v(12.0, 1.0, 4.0)],
        _ => [v(-2.0, -6.0, -4.0), v(22.0, 6.0, 6.0)],
    }
}

fn panel(s: &mut Scene, style: Style, i: usize, rng: &mut Rng) {
    let mut crowd = Crowd::new(style);
    match i {
        0 => {
            let band = Band::new(&kit::smooth(&[v(-8.0, -2.0, 0.0), v(0.0, 0.0, 0.0), v(8.0, 3.0, 0.0)], 3), 7.0).unwrap();
            kit::ribbon(s, &band, 0.0, Marking::Dashed);
            kit::zebra(s, &band, band.length() * 0.45, 3.0);
            let (a, _) = band.at(band.length() * 0.45 + 1.5, -4.5);
            let (b, _) = band.at(band.length() * 0.45 + 1.5, 4.5);
            crowd.crossing(s, rng, a, b, 2.4, 0.9);
            crowd.queue(s, rng, v(-6.0, 4.0, 0.0), v(2.0, 7.0, 0.0));
        }
        1 => {
            crowd.stands(s, rng, v(-7.0, 2.0, 0.0), v(14.0, 0.0, 0.0), v(0.0, 0.9, 0.0), 0.45, 6, 0.85);
            crowd.field(s, rng, v(-7.0, -7.0, 0.0), v(7.0, 1.0, 0.0), 1.2);
        }
        2 => {
            crowd.cluster(s, rng, v(-4.0, -3.0, 0.0), 2.2, 12);
            crowd.cluster(s, rng, v(4.0, 4.0, 0.0), 1.6, 7);
            crowd.dance(s, rng, v(2.0, -3.0, 0.0), 3);
        }
        3 => {
            let path = kit::smooth(&[v(-20.0, -10.0, 0.0), v(-5.0, -8.0, 0.0), v(5.0, 8.0, 0.0), v(20.0, 10.0, 0.0)], 4);
            let band = Band::new(&path, 9.0).unwrap();
            kit::ribbon(s, &band, 0.0, Marking::Dashed);
            kit::convoy(s, &band, &[Vehicle::Car, Vehicle::Car, Vehicle::Bus, Vehicle::Moto], 2.2, 5.0, rng);
            kit::convoy(s, &band, &[Vehicle::Car, Vehicle::Moto], -2.2, 6.0, rng);
        }
        4 => {
            let path: Vec<V3> = (0..=48).map(|k| {
                let a = std::f64::consts::TAU * k as f64 / 48.0 * 0.8;
                v(a.cos() * 15.0, a.sin() * 10.0, 3.0 + 2.5 * (a * 2.0).sin())
            }).collect();
            let band = Band::new(&path, 8.0).unwrap();
            kit::ribbon(s, &band, 0.8, Marking::Kerbs);
            kit::convoy(s, &band, &[Vehicle::Car], 0.0, 9.0, rng);
            kit::house(s, style, v(-4.0, -3.0, 0.0), v(4.0, 3.0, 4.0), Roof::Flat, rng);
        }
        5 => {
            let rail = Band::new(&kit::smooth(&[v(-20.0, 6.0, 0.0), v(0.0, 2.0, 0.0), v(20.0, 12.0, 0.0)], 3), 3.0).unwrap();
            kit::ribbon(s, &rail, 0.0, Marking::Rails);
            kit::convoy(s, &rail, &[Vehicle::Train], 0.0, 0.0, rng);
            kit::vehicle(s, Vehicle::Boat, v(-12.0, -10.0, 0.0), v(1.0, 0.3, 0.0));
            kit::vehicle(s, Vehicle::Boat, v(4.0, -14.0, 0.0), v(-0.4, 1.0, 0.0));
        }
        6 => {
            let (min, max) = (v(0.0, 0.0, 0.0), v(14.0, 10.0, 16.0));
            kit::house(s, style, min, max, Roof::Flat, rng);
            let walls = kit::walls(min, max);
            for (k, wall) in walls.into_iter().enumerate() {
                if !wall.faces_camera(s) {
                    continue;
                }
                let rhythm = [Rhythm::Regular, Rhythm::Paired, Rhythm::Tall, Rhythm::Ribbon][k];
                kit::window_grid(s, style, wall, 4.0, 3.0, rhythm);
                kit::balconies(s, style, wall, 7.0, rhythm, 1.0, &mut crowd, rng);
                kit::shopfront(s, style, wall, 0.5, wall.width * 0.5 - 0.7, 3.8, Some("PAN"), true);
                kit::shopfront(s, style, wall, wall.width * 0.5 + 0.2, wall.width * 0.5 - 0.7, 3.8, Some("LIBROS"), true);
            }
        }
        7 => {
            kit::glass_tower(s, style, v(0.0, 0.0, 0.0), v(8.0, 8.0, 30.0), 3.4, rng);
            kit::house(s, style, v(9.5, 0.0, 0.0), v(16.0, 10.0, 8.0), Roof::Flat, rng);
            kit::roof_terrace(s, v(9.5, 0.0, 8.0), v(16.0, 10.0, 8.0), &mut crowd, rng);
        }
        8 => {
            kit::house(s, style, v(-12.0, 8.0, -2.0), v(-6.0, 13.0, 8.0), Roof::Gable, rng);
            let dome = kit::Dome { center: v(0.0, 0.0, 0.0), radius: 10.0, plinth: 1.5, ribs: 12 };
            kit::dome(s, dome, |s, floor| {
                let mut rng = Rng::new(7);
                let mut crowd = Crowd::new(style);
                kit::house(s, style, floor.center + v(-5.0, -2.0, 0.0), floor.center + v(-1.0, 2.0, 3.0), Roof::Gable, &mut rng);
                kit::house(s, style, floor.center + v(1.0, -4.0, 0.0), floor.center + v(5.0, -1.0, 4.0), Roof::Hip, &mut rng);
                kit::vegetation(s, style, floor.center + v(3.0, 3.0, 0.0), 5.0, kit::Plant::Cypress, &mut rng);
                crowd.cluster(s, &mut rng, floor.center + v(0.0, 4.0, 0.0), 2.5, 10);
            });
        }
        9..=11 => {
            let species = [Species::Cetacean, Species::Cephalopod, Species::Saurian][i - 9];
            kit::creature(s, style, species, v(20.0, 0.0, 0.0), v(1.0, 0.0, 0.0), 20.0, 0.8, rng);
        }
        12 => kit::rocket(s, style, v(10.0, 0.0, -3.0), v(0.3, 0.0, 1.0), 9.0),
        13 => kit::station(s, style, v(0.0, 0.0, 0.0), v(0.3, 0.2, 1.0), 10.0),
        14 => {
            kit::sign(s, v(0.0, 0.0, 0.5), v(1.0, 0.0, 0.0), "MERCADO", 1.4);
            kit::sign(s, v(0.0, 0.0, 2.3), v(0.0, 1.0, 0.0), "Café del puerto", 1.0);
        }
        _ => {
            let dome = kit::Dome { center: v(0.0, 0.0, 0.0), radius: 9.0, plinth: 1.0, ribs: 10 };
            kit::dome(s, dome, |s, floor| {
                let mut rng = Rng::new(3);
                let mut crowd = Crowd::new(style);
                crowd.field(s, &mut rng, floor.center - v(5.0, 5.0, 0.0), floor.center + v(5.0, 5.0, 0.0), 0.5);
            });
            kit::creature(s, style, Species::Cetacean, v(14.0, -10.0, 10.0), v(-1.0, 0.6, -0.15), 26.0, 0.6, rng);
        }
    }
}

fn main() -> std::io::Result<()> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "kit_denso.svg".into());
    let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="297mm" height="210mm" viewBox="0 0 297 210"><rect width="297" height="210" fill="white"/>"#);
    for (i, label) in LABELS.iter().enumerate() {
        let x = 5.0 + (i % 4) as f64 * 73.0;
        let y = 4.0 + (i / 4) as f64 * 51.0;
        let area = BBox { min: p(x + 2.0, y + 3.0), max: p(x + 68.0, y + 45.0) };
        let ctx = GenContext { area, seed: i as u64, pen_mm: 0.3, detail: Detail::Final };
        let style = Style::new(&ctx, 0.8);
        let mut camera = Camera::default();
        camera.fit(world(i), area, 1.0);
        let start = std::time::Instant::now();
        let scene = fit_scene(area, camera, |camera, paper| {
            let mut s = Scene::new(camera, paper);
            style.prepare(&mut s);
            s.set_sun(Some(v(-1.0, 1.0, 1.0)));
            let mut rng = Rng::new(i as u64);
            panel(&mut s, style, i, &mut rng);
            s
        });
        let load = scene.load();
        let output = style.finish(&scene);
        eprintln!("{label}: {} caras, {} segmentos, {:?}", load.faces, load.segments, start.elapsed());
        support::paths(&mut svg, &output);
        write!(svg, r#"<text x="{x}" y="{}" font-family="Helvetica" font-size="2.6">{label}</text>"#, y + 48.5).unwrap();
    }
    svg.push_str("</svg>");
    std::fs::write(path, svg)
}
