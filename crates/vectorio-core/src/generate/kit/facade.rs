//! Gramática de fachada para las escenas densas: rejillas de ventanas con ritmo,
//! bajos comerciales con escaparate, toldo y rótulo, balcones con gente, azoteas
//! vividas y torres de cristal con sus plantas a la vista.
//!
//! Todo se coloca sobre un `Wall`: un rectángulo vertical con su dirección de
//! izquierda a derecha vista desde fuera. Así la misma pieza sirve en cualquier
//! cara de cualquier edificio.
use super::*;
use super::crowd::{Crowd, Pose};
use crate::font::{self, Align, FontStyle, TextSpec};

/// Pared vertical vista desde fuera: `origin` es su esquina inferior izquierda y
/// `along` (horizontal y unitario) va hacia la derecha.
#[derive(Clone, Copy, Debug)]
pub struct Wall { pub origin: V3, pub along: V3, pub width: f64, pub height: f64 }

impl Wall {
    /// Normal exterior, horizontal.
    pub fn outward(&self) -> V3 {
        self.along.cross(v(0.0, 0.0, 1.0))
    }
    /// Punto a `u` metros del borde izquierdo, a `w` de altura y `out` por fuera.
    pub fn at(&self, u: f64, w: f64, out: f64) -> V3 {
        self.origin + self.along * u + v(0.0, 0.0, w) + self.outward() * out
    }
    pub fn faces_camera(&self, scene: &Scene) -> bool {
        scene.camera().facing(self.outward())
    }
    fn valid(&self) -> bool {
        self.origin.finite() && (self.along.length() - 1.0).abs() < 1e-6 && self.along.z.abs() < 1e-9 &&
            self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0
    }
}

/// Las cuatro paredes de una caja, en el orden +y, +x, −y, −x.
pub fn walls(min: V3, max: V3) -> [Wall; 4] {
    let height = max.z - min.z;
    let (dx, dy) = (max.x - min.x, max.y - min.y);
    [
        Wall { origin: v(max.x, max.y, min.z), along: v(-1.0, 0.0, 0.0), width: dx, height },
        Wall { origin: v(max.x, min.y, min.z), along: v(0.0, 1.0, 0.0), width: dy, height },
        Wall { origin: v(min.x, min.y, min.z), along: v(1.0, 0.0, 0.0), width: dx, height },
        Wall { origin: v(min.x, max.y, min.z), along: v(0.0, -1.0, 0.0), width: dy, height },
    ]
}

/// Ritmo de huecos de una rejilla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rhythm {
    /// Ventanas iguales a paso fijo.
    Regular,
    /// Ventanas por parejas con un macizo entre parejas.
    Paired,
    /// Bandas corridas de planta a planta.
    Ribbon,
    /// Huecos altos y estrechos, como balconeras.
    Tall,
}

pub const RHYTHMS: [Rhythm; 4] = [Rhythm::Regular, Rhythm::Paired, Rhythm::Ribbon, Rhythm::Tall];

/// Huecos de una planta: (desde u, anchura) dentro de `width`.
fn bays(rhythm: Rhythm, width: f64) -> Vec<(f64, f64)> {
    let margin = 0.7;
    let usable = width - margin * 2.0;
    if usable < 0.8 {
        return Vec::new();
    }
    match rhythm {
        Rhythm::Ribbon => vec![(margin, usable)],
        Rhythm::Regular | Rhythm::Tall => {
            let pitch = if rhythm == Rhythm::Tall { 2.2 } else { 2.6 };
            let count = (usable / pitch).floor().max(1.0) as usize;
            let pitch = usable / count as f64;
            let glass: f64 = if rhythm == Rhythm::Tall { 0.95 } else { 1.3 };
            let glass = glass.min(pitch * 0.7);
            (0..count).map(|k| (margin + pitch * (k as f64 + 0.5) - glass * 0.5, glass)).collect()
        }
        Rhythm::Paired => {
            let pitch = 4.4;
            let count = (usable / pitch).floor().max(1.0) as usize;
            let pitch = usable / count as f64;
            let glass = 1.1f64.min(pitch * 0.3);
            let mut out = Vec::new();
            for k in 0..count {
                let middle = margin + pitch * (k as f64 + 0.5);
                out.push((middle - glass - 0.2, glass));
                out.push((middle + 0.2, glass));
            }
            out
        }
    }
}

/// Rejilla de ventanas desde la altura `from` (sobre la base de la pared) con
/// plantas de `floor` metros. Devuelve las plantas dibujadas.
pub fn window_grid(scene: &mut Scene, style: Style, wall: Wall, from: f64, floor: f64, rhythm: Rhythm) -> usize {
    if !wall.valid() || !from.is_finite() || !floor.is_finite() || floor < 2.4 || !wall.faces_camera(scene) {
        return 0;
    }
    let floors = ((wall.height - from - 0.6) / floor).floor().max(0.0) as usize;
    let sill = if rhythm == Rhythm::Tall { 0.25 } else { 0.9 };
    let tall = if rhythm == Rhythm::Tall { floor - 0.75 } else { (floor - 1.6).max(1.0) };
    for level in 0..floors {
        let z = from + floor * level as f64 + sill;
        for (u, width) in bays(rhythm, wall.width) {
            opening(scene, style, wall.at(u, z, 0.0), wall.along, width, tall, style.openings);
        }
    }
    floors
}

/// Balcones volados delante de los huecos de una planta, con barandilla y, a
/// veces, alguien asomado.
#[allow(clippy::too_many_arguments)]
pub fn balconies(scene: &mut Scene, style: Style, wall: Wall, z: f64, rhythm: Rhythm, depth: f64, crowd: &mut Crowd, rng: &mut Rng) {
    if !wall.valid() || !wall.faces_camera(scene) || !scene.visible(1.0, 1.8) {
        return;
    }
    let slab = 0.15f64.max(style.quantum(scene) * 0.25);
    let depth = depth.max(super::street::apart(scene) * 1.2);
    for (a, b) in ledges(rhythm, wall.width) {
        let floor = [wall.at(a, z, 0.0), wall.at(b, z, 0.0), wall.at(b, z, depth), wall.at(a, z, depth)];
        part(scene, "contorno");
        scene.prism(&floor, v(0.0, 0.0, slab));
        let top = z + slab;
        part(scene, "detalle");
        let rail = [wall.at(a, top + 0.95, 0.0), wall.at(a, top + 0.95, depth), wall.at(b, top + 0.95, depth), wall.at(b, top + 0.95, 0.0)];
        scene.line(&rail);
        let bars = ((b - a) / 0.3).round().clamp(2.0, 40.0) as usize;
        for k in 0..=bars {
            let s = a + (b - a) * k as f64 / bars as f64;
            scene.line(&[wall.at(s, top, depth), wall.at(s, top + 0.95, depth)]);
        }
        let leaning = rng.f() < 0.35;
        let spot = a + 0.3 + rng.f() * (b - a - 0.6).max(0.0);
        if leaning {
            let mirror = rng.f() < 0.5;
            crowd.person(scene, rng, wall.at(spot, top, depth * 0.45), Pose::Stand, mirror);
        }
    }
}

/// Tramos de balcón: cada hueco con 0,25 m a cada lado, dentro de la pared. Los
/// que se tocan o se solapan (huecos pareados) se funden en una sola losa, para
/// que no queden dos losas coplanarias montadas.
fn ledges(rhythm: Rhythm, width: f64) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    for (u, w) in bays(rhythm, width) {
        let a = (u - 0.25).max(0.0);
        let b = (u + w + 0.25).min(width);
        match out.last_mut() {
            Some(last) if a <= last.1 + 0.05 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// Rótulo de una línea en el plano de la pared: letra de arquitecto sin pulso.
/// `origin` es la esquina inferior izquierda; devuelve la anchura ocupada en metros.
pub fn sign(scene: &mut Scene, origin: V3, along: V3, text: &str, height: f64) -> f64 {
    if !origin.finite() || !height.is_finite() || height <= 0.0 || text.trim().is_empty() || !scene.visible(height, 0.9) {
        return 0.0;
    }
    let spec = TextSpec {
        text: text.into(),
        size_mm: 10.0,
        style: FontStyle::Arquitecto,
        jitter: 0.0,
        align: Align::Left,
        ..Default::default()
    };
    let strokes = font::layout(&spec);
    let k = height / 10.0;
    let width = strokes.iter().flatten().fold(0.0f64, |m, q| m.max(q.x * k));
    // Si la dirección va hacia la izquierda de la hoja, el rótulo se escribe desde
    // el otro extremo para que no salga en espejo.
    let reversed = along.dot(scene.camera().axes().0) < 0.0;
    let (start, along) = if reversed { (origin + along * width, along * -1.0) } else { (origin, along) };
    part(scene, "rótulos");
    for stroke in strokes {
        let points: Vec<V3> = stroke.iter().map(|q| start + along * (q.x * k) + v(0.0, 0.0, (10.0 - q.y) * k)).collect();
        scene.line(&points);
    }
    width
}

/// Bajo comercial de `width` metros desde `u`: escaparate, puerta, toldo a rayas
/// y, si hay texto, un rótulo sobre el toldo.
#[allow(clippy::too_many_arguments)]
pub fn shopfront(scene: &mut Scene, style: Style, wall: Wall, u: f64, width: f64, height: f64, text: Option<&str>, awning: bool) {
    if !wall.valid() || !wall.faces_camera(scene) || width < 2.0 || height < 2.6 || u < 0.0 || u + width > wall.width + 1e-9 {
        return;
    }
    let door = 1.0f64.min(width * 0.3);
    let glass = width - door - 0.6;
    let kind = OpeningKind::Clear;
    opening(scene, style, wall.at(u + 0.2, 0.45, 0.0), wall.along, glass, height - 1.1, kind);
    opening(scene, style, wall.at(u + width - door - 0.2, 0.0, 0.0), wall.along, door, height - 0.65, OpeningKind::Dark);
    let top = height - 0.3;
    if awning && scene.visible(0.8, 1.2) {
        let reach = 1.3f64.max(super::street::apart(scene) * 2.0);
        let drop = 0.7;
        let profile = [wall.at(u, top, 0.0), wall.at(u, top, reach), wall.at(u, top - drop, reach), wall.at(u, top - drop * 0.15, 0.0)];
        part(scene, "tejados");
        scene.prism(&profile, wall.along * width);
        let stripes = (width / 0.5).round().clamp(2.0, 60.0) as usize;
        part(scene, "detalle");
        for k in 1..stripes {
            let s = u + width * k as f64 / stripes as f64;
            scene.line(&[wall.at(s, top, 0.0), wall.at(s, top, reach), wall.at(s, top - drop, reach)]);
        }
    }
    if let Some(text) = text {
        let letters = 0.45f64.min((width - 0.4) / (text.chars().count().max(1) as f64 * 0.75));
        let lift = super::street::apart(scene).min(0.06);
        sign(scene, wall.at(u + 0.2, top + 0.2, lift), wall.along, text, letters);
    }
}

/// Torre de cristal: un prisma con los forjados y los montantes trazados en las
/// caras que miran a la cámara y un remate retranqueado con antena.
pub fn glass_tower(scene: &mut Scene, style: Style, min: V3, max: V3, floor: f64, rng: &mut Rng) {
    if !dimensions(min, max) || !floor.is_finite() || floor < 2.8 {
        return;
    }
    let crown = (floor * (1.0 + rng.f())).min((max.z - min.z) * 0.15);
    let shaft_top = max.z - crown;
    // El vidrio no lleva trama de luz: lo cuentan los forjados y los montantes.
    part(scene, "contorno");
    scene.box3(min, v(max.x, max.y, shaft_top));
    let inset = style.quantum(scene).max((max.x - min.x).min(max.y - min.y) * 0.12);
    let crown_min = v(min.x + inset, min.y + inset, shaft_top);
    let crown_max = v(max.x - inset, max.y - inset, max.z);
    if dimensions(crown_min, crown_max) {
        box_piece(scene, style, crown_min, crown_max, "contorno");
        let mast = (crown_min + crown_max) * 0.5;
        part(scene, "detalle");
        scene.line(&[v(mast.x, mast.y, max.z), v(mast.x, mast.y, max.z + crown * 1.5)]);
    }
    if !scene.visible(floor, 1.2) {
        return;
    }
    part(scene, "detalle");
    let pitch = 1.5f64.max(1.2 / scene.camera().scale);
    for wall in walls(min, v(max.x, max.y, shaft_top)) {
        if !wall.faces_camera(scene) {
            continue;
        }
        let mut z = floor;
        while z < wall.height - 0.3 {
            scene.line(&[wall.at(0.0, z, 0.0), wall.at(wall.width, z, 0.0)]);
            z += floor;
        }
        let count = (wall.width / pitch).floor() as usize;
        for k in 1..count {
            let u = wall.width * k as f64 / count as f64;
            scene.line(&[wall.at(u, 0.0, 0.0), wall.at(u, wall.height, 0.0)]);
        }
    }
}

/// Azotea vivida dentro de [min, max] (min.z es el suelo de la azotea): sombrillas
/// y un corrillo. El pretil lo pone el edificio.
pub fn roof_terrace(scene: &mut Scene, min: V3, max: V3, crowd: &mut Crowd, rng: &mut Rng) {
    if !min.finite() || !max.finite() || max.x - min.x < 4.0 || max.y - min.y < 4.0 {
        return;
    }
    let center = v((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, min.z);
    let radius = ((max.x - min.x).min(max.y - min.y) * 0.5 - 1.2).max(0.6);
    let umbrellas = 1 + (rng.f() * 3.0) as usize;
    let canopy = 1.1;
    let mut feet: Vec<V3> = Vec::new();
    // Cada sombrilla prueba unos sitios; si ninguno deja libre su copa, no se pone.
    for _ in 0..umbrellas * 4 {
        if feet.len() == umbrellas {
            break;
        }
        let angle = rng.f() * std::f64::consts::TAU;
        let r = radius * rng.f().sqrt() * 0.8;
        let foot = center + v(angle.cos() * r, angle.sin() * r, 0.0);
        if feet.iter().all(|f| (*f - foot).length() > canopy * 2.0 + 0.3) {
            feet.push(foot);
        }
    }
    for foot in feet {
        parasol(scene, foot, 2.3, canopy);
    }
    let count = 3 + (rng.f() * 8.0) as usize;
    crowd.cluster(scene, rng, center, radius, count);
}

/// Sombrilla: mástil y copa de ocho gajos; la copa oculta lo que queda debajo.
pub fn parasol(scene: &mut Scene, foot: V3, height: f64, radius: f64) {
    if !foot.finite() || !scene.visible(radius, 1.0) {
        return;
    }
    let top = foot + v(0.0, 0.0, height);
    let rim_z = height - radius * 0.35;
    let rim: Vec<V3> = (0..8)
        .map(|k| {
            let angle = std::f64::consts::TAU * k as f64 / 8.0;
            foot + v(angle.cos() * radius, angle.sin() * radius, rim_z)
        })
        .collect();
    part(scene, "detalle");
    let canopy: Vec<Vec<V3>> = (0..8).map(|k| vec![top, rim[k], rim[(k + 1) % 8]]).chain(std::iter::once(rim.clone())).collect();
    if scene.polyhedron(&canopy).is_empty() {
        return;
    }
    scene.line(&[foot, foot + v(0.0, 0.0, rim_z)]);
}
