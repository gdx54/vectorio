//! Seis interiores con plantas distintas; el vidrio conserva un pasillo alrededor.
use crate::{geom::Rng, scene::{Scene, V3}};
use super::super::{kit::{self, v, Crowd, Floor, Plant, Pose, Roof}, style::part};
use super::World;

impl World<'_> {
    pub(super) fn habitat(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, floor: Floor, theme: usize) {
        match theme {
            0 => self.village(scene, rng, floor),
            1 => self.garden(scene, rng, floor),
            2 => self.court(scene, floor),
            3 => self.market(scene, floor),
            4 => self.farm(scene, rng, floor),
            _ => self.laboratory(scene, floor),
        }
        self.visitors(scene, crowd, rng, floor, theme);
    }

    fn village(&self, scene: &mut Scene, rng: &mut Rng, floor: Floor) {
        let r = floor.radius;
        for side in [-1.0, 1.0] {
            let min = floor.center + v(side * r * 0.4 - r * 0.15, -r * 0.08, 0.0);
            let max = min + v(r * 0.30, r * 0.35, r * (0.28 + rng.f() * 0.12));
            kit::house(scene, self.style, min, max, Roof::Gable, rng);
            if r > 8.5 {
                let front = floor.center + v(side * r * 0.4 - r * 0.13, -r * 0.5, 0.0);
                kit::house(scene, self.style, front, front + v(r * 0.26, r * 0.25, r * 0.23), Roof::Flat, rng);
            }
        }
        let tower = floor.center + v(-r * 0.10, r * 0.35, 0.0);
        kit::house(scene, self.style, tower, tower + v(r * 0.20, r * 0.22, r * 0.63), Roof::Hip, rng);
        part(scene, "terreno");
        for side in [-1.0, 1.0] {
            scene.line(&[floor.center + v(side * r * 0.14, -r * 0.73, 0.0), floor.center + v(side * r * 0.14, r * 0.28, 0.0)]);
            scene.line(&[floor.center + v(-r * 0.7, -r * (0.20 + side * 0.05), 0.0), floor.center + v(r * 0.7, -r * (0.20 + side * 0.05), 0.0)]);
        }
        kit::vegetation(scene, self.style, floor.center + v(-r * 0.34, r * 0.57, 0.0), r * 0.45, Plant::Cypress, rng);
    }

    fn garden(&self, scene: &mut Scene, rng: &mut Rng, floor: Floor) {
        let r = floor.radius;
        kit::fountain(scene, self.style, floor.center, (r * 0.13).clamp(0.6, 1.5));
        for (i, (x, y)) in [(-0.50, 0.12), (0.37, 0.45), (0.37, -0.42), (-0.33, -0.43), (-0.27, 0.54)].into_iter().enumerate() {
            let plant = if i % 2 == 0 { Plant::Palm } else { Plant::Olive };
            kit::vegetation(scene, self.style, floor.center + v(r * x, r * y, 0.0), r * (0.46 + i as f64 * 0.035), plant, rng);
        }
        part(scene, "terreno");
        for offset in [-0.12, 0.12] {
            scene.line(&[floor.center + v(-r * 0.7, r * offset, 0.0), floor.center + v(-r * 0.22, r * offset, 0.0)]);
        }
        kit::bench(scene, floor.center + v(r * 0.10, -r * 0.54, 0.0), true);
    }

    fn court(&self, scene: &mut Scene, floor: Floor) {
        let r = floor.radius;
        let c = floor.center + v(0.0, 0.0, 0.03);
        part(scene, "agua");
        scene.closed(&[c + v(-r * 0.58, -r * 0.4, 0.0), c + v(r * 0.58, -r * 0.4, 0.0), c + v(r * 0.58, r * 0.4, 0.0), c + v(-r * 0.58, r * 0.4, 0.0)]);
        scene.line(&[c + v(0.0, -r * 0.4, 0.0), c + v(0.0, r * 0.4, 0.0)]);
        let circle: Vec<V3> = (0..24).map(|k| {
            let a = std::f64::consts::TAU * k as f64 / 24.0;
            c + v(a.cos(), a.sin(), 0.0) * (r * 0.16)
        }).collect();
        scene.closed(&circle);
        for side in [-1.0, 1.0] {
            let center = c + v(side * r * 0.58, 0.0, 0.0);
            part(scene, "detalle");
            scene.line(&[center + v(0.0, -r * 0.14, 0.0), center + v(0.0, -r * 0.14, 1.2), center + v(0.0, r * 0.14, 1.2), center + v(0.0, r * 0.14, 0.0)]);
            for row in 0..4 {
                let y = -r * 0.29 + row as f64 * r * 0.19;
                scene.line(&[center + v(0.0, y, 0.0), center + v(0.0, y, 1.2)]);
            }
        }
    }

    fn market(&self, scene: &mut Scene, floor: Floor) {
        let r = floor.radius;
        let height = 0.9_f64.max(1.8 / scene.camera().scale);
        for side in [-1.0, 1.0] {
            let center = floor.center + v(side * r * 0.35, r * 0.18, 0.0);
            part(scene, "contorno");
            scene.box3(center + v(-r * 0.20, -0.5, 0.0), center + v(r * 0.20, 0.5, height));
            kit::parasol(scene, center + v(0.0, 0.0, height), 1.5, r * 0.23);
            part(scene, "gente");
            for k in 0..4 {
                let item = center + v((k as f64 - 1.5) * r * 0.09, -0.3, height + 0.03);
                scene.line(&[item, item + v(0.0, 0.0, 0.3), item + v(r * 0.05, 0.0, 0.3), item + v(r * 0.05, 0.0, 0.0)]);
            }
        }
        let far = floor.center + v(-r * 0.19, r * 0.52, 0.0);
        let front = far + v(r * 0.38, 0.0, 0.0);
        part(scene, "contorno");
        scene.line(&[far, far + v(0.0, 0.0, 2.6), front + v(0.0, 0.0, 2.6), front]);
        part(scene, "tejados");
        scene.closed(&[far + v(0.0, 0.0, 2.6), front + v(0.0, 0.0, 2.6), front + v(0.0, -r * 0.2, 2.1), far + v(0.0, -r * 0.2, 2.1)]);
    }

    fn farm(&self, scene: &mut Scene, rng: &mut Rng, floor: Floor) {
        let r = floor.radius;
        let min = floor.center + v(-r * 0.22, r * 0.25, 0.0);
        kit::house(scene, self.style, min, min + v(r * 0.44, r * 0.3, r * 0.24), Roof::Gable, rng);
        part(scene, "vegetación");
        for row in 0..5 {
            let y = -r * 0.55 + row as f64 * r * 0.13;
            scene.line(&[floor.center + v(-r * 0.55, y, 0.0), floor.center + v(r * 0.55, y, 0.0)]);
            for col in 0..5 {
                let root = floor.center + v(-r * 0.46 + col as f64 * r * 0.23, y, 0.0);
                scene.line(&[root + v(-0.2, 0.0, 0.3), root, root + v(0.15, 0.0, 0.5)]);
            }
        }
    }

    fn laboratory(&self, scene: &mut Scene, floor: Floor) {
        let r = floor.radius;
        for side in [-1.0, 1.0] {
            let center = floor.center + v(side * r * 0.33, r * 0.20, 0.0);
            part(scene, "contorno");
            let faces = scene.lathe(center, &[(r * 0.16, 0.0), (r * 0.16, r * 0.35), (r * 0.12, r * 0.40)], true);
            self.style.shade(scene, &faces, super::super::style::Surface::Wall, "trama");
            part(scene, "agua");
            scene.line(&[center + v(0.0, 0.0, r * 0.4), center + v(0.0, 0.0, r * 0.55)]);
            let panel = center + v(0.0, -r * 0.42, 0.8);
            scene.closed(&[panel + v(-r * 0.18, 0.0, 0.0), panel + v(r * 0.18, 0.0, 0.0), panel + v(r * 0.18, r * 0.10, 0.6), panel + v(-r * 0.18, r * 0.10, 0.6)]);
            for k in 0..3 {
                let at = panel + v((k as f64 - 1.0) * r * 0.12, 0.0, 0.1);
                scene.line(&[at, at + v(0.0, r * 0.06, 0.3)]);
            }
        }
    }

    fn visitors(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, floor: Floor, theme: usize) {
        let count = ((8.0 + floor.radius * 2.0) * self.prm.f64("people")).round() as usize;
        let r = floor.radius;
        for k in 0..count {
            let angle = (k as f64 + rng.f() * 0.35) / count as f64 * std::f64::consts::TAU;
            let distance = r * (0.70 + rng.f() * 0.07);
            let (x, y) = if theme == 2 && k % 2 == 0 { ((rng.f() - 0.5) * r, (rng.f() - 0.5) * r * 0.6) } else { (distance * angle.cos(), distance * angle.sin()) };
            let pose = if k % 5 == 0 { Pose::Wave } else if theme == 2 && k % 3 == 0 { Pose::Cheer } else { Pose::Walk };
            crowd.person(scene, rng, floor.center + v(x, y, 0.0), pose, k % 2 == 0);
        }
    }
}
