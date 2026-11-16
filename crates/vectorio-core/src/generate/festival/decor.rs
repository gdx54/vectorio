//! Adornos ligeros: banderas y banderolas son solo trazos, sin caras oclusoras.
use crate::{geom::Rng, scene::{Scene, V3}};
use super::super::{kit::{self, v, Crowd, Wall}, style::part};
use super::City;

pub(super) fn flag(scene: &mut Scene, base: V3, height: f64) {
    let right = scene.camera().axes().0;
    let top = base + v(0.0, 0.0, height);
    part(scene, "detalle");
    scene.line(&[base, top]);
    scene.closed(&[top, top + right * 1.6 - v(0.0, 0.0, 0.15), top + right * 1.3 - v(0.0, 0.0, 0.85), top - v(0.0, 0.0, 0.7)]);
}

pub(super) fn banner(scene: &mut Scene, pole: V3, height: f64) {
    let right = scene.camera().axes().0 * 0.7;
    let top = pole + v(0.0, 0.0, height - 0.5);
    part(scene, "detalle");
    scene.closed(&[top, top + right, top + right - v(0.0, 0.0, 1.8), top + right * 0.5 - v(0.0, 0.0, 1.5), top - v(0.0, 0.0, 1.8)]);
}

impl City<'_> {
    pub(super) fn roof_flag(&self, scene: &mut Scene, min: V3, max: V3) {
        flag(scene, v(min.x + 1.0, min.y + 1.0, max.z), 3.0);
    }

    /// Un pequeño aparcabicis bajo la fachada visible, fuera de la fila de peatones.
    pub(super) fn parked_bikes(&self, scene: &mut Scene, origin: V3) {
        if self.prm.f64("stories") < 0.3 || !self.room(scene, 0.8) {
            return;
        }
        let col = (origin.x / super::PITCH).round() as i32;
        let row = (origin.y / super::PITCH).round() as i32;
        if col.abs() > 1 || row.abs() > 1 || (col + row).rem_euclid(3) != 1 {
            return;
        }
        let x = if scene.camera().yaw_deg.to_radians().cos() < 0.0 { 1.8 } else { super::BLOCK - 1.8 };
        let foot = origin + v(x, 7.5, 0.0);
        if !self.on_paper(scene, foot, 4.0) {
            return;
        }
        for offset in [0.0, 2.4] {
            bicycle(scene, foot + v(0.0, offset, 0.0), v(0.0, 1.0, 0.0));
        }
    }

    pub(super) fn cafe_tables(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, wall: Wall) {
        for k in 0..2 {
            let foot = wall.at(wall.width * (k as f64 + 1.0) / 3.0, 0.0, 2.1);
            let a = wall.along * 0.4;
            let b = wall.outward() * 0.35;
            let top = foot + v(0.0, 0.0, 0.75);
            part(scene, "detalle");
            scene.closed(&[top - a - b, top + a - b, top + a + b, top - a + b]);
            scene.line(&[foot, top]);
            if self.people() > 0.0 {
                crowd.person(scene, rng, foot + a * 1.8, kit::Pose::Sit, k == 0);
            }
        }
    }
}

/// Bicicleta sin caras: ruedas, cuadro, sillín, manillar y soporte de aparcamiento.
fn bicycle(scene: &mut Scene, foot: V3, along: V3) {
    let at = |x, z| foot + along * x + v(0.0, 0.0, z);
    part(scene, "detalle");
    for x in [0.35, 1.55] {
        let wheel: Vec<V3> = (0..20).map(|k| {
            let angle = std::f64::consts::TAU * k as f64 / 20.0;
            at(x + 0.33 * angle.cos(), 0.33 + 0.33 * angle.sin())
        }).collect();
        scene.closed(&wheel);
    }
    scene.line(&[at(0.35, 0.33), at(0.9, 0.33), at(0.7, 0.88), at(0.35, 0.33), at(1.25, 0.88), at(0.9, 0.33)]);
    scene.line(&[at(1.55, 0.33), at(1.2, 1.08), at(1.5, 1.08)]);
    scene.line(&[at(0.55, 0.95), at(0.9, 0.95)]);
    scene.line(&[at(0.7, 0.88), at(0.7, 0.95)]);
    let across = v(-along.y, along.x, 0.0) * 0.22;
    let support = at(0.95, 0.0);
    scene.line(&[support - across, support - across + v(0.0, 0.0, 0.8), support + across + v(0.0, 0.0, 0.8), support + across]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{p, BBox, scene::Camera};

    #[test]
    fn parked_bicycles_stand_on_the_ground_without_occluders() {
        let paper = BBox { min: p(0.0, 0.0), max: p(210.0, 297.0) };
        let mut scene = Scene::new(Camera::default(), paper);
        bicycle(&mut scene, v(0.0, 0.0, 0.0), v(0.0, 1.0, 0.0));
        assert_eq!(scene.load().faces, 0);
        let output = scene.render();
        let lines: Vec<_> = output.parts.iter().flat_map(|part| &part.lines).collect();
        assert!(lines.len() >= 7);
        for point in lines.iter().flat_map(|line| line.iter()) {
            assert!(point.x.is_finite() && point.y.is_finite() && paper.contains(*point, 1e-7));
        }
    }
}
