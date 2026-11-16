//! Grandes formas que cruzan la escena: criaturas de cuerpo tubular sobre un eje
//! curvo (cetáceo, cefalópodo, saurio), cohete y estación.
//!
//! Todas se apoyan en `tube`: anillos perpendiculares al eje, unidos en
//! triángulos (los cuadriláteros entre anillos girados no son planos) y con un
//! solo grupo de suavizado, así que solo se dibuja la silueta.
use super::*;
use std::f64::consts::TAU;

/// Tubo cerrado a lo largo de `spine`, con un radio por punto. Un radio nulo solo
/// puede ir en un extremo (punta); los extremos con radio llevan tapa plana.
/// `closed` une el último anillo con el primero (anillo de una estación).
/// Devuelve las caras exteriores, o nada si el tubo no es válido.
pub fn tube(scene: &mut Scene, style: Style, spine: &[V3], radii: &[f64], closed: bool) -> Vec<Vec<V3>> {
    let n = spine.len();
    let valid_input = n >= 2 && radii.len() == n && spine.iter().all(|q| q.finite()) &&
        radii.iter().all(|r| r.is_finite() && *r >= 0.0 && *r < 1_000.0);
    if !valid_input || (closed && (n < 3 || radii.iter().any(|r| *r <= 0.0))) {
        return Vec::new();
    }
    if radii[1..n - 1].iter().any(|r| *r <= 0.0) {
        return Vec::new();
    }
    let largest = radii.iter().copied().fold(0.0, f64::max);
    if largest <= 0.0 || !scene.visible(largest, 0.3) {
        return Vec::new();
    }
    let sides = ((scene.paper_mm(largest) * 1.6) as usize).clamp(6, 18);
    let frames = transport(spine, closed);
    let mut vertices: Vec<V3> = Vec::new();
    let mut rings: Vec<Vec<usize>> = Vec::new();
    for i in 0..n {
        let (t, n1, n2) = frames[i];
        let mut ring = Vec::new();
        if radii[i] <= 0.0 {
            // La punta se adelanta un poco para que la tapa no quede plana.
            let along = if i == 0 { -1.0 } else { 1.0 };
            let reach = radii[if i == 0 { 1 } else { n - 2 }] * 0.15;
            ring.push(vertices.len());
            vertices.push(spine[i] + t * (along * reach));
        } else {
            for k in 0..sides {
                let angle = TAU * k as f64 / sides as f64;
                ring.push(vertices.len());
                vertices.push(spine[i] + n1 * (angle.cos() * radii[i]) + n2 * (angle.sin() * radii[i]));
            }
        }
        rings.push(ring);
    }
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut groups: Vec<u16> = Vec::new();
    let pairs = if closed { n } else { n - 1 };
    for i in 0..pairs {
        let a = &rings[i];
        let b = &rings[(i + 1) % n];
        for k in 0..sides {
            let j = (k + 1) % sides;
            if a.len() == 1 {
                faces.push(vec![a[0], b[j], b[k]]);
                groups.push(1);
            } else if b.len() == 1 {
                faces.push(vec![a[k], a[j], b[0]]);
                groups.push(1);
            } else {
                faces.push(vec![a[k], a[j], b[j]]);
                faces.push(vec![a[k], b[j], b[k]]);
                groups.push(1);
                groups.push(1);
            }
        }
    }
    if !closed {
        if rings[0].len() > 1 {
            faces.push(rings[0].iter().rev().copied().collect());
            groups.push(0);
        }
        if rings[n - 1].len() > 1 {
            faces.push(rings[n - 1].clone());
            groups.push(0);
        }
    }
    let built = scene.solid_oriented(&vertices, &faces, &groups, false);
    if !built.is_empty() {
        shade_curved(scene, style, &built);
    }
    built
}

/// Marcos por transporte paralelo: tangente y dos normales que giran lo mínimo.
fn transport(spine: &[V3], closed: bool) -> Vec<(V3, V3, V3)> {
    let n = spine.len();
    let tangent = |i: usize| {
        let (a, b) = if closed {
            (spine[(i + n - 1) % n], spine[(i + 1) % n])
        } else {
            (spine[i.saturating_sub(1)], spine[(i + 1).min(n - 1)])
        };
        (b - a).normalized()
    };
    let t0 = tangent(0);
    let seed = if t0.z.abs() < 0.9 { v(0.0, 0.0, 1.0) } else { v(1.0, 0.0, 0.0) };
    let mut n1 = (seed - t0 * seed.dot(t0)).normalized();
    let mut frames = Vec::with_capacity(n);
    for i in 0..n {
        let t = tangent(i);
        let projected = (n1 - t * n1.dot(t)).normalized();
        if projected.length() > 0.5 {
            n1 = projected;
        }
        frames.push((t, n1, t.cross(n1)));
    }
    frames
}

/// Trama de las caras en sombra, de un solo paso sobre toda la superficie.
fn shade_curved(scene: &mut Scene, style: Style, faces: &[Vec<V3>]) {
    if style.detail == Detail::Preview {
        return;
    }
    let dark: Vec<Vec<V3>> = faces
        .iter()
        .filter(|f| scene.camera().facing(super::super::style::normal(f)))
        .filter(|f| scene.tone(super::super::style::normal(f)) == crate::scene::Tone::Dark)
        .cloned()
        .collect();
    if dark.is_empty() {
        return;
    }
    part(scene, "trama");
    scene.hatch_surface(&dark, 0.0, style.spacing_mm * 1.2);
}

/// Aleta plana: triángulo con un canto de al menos un milímetro en la hoja.
fn fin(scene: &mut Scene, a: V3, b: V3, c: V3) {
    let n = (b - a).cross(c - a).normalized();
    if n.length() < 0.5 {
        return;
    }
    let thick = (0.02 * (b - a).length()).max(1.0 / scene.camera().scale);
    let shift = n * (thick * 0.5);
    part(scene, "contorno");
    scene.prism(&[a - shift, b - shift, c - shift], n * thick);
}

/// Especie de criatura.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Species { Cetacean, Cephalopod, Saurian }

pub const SPECIES: [Species; 3] = [Species::Cetacean, Species::Cephalopod, Species::Saurian];

/// Criatura de longitud `length` con la cabeza en `head`, avanzando según
/// `heading` (horizontal o no). `bend` curva el cuerpo en el plano vertical.
#[allow(clippy::too_many_arguments)]
pub fn creature(scene: &mut Scene, style: Style, species: Species, head: V3, heading: V3, length: f64, bend: f64, rng: &mut Rng) {
    let ahead = heading.normalized();
    if !head.finite() || ahead.length() < 0.5 || !length.is_finite() || length <= 1.0 || !bend.is_finite() {
        return;
    }
    let side = if ahead.z.abs() < 0.95 { ahead.cross(v(0.0, 0.0, 1.0)).normalized() } else { v(1.0, 0.0, 0.0) };
    let up = side.cross(ahead);
    let body = Body { head, back: ahead * -1.0, side, up, length, bend };
    part(scene, "contorno");
    match species {
        Species::Cetacean => cetacean(scene, style, body),
        Species::Cephalopod => cephalopod(scene, style, body, rng),
        Species::Saurian => saurian(scene, style, body),
    }
}

/// Ejes de una criatura: `back` va de la cabeza a la cola.
#[derive(Clone, Copy)]
struct Body { head: V3, back: V3, side: V3, up: V3, length: f64, bend: f64 }

impl Body {
    /// Punto del eje a la fracción `s` (0 cabeza, 1 cola), con la curva del cuerpo.
    fn axis(&self, s: f64) -> V3 {
        let wave = (s * std::f64::consts::PI).sin() * self.bend * self.length * 0.12;
        self.head + self.back * (s * self.length) + self.up * wave
    }
    fn local(&self, s: f64, across: f64, lift: f64) -> V3 {
        self.axis(s) + self.side * (across * self.length) + self.up * (lift * self.length)
    }
}

fn cetacean(scene: &mut Scene, style: Style, body: Body) {
    let count = 28;
    let profile = |s: f64| {
        let swell = (0.35 + 0.65 * (s / 0.18).min(1.0)).powf(0.6);
        let taper = 1.0 - ((s - 0.35).max(0.0) / 0.65).powf(1.3) * 0.85;
        0.11 * swell * taper
    };
    let spine: Vec<V3> = (0..=count).map(|k| body.axis(k as f64 / count as f64)).collect();
    let mut radii: Vec<f64> = (0..=count).map(|k| profile(k as f64 / count as f64) * body.length).collect();
    radii[count] = 0.0;
    let skin = tube(scene, style, &spine, &radii, false);
    eye(scene, &skin, body, 0.12, profile(0.12));
    part(scene, "detalle");
    let mouth: Vec<V3> = (0..=12).map(|k| {
        let s = 0.01 + k as f64 / 12.0 * 0.22;
        body.axis(s) - body.up * (profile(s) * body.length * 0.25)
    }).collect();
    skin_line(scene, &skin, &mouth, false);
    // Cola horizontal en dos lóbulos, aletas pectorales y aleta dorsal.
    let tail = body.axis(0.98);
    for sign in [1.0, -1.0] {
        fin(scene, body.axis(0.93), tail + body.side * (sign * 0.12 * body.length) + body.back * (0.06 * body.length), tail);
        fin(scene, body.local(0.28, 0.06 * sign, -0.04), body.local(0.42, 0.17 * sign, -0.1), body.local(0.36, 0.05 * sign, -0.05));
    }
    fin(scene, body.local(0.5, 0.0, 0.07), body.local(0.62, 0.0, 0.13), body.local(0.64, 0.0, 0.05));
}

fn cephalopod(scene: &mut Scene, style: Style, body: Body, rng: &mut Rng) {
    // El manto va delante (hacia `heading`) y los brazos se arrastran detrás.
    let count = 18;
    let mantle_len = 0.45;
    let spine: Vec<V3> = (0..=count).map(|k| body.axis(mantle_len * k as f64 / count as f64)).collect();
    let mut radii: Vec<f64> = (0..=count)
        .map(|k| {
            let s = k as f64 / count as f64;
            0.09 * body.length * (s.powf(0.7) * (1.0 - 0.25 * s))
        })
        .collect();
    radii[0] = 0.0;
    let skin = tube(scene, style, &spine, &radii, false);
    eye(scene, &skin, body, 0.37, radii[15] / body.length);
    // Aletas en la punta del manto.
    for sign in [1.0, -1.0] {
        fin(scene, body.local(0.02, 0.0, 0.0), body.local(0.1, 0.1 * sign, 0.0), body.local(0.16, 0.02 * sign, 0.0));
    }
    let root = body.axis(mantle_len);
    let base_radius = radii[count] * 0.7;
    let arms = 8;
    for a in 0..arms {
        let angle = TAU * a as f64 / arms as f64;
        let spread = body.side * angle.cos() + body.up * angle.sin();
        let long = a % 4 == 1;
        let reach = if long { 0.75 } else { 0.5 } * body.length;
        let phase = rng.f() * TAU;
        let steps = 16;
        let arm: Vec<V3> = (0..=steps)
            .map(|k| {
                let s = k as f64 / steps as f64;
                let curl = (s * 5.0 + phase).sin() * 0.08 * reach * s;
                root + spread * (base_radius * (1.0 - s) + reach * 0.18 * s) + body.back * (reach * s) + (spread.cross(body.back)) * curl
            })
            .collect();
        let thick = base_radius * 0.32;
        let mut sizes: Vec<f64> = (0..=steps).map(|k| thick * (1.0 - k as f64 / steps as f64).powf(0.8)).collect();
        sizes[steps] = 0.0;
        tube(scene, style, &arm, &sizes, false);
    }
}

fn eye(scene: &mut Scene, skin: &[Vec<V3>], body: Body, along: f64, radius: f64) {
    let center = body.axis(along) + body.up * (body.length * radius * 0.25);
    let (right, down, _) = scene.camera().axes();
    let size = body.length * 0.004;
    part(scene, "detalle");
    let rim: Vec<V3> = (0..12).map(|k| {
        let angle = TAU * k as f64 / 12.0;
        center + (right * angle.cos() + down * angle.sin()) * size
    }).collect();
    skin_line(scene, skin, &rim, true);
    skin_line(scene, skin, &[center - down * size * 0.35, center + down * size * 0.35], false);
}

/// Proyecta los rasgos sobre la primera cara visible del cuerpo, incluso al
/// curvarse el eje. Evita ojos y bocas cortados por la propia piel del tubo.
fn skin_line(scene: &mut Scene, skin: &[Vec<V3>], points: &[V3], closed: bool) {
    let away = scene.camera().axes().2;
    let mut curve = Vec::new();
    for &point in points {
        let mut nearest: Option<(f64, V3)> = None;
        for face in skin {
            for i in 1..face.len().saturating_sub(1) {
                let (a, b, c) = (face[0], face[i], face[i + 1]);
                let normal = (b - a).cross(c - a);
                let divisor = away.dot(normal);
                if divisor.abs() < 1e-12 { continue; }
                let hit = point + away * ((a - point).dot(normal) / divisor);
                if [(a, b), (b, c), (c, a)].iter().any(|(u, w)| (*w - *u).cross(hit - *u).dot(normal) < -1e-9) { continue; }
                let depth = hit.dot(away);
                if nearest.is_none_or(|(old, _)| depth < old) { nearest = Some((depth, hit)); }
            }
        }
        let Some((_, hit)) = nearest else { return; };
        curve.push(hit - away * (0.03 / scene.camera().scale));
    }
    if closed { scene.closed(&curve); } else { scene.line(&curve); }
}

fn saurian(scene: &mut Scene, style: Style, body: Body) {
    // Cuello largo, tronco ovalado y cola corta, con cuatro aletas.
    let count = 34;
    let profile = |s: f64| {
        if s < 0.06 {
            0.025 + 0.01 * (s / 0.06)
        } else if s < 0.38 {
            0.022 + 0.008 * ((s - 0.06) / 0.32)
        } else if s < 0.75 {
            let t = (s - 0.38) / 0.37;
            0.03 + 0.07 * (t * std::f64::consts::PI).sin()
        } else {
            0.03 * (1.0 - (s - 0.75) / 0.25)
        }
    };
    let spine: Vec<V3> = (0..=count)
        .map(|k| {
            let s = k as f64 / count as f64;
            let neck = (1.0 - s / 0.38).max(0.0).powi(2) * 0.12;
            body.axis(s) + body.up * (neck * body.length)
        })
        .collect();
    let mut radii: Vec<f64> = (0..=count).map(|k| profile(k as f64 / count as f64) * body.length).collect();
    radii[count] = 0.0;
    tube(scene, style, &spine, &radii, false);
    for (s, size) in [(0.45, 0.16), (0.66, 0.13)] {
        for sign in [1.0, -1.0] {
            fin(scene, body.local(s - 0.03, 0.04 * sign, -0.02), body.local(s + 0.06, size * sign, -0.07), body.local(s + 0.05, 0.04 * sign, -0.03));
        }
    }
}

/// Cohete de `length` metros con la tobera en `base` y la punta según `axis`.
pub fn rocket(scene: &mut Scene, style: Style, base: V3, axis: V3, length: f64) {
    let up = axis.normalized();
    if !base.finite() || up.length() < 0.5 || !length.is_finite() || length <= 1.0 {
        return;
    }
    let r = length * 0.08;
    let stations = [(0.0, r * 0.7), (0.05, r * 0.85), (0.08, r), (0.7, r), (0.82, r * 0.82), (0.92, r * 0.45), (1.0, 0.0)];
    let spine: Vec<V3> = stations.iter().map(|(s, _)| base + up * (s * length)).collect();
    let radii: Vec<f64> = stations.iter().map(|(_, radius)| *radius).collect();
    part(scene, "contorno");
    let skin = tube(scene, style, &spine, &radii, false);
    let (right, down, _) = scene.camera().axes();
    part(scene, "agua");
    for s in [0.35, 0.46, 0.57] {
        let center = base + up * (s * length);
        let window: Vec<V3> = (0..16).map(|k| {
            let angle = TAU * k as f64 / 16.0;
            center + (right * angle.cos() + down * angle.sin()) * (r * 0.28)
        }).collect();
        skin_line(scene, &skin, &window, true);
    }
    let side = if up.z.abs() < 0.95 { up.cross(v(0.0, 0.0, 1.0)).normalized() } else { v(1.0, 0.0, 0.0) };
    let other = up.cross(side);
    for k in 0..4 {
        let angle = TAU * k as f64 / 4.0 + 0.4;
        let out = side * angle.cos() + other * angle.sin();
        fin(scene, base + up * (0.28 * length) + out * (r * 0.9), base + out * (r * 2.3) - up * (0.04 * length), base + up * (0.05 * length) + out * (r * 0.9));
    }
    if !scene.visible(r, 3.0) {
        return;
    }
    part(scene, "detalle");
    for s in [0.62, 0.74] {
        let ring: Vec<V3> = (0..=24)
            .map(|k| {
                let angle = TAU * k as f64 / 24.0;
                let radius = if s > 0.7 { r * 0.97 } else { r };
                base + up * (s * length) + side * (angle.cos() * radius * 1.001) + other * (angle.sin() * radius * 1.001)
            })
            .collect();
        scene.line(&ring);
    }
}

/// Estación: anillo habitado alrededor de un eje con su núcleo y cuatro radios.
pub fn station(scene: &mut Scene, style: Style, center: V3, axis: V3, radius: f64) {
    let up = axis.normalized();
    if !center.finite() || up.length() < 0.5 || !radius.is_finite() || radius <= 2.0 {
        return;
    }
    let side = if up.z.abs() < 0.95 { up.cross(v(0.0, 0.0, 1.0)).normalized() } else { v(1.0, 0.0, 0.0) };
    let other = up.cross(side);
    let count = 36;
    let ring: Vec<V3> = (0..count)
        .map(|k| {
            let angle = TAU * k as f64 / count as f64;
            center + side * (angle.cos() * radius) + other * (angle.sin() * radius)
        })
        .collect();
    let thick = radius * 0.09;
    part(scene, "contorno");
    tube(scene, style, &ring, &vec![thick; count], true);
    let hub = radius * 0.16;
    let core = [center - up * (radius * 0.35), center - up * (radius * 0.3), center + up * (radius * 0.3), center + up * (radius * 0.35)];
    tube(scene, style, &core, &[hub * 0.6, hub, hub, hub * 0.6], false);
    for k in 0..4 {
        let angle = TAU * k as f64 / 4.0 + 0.2;
        let out = side * angle.cos() + other * angle.sin();
        let spoke = [center + out * (hub * 0.9), center + out * (radius - thick * 0.9)];
        tube(scene, style, &spoke, &[thick * 0.3, thick * 0.3], false);
    }
}
