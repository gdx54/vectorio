//! Lo que pasa en la plaza: verbena, mercadillo, feria con noria o cabalgata con
//! gigantes por la calle de delante.
use crate::{geom::Rng, scene::{Scene, V3}};
use super::super::kit::{self, v, figure, Band, Crowd, Plant, Pose};
use super::super::style::{part, Surface};
use super::{between, bunting, flat, paving_around, readable, sides, City, BLOCK};

/// Opciones del control «Evento».
pub(super) const EVENTS: &[(&str, &str)] = &[("verbena", "Verbena"), ("market", "Mercadillo"), ("fair", "Feria"), ("parade", "Cabalgata")];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Event { Verbena, Market, Fair, Parade }

impl Event {
    pub(super) fn parse(id: &str) -> Self {
        match id {
            "market" => Event::Market,
            "fair" => Event::Fair,
            "parade" => Event::Parade,
            _ => Event::Verbena,
        }
    }
}

const STALLS: [&str; 8] = ["FRUTA", "FLORES", "QUESOS", "MIEL", "PAN", "LIBROS", "ROPA", "CERÁMICA"];
const BOOTHS: [&str; 2] = ["TÓMBOLA", "CHURROS"];
/// Radio de la noria y altura de su cabina más baja.
const WHEEL: f64 = 8.5;
const WHEEL_CLEARANCE: f64 = 2.6;

/// Recorrido de la cabalgata: la calle de la plaza que queda más cerca de la cámara.
#[derive(Clone, Copy, Debug)]
pub(super) struct Route {
    /// Eje de la calle en la esquina de menor coordenada de la plaza.
    pub start: V3,
    /// A lo largo de la calle, siempre hacia coordenadas crecientes.
    pub along: V3,
    /// De la plaza hacia la calle.
    pub out: V3,
}

impl Route {
    pub(super) fn of(scene: &Scene, origin: V3) -> Self {
        let away = flat(scene.camera().axes().2);
        let center = origin + v(BLOCK * 0.5, BLOCK * 0.5, 0.0);
        let (out, along) = sides().into_iter().min_by(|a, b| a.0.dot(away).total_cmp(&b.0.dot(away))).unwrap();
        let along = v(along.x.abs(), along.y.abs(), 0.0);
        let start = center + out * (BLOCK * 0.5 + 4.0) - along * (BLOCK * 0.5);
        Route { start, along, out }
    }

    /// Punto a `s` metros de la esquina y `across` metros del eje, hacia fuera.
    fn at(&self, s: f64, across: f64) -> V3 {
        self.start + self.along * s + self.out * across
    }

    /// El carril cae en el tramo de la cabalgata: allí no hay tráfico.
    pub(super) fn blocks(&self, lane: &Band) -> bool {
        let middle = lane.at(lane.length() * 0.5, 0.0).0 - self.start;
        let s = middle.dot(self.along);
        middle.dot(self.out).abs() < 4.5 && s > 0.0 && s < BLOCK
    }
}

impl City<'_> {
    /// Verbena: enlosado, fuente con corros, olivos, farolas y banderines que
    /// cruzan también las calles de alrededor.
    pub(super) fn verbena(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3) {
        let center = origin + v(BLOCK * 0.5, BLOCK * 0.5, 0.0);
        paving_around(scene, origin, origin + v(BLOCK, BLOCK, 0.0), 3.0, center, 2.6);
        kit::fountain(scene, self.style, center, 2.4);
        self.dancers(scene, crowd, rng, center, 3);
        self.olives(scene, rng, origin);
        self.garlands(scene, origin);
    }

    fn olives(&self, scene: &mut Scene, rng: &mut Rng, origin: V3) {
        for (x, y) in [(3.8, 3.8), (20.2, 3.8), (20.2, 20.2), (3.8, 20.2)] {
            kit::vegetation(scene, self.style, origin + v(x, y, 0.0), 4.2 + rng.f() * 0.6, Plant::Olive, rng);
            self.bench(scene, rng, origin + v(x, y, 0.0), y < BLOCK * 0.5);
        }
    }

    /// Banco al pie del olivo, del lado de la plaza, como en el pueblo; a veces
    /// con alguien sentado mirando la fiesta.
    fn bench(&self, scene: &mut Scene, rng: &mut Rng, olive: V3, near: bool) {
        let sitting = rng.f() < self.people();
        let mirror = rng.f() < 0.5;
        // El respaldo queda del lado del olivo, y el asiento hacia el centro.
        let foot = olive + if near { v(-0.75, 1.0, 0.0) } else { v(-0.75, -1.6, 0.0) };
        kit::bench(scene, foot, true);
        if sitting {
            figure(scene, foot + v(0.75, -0.25, 0.0), 1.7, Pose::Sit, mirror);
        }
    }

    /// Corros que bailan alrededor de la fuente; más anillos con más gente.
    fn dancers(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, center: V3, most: usize) {
        let rings = (self.people() * most as f64).round() as usize;
        for ring in 0..rings {
            let r = 3.6 + ring as f64 * 1.0;
            let count = (std::f64::consts::TAU * r / 0.9).floor() as usize;
            let phase = rng.f() * std::f64::consts::TAU;
            for k in 0..count {
                let angle = phase + std::f64::consts::TAU * k as f64 / count as f64;
                let at = center + v(angle.cos() * r, angle.sin() * r, 0.0);
                let pose = if rng.f() < 0.6 { Pose::Dance } else { Pose::Cheer };
                let mirror = rng.f() < 0.5;
                crowd.person(scene, rng, at, pose, mirror);
            }
        }
    }

    /// Farolas en las esquinas y en el centro de cada lado de la plaza, y otras en
    /// la acera de enfrente; de unas a otras cuelgan las guirnaldas.
    fn garlands(&self, scene: &mut Scene, origin: V3) {
        let height = 7.0;
        let top = |q: V3| q + v(0.0, 0.0, height - 0.2);
        let ring = self.corner_lamps(scene, origin, true);
        bunting(scene, top(ring[0]), top(ring[4]), 2.0);
        bunting(scene, top(ring[2]), top(ring[6]), 2.0);
        // En zigzag sobre las cuatro calles que rodean la plaza.
        for side in 0..4 {
            let (a, b) = (ring[side * 2], ring[(side * 2 + 2) % ring.len()]);
            let along = (b - a) * (1.0 / 22.0);
            let out = v(along.y, -along.x, 0.0);
            let path = [a, a + along * 5.5 + out * 10.0, a + along * 11.0, a + along * 16.5 + out * 10.0, b];
            for pole in [path[1], path[3]] {
                kit::lamp(scene, pole, height);
            }
            for pair in path.windows(2) {
                bunting(scene, top(pair[0]), top(pair[1]), 1.0);
            }
        }
    }

    /// Farolas del borde de la plaza (con o sin las del centro de cada lado) unidas
    /// por una guirnalda. Devuelve los postes en orden.
    fn corner_lamps(&self, scene: &mut Scene, origin: V3, middles: bool) -> Vec<V3> {
        let height = 7.0;
        let spots: &[(f64, f64)] = if middles {
            &[(1.0, 1.0), (12.0, 1.0), (23.0, 1.0), (23.0, 12.0), (23.0, 23.0), (12.0, 23.0), (1.0, 23.0), (1.0, 12.0)]
        } else {
            &[(1.0, 1.0), (23.0, 1.0), (23.0, 23.0), (1.0, 23.0)]
        };
        let ring: Vec<V3> = spots.iter().map(|(x, y)| origin + v(*x, *y, 0.0)).collect();
        for (k, pole) in ring.iter().enumerate() {
            kit::lamp(scene, *pole, height);
            if k % 3 == 0 {
                super::decor::banner(scene, *pole, height);
            }
        }
        let top = |q: V3| q + v(0.0, 0.0, height - 0.2);
        for k in 0..ring.len() {
            bunting(scene, top(ring[k]), top(ring[(k + 1) % ring.len()]), 0.8);
        }
        ring
    }

    /// Mercadillo: un cuadro de puestos con toldo a rayas alrededor de la fuente y
    /// gente comprando por el pasillo.
    pub(super) fn market(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3) {
        let center = origin + v(BLOCK * 0.5, BLOCK * 0.5, 0.0);
        paving_around(scene, origin, origin + v(BLOCK, BLOCK, 0.0), 3.0, center, 2.6);
        kit::fountain(scene, self.style, center, 2.4);
        let mut label = (rng.f() * STALLS.len() as f64) as usize;
        for (out, along) in sides() {
            for s in [-3.6, 0.0, 3.6] {
                let at = center + out * 7.6 + along * s;
                self.stall(scene, crowd, rng, at, along, out * -1.0, STALLS[label % STALLS.len()]);
                label += 1;
            }
        }
        self.aisle(scene, crowd, rng, center, 4.6, 0.5);
        self.aisle(scene, crowd, rng, center, 10.2, 0.3);
        self.olives(scene, rng, origin);
        self.corner_lamps(scene, origin, false);
    }

    /// Paseantes por un cuadrado de `half` metros alrededor del centro.
    fn aisle(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, center: V3, half: f64, density: f64) {
        let corners = [v(-half, -half, 0.0), v(half, -half, 0.0), v(half, half, 0.0), v(-half, half, 0.0)];
        for k in 0..4 {
            let from = center + corners[k];
            let to = center + corners[(k + 1) % 4];
            crowd.crossing(scene, rng, from, to, 1.0, self.people() * density);
        }
    }

    /// Puesto: mostrador, cuatro palos y un toldo a rayas inclinado hacia el
    /// cliente. `at` es el centro del fondo; `front` mira al cliente.
    #[allow(clippy::too_many_arguments)]
    fn stall(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, at: V3, along: V3, front: V3, label: &str) {
        part(scene, "contorno");
        let faces = between(scene, at - along * 1.4 + front * 0.3, at + along * 1.4 + front * 1.2 + v(0.0, 0.0, 0.9));
        self.style.shade(scene, &faces, Surface::Wall, "trama");
        self.canopy(scene, at, along, front);
        kit::balloon_bunch(scene, rng, at - along * 1.4 + v(0.0, 0.0, 1.0), 3);
        let text_along = readable(scene, along);
        let sign_at = at + front * 1.22 - text_along * 1.25 + v(0.0, 0.0, 0.25);
        part(scene, "rótulos");
        kit::sign(scene, sign_at, text_along, label, 0.4);
        let seller = at + along * ((rng.f() - 0.5) * 1.6);
        let mirror = rng.f() < 0.5;
        crowd.person(scene, rng, seller, Pose::Stand, mirror);
        if rng.f() < self.people() {
            let client = at + front * 1.9 + along * ((rng.f() - 0.5) * 2.0);
            let mirror = rng.f() < 0.5;
            crowd.person(scene, rng, client, Pose::Stand, mirror);
        }
    }

    /// Toldo inclinado sobre cuatro palos, con rayas de color.
    fn canopy(&self, scene: &mut Scene, at: V3, along: V3, front: V3) {
        let back = |s: f64| at + along * s + v(0.0, 0.0, 2.7);
        let fore = |s: f64| at + along * s + front * 1.6 + v(0.0, 0.0, 2.2);
        part(scene, "detalle");
        for s in [-1.5, 1.5] {
            scene.line(&[at + along * s, back(s)]);
            let foot = at + along * s + front * 1.3;
            scene.line(&[foot, foot + v(0.0, 0.0, 2.29)]);
        }
        let roof = [back(-1.6), back(1.6), fore(1.6), fore(-1.6)];
        scene.occluder(&roof);
        scene.closed(&roof);
        if !scene.visible(0.4, 1.0) {
            return;
        }
        part(scene, "acento");
        for k in 1..8 {
            let s = -1.6 + 3.2 * k as f64 / 8.0;
            scene.line(&[back(s), fore(s)]);
        }
    }

    /// Feria: noria al fondo de la plaza, tiovivo delante, casetas a los lados y
    /// guirnaldas alrededor.
    pub(super) fn fair(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3) {
        let center = origin + v(BLOCK * 0.5, BLOCK * 0.5, 0.0);
        let (right, _, away) = scene.camera().axes();
        let (right, away) = (flat(right), flat(away));
        paving_around(scene, origin, origin + v(BLOCK, BLOCK, 0.0), 3.0, center, 0.0);
        let hub = center + away * 4.5;
        self.wheel(scene, hub, right, away);
        self.carousel(scene, rng, center - away * 5.5);
        for (k, side) in [-1.0, 1.0].into_iter().enumerate() {
            let at = center + right * (8.0 * side) - away * 1.5;
            let front = axis_toward(center - at);
            let along = v(front.y, -front.x, 0.0);
            self.stall(scene, crowd, rng, at - front * 0.6, along, front, BOOTHS[k]);
        }
        let head = hub - away * 2.6;
        crowd.queue(scene, rng, head, head - right * (1.0 + 4.0 * self.people()));
        self.onlookers(scene, crowd, rng, center - away * 5.5, origin);
        self.aisle(scene, crowd, rng, center, 10.0, 0.3);
        self.corner_lamps(scene, origin, true);
    }

    /// Noria de dos aros vista de frente, con cabinas colgadas y patas en A sobre
    /// una plataforma de embarque.
    fn wheel(&self, scene: &mut Scene, ground: V3, right: V3, axle: V3) {
        let hub = ground + v(0.0, 0.0, WHEEL + WHEEL_CLEARANCE);
        let point = |angle: f64, side: f64| hub + right * (angle.cos() * WHEEL) + v(0.0, 0.0, angle.sin() * WHEEL) + axle * (side * 0.7);
        let tau = std::f64::consts::TAU;
        part(scene, "contorno");
        let faces = scene.box3(ground - v(2.0, 2.0, 0.0), ground + v(2.0, 2.0, 0.6));
        self.style.shade(scene, &faces, Surface::Wall, "trama");
        for side in [-1.0, 1.0] {
            let rim: Vec<V3> = (0..48).map(|k| point(tau * k as f64 / 48.0, side)).collect();
            scene.closed(&rim);
            for k in 0..16 {
                scene.line(&[hub + axle * (side * 0.7), point(tau * k as f64 / 16.0, side)]);
            }
            for foot in [-1.0, 1.0] {
                scene.line(&[hub + axle * (side * 1.0), ground + right * (foot * 5.0) + axle * (side * 1.8)]);
            }
        }
        for k in 0..16 {
            let angle = tau * k as f64 / 16.0;
            scene.line(&[point(angle, -1.0), point(angle, 1.0)]);
        }
        scene.line(&[hub - axle * 1.2, hub + axle * 1.2]);
        for k in 0..12 {
            let q = point(tau * (k as f64 + 0.5) / 12.0, 0.0);
            scene.line(&[q, q - v(0.0, 0.0, 0.5)]);
            let faces = scene.box3(q + v(-0.6, -0.6, -1.9), q + v(0.6, 0.6, -0.5));
            self.style.shade(scene, &faces, Surface::Wall, "trama");
        }
    }

    /// Tiovivo: plataforma, columna y techo cónico en un solo torno, con barras,
    /// niños montados, rayas en el techo y banderín.
    fn carousel(&self, scene: &mut Scene, rng: &mut Rng, center: V3) {
        // Tres tornos sin caras compartidas: plataforma, columna (que arranca un
        // dedo por encima) y techo.
        part(scene, "contorno");
        let floor = scene.lathe(center, &[(4.2, 0.0), (4.2, 0.5), (0.0, 0.5)], true);
        self.style.shade(scene, &floor, Surface::Wall, "trama");
        scene.lathe(center + v(0.0, 0.0, 0.52), &[(0.6, 0.0), (0.6, 3.36), (0.0, 3.36)], true);
        let roof = scene.lathe(center + v(0.0, 0.0, 3.9), &[(4.6, 0.0), (4.6, 0.5), (0.0, 2.1)], true);
        self.style.shade(scene, &roof, Surface::Roof, "trama");
        let tau = std::f64::consts::TAU;
        let ray = |angle: f64, r: f64, z: f64| center + v(angle.cos() * r, angle.sin() * r, z);
        part(scene, "detalle");
        for k in 0..12 {
            let angle = tau * k as f64 / 12.0;
            scene.line(&[ray(angle, 3.4, 0.5), ray(angle, 3.4, 3.88)]);
        }
        part(scene, "acento");
        for k in 0..12 {
            let angle = tau * (k as f64 + 0.5) / 12.0;
            scene.line(&[ray(angle, 4.65, 4.42), ray(angle, 0.05, 6.02)]);
        }
        let apex = center + v(0.0, 0.0, 6.0);
        scene.line(&[apex, apex + v(0.0, 0.0, 1.3)]);
        scene.closed(&[apex + v(0.0, 0.0, 1.3), apex + v(0.9, 0.0, 1.05), apex + v(0.0, 0.0, 0.8)]);
        for k in 0..6 {
            let angle = tau * (k as f64 * 2.0 + 0.5) / 12.0;
            figure(scene, ray(angle, 3.4, 0.9), 1.15, Pose::Sit, rng.f() < 0.5);
        }
    }

    /// Curiosos alrededor del tiovivo, sin salirse de la plaza.
    fn onlookers(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, center: V3, origin: V3) {
        let count = (34.0 * self.people()) as usize;
        for k in 0..count {
            let angle = std::f64::consts::TAU * (k as f64 + rng.f() * 0.6) / count.max(1) as f64;
            let at = center + v(angle.cos(), angle.sin(), 0.0) * (5.2 + rng.f() * 0.8);
            let inside = |c: f64, o: f64| c > o + 0.8 && c < o + BLOCK - 0.8;
            if !inside(at.x, origin.x) || !inside(at.y, origin.y) {
                continue;
            }
            let pose = if rng.f() < 0.3 { Pose::Wave } else { Pose::Stand };
            let mirror = rng.f() < 0.5;
            crowd.person(scene, rng, at, pose, mirror);
        }
    }

    /// Cabalgata: la plaza mira a la calle de delante, por donde pasan una carroza,
    /// dos gigantes, la banda y los cabezudos entre dos filas de público.
    pub(super) fn parade(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3) {
        let center = origin + v(BLOCK * 0.5, BLOCK * 0.5, 0.0);
        paving_around(scene, origin, origin + v(BLOCK, BLOCK, 0.0), 3.0, center, 2.6);
        kit::fountain(scene, self.style, center, 2.4);
        self.dancers(scene, crowd, rng, center, 1);
        self.olives(scene, rng, origin);
        self.corner_lamps(scene, origin, false);
        let route = Route::of(scene, origin);
        let forward = rng.f() < 0.5;
        self.float(scene, crowd, rng, &route);
        self.marchers(scene, crowd, rng, &route, forward);
        for across in [-4.7, 4.7] {
            crowd.queue(scene, rng, route.at(5.0, across), route.at(22.5, across));
        }
    }

    /// Carroza con banderines y gente saludando encima.
    fn float(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, route: &Route) {
        part(scene, "contorno");
        let faces = between(scene, route.at(5.8, -1.3), route.at(10.8, 1.3) + v(0.0, 0.0, 1.1));
        self.style.shade(scene, &faces, Surface::Wall, "trama");
        let top = v(0.0, 0.0, 1.1);
        part(scene, "detalle");
        let (a, b) = (route.at(6.0, 0.0) + top, route.at(10.6, 0.0) + top);
        scene.line(&[a, a + v(0.0, 0.0, 2.4)]);
        scene.line(&[b, b + v(0.0, 0.0, 2.4)]);
        bunting(scene, a + v(0.0, 0.0, 2.4), b + v(0.0, 0.0, 2.4), 0.5);
        for s in [7.0, 8.3, 9.6] {
            let mirror = rng.f() < 0.5;
            crowd.person(scene, rng, route.at(s, 0.0) + top, Pose::Wave, mirror);
        }
        // El rótulo va en el costado que da a la cámara.
        let away = flat(scene.camera().axes().2);
        let side = if route.out.dot(away) < 0.0 { 1.0 } else { -1.0 };
        let text_along = readable(scene, route.along);
        let start = if text_along.dot(route.along) > 0.0 { 6.4 } else { 10.2 };
        part(scene, "rótulos");
        kit::sign(scene, route.at(start, side * 1.33) + v(0.0, 0.0, 0.25), text_along, "FIESTAS", 0.55);
    }

    /// Gigantes, banda de música y cabezudos detrás de la carroza.
    fn marchers(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, route: &Route, forward: bool) {
        let heading = if forward { route.along } else { route.along * -1.0 };
        let mirror = heading.dot(scene.camera().axes().0) < 0.0;
        for (s, across) in [(12.6, -0.9), (14.6, 0.9)] {
            let pose = if rng.f() < 0.5 { Pose::Dance } else { Pose::Wave };
            figure(scene, route.at(s, across), 4.4 + rng.f() * 0.5, pose, mirror);
        }
        for row in 0..3 {
            for across in [-1.4, 0.0, 1.4] {
                crowd.person(scene, rng, route.at(16.2 + row as f64 * 1.3, across), Pose::Walk, mirror);
            }
        }
        for (s, across) in [(20.4, -1.0), (21.2, 0.8)] {
            figure(scene, route.at(s, across), 1.2, Pose::Dance, mirror);
        }
    }
}

/// Eje del plano más parecido a `d`, de largo uno.
fn axis_toward(d: V3) -> V3 {
    if d.x.abs() >= d.y.abs() {
        v(d.x.signum(), 0.0, 0.0)
    } else {
        v(0.0, d.y.signum(), 0.0)
    }
}
