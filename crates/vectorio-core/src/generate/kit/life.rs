//! Vida suelta que comparten las escenas: bandadas de palomas, vendedores de
//! globos y figuras que dejan su sombra en el suelo.
use super::*;
use super::crowd::{figure, Pose};

/// Bandada de pájaros como uves, con las alas arriba o abajo, alrededor de
/// `center` en un radio de seis metros.
pub fn flock(scene: &mut Scene, rng: &mut Rng, center: V3, count: usize) {
    if !center.finite() || !scene.visible(0.45, 1.2) {
        return;
    }
    let right = scene.camera().axes().0;
    part(scene, "detalle");
    for _ in 0..count {
        let angle = rng.f() * std::f64::consts::TAU;
        let r = 6.0 * rng.f().sqrt();
        let c = center + v(angle.cos() * r, angle.sin() * r, (rng.f() - 0.5) * 4.0);
        let span = 0.22 + rng.f() * 0.1;
        let lift = if rng.f() < 0.7 { span * 0.6 } else { -span * 0.4 };
        scene.line(&[c - right * span + v(0.0, 0.0, lift), c, c + right * span + v(0.0, 0.0, lift)]);
    }
}

/// Vendedor de globos: una figura con un ramillete atado a la mano. Los globos
/// van en la parte «acento».
pub fn balloon_seller(scene: &mut Scene, rng: &mut Rng, foot: V3, count: usize, height: f64) -> bool {
    let mirror = rng.f() < 0.5;
    if !figure(scene, foot, height, Pose::Stand, mirror) || !scene.visible(0.25, 0.6) {
        return false;
    }
    let right = scene.camera().axes().0;
    let side = if mirror { -1.0 } else { 1.0 };
    let hand = foot + right * (0.22 * side * height / 1.72) + v(0.0, 0.0, 0.62 * height);
    balloons(scene, rng, hand, count, true);
    true
}

/// Racimo atado a un soporte: comparte el dibujo del vendedor, sin oclusores.
pub fn balloon_bunch(scene: &mut Scene, rng: &mut Rng, hand: V3, count: usize) {
    if !hand.finite() || !scene.visible(0.25, 0.6) {
        return;
    }
    balloons(scene, rng, hand, count.min(12), false);
}

fn balloons(scene: &mut Scene, rng: &mut Rng, hand: V3, count: usize, cover: bool) {
    let (right, down, _) = scene.camera().axes();
    let up = down * -1.0;
    for _ in 0..count {
        let center = hand + right * ((rng.f() - 0.5) * 0.9) + v(0.0, 0.0, 1.3 + rng.f() * 0.9);
        let ring: Vec<V3> = (0..10).map(|k| {
            let angle = std::f64::consts::TAU * k as f64 / 10.0;
            center + right * (angle.cos() * 0.24) + up * (angle.sin() * 0.29)
        }).collect();
        part(scene, "acento");
        if cover {
            scene.occluder(&ring);
        }
        scene.closed(&ring);
        part(scene, "detalle");
        scene.line(&[hand, center - up * 0.29]);
    }
}

/// Figura que además deja sombra, como la dejaría un cuerpo de su talla: un
/// prisma estrecho que no se dibuja. Solo cuenta si hay un grupo de sombras abierto.
pub fn figure_with_shadow(scene: &mut Scene, origin: V3, height: f64, pose: Pose, mirror: bool) -> bool {
    if !figure(scene, origin, height, pose, mirror) {
        return false;
    }
    let r = 0.2 * height / 1.72;
    let mut body = Vec::with_capacity(8);
    for z in [0.0, height] {
        for (x, y) in [(-r, -r), (r, -r), (r, r), (-r, r)] {
            body.push(origin + v(x, y, z));
        }
    }
    scene.cast_shadow(&body);
    true
}
