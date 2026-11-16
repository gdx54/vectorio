//! Cúpulas de vidrio con un mundo dentro.
//!
//! La cúpula no tapa su interior: el vidrio son trazos (silueta, nervios y un
//! brillo) y solo su mitad trasera oculta, para que lo que queda detrás de la
//! cúpula no se cuele por el cristal. El interior lo dibuja quien llama, con las
//! piezas de siempre, y después se recorta a la esfera.
use super::*;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// Cúpula semiesférica de radio `radius` apoyada en un zócalo de `plinth` metros.
/// `center` es el centro del suelo interior (lo alto del zócalo).
#[derive(Clone, Copy, Debug)]
pub struct Dome { pub center: V3, pub radius: f64, pub plinth: f64, pub ribs: usize }

/// Suelo útil que recibe el mundo interior: un disco algo menor que la cúpula.
#[derive(Clone, Copy, Debug)]
pub struct Floor { pub center: V3, pub radius: f64, pub dome: f64 }

impl Floor {
    /// Altura libre sobre un punto del suelo a `distance` metros del centro.
    pub fn headroom(&self, distance: f64) -> f64 {
        (self.dome * self.dome - distance * distance).max(0.0).sqrt()
    }
}

impl Dome {
    fn valid(&self) -> bool {
        self.center.finite() && self.radius.is_finite() && self.radius > 0.5 && self.radius < 2_000.0 &&
            self.plinth.is_finite() && self.plinth >= 0.0
    }
}

/// Dibuja la cúpula y llama a `interior` con su suelo; lo que el interior deje
/// fuera de la esfera se descarta.
pub fn dome(scene: &mut Scene, dome: Dome, interior: impl FnOnce(&mut Scene, Floor)) {
    if !dome.valid() {
        return;
    }
    let r = dome.radius;
    let c = dome.center;
    if dome.plinth > 0.0 {
        let rim = r * 1.04 + super::street::apart(scene);
        // El zócalo va sin trama: su tapa es el suelo del mundo interior.
        part(scene, "contorno");
        scene.lathe(c - v(0.0, 0.0, dome.plinth), &[(rim, 0.0), (rim, dome.plinth)], true);
    }
    let mark = scene.mark();
    interior(scene, Floor { center: c, radius: r * 0.9, dome: r * 0.97 });
    scene.clip_to_sphere(mark, c, r * 0.97);
    back_shell(scene, c, r * 0.98);
    glass(scene, dome);
}

/// Mitad trasera de la cúpula como caras que ocultan sin dibujar nada.
fn back_shell(scene: &mut Scene, c: V3, r: f64) {
    let (_, _, away) = scene.camera().axes();
    let meridians = 24;
    let parallels = 8;
    let point = |m: usize, p: usize| {
        let theta = TAU * m as f64 / meridians as f64;
        let phi = FRAC_PI_2 * p as f64 / parallels as f64;
        c + v(theta.cos() * phi.cos() * r, theta.sin() * phi.cos() * r, phi.sin() * r)
    };
    for m in 0..meridians {
        let n = (m + 1) % meridians;
        for p in 0..parallels {
            let face = if p + 1 == parallels {
                vec![point(m, p), point(n, p), c + v(0.0, 0.0, r)]
            } else {
                vec![point(m, p), point(n, p), point(n, p + 1), point(m, p + 1)]
            };
            let mean = face.iter().fold(V3::default(), |s, q| s + *q) * (1.0 / face.len() as f64);
            if (mean - c).dot(away) > 0.0 {
                scene.occluder(&face);
            }
        }
    }
}

/// Vidrio: silueta, anillo de base, nervios y paralelos de la mitad delantera y
/// un brillo.
fn glass(scene: &mut Scene, dome: Dome) {
    let r = dome.radius;
    let c = dome.center;
    let (right, down, away) = scene.camera().axes();
    let samples = scene.circle_segments(r).clamp(24, 256);
    part(scene, "contorno");
    // La silueta es el círculo perpendicular a la mirada; solo la parte sobre el suelo.
    let rim: Vec<V3> = (0..=samples)
        .map(|k| {
            let angle = TAU * k as f64 / samples as f64;
            c + right * (angle.cos() * r) + down * (angle.sin() * r)
        })
        .collect();
    for piece in runs(&rim, |q| q.z >= c.z - 1e-9) {
        scene.line(&piece);
    }
    let base: Vec<V3> = (0..=samples)
        .map(|k| {
            let angle = TAU * k as f64 / samples as f64;
            c + v(angle.cos(), angle.sin(), 0.0) * (r * 0.975)
        })
        .collect();
    scene.line(&base);
    if !scene.visible(r, 6.0) {
        return;
    }
    part(scene, "agua");
    let front = |q: V3| (q - c).dot(away) < 0.0;
    let ribs = dome.ribs.clamp(4, 32);
    let steps = (samples / 4).max(8);
    for m in 0..ribs {
        let theta = TAU * (m as f64 + 0.5) / ribs as f64;
        let meridian: Vec<V3> = (0..=steps)
            .map(|k| {
                let phi = FRAC_PI_2 * k as f64 / steps as f64;
                c + v(theta.cos() * phi.cos() * r, theta.sin() * phi.cos() * r, phi.sin() * r)
            })
            .collect();
        for piece in runs(&meridian, front) {
            scene.line(&piece);
        }
    }
    for phi in [PI / 6.0, PI / 3.0] {
        let ring: Vec<V3> = (0..=samples)
            .map(|k| {
                let theta = TAU * k as f64 / samples as f64;
                c + v(theta.cos() * phi.cos() * r, theta.sin() * phi.cos() * r, phi.sin() * r)
            })
            .collect();
        for piece in runs(&ring, front) {
            scene.line(&piece);
        }
    }
    // Brillo: un arco corto dentro de la silueta, arriba a la izquierda.
    let glare: Vec<V3> = (0..=12)
        .map(|k| {
            let angle = PI * (1.12 + 0.22 * k as f64 / 12.0);
            c + right * (angle.cos() * r * 0.82) + down * (angle.sin() * r * 0.82) - away * r
        })
        .collect();
    scene.whole_line(&glare);
}

/// Tramos consecutivos de puntos que cumplen `keep`.
fn runs(points: &[V3], keep: impl Fn(V3) -> bool) -> Vec<Vec<V3>> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    for &q in points {
        if keep(q) {
            current.push(q);
        } else if current.len() >= 2 {
            out.push(std::mem::take(&mut current));
        } else {
            current.clear();
        }
    }
    if current.len() >= 2 {
        out.push(current);
    }
    out
}
