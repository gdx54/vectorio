//! Encuentros pequeños fuera de las cúpulas: arrecife, buzos y exploración orbital.
use crate::{geom::Rng, scene::{Scene, V3}};
use super::super::{kit, style::part};
use super::World;

impl World<'_> {
    pub(super) fn reef(&self, scene: &mut Scene) {
        let mut rng = Rng::new(self.ctx.seed ^ 0xA12EC1FE);
        let (right, down, _) = scene.camera().axes();
        for k in 0..26 {
            let x = (k as f64 + rng.f() * 0.5) / 25.0;
            let root = self.at(scene, x, 1.015 + (x * 17.0).sin() * 0.018, -52.0);
            let height = 1.5 + rng.f() * 4.5;
            part(scene, if k % 3 == 0 { "gente" } else { "vegetación" });
            for branch in -2..=2 {
                let dx = branch as f64 * height * 0.15;
                let tip = root + right * dx - down * (height * (1.0 - 0.1 * (branch as f64).abs()));
                let elbow = root + right * dx * 0.4 - down * height * 0.4;
                scene.line(&[root, elbow, tip]);
                for side in [-1.0, 1.0] {
                    scene.line(&[elbow, tip + right * (side * 0.35) + down * height * 0.25]);
                }
            }
        }
        // Rocas espaciadas: el coral puede cruzarlas, las bases permanecen enterradas.
        for x in [0.04, 0.12, 0.33, 0.65, 0.92, 0.99] {
            let root = self.at(scene, x, 1.015, -50.0);
            let height = 1.5 + rng.f() * 1.4;
            self.style.lathe(scene, root, &[(1.2, 0.0), (1.1, height * 0.5), (0.5, height), (0.0, height * 1.05)], "terreno");
        }
    }

    pub(super) fn explorers(&self, scene: &mut Scene, space: bool) {
        let (right, down, away) = scene.camera().axes();
        for (i, (x, y)) in [(0.07, 0.62), (0.91, 0.65)].into_iter().enumerate() {
            let center = self.at(scene, x, y, -42.0);
            let along = if i == 0 { right } else { right * -1.0 };
            self.explorer(scene, center, along, down, space);
        }
        if space {
            let station = self.at(scene, 0.49, 0.30, -46.0);
            kit::station(scene, self.style, station, away, 3.0);
            let moon = self.at(scene, 0.05, 0.995, -48.0);
            part(scene, "terreno");
            for (dx, dy, radius) in [(0.0, 0.0, 1.4), (3.0, 0.3, 0.7), (5.0, 0.0, 1.0), (8.0, 0.2, 0.6)] {
                let center = moon + right * dx + down * dy;
                let rim: Vec<V3> = (0..24).map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / 24.0;
                    center + right * (a.cos() * radius) + down * (a.sin() * radius * 0.35)
                }).collect();
                scene.closed(&rim);
            }
        } else {
            self.submarine(scene, self.at(scene, 0.49, 0.30, -46.0));
        }
    }

    fn explorer(&self, scene: &mut Scene, center: V3, along: V3, down: V3, space: bool) {
        let at = |x, y| center + along * x + down * y;
        part(scene, "gente");
        self.bubble(scene, at(0.85, -0.15), if space { 0.40 } else { 0.24 });
        part(scene, "gente");
        scene.closed(&[at(0.5, -0.3), at(0.45, 0.22), at(-0.35, 0.18), at(-0.45, -0.25)]);
        for side in [-1.0, 1.0] {
            scene.line(&[at(-0.35, side * 0.2), at(-0.95, side * 0.45), at(-1.5, side * 0.25)]);
            scene.line(&[at(0.30, side * 0.25), at(0.45, side * 0.60), at(1.1, side * 0.65)]);
            if !space {
                scene.closed(&[at(-1.45, side * 0.25), at(-1.95, side * 0.5), at(-1.9, side * 0.10)]);
            }
        }
        part(scene, "detalle");
        scene.closed(&[at(-0.3, -0.3), at(-0.3, -0.7), at(0.35, -0.7), at(0.35, -0.3)]);
        if space {
            let tether: Vec<V3> = (0..24).map(|k| {
                let t = k as f64 / 23.0;
                at(-0.5 - t * 5.0, -0.6 - (t * 4.0).sin())
            }).collect();
            scene.line(&tether);
        } else {
            for k in 0..3 { self.bubble(scene, at(1.1 + k as f64 * 0.3, -0.7 - k as f64 * 0.5), 0.08 + k as f64 * 0.04); }
        }
    }

    fn submarine(&self, scene: &mut Scene, center: V3) {
        let (right, down, away) = scene.camera().axes();
        let spine = [center - right * 2.6, center - right * 1.8, center + right * 1.8, center + right * 2.6];
        kit::tube(scene, self.style, &spine, &[0.0, 0.8, 0.8, 0.0], false);
        for k in -1..=1 { self.bubble(scene, center + right * k as f64 - away * 0.82, 0.24); }
        part(scene, "detalle");
        let top = center - down * 0.8;
        scene.line(&[top, top - down * 0.8, top - down * 0.8 + right * 0.7]);
        scene.closed(&[center - right * 2.3, center - right * 3.1 - down * 0.7, center - right * 3.1 + down * 0.7]);
    }
}
