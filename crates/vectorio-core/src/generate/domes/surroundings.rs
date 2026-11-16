//! Decorado a sangre y un protagonista grande detrás de los mundos habitados.
use crate::{geom::Rng, scene::{Scene, V3}};
use super::super::{kit::{self, v, Species, Vehicle}, style::part};
use super::World;

impl World<'_> {
    pub(super) fn at(&self, scene: &Scene, x: f64, y: f64, z: f64) -> V3 {
        scene.camera().ground(self.paper_point(x, y), z)
    }

    pub(super) fn protagonist(&self, scene: &mut Scene) {
        let kind = self.prm.choice(if self.space { "craft" } else { "creature" });
        if kind == "none" { return; }
        let head = self.at(scene, 0.95, 0.46, -18.0);
        let tail = self.at(scene, 0.07, 0.42, -18.0);
        let direction = (head - tail).normalized();
        let length = (head - tail).length();
        let mut rng = Rng::new(self.ctx.seed ^ 0xC12EA7);
        match kind {
            "rocket" => kit::rocket(scene, self.style, tail, direction, length),
            "station" => kit::station(scene, self.style, (head + tail) * 0.5, v(0.3, 0.4, 1.0), length * 0.27),
            _ => {
                let species = match kind { "squid" => Species::Cephalopod, "saurian" => Species::Saurian, _ => Species::Cetacean };
                kit::creature(scene, self.style, species, head, direction, length, 0.65, &mut rng);
            }
        }
    }

    pub(super) fn background(&self, scene: &mut Scene) {
        let mut rng = Rng::new(self.ctx.seed ^ 0xF0AD0);
        if self.space { self.sky(scene); } else { self.sea(scene); }
        let count = (180.0 * self.prm.f64("background")).round() as usize;
        for k in 0..count {
            let x = rng.f();
            let y = 0.08 + rng.f() * 0.92;
            let center = self.at(scene, x, y, -65.0);
            if self.space { self.star(scene, center, &mut rng); }
            else if k % 3 == 0 {
                let right = scene.camera().axes().0;
                for j in 0..3 { self.fish(scene, center + right * (j as f64 * 1.1), &mut rng); }
            }
            else { self.bubble(scene, center, 0.2 + rng.f() * 0.4); }
        }
    }

    pub(super) fn bubble(&self, scene: &mut Scene, center: V3, radius: f64) {
        let (right, down, _) = scene.camera().axes();
        part(scene, "agua");
        let ring: Vec<V3> = (0..16).map(|i| {
            let angle = std::f64::consts::TAU * i as f64 / 16.0;
            center + right * (angle.cos() * radius) + down * (angle.sin() * radius)
        }).collect();
        scene.closed(&ring);
    }

    fn fish(&self, scene: &mut Scene, center: V3, rng: &mut Rng) {
        let (mut right, down, _) = scene.camera().axes();
        if rng.f() < 0.5 { right = right * -1.0; }
        let size = 0.5 + rng.f() * 0.7;
        let at = |x, y| center + (right * x + down * y) * size;
        part(scene, "agua");
        scene.closed(&[at(0.7, 0.0), at(0.0, -0.27), at(-0.5, 0.0), at(0.0, 0.27)]);
        scene.closed(&[at(-0.5, 0.0), at(-0.85, -0.3), at(-0.85, 0.3)]);
    }

    fn star(&self, scene: &mut Scene, center: V3, rng: &mut Rng) {
        let (right, down, _) = scene.camera().axes();
        let r = 0.18 + rng.f() * 0.25;
        part(scene, "agua");
        scene.line(&[center - right * r, center + right * r]);
        scene.line(&[center - down * r, center + down * r]);
    }

    fn sea(&self, scene: &mut Scene) {
        part(scene, "agua");
        let surface: Vec<V3> = (0..=80).map(|k| {
            let x = k as f64 / 80.0;
            self.at(scene, x, 0.06 + (x * 48.0).sin() * 0.004, -50.0)
        }).collect();
        scene.line(&surface);
        let boat = self.at(scene, 0.84, 0.052, -48.0);
        kit::vehicle(scene, Vehicle::Boat, boat, scene.camera().axes().0);
        part(scene, "terreno");
        for row in 0..3 {
            let floor: Vec<V3> = (0..=60).map(|k| {
                let x = k as f64 / 60.0;
                self.at(scene, x, 0.96 + row as f64 * 0.025 + (x * 17.0).sin() * 0.018, -55.0)
            }).collect();
            scene.line(&floor);
        }
        for k in 0..18 {
            let x = (k as f64 + 0.5) / 18.0;
            let root = self.at(scene, x, 0.99 + (x * 17.0).sin() * 0.018, -55.0);
            let (right, down, _) = scene.camera().axes();
            part(scene, "vegetación");
            scene.line(&[root, root - down * 1.5 + right * 0.3, root - down * 3.0]);
            scene.line(&[root - down * 0.7, root - down * 1.8 - right * 0.8]);
            scene.line(&[root - down * 1.5, root - down * 2.7 + right * 0.7]);
        }
        self.reef(scene);
        self.explorers(scene, false);
    }

    fn sky(&self, scene: &mut Scene) {
        for (x, y, radius) in [(0.10, 0.10, 3.5), (0.90, 0.12, 2.3)] {
            let center = self.at(scene, x, y, -45.0);
            self.bubble(scene, center, radius);
            let (right, down, _) = scene.camera().axes();
            part(scene, "terreno");
            let orbit: Vec<V3> = (0..48).map(|k| {
                let angle = std::f64::consts::TAU * k as f64 / 48.0;
                center + right * (angle.cos() * radius * 1.65) + down * (angle.sin() * radius * 0.35)
            }).collect();
            scene.closed(&orbit);
        }
        let center = self.at(scene, 0.52, 0.07, -70.0);
        let (right, down, _) = scene.camera().axes();
        part(scene, "agua");
        for side in [-1.0, 1.0] {
            let spiral: Vec<V3> = (0..60).map(|k| {
                let angle = k as f64 / 60.0 * 7.0;
                let radius = 0.2 + angle * 0.65;
                center + right * (angle.cos() * radius * side) + down * (angle.sin() * radius * 0.32 * side)
            }).collect();
            scene.line(&spiral);
        }
        part(scene, "terreno");
        let horizon: Vec<V3> = (0..=60).map(|k| {
            let x = k as f64 / 60.0;
            self.at(scene, x, 0.96 + (x - 0.5).powi(2) * 0.25, -55.0)
        }).collect();
        scene.line(&horizon);
        self.explorers(scene, true);
    }
}
