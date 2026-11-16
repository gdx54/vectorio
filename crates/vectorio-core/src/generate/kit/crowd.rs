//! Multitudes. Cada persona es una figura mínima de cinco o seis trazos y una
//! cabeza, dibujada en un cartel vertical que mira a la cámara. Una sola cara
//! oculta lo que queda detrás: así caben miles de personas en una hoja.
//!
//! Los grupos (cola, paso de cebra, grada, corrillo, baile, campo) cuentan algo;
//! la densidad sale de muchas figuras simples, no de dibujar mucho cada una.
use super::*;

/// Postura de una figura. `Sit` apoya la cadera a 0,45 m del pie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose { Stand, Walk, Wave, Cheer, Sit, Dance }

pub const POSES: [Pose; 6] = [Pose::Stand, Pose::Walk, Pose::Wave, Pose::Cheer, Pose::Sit, Pose::Dance];

/// Por debajo de esta altura en la hoja una figura no se lee: no se dibuja.
const MIN_FIGURE_MM: f64 = 1.6;
/// Por debajo de esta altura se dibuja sin brazos y con el tronco de un trazo.
const SIMPLE_FIGURE_MM: f64 = 2.6;

/// Trazos de una figura en unidades de su altura: u a la derecha, w hacia arriba.
struct Sketch {
    head: (f64, f64, f64),
    torso: [(f64, f64); 4],
    legs: Vec<Vec<(f64, f64)>>,
    arms: Vec<Vec<(f64, f64)>>,
}

fn sketch(pose: Pose) -> Sketch {
    let torso = [(-0.12, 0.80), (0.12, 0.80), (0.085, 0.50), (-0.085, 0.50)];
    let head = (0.0, 0.895, 0.075);
    let shoulder = |side: f64| (0.11 * side, 0.79);
    let hip = |side: f64| (0.045 * side, 0.50);
    match pose {
        Pose::Stand => Sketch {
            head,
            torso,
            legs: vec![vec![hip(-1.0), (-0.06, 0.0)], vec![hip(1.0), (0.06, 0.0)]],
            arms: vec![vec![shoulder(-1.0), (-0.145, 0.48)], vec![shoulder(1.0), (0.145, 0.48)]],
        },
        Pose::Walk => Sketch {
            head,
            torso,
            legs: vec![vec![hip(-1.0), (-0.12, 0.0)], vec![hip(1.0), (0.16, 0.0)]],
            arms: vec![vec![shoulder(-1.0), (-0.17, 0.53)], vec![shoulder(1.0), (0.21, 0.52)]],
        },
        Pose::Wave => Sketch {
            head,
            torso,
            legs: vec![vec![hip(-1.0), (-0.06, 0.0)], vec![hip(1.0), (0.06, 0.0)]],
            arms: vec![vec![shoulder(-1.0), (-0.145, 0.48)], vec![shoulder(1.0), (0.2, 0.9), (0.24, 1.02)]],
        },
        Pose::Cheer => Sketch {
            head,
            torso,
            legs: vec![vec![hip(-1.0), (-0.08, 0.0)], vec![hip(1.0), (0.08, 0.0)]],
            arms: vec![vec![shoulder(-1.0), (-0.24, 1.02)], vec![shoulder(1.0), (0.24, 1.02)]],
        },
        Pose::Dance => Sketch {
            head,
            torso,
            legs: vec![vec![hip(-1.0), (-0.1, 0.0)], vec![hip(1.0), (0.14, 0.24), (0.08, 0.0)]],
            arms: vec![vec![shoulder(-1.0), (-0.24, 0.70)], vec![shoulder(1.0), (0.25, 0.95)]],
        },
        Pose::Sit => {
            let drop = 0.24;
            let lower = |q: (f64, f64)| (q.0, q.1 - drop);
            Sketch {
                head: (0.0, head.1 - drop, head.2),
                torso: torso.map(lower),
                legs: vec![
                    vec![lower(hip(-1.0)), (0.17, 0.27), (0.19, 0.0)],
                    vec![lower(hip(1.0)), (0.22, 0.27), (0.24, 0.0)],
                ],
                arms: vec![vec![lower(shoulder(-1.0)), (0.12, 0.33)], vec![lower(shoulder(1.0)), (0.18, 0.33)]],
            }
        }
    }
}

/// Envolvente convexa en el plano (cadena monótona); sin puntos repetidos.
fn hull(mut points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    points.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let turn = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let mut lower: Vec<(f64, f64)> = Vec::new();
    for &q in &points {
        while lower.len() >= 2 && turn(lower[lower.len() - 2], lower[lower.len() - 1], q) <= 1e-12 {
            lower.pop();
        }
        lower.push(q);
    }
    let mut upper: Vec<(f64, f64)> = Vec::new();
    for &q in points.iter().rev() {
        while upper.len() >= 2 && turn(upper[upper.len() - 2], upper[upper.len() - 1], q) <= 1e-12 {
            upper.pop();
        }
        upper.push(q);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Figura de pie (o sentada) con el pie en `origin`. `mirror` la vuelve hacia la
/// izquierda de la hoja. Devuelve si se ha dibujado.
pub fn figure(scene: &mut Scene, origin: V3, height: f64, pose: Pose, mirror: bool) -> bool {
    if !origin.finite() || !height.is_finite() || height <= 0.0 || height > 50.0 {
        return false;
    }
    let paper = scene.paper_mm(height);
    if paper < MIN_FIGURE_MM {
        return false;
    }
    let (right, _, _) = scene.camera().axes();
    let side = if mirror { -1.0 } else { 1.0 };
    let at = |q: (f64, f64)| origin + right * (q.0 * height * side) + v(0.0, 0.0, q.1 * height);
    let shape = sketch(pose);
    let (hu, hw, hr) = shape.head;
    let sides = if paper < SIMPLE_FIGURE_MM { 6 } else { 8 };
    let head: Vec<(f64, f64)> = (0..sides)
        .map(|k| {
            let angle = std::f64::consts::TAU * (k as f64 + 0.5) / sides as f64;
            (hu + hr * angle.cos(), hw + hr * angle.sin())
        })
        .collect();
    let mut outline: Vec<(f64, f64)> = head.clone();
    outline.extend(shape.torso);
    for leg in &shape.legs {
        outline.extend(leg.iter().copied());
    }
    part(scene, "gente");
    let cover: Vec<V3> = hull(outline).into_iter().map(at).collect();
    scene.occluder(&cover);
    let head3: Vec<V3> = head.into_iter().map(at).collect();
    scene.closed(&head3);
    if paper < SIMPLE_FIGURE_MM {
        let neck = (0.0, shape.torso[0].1);
        let waist = (0.0, shape.torso[3].1);
        scene.line(&[at(neck), at(waist)]);
    } else {
        let torso: Vec<V3> = shape.torso.into_iter().map(at).collect();
        scene.closed(&torso);
        for arm in &shape.arms {
            let arm3: Vec<V3> = arm.iter().copied().map(at).collect();
            scene.line(&arm3);
        }
    }
    for leg in &shape.legs {
        let leg3: Vec<V3> = leg.iter().copied().map(at).collect();
        scene.line(&leg3);
    }
    true
}

/// Reparte figuras con un tope por hoja. En vista previa dibuja la mitad.
#[derive(Clone, Copy, Debug)]
pub struct Crowd {
    /// Altura media de un adulto, en metros.
    pub height: f64,
    /// Vista previa: se salta una de cada dos figuras.
    pub thin: bool,
    pub drawn: usize,
    pub limit: usize,
}

impl Crowd {
    pub fn new(style: Style) -> Self {
        Self { height: 1.72, thin: style.detail == Detail::Preview, drawn: 0, limit: 3_000 }
    }
    /// Una persona con altura y sentido sorteados. Devuelve si se ha dibujado.
    pub fn person(&mut self, scene: &mut Scene, rng: &mut Rng, at: V3, pose: Pose, mirror: bool) -> bool {
        // El sorteo se hace siempre, para que la vista previa no cambie la escena.
        let child = rng.f() < 0.08;
        let jitter = 0.9 + rng.f() * 0.16;
        let coin = rng.f();
        let skip = self.thin && coin < 0.5;
        if skip || self.drawn >= self.limit {
            return false;
        }
        let height = self.height * jitter * if child { 0.62 } else { 1.0 };
        let drawn = figure(scene, at, height, pose, mirror);
        if drawn {
            self.drawn += 1;
        }
        drawn
    }
    /// Cola de `from` a `to`: gente de pie, a 0,55–0,7 m, mirando hacia la cabeza.
    pub fn queue(&mut self, scene: &mut Scene, rng: &mut Rng, from: V3, to: V3) -> usize {
        let start = self.drawn;
        let length = (to - from).length();
        if !length.is_finite() || length < 0.3 {
            return 0;
        }
        let along = (to - from) * (1.0 / length);
        let across = v(-along.y, along.x, 0.0);
        let facing = along.dot(scene.camera().axes().0) < 0.0;
        let mut s = 0.0;
        while s <= length {
            let wobble = across * ((rng.f() - 0.5) * 0.16);
            let pose = if rng.f() < 0.12 { Pose::Wave } else { Pose::Stand };
            self.person(scene, rng, from + along * s + wobble, pose, facing);
            s += 0.55 + rng.f() * 0.15;
        }
        self.drawn - start
    }
    /// Gente cruzando de `a` a `b` por una franja de `width` metros, en los dos
    /// sentidos. `density` en personas por metro cuadrado.
    pub fn crossing(&mut self, scene: &mut Scene, rng: &mut Rng, a: V3, b: V3, width: f64, density: f64) -> usize {
        let start = self.drawn;
        let length = (b - a).length();
        if !length.is_finite() || length < 0.5 || !width.is_finite() || width <= 0.0 || !density.is_finite() {
            return 0;
        }
        let along = (b - a) * (1.0 / length);
        let across = v(-along.y, along.x, 0.0);
        let count = (length * width * density.clamp(0.0, 3.0)).round() as usize;
        let rightward = along.dot(scene.camera().axes().0) >= 0.0;
        for _ in 0..count {
            let at = a + along * (rng.f() * length) + across * ((rng.f() - 0.5) * width);
            let back = rng.f() < 0.45;
            self.person(scene, rng, at, Pose::Walk, rightward == back);
        }
        self.drawn - start
    }
    /// Grada: `rows` filas que suben `rise` y retroceden `step` (vector horizontal)
    /// a partir de `origin`, a lo largo de `along` (vector con su longitud). La gente
    /// se sienta o se levanta a animar; `fill` es la fracción de asientos ocupados.
    #[allow(clippy::too_many_arguments)]
    pub fn stands(&mut self, scene: &mut Scene, rng: &mut Rng, origin: V3, along: V3, step: V3, rise: f64, rows: usize, fill: f64) -> usize {
        let start = self.drawn;
        let length = along.length();
        if !origin.finite() || !step.finite() || !length.is_finite() || length < 0.5 || !rise.is_finite() {
            return 0;
        }
        let unit = along * (1.0 / length);
        let seats = (length / 0.55).floor() as usize;
        for row in 0..rows.min(200) {
            let base = origin + step * (row as f64 + 0.5) + v(0.0, 0.0, rise * row as f64);
            for seat in 0..seats {
                let at = base + unit * ((seat as f64 + 0.5) * 0.55);
                let taken = rng.f() < fill;
                let cheering = rng.f() < 0.25;
                let mirror = rng.f() < 0.5;
                if !taken {
                    continue;
                }
                let pose = if cheering { Pose::Cheer } else { Pose::Sit };
                self.person(scene, rng, at, pose, mirror);
            }
        }
        self.drawn - start
    }
    /// Corrillo de `count` personas alrededor de `center`, separadas al menos 0,5 m.
    pub fn cluster(&mut self, scene: &mut Scene, rng: &mut Rng, center: V3, radius: f64, count: usize) -> usize {
        let start = self.drawn;
        if !center.finite() || !radius.is_finite() || radius <= 0.0 {
            return 0;
        }
        let mut placed: Vec<V3> = Vec::new();
        let mut tries = 0;
        while placed.len() < count.min(400) && tries < count * 30 {
            tries += 1;
            let angle = rng.f() * std::f64::consts::TAU;
            let r = radius * rng.f().sqrt();
            let at = center + v(angle.cos() * r, angle.sin() * r, 0.0);
            if placed.iter().any(|q| (*q - at).length() < 0.5) {
                continue;
            }
            placed.push(at);
            // Cada uno mira hacia el centro del corrillo.
            let inward = (center - at).dot(scene.camera().axes().0) < 0.0;
            let pose = if rng.f() < 0.15 { Pose::Wave } else { Pose::Stand };
            self.person(scene, rng, at, pose, inward);
        }
        self.drawn - start
    }
    /// Baile en corros concéntricos de `rings` anillos a 0,8 m.
    pub fn dance(&mut self, scene: &mut Scene, rng: &mut Rng, center: V3, rings: usize) -> usize {
        let start = self.drawn;
        if !center.finite() {
            return 0;
        }
        for ring in 1..=rings.min(30) {
            let r = ring as f64 * 0.8;
            let count = (std::f64::consts::TAU * r / 0.6).floor() as usize;
            let phase = rng.f() * std::f64::consts::TAU;
            for k in 0..count {
                let angle = phase + std::f64::consts::TAU * k as f64 / count as f64;
                let at = center + v(angle.cos() * r, angle.sin() * r, 0.0);
                let pose = if rng.f() < 0.6 { Pose::Dance } else { Pose::Cheer };
                let mirror = rng.f() < 0.5;
                self.person(scene, rng, at, pose, mirror);
            }
        }
        self.drawn - start
    }
    /// Público de pie en un rectángulo (explanada, concierto), en rejilla
    /// desordenada. `density` en personas por metro cuadrado.
    pub fn field(&mut self, scene: &mut Scene, rng: &mut Rng, min: V3, max: V3, density: f64) -> usize {
        let start = self.drawn;
        if !min.finite() || !max.finite() || max.x <= min.x || max.y <= min.y || !density.is_finite() || density <= 0.0 {
            return 0;
        }
        let pitch = 1.0 / density.clamp(0.05, 3.0).sqrt();
        let mut y = min.y + pitch * 0.5;
        while y < max.y {
            let mut x = min.x + pitch * 0.5;
            while x < max.x {
                let at = v(x + (rng.f() - 0.5) * pitch * 0.7, y + (rng.f() - 0.5) * pitch * 0.7, min.z);
                let pose = match (rng.f() * 10.0) as usize {
                    0..=5 => Pose::Stand,
                    6 | 7 => Pose::Cheer,
                    8 => Pose::Wave,
                    _ => Pose::Dance,
                };
                let mirror = rng.f() < 0.5;
                self.person(scene, rng, at, pose, mirror);
                x += pitch;
            }
            y += pitch;
        }
        self.drawn - start
    }
}
