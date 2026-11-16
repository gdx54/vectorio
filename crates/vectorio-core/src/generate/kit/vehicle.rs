//! Vehículos: coche, moto, autobús, barca y tren. Cada uno es un perfil lateral
//! extruido en su anchura (un solo sólido), con ruedas y ventanillas dibujadas en
//! los costados. Se orientan con un rumbo horizontal cualquiera.
use super::*;
use super::crowd::{figure, Pose};
use super::ribbon::Band;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vehicle { Car, Moto, Bus, Boat, Train }

impl Vehicle {
    /// Longitud en metros, para separarlos en una fila.
    pub fn length(self) -> f64 {
        match self {
            Vehicle::Car => 4.2,
            Vehicle::Moto => 1.9,
            Vehicle::Bus => 11.0,
            Vehicle::Boat => 6.0,
            Vehicle::Train => 18.0,
        }
    }
}

/// Marco local: x avanza con el rumbo, y es lateral (izquierda), z sube.
#[derive(Clone, Copy)]
struct Frame { origin: V3, ahead: V3, left: V3 }

impl Frame {
    fn new(origin: V3, heading: V3) -> Option<Self> {
        let ahead = v(heading.x, heading.y, 0.0).normalized();
        if !origin.finite() || ahead.length() < 0.5 {
            return None;
        }
        Some(Self { origin, ahead, left: v(-ahead.y, ahead.x, 0.0) })
    }
    fn at(&self, x: f64, y: f64, z: f64) -> V3 {
        self.origin + self.ahead * x + self.left * y + v(0.0, 0.0, z)
    }
}

/// Cuerpo de perfil lateral `profile` (x, z) y anchura `width`, centrado en el
/// eje. Las ventanillas se trazan en los dos costados; la del costado lejano la
/// oculta el propio cuerpo.
fn body(scene: &mut Scene, frame: Frame, profile: &[(f64, f64)], width: f64, windows: &[Vec<(f64, f64)>]) {
    let half = width * 0.5;
    let outline: Vec<V3> = profile.iter().map(|q| frame.at(q.0, -half, q.1)).collect();
    part(scene, "contorno");
    if scene.prism(&outline, frame.left * width).is_empty() {
        return;
    }
    if !scene.visible(0.4, 0.5) {
        return;
    }
    part(scene, "detalle");
    for window in windows {
        for y in [-half, half] {
            let pane: Vec<V3> = window.iter().map(|q| frame.at(q.0, y, q.1)).collect();
            scene.closed(&pane);
        }
    }
}

/// Rueda en los dos costados, algo por fuera del cuerpo: un disco que oculta y su aro.
fn wheels(scene: &mut Scene, frame: Frame, xs: &[f64], radius: f64, half: f64) {
    if !scene.visible(radius, 0.4) {
        return;
    }
    let sides = if scene.paper_mm(radius) < 1.0 { 6 } else { 10 };
    part(scene, "contorno");
    let out = super::street::apart(scene).min(0.05);
    for &x in xs {
        for y in [-(half + out), half + out] {
            let rim: Vec<V3> = (0..sides)
                .map(|k| {
                    let angle = std::f64::consts::TAU * k as f64 / sides as f64;
                    frame.at(x + radius * angle.cos(), y, radius + radius * angle.sin())
                })
                .collect();
            scene.occluder(&rim);
            scene.closed(&rim);
        }
    }
}

/// Fila de ventanillas rectangulares de `x0` a `x1`, entre `z0` y `z1`.
fn window_row(x0: f64, x1: f64, z0: f64, z1: f64, pitch: f64) -> Vec<Vec<(f64, f64)>> {
    let count = ((x1 - x0) / pitch).floor().max(1.0) as usize;
    let pitch = (x1 - x0) / count as f64;
    (0..count)
        .map(|k| {
            let a = x0 + pitch * k as f64 + pitch * 0.12;
            let b = x0 + pitch * (k + 1) as f64 - pitch * 0.12;
            vec![(a, z0), (b, z0), (b, z1), (a, z1)]
        })
        .collect()
}

/// Dibuja un vehículo con su centro trasero en `origin` y rumbo `heading`.
/// El origen de la barca está en la línea de agua.
pub fn vehicle(scene: &mut Scene, kind: Vehicle, origin: V3, heading: V3) {
    let Some(frame) = Frame::new(origin, heading) else { return };
    if !scene.visible(kind.length(), 2.0) {
        return;
    }
    match kind {
        Vehicle::Car => car(scene, frame),
        Vehicle::Moto => moto(scene, frame),
        Vehicle::Bus => bus(scene, frame),
        Vehicle::Boat => boat(scene, frame),
        Vehicle::Train => carriage(scene, frame, false),
    }
}

fn car(scene: &mut Scene, frame: Frame) {
    let l = Vehicle::Car.length();
    let profile = [
        (0.0, 0.28), (l, 0.28), (l, 0.72), (0.8 * l, 0.8),
        (0.66 * l, 1.38), (0.3 * l, 1.38), (0.16 * l, 0.82), (0.0, 0.78),
    ];
    let windows = vec![vec![(0.2 * l, 0.86), (0.76 * l, 0.86), (0.645 * l, 1.3), (0.315 * l, 1.3)]];
    body(scene, frame, &profile, 1.8, &windows);
    wheels(scene, frame, &[0.2 * l, 0.8 * l], 0.32, 0.9);
}

fn bus(scene: &mut Scene, frame: Frame) {
    let l = Vehicle::Bus.length();
    let profile = [(0.0, 0.35), (l, 0.35), (l, 2.9), (l - 0.3, 3.15), (0.2, 3.15), (0.0, 2.95)];
    let windows = window_row(0.8, l - 1.4, 1.55, 2.65, 1.3);
    body(scene, frame, &profile, 2.5, &windows);
    wheels(scene, frame, &[2.3, l - 2.0], 0.5, 1.25);
}

fn carriage(scene: &mut Scene, frame: Frame, engine: bool) {
    let l = Vehicle::Train.length();
    let profile: Vec<(f64, f64)> = if engine {
        vec![(0.0, 0.6), (l, 0.6), (l, 1.8), (l - 1.6, 3.9), (0.6, 3.9), (0.0, 3.4)]
    } else {
        vec![(0.0, 0.6), (l, 0.6), (l, 3.4), (l - 0.6, 3.9), (0.6, 3.9), (0.0, 3.4)]
    };
    let end = if engine { l - 3.2 } else { l - 1.4 };
    let windows = window_row(1.4, end, 2.0, 3.0, 1.6);
    body(scene, frame, &profile, 2.9, &windows);
    wheels(scene, frame, &[2.4, 3.6, l - 3.6, l - 2.4], 0.42, 1.45);
}

fn moto(scene: &mut Scene, frame: Frame) {
    let r = 0.3;
    let (back, front) = (0.3, 1.6);
    let sides = 8;
    part(scene, "contorno");
    for x in [back, front] {
        let rim: Vec<V3> = (0..sides)
            .map(|k| {
                let angle = std::f64::consts::TAU * k as f64 / sides as f64;
                frame.at(x + r * angle.cos(), 0.0, r + r * angle.sin())
            })
            .collect();
        scene.occluder(&rim);
        scene.closed(&rim);
    }
    let frame_line = [
        frame.at(back, 0.0, r), frame.at(0.75, 0.0, 0.78), frame.at(1.25, 0.0, 0.78),
        frame.at(front, 0.0, r), frame.at(1.4, 0.0, 1.0), frame.at(1.3, 0.0, 1.05),
    ];
    scene.line(&frame_line);
    // El piloto va sentado; su cartel mira a la cámara como el resto de la gente.
    let mirror = frame.ahead.dot(scene.camera().axes().0) < 0.0;
    figure(scene, frame.at(0.85, 0.0, 0.32), 1.6, Pose::Sit, mirror);
}

/// Casco de cubierta en punta y fondo más estrecho (caras laterales planas), con
/// una caseta a popa.
fn boat(scene: &mut Scene, frame: Frame) {
    let l = Vehicle::Boat.length();
    let w = 2.2;
    let deck = [(0.0, -w * 0.5), (0.62 * l, -w * 0.5), (l, 0.0), (0.62 * l, w * 0.5), (0.0, w * 0.5)];
    let pivot = (0.45 * l, 0.0);
    let shrink = 0.72;
    let height = 0.9;
    let top: Vec<V3> = deck.iter().map(|q| frame.at(q.0, q.1, height)).collect();
    let bottom: Vec<V3> = deck
        .iter()
        .map(|q| frame.at(pivot.0 + (q.0 - pivot.0) * shrink, pivot.1 + (q.1 - pivot.1) * shrink, 0.0))
        .collect();
    let mut faces = vec![top.clone(), bottom.clone()];
    for i in 0..deck.len() {
        let j = (i + 1) % deck.len();
        faces.push(vec![top[i], top[j], bottom[j], bottom[i]]);
    }
    part(scene, "contorno");
    if scene.polyhedron(&faces).is_empty() {
        return;
    }
    let cabin = [(0.35, height), (2.2, height), (2.2, height + 1.1), (0.55, height + 1.1)];
    let gap = super::street::apart(scene).max(0.2);
    let half = w * 0.5 - gap;
    let outline: Vec<V3> = cabin.iter().map(|q| frame.at(q.0, -half, q.1)).collect();
    scene.prism(&outline, frame.left * (half * 2.0));
}

/// Fila de vehículos a lo largo de una banda, por el carril `offset`, con huecos
/// sorteados. Un tren va siempre enganchado: locomotora y vagones seguidos.
pub fn convoy(scene: &mut Scene, band: &Band, kinds: &[Vehicle], offset: f64, gap: f64, rng: &mut Rng) -> usize {
    if kinds.is_empty() || !offset.is_finite() || !gap.is_finite() || gap < 0.0 {
        return 0;
    }
    let mut count = 0;
    let mut s = rng.f() * gap;
    let total = band.length();
    let train = kinds.iter().all(|k| *k == Vehicle::Train);
    let couple = super::street::apart(scene).max(0.8);
    while s < total {
        let kind = kinds[(rng.f() * kinds.len() as f64) as usize % kinds.len()];
        let length = kind.length();
        if s + length > total {
            break;
        }
        let (rear, _) = band.at(s, offset);
        let (front, _) = band.at(s + length, offset);
        let heading = front - rear;
        if let Some(frame) = Frame::new(rear, heading) {
            if train {
                carriage(scene, frame, count == 0);
            } else {
                vehicle(scene, kind, rear, heading);
            }
            count += 1;
        }
        s += length + if train { couple } else { gap * (0.5 + rng.f()) };
    }
    count
}
