//! Escenas sueltas por la ciudad: una obra con grúa, un globo aerostático,
//! bandadas de palomas, vendedores de globos, tendederos en las azoteas y
//! pintores en andamios colgados.
use crate::{geom::Rng, scene::{Scene, V3}};
use super::super::kit::{self, v, figure, Crowd, Pose, Wall};
use super::super::style::{part, Surface};
use super::{between, depth, flat, City, BLOCK, GROUND, PITCH, SET, STOREY};

impl City<'_> {
    fn stories(&self) -> f64 {
        self.prm.f64("stories")
    }

    /// Manzana junto a la plaza, libre de hitos y que no la tape de frente, que se
    /// convierte en obra si su grúa cabe en el papel; si no queda ninguna, una de
    /// la segunda corona.
    pub(super) fn site(&self, scene: &Scene, rng: &mut Rng, cells: &[(i32, i32)], taken: &[(i32, i32)]) -> Option<(i32, i32)> {
        let chance = rng.f();
        let ring = |r: i32| -> Vec<(i32, i32)> {
            cells.iter().copied().filter(|(x, y)| {
                x.abs().max(y.abs()) == r && depth(scene.camera(), (*x, *y)) > -0.9 && !taken.contains(&(*x, *y))
                    && self.crane_fits(scene, (*x, *y))
            }).collect()
        };
        let mut options = ring(1);
        if options.is_empty() {
            options = ring(2);
        }
        if options.is_empty() || chance >= self.stories() {
            return None;
        }
        let pick = ((rng.f() * options.len() as f64) as usize).min(options.len() - 1);
        Some(options[pick])
    }

    /// La punta del mástil queda dentro del papel, con margen: una obra al fondo
    /// de la hoja dejaría la grúa fuera.
    fn crane_fits(&self, scene: &Scene, cell: (i32, i32)) -> bool {
        let center = v(cell.0 as f64 * PITCH + BLOCK * 0.5, cell.1 as f64 * PITCH + BLOCK * 0.5, 30.0);
        self.ctx.area.contains(scene.camera().project(center).0, -6.0)
    }

    /// Obra: valla, estructura de forjados a medio levantar, obreros y grúa torre
    /// con el mástil en el borde de fuera y el brazo hacia la plaza; delante de
    /// la plaza la estructura es más baja para no taparla.
    pub(super) fn construction(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3, cell: (i32, i32)) {
        self.hoarding(scene, origin);
        let low = depth(scene.camera(), cell) < -0.2;
        let floors = if low { 3 } else { 4 } + (rng.f() * 2.0) as usize;
        let top = GROUND + STOREY * (floors - 1) as f64;
        let outward = if cell.0.abs() >= cell.1.abs() { v(cell.0.signum() as f64, 0.0, 0.0) } else { v(0.0, cell.1.signum() as f64, 0.0) };
        // La estructura deja libre la franja de fuera, donde va el mástil.
        let center = origin + v(BLOCK * 0.5, BLOCK * 0.5, 0.0);
        let inner = BLOCK * 0.5 - SET - 0.5;
        let mut min = center - v(inner, inner, 0.0);
        let mut max = center + v(inner, inner, 0.0);
        min.x += outward.x.min(0.0) * -5.0;
        min.y += outward.y.min(0.0) * -5.0;
        max.x -= outward.x.max(0.0) * 5.0;
        max.y -= outward.y.max(0.0) * 5.0;
        self.frame(scene, crowd, rng, min, max, floors);
        let mast = center + outward * (BLOCK * 0.5 - SET - 2.5);
        self.crane(scene, rng, mast, top + 9.0, outward * -1.0);
    }

    /// Valla de obra en el borde de la acera, con postes cada tres metros.
    fn hoarding(&self, scene: &mut Scene, origin: V3) {
        let (a, b) = (SET - 0.4, BLOCK - SET + 0.4);
        let corners = [v(a, a, 0.0), v(b, a, 0.0), v(b, b, 0.0), v(a, b, 0.0)];
        part(scene, "detalle");
        let rail: Vec<V3> = corners.iter().map(|c| origin + *c + v(0.0, 0.0, 2.0)).collect();
        scene.closed(&rail);
        for k in 0..4 {
            let (from, to) = (corners[k], corners[(k + 1) % 4]);
            let steps = ((to - from).length() / 3.0).round() as usize;
            for j in 0..steps {
                let foot = origin + from + (to - from) * (j as f64 / steps as f64);
                scene.line(&[foot, foot + v(0.0, 0.0, 2.0)]);
            }
        }
    }

    /// Forjados apilados sobre pilares, el último a medias, con obreros encima.
    fn frame(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, min: V3, max: V3, floors: usize) {
        let top = GROUND + STOREY * (floors - 1) as f64;
        part(scene, "detalle");
        for x in [min.x + 0.4, (min.x + max.x) * 0.5, max.x - 0.4] {
            for y in [min.y + 0.4, (min.y + max.y) * 0.5, max.y - 0.4] {
                scene.line(&[v(x, y, 0.0), v(x, y, top + 1.2)]);
            }
        }
        for k in 0..floors {
            let z = GROUND + STOREY * k as f64;
            let last = k + 1 == floors;
            let far = if last { (min.x + max.x) * 0.5 } else { max.x };
            part(scene, "contorno");
            let faces = scene.box3(v(min.x, min.y, z - 0.3), v(far, max.y, z));
            self.style.shade(scene, &faces, Surface::Wall, "trama");
            if rng.f() < 0.6 {
                let at = v(min.x + 1.0 + rng.f() * (far - min.x - 2.0), min.y + 1.0 + rng.f() * (max.y - min.y - 2.0), z);
                let pose = if rng.f() < 0.4 { Pose::Wave } else { Pose::Stand };
                let mirror = rng.f() < 0.5;
                crowd.person(scene, rng, at, pose, mirror);
            }
        }
    }

    /// Grúa torre: mástil de celosía, pluma y contrapluma con tirantes, cabina,
    /// contrapeso y una carga colgada del gancho.
    fn crane(&self, scene: &mut Scene, rng: &mut Rng, base: V3, height: f64, jib: V3) {
        self.mast(scene, base, height);
        let side = v(-jib.y, jib.x, 0.0);
        let at = |s: f64, z: f64| base + jib * s + v(0.0, 0.0, z);
        part(scene, "detalle");
        scene.line(&[at(-7.5, height), at(22.0, height)]);
        scene.line(&[at(0.0, height + 1.4), at(22.0, height + 0.3)]);
        let mut s = 0.0;
        while s < 21.0 {
            let rise = |q: f64| height + 1.4 - 1.1 * q / 22.0;
            scene.line(&[at(s, height), at(s + 1.0, rise(s + 1.0)), at(s + 2.0, height)]);
            s += 2.0;
        }
        let apex = at(0.0, height + 4.5);
        scene.line(&[at(-7.5, height), apex, at(22.0, height + 0.3)]);
        scene.line(&[at(0.0, height), apex]);
        part(scene, "contorno");
        let weight = between(scene, at(-7.5, height - 1.8) - side * 0.8, at(-6.0, height) + side * 0.8);
        self.style.shade(scene, &weight, Surface::Wall, "trama");
        let cab = between(scene, at(0.9, height - 2.2) - side * 0.7, at(2.3, height - 0.3) + side * 0.7);
        self.style.shade(scene, &cab, Surface::Wall, "trama");
        // El mástil está a 6,5 m del centro de la manzana: la carga cuelga sobre
        // la calle de dentro, entre el bordillo (18,5 m) y la punta del brazo.
        let reach = 19.0 + rng.f() * 2.0;
        let load = height * (0.35 + rng.f() * 0.25);
        part(scene, "detalle");
        scene.line(&[at(reach, height), at(reach, load + 1.6)]);
        scene.line(&[at(reach, load + 1.6), at(reach - 0.6, load + 0.8)]);
        scene.line(&[at(reach, load + 1.6), at(reach + 0.6, load + 0.8)]);
        part(scene, "contorno");
        let faces = between(scene, at(reach - 0.7, load) - side * 0.5, at(reach + 0.7, load + 0.8) + side * 0.5);
        self.style.shade(scene, &faces, Surface::Wall, "trama");
    }

    /// Mástil de cuatro montantes con celosía en las dos caras que dan a la cámara.
    fn mast(&self, scene: &mut Scene, base: V3, height: f64) {
        let half = 0.8;
        let corner = |sx: f64, sy: f64, z: f64| base + v(sx * half, sy * half, z);
        part(scene, "detalle");
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            scene.line(&[corner(sx, sy, 0.0), corner(sx, sy, height)]);
        }
        let away = scene.camera().axes().2;
        let fx = if away.x > 0.0 { -1.0 } else { 1.0 };
        let fy = if away.y > 0.0 { -1.0 } else { 1.0 };
        let steps = (height / 1.6).floor() as usize;
        for k in 0..steps {
            let (z0, z1) = (k as f64 * 1.6, (k as f64 + 1.0) * 1.6);
            let flip = if k % 2 == 0 { 1.0 } else { -1.0 };
            scene.line(&[corner(fx, -flip, z0), corner(fx, flip, z1)]);
            scene.line(&[corner(-flip, fy, z0), corner(flip, fy, z1)]);
        }
    }

    /// Globo aerostático a un lado de la plaza, por encima de las azoteas y lejos
    /// de la torre y de la grúa (`tall`), entero dentro de la hoja.
    pub(super) fn balloon(&self, scene: &mut Scene, rng: &mut Rng, tall: &[(i32, i32)]) {
        let chance = rng.f();
        if chance >= self.stories() * 0.8 || !self.room(scene, 0.85) {
            return;
        }
        let (right, _, away) = scene.camera().axes();
        let (right, away) = (flat(right), flat(away));
        let center = v(BLOCK * 0.5, BLOCK * 0.5, 0.0);
        for _ in 0..24 {
            let side = if rng.f() < 0.5 { -1.0 } else { 1.0 };
            let ground = center + away * ((rng.f() - 0.75) * 20.0) + right * (side * (10.0 + rng.f() * 18.0));
            let at = ground + v(0.0, 0.0, 30.0 + rng.f() * 6.0);
            if self.fits(scene, at) && clear_of(ground, tall) {
                self.envelope(scene, rng, at);
                return;
            }
        }
    }

    /// La caja del globo, de la cesta a la cúspide, cae dentro del papel.
    fn fits(&self, scene: &Scene, at: V3) -> bool {
        let mut inside = true;
        for x in [-5.5, 5.5] {
            for y in [-5.5, 5.5] {
                for z in [-3.4, 12.6] {
                    inside &= self.on_paper(scene, at + v(x, y, z), -8.0);
                }
            }
        }
        inside
    }

    /// Envoltura en un solo torno con gajos de color, cesta, cuerdas y dos
    /// pasajeros que saludan.
    fn envelope(&self, scene: &mut Scene, rng: &mut Rng, at: V3) {
        let profile = [(0.9, 0.0), (2.6, 1.4), (4.6, 4.2), (5.3, 7.0), (5.0, 9.4), (3.8, 11.2), (2.0, 12.2), (0.0, 12.5)];
        part(scene, "contorno");
        let faces = scene.lathe(at, &profile, true);
        self.style.shade(scene, &faces, Surface::Wall, "trama");
        part(scene, "acento");
        let gores = 10;
        let phase = rng.f();
        for k in 0..gores {
            let angle = std::f64::consts::TAU * (k as f64 + phase) / gores as f64;
            let (c, s) = (angle.cos(), angle.sin());
            let line: Vec<V3> = profile.iter().map(|(r, z)| at + v(c * r * 1.015, s * r * 1.015, *z)).collect();
            scene.line(&line);
        }
        let floor = at - v(0.0, 0.0, 3.2);
        part(scene, "contorno");
        scene.box3(floor - v(0.7, 0.7, 0.0), floor + v(0.7, 0.7, 1.0));
        part(scene, "detalle");
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            scene.line(&[floor + v(sx * 0.7, sy * 0.7, 1.0), at + v(sx * 0.64, sy * 0.64, 0.0)]);
        }
        let right = flat(scene.camera().axes().0);
        figure(scene, floor - right * 0.3, 1.7, Pose::Wave, false);
        figure(scene, floor + right * 0.3, 1.7, Pose::Stand, true);
    }

    /// Bandadas de palomas: una sobre la plaza y otras sobre cruces del núcleo.
    pub(super) fn pigeons(&self, scene: &mut Scene, rng: &mut Rng) {
        let flocks = (self.stories() * 3.0).round() as usize;
        for k in 0..flocks {
            let corner = if k == 0 { v(BLOCK * 0.5, BLOCK * 0.5, 0.0) } else {
                let pick = |r: f64| if r < 0.5 { -4.0 } else { PITCH - 4.0 };
                v(pick(rng.f()), pick(rng.f()), 0.0)
            };
            let count = 8 + (rng.f() * 10.0) as usize;
            let height = 13.0 + rng.f() * 8.0;
            kit::flock(scene, rng, corner + v(0.0, 0.0, height), count);
        }
    }

    /// Vendedores en dos esquinas de la plaza y algún niño con su globo por la acera.
    pub(super) fn sellers(&self, scene: &mut Scene, rng: &mut Rng, origin: V3) {
        if self.stories() <= 0.0 {
            return;
        }
        for (x, y) in [(2.2, 6.0), (21.8, 18.0)] {
            let count = 5 + (rng.f() * 4.0) as usize;
            kit::balloon_seller(scene, rng, origin + v(x, y, 0.0), count, 1.72);
        }
    }

    /// Tendedero en una azotea, con la vecina que tiende.
    pub(super) fn laundry(&self, scene: &mut Scene, rng: &mut Rng, min: V3, max: V3) {
        if rng.f() >= self.stories() * 0.3 || max.x - min.x < 5.0 || max.y - min.y < 5.0 {
            return;
        }
        let inset = self.style.quantum(scene) + 2.0 / scene.camera().scale;
        if (max.x-min.x).min(max.y-min.y) < inset * 2.0 + 1.0 {
            return;
        }
        let (a, b, across) = if max.x - min.x >= max.y - min.y {
            let y = (min.y + max.y) * 0.5;
            (v(min.x + inset, y, max.z), v(max.x - inset, y, max.z), v(0.0, 0.9, 0.0))
        } else {
            let x = (min.x + max.x) * 0.5;
            (v(x, min.y + inset, max.z), v(x, max.y - inset, max.z), v(0.9, 0.0, 0.0))
        };
        kit::laundry(scene, a, b, 1.8, rng);
        let pose = if rng.f() < 0.5 { Pose::Wave } else { Pose::Stand };
        figure(scene, a + (b - a) * 0.35 + across, 1.65, pose, rng.f() < 0.5);
    }

    /// Pintor en un andamio colgado de la azotea, en una fachada sin balcones.
    pub(super) fn painter(&self, scene: &mut Scene, rng: &mut Rng, wall: Wall, floors: usize, roof: f64) {
        if floors < 2 || wall.width < 6.0 || rng.f() >= self.stories() * 0.2 {
            return;
        }
        let level = 1 + (rng.f() * (floors - 1) as f64) as usize;
        let z = GROUND + STOREY * level as f64 - 0.6;
        let u = 0.8 + rng.f() * (wall.width - 4.6);
        part(scene, "contorno");
        let faces = between(scene, wall.at(u, z, 0.25), wall.at(u + 3.6, z + 0.12, 0.95));
        self.style.shade(scene, &faces, Surface::Wall, "trama");
        part(scene, "detalle");
        scene.line(&[wall.at(u, z + 1.0, 0.95), wall.at(u + 3.6, z + 1.0, 0.95)]);
        for end in [u + 0.2, u + 3.4] {
            scene.line(&[wall.at(end, z + 0.12, 0.95), wall.at(end, z + 1.0, 0.95)]);
            scene.line(&[wall.at(end, z + 1.0, 0.6), wall.at(end, roof + 0.6, 0.6), wall.at(end, roof + 0.6, -0.4)]);
        }
        let pose = if rng.f() < 0.6 { Pose::Wave } else { Pose::Stand };
        figure(scene, wall.at(u + 1.0 + rng.f() * 1.6, z + 0.12, 0.6), 1.7, pose, rng.f() < 0.5);
    }
}

/// El suelo bajo el globo no toca ninguna de las manzanas altas, con margen.
fn clear_of(ground: V3, tall: &[(i32, i32)]) -> bool {
    tall.iter().all(|(x, y)| {
        let (x0, y0) = (*x as f64 * PITCH - 8.0, *y as f64 * PITCH - 8.0);
        let (x1, y1) = (x0 + BLOCK + 16.0, y0 + BLOCK + 16.0);
        ground.x + 6.0 < x0 || ground.x - 6.0 > x1 || ground.y + 6.0 < y0 || ground.y - 6.0 > y1
    })
}
