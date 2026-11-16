//! Costa de la ciudad: en uno de los lados delanteros de la vista, la calle de la
//! primera corona se asoma al mar. Con playa, un paseo marítimo con palmeras sobre
//! un muro y la arena con sombrillas; con puerto, un muelle con barcas amarradas,
//! un pantalán, una lonja y un espigón con su faro.
use crate::{geom::Rng, scene::{Camera, Scene, V3}};
use super::super::kit::{self, v, figure, Crowd, Plant, Pose, Vehicle};
use super::super::style::part;
use super::{flat, readable, sides, straight_band, City, RACE_MAX, RACE_MIN};

pub(super) const COASTS: &[(&str, &str)] = &[("none", "Sin costa"), ("beach", "Playa"), ("port", "Puerto")];

/// Cota del agua y de la arena: el paseo queda un muro por encima de ellas.
const SEA: f64 = -1.5;
/// Del eje de la calle de costa al borde del paseo o del muelle.
const EDGE: f64 = 13.0;
/// Largo del pantalán y largo mínimo del espigón.
const PIER: f64 = 18.0;
const MOLE_MIN: f64 = 20.0;
/// Separación de las escaleras que bajan a la arena.
const STAIRS: f64 = 36.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Shore { Beach, Port }

/// Costa a lo largo de una calle: `out` apunta al mar, `along` sigue la orilla y
/// `street` es la distancia del eje de la calle al origen, medida sobre `out`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Coast {
    pub kind: Shore,
    pub out: V3,
    pub along: V3,
    pub street: f64,
}

impl Coast {
    /// Lado delantero sorteado con la semilla; con carrera, nunca el de la meta.
    pub fn choose(kind: &str, camera: &Camera, race: bool, seed: u64) -> Option<Self> {
        let kind = match kind {
            "beach" => Shore::Beach,
            "port" => Shore::Port,
            _ => return None,
        };
        let away = flat(camera.axes().2);
        let options: Vec<V3> = sides().iter().map(|(n, _)| *n).filter(|n| n.dot(away) < -0.1 && !(race && n.y < -0.5)).collect();
        if options.is_empty() {
            return None;
        }
        let pick = Rng::new(seed ^ 0xC057A).f();
        let out = options[((pick * options.len() as f64) as usize).min(options.len() - 1)];
        let street = if out.x + out.y < 0.0 { -RACE_MIN } else { RACE_MAX };
        Some(Self { kind, out, along: v(-out.y, out.x, 0.0), street })
    }

    /// Punto a `u` metros del eje de la calle hacia el mar y `s` a lo largo.
    pub fn at(&self, u: f64, s: f64, z: f64) -> V3 {
        self.out * (self.street + u) + self.along * s + v(0.0, 0.0, z)
    }

    /// Más allá de `margin` metros del eje de la calle de costa, hacia el mar.
    pub fn beyond(&self, point: V3, margin: f64) -> bool {
        self.out.dot(v(point.x, point.y, 0.0)) > self.street + margin + 1e-9
    }

    /// Recorta un tramo de calle a tierra firme; nada si queda menos de un metro.
    pub fn keep(&self, a: V3, b: V3) -> Option<(V3, V3)> {
        let limit = self.street + 4.0;
        let (da, db) = (self.out.dot(a) - limit, self.out.dot(b) - limit);
        if da > 1e-9 && db > 1e-9 {
            return None;
        }
        let cut = a + (b - a) * (da / (da - db));
        let (a, b) = if da > 1e-9 { (cut, b) } else if db > 1e-9 { (a, cut) } else { (a, b) };
        ((b - a).length() >= 1.0).then_some((a, b))
    }

    /// Las calles perpendiculares a la costa no tienen bordillo en el cruce con
    /// la calle de costa: lo cierra esta línea.
    pub fn crosses_streets(&self) -> bool {
        self.out.x.abs() > 0.5
    }
}

/// Tramos de `from` a `to` que dejan libres los huecos de las escaleras.
fn pieces((from, to): (f64, f64), stairs: &[f64]) -> Vec<(f64, f64)> {
    let mut cuts = Vec::new();
    let mut start = from;
    for s in stairs {
        cuts.push((start, s - 1.25));
        start = s + 1.25;
    }
    cuts.push((start, to));
    cuts.into_iter().filter(|(a, b)| b - a > 0.5).collect()
}

/// Escaleras a la arena cada `STAIRS` metros, siempre en las mismas abscisas.
fn stair_spots((from, to): (f64, f64)) -> Vec<f64> {
    let mut s = (from / STAIRS).ceil() * STAIRS + 10.0;
    let mut spots = Vec::new();
    while s < to - 4.0 {
        spots.push(s);
        s += STAIRS;
    }
    spots
}

/// Posiciones cada `step` metros con fase `phase` dentro del tramo.
fn every((from, to): (f64, f64), step: f64, phase: f64) -> Vec<f64> {
    let mut s = (from / step).ceil() * step + phase;
    let mut spots = Vec::new();
    while s < to {
        spots.push(s);
        s += step;
    }
    spots
}

/// Bloque de escollera: polígono irregular de 6 lados alrededor de (`u`, `t`)
/// en las coordenadas del talud.
fn boulder(rng: &mut Rng, u: f64, t: f64, ru: f64, rt: f64) -> Vec<(f64, f64)> {
    (0..6).map(|k| {
        let a = std::f64::consts::TAU * (k as f64 + rng.f() * 0.5) / 6.0;
        let r = 0.75 + rng.f() * 0.25;
        (u + a.cos() * ru * r, t + a.sin() * rt * r)
    }).collect()
}

/// Línea de la orilla: la arena entra y sale con dos ondas suaves.
fn waterline(shore: f64, phase: f64, s: f64) -> f64 {
    shore + 0.9 * (s * 0.13 + phase).sin() + 0.45 * (s * 0.41 + phase * 1.7).sin()
}

impl City<'_> {
    /// Tramo de orilla, en metros de `along`, que cae en el papel a la distancia `u`.
    fn span(&self, scene: &Scene, coast: &Coast, u: f64, z: f64) -> Option<(f64, f64)> {
        let found: Vec<f64> = (-120..=130).map(|k| k as f64 * 2.0).filter(|s| self.on_paper(scene, coast.at(u, *s, z), 6.0)).collect();
        Some((*found.first()?, *found.last()?))
    }

    /// Hasta dónde se ve el mar desde el borde del paseo.
    fn reach(&self, scene: &Scene, coast: &Coast) -> f64 {
        (0..60).map(|k| EDGE + k as f64 * 4.0).take_while(|u| self.span(scene, coast, *u, SEA).is_some()).last().unwrap_or(EDGE)
    }

    /// Bordillos que cierran los cruces con la calle de costa.
    pub(super) fn coast_kerbs(&self, scene: &mut Scene, coast: &Coast, lo: i32, hi: i32) {
        if !coast.crosses_streets() {
            return;
        }
        part(scene, "contorno");
        for row in lo..=hi + 1 {
            let c = row as f64 * super::PITCH - 4.0;
            let s = coast.along.y * c;
            scene.line(&[coast.at(4.0, s - 4.0, 0.0), coast.at(4.0, s + 4.0, 0.0)]);
        }
    }

    /// Lo que queda a la altura del agua, con su propio grupo de sombras.
    pub(super) fn shore(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast) {
        let Some(span) = self.span(scene, coast, EDGE, 0.0) else { return };
        scene.begin_shadows(SEA, None, self.style.shadow_angle());
        match coast.kind {
            Shore::Beach => {
                let stairs = stair_spots(span);
                self.seawall(scene, coast, span, &stairs);
                self.beach(scene, crowd, rng, coast);
            }
            Shore::Port => {
                self.seawall(scene, coast, span, &[]);
                self.harbour(scene, crowd, rng, coast, span);
            }
        }
        self.shadows(scene);
    }

    /// Lo que queda a la altura de la calle: el paseo marítimo o el muelle.
    pub(super) fn waterfront(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast) {
        let Some(span) = self.span(scene, coast, EDGE, 0.0) else { return };
        match coast.kind {
            Shore::Beach => self.promenade(scene, crowd, rng, coast, span),
            Shore::Port => self.quay(scene, crowd, rng, coast, span),
        }
    }

    /// Muro del paseo o cantil del muelle, del suelo de la calle al agua, con
    /// huecos para las escaleras.
    fn seawall(&self, scene: &mut Scene, coast: &Coast, span: (f64, f64), stairs: &[f64]) {
        part(scene, "contorno");
        for (a, b) in pieces(span, stairs) {
            let face = [coast.at(EDGE, a, 0.0), coast.at(EDGE, b, 0.0), coast.at(EDGE, b, SEA), coast.at(EDGE, a, SEA)];
            scene.occluder(&face);
            scene.line(&[face[0], face[1]]);
            scene.line(&[face[3], face[2]]);
        }
        for s in stairs {
            self.stairs(scene, coast, *s);
        }
    }

    /// Cinco peldaños de una pieza que bajan del paseo a la arena.
    fn stairs(&self, scene: &mut Scene, coast: &Coast, s: f64) {
        let steps = 5;
        let rise = -SEA / steps as f64;
        let side = s - 1.25;
        let mut profile = vec![coast.at(EDGE, side, SEA)];
        for k in 0..steps {
            let u = EDGE + 0.4 * (steps - k) as f64;
            let z = SEA + rise * k as f64;
            profile.push(coast.at(u, side, z));
            profile.push(coast.at(u, side, z + rise));
        }
        profile.push(coast.at(EDGE, side, 0.0));
        kit::prism_piece(scene, self.style, &profile, coast.along * 2.5, "contorno");
    }

    /// Arena punteada, orilla con espuma, sombrillas con toallas y bañistas, una
    /// torre de socorrista, una red de vóley y, en el mar, nadadores y barcos.
    fn beach(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast) {
        let shore = EDGE + 13.0 + rng.f() * 4.0;
        let phase = rng.f() * std::f64::consts::TAU;
        let Some(span) = self.span(scene, coast, (EDGE + shore) * 0.5, SEA) else { return };
        self.sand(scene, rng, coast, span, shore, phase);
        self.foam(scene, rng, coast, shore, phase);
        let guard = (span.0 + span.1) * 0.5 + (rng.f() - 0.5) * 20.0;
        let net = span.0 + (span.1 - span.0) * (0.2 + rng.f() * 0.15);
        let play = self.people() > 0.2;
        let mut keep_clear = vec![(guard, 4.5)];
        if play {
            keep_clear.push((net, 7.5));
        }
        self.parasols(scene, crowd, rng, coast, span, shore, &keep_clear);
        self.lifeguard(scene, crowd, rng, coast, guard, shore - 4.0);
        if play {
            self.volley(scene, crowd, rng, coast, net);
        }
        self.swimmers(scene, rng, coast, span, shore);
        self.waves(scene, rng, coast, shore + 4.0, 6.0);
        self.boats(scene, rng, coast, shore + 16.0, 3);
    }

    /// Puntos sueltos en la arena, más juntos cerca del agua.
    fn sand(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, span: (f64, f64), shore: f64, phase: f64) {
        if !scene.visible(1.0, 1.5) {
            return;
        }
        part(scene, "terreno");
        let count = ((span.1 - span.0) * (shore - EDGE) * 0.22) as usize;
        for _ in 0..count {
            let s = span.0 + rng.f() * (span.1 - span.0);
            let wet = rng.f().sqrt();
            let u = EDGE + 0.6 + (waterline(shore, phase, s) - EDGE - 1.2) * wet;
            let dot = coast.at(u, s, SEA);
            if self.on_paper(scene, dot, 0.0) {
                scene.line(&[dot, dot + coast.along * 0.12]);
            }
        }
    }

    /// La orilla y dos líneas de espuma rotas que la siguen mar adentro.
    fn foam(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, shore: f64, phase: f64) {
        let Some(span) = self.span(scene, coast, shore, SEA) else { return };
        part(scene, "agua");
        let edge: Vec<V3> = every(span, 1.5, 0.0).into_iter().map(|s| coast.at(waterline(shore, phase, s), s, SEA)).collect();
        scene.line(&edge);
        for (gap, offset) in [(1.6, 0.6), (4.0, 1.9)] {
            let mut s = span.0;
            while s < span.1 {
                let length = 3.0 + rng.f() * 5.0;
                let stroke: Vec<V3> = (0..=6).map(|k| {
                    let t = s + length * k as f64 / 6.0;
                    coast.at(waterline(shore, phase + offset, t) + gap, t, SEA)
                }).collect();
                scene.line(&stroke);
                s += length + 1.5 + rng.f() * 4.0;
            }
        }
    }

    /// Hileras sueltas de sombrillas, cada una con su toalla y alguien sentado o
    /// de pie al lado. `keep_clear` son tramos (centro, semiancho) sin sombrillas.
    #[allow(clippy::too_many_arguments)]
    fn parasols(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast, span: (f64, f64), shore: f64, keep_clear: &[(f64, f64)]) {
        let fill = 0.35 + 0.5 * self.people();
        let mut u = EDGE + 4.5;
        while u < shore - 4.0 {
            for s in every(span, 5.5, rng.f() * 3.0) {
                let s = s + (rng.f() - 0.5) * 2.0;
                let wobble = (rng.f() - 0.5) * 1.2;
                let busy = keep_clear.iter().any(|(c, w)| (s - c).abs() < *w);
                if rng.f() > fill || busy {
                    continue;
                }
                self.parasol(scene, crowd, rng, coast, u + wobble, s);
            }
            u += 4.5;
        }
    }

    fn parasol(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast, u: f64, s: f64) {
        let foot = coast.at(u, s, SEA);
        if !self.on_paper(scene, foot, 4.0) {
            return;
        }
        kit::parasol(scene, foot, 2.2, 1.1);
        let towel = [coast.at(u + 0.5, s - 0.9, SEA), coast.at(u + 0.5, s + 0.9, SEA), coast.at(u + 1.3, s + 0.9, SEA), coast.at(u + 1.3, s - 0.9, SEA)];
        part(scene, "acento");
        scene.closed(&towel);
        if rng.f() < self.people() {
            let pose = if rng.f() < 0.7 { Pose::Sit } else { Pose::Stand };
            let at = coast.at(u + 0.9, s + (rng.f() - 0.5), SEA);
            let mirror = rng.f() < 0.5;
            crowd.person(scene, rng, at, pose, mirror);
        }
    }

    /// Torre de socorrista: cuatro patas, garita, escalera hacia tierra y bandera.
    fn lifeguard(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast, s: f64, u: f64) {
        let deck = SEA + 2.4;
        if !self.on_paper(scene, coast.at(u, s, deck), 4.0) {
            return;
        }
        part(scene, "contorno");
        for (du, ds) in [(-0.7, -0.7), (0.7, -0.7), (0.7, 0.7), (-0.7, 0.7)] {
            scene.line(&[coast.at(u + du * 1.3, s + ds * 1.3, SEA), coast.at(u + du, s + ds, deck)]);
        }
        let a = coast.at(u - 0.8, s - 0.8, deck);
        let b = coast.at(u + 0.8, s + 0.8, deck + 1.3);
        kit::box_piece(scene, self.style, super::lower(a, b), super::upper(a, b), "contorno");
        part(scene, "detalle");
        for side in [-0.35, 0.35] {
            scene.line(&[coast.at(u - 2.6, s + side, SEA), coast.at(u - 0.8, s + side, deck)]);
        }
        for k in 1..6 {
            let t = k as f64 / 6.0;
            scene.line(&[coast.at(u - 2.6 + 1.8 * t, s - 0.35, SEA + 2.4 * t), coast.at(u - 2.6 + 1.8 * t, s + 0.35, SEA + 2.4 * t)]);
        }
        let pole = coast.at(u + 0.6, s + 0.6, deck + 1.3);
        scene.line(&[pole, pole + v(0.0, 0.0, 1.8)]);
        part(scene, "acento");
        scene.closed(&[pole + v(0.0, 0.0, 1.8), pole + coast.along * 0.9 + v(0.0, 0.0, 1.5), pole + v(0.0, 0.0, 1.2)]);
        let mirror = rng.f() < 0.5;
        crowd.person(scene, rng, coast.at(u - 1.4, s + 1.2, SEA), Pose::Stand, mirror);
    }

    /// Red de vóley entre dos postes y dos jugadores a cada lado.
    fn volley(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast, s: f64) {
        let u = EDGE + 5.0;
        if !self.on_paper(scene, coast.at(u, s, SEA), 2.0) || !scene.visible(0.5, 1.0) {
            return;
        }
        part(scene, "detalle");
        let post = |k: f64, z: f64| coast.at(u + k, s, SEA + z);
        for k in [-4.0, 4.0] {
            scene.line(&[post(k, 0.0), post(k, 2.45)]);
        }
        for z in [1.6, 2.4] {
            scene.line(&[post(-4.0, z), post(4.0, z)]);
        }
        for k in 1..16 {
            let x = -4.0 + k as f64 * 0.5;
            scene.line(&[post(x, 1.6), post(x, 2.4)]);
        }
        for (du, ds) in [(-2.0, -3.0), (2.0, -4.5), (-1.5, 3.5), (2.5, 4.0)] {
            let pose = kit::pick(rng, &[Pose::Cheer, Pose::Wave, Pose::Stand]);
            crowd.person(scene, rng, coast.at(u + du, s + ds, SEA), pose, ds > 0.0);
        }
    }

    /// Cabezas de nadadores con su onda en el agua, cerca de la orilla.
    fn swimmers(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, span: (f64, f64), shore: f64) {
        if !scene.visible(0.3, 0.8) {
            return;
        }
        let (right, down, _) = scene.camera().axes();
        let count = (self.people() * 14.0) as usize;
        for _ in 0..count {
            let center = coast.at(shore + 4.0 + rng.f() * 14.0, span.0 + rng.f() * (span.1 - span.0), SEA);
            if !self.on_paper(scene, center, 0.0) {
                continue;
            }
            let head: Vec<V3> = (0..8).map(|k| {
                let a = std::f64::consts::TAU * k as f64 / 8.0;
                center + v(0.0, 0.0, 0.16) + right * (a.cos() * 0.13) - down * (a.sin() * 0.13)
            }).collect();
            let ripple: Vec<V3> = (0..12).map(|k| {
                let a = std::f64::consts::TAU * k as f64 / 12.0;
                center + v(a.cos() * 0.5, a.sin() * 0.5, 0.0)
            }).collect();
            part(scene, "gente");
            scene.occluder(&head);
            scene.closed(&head);
            part(scene, "agua");
            scene.closed(&ripple);
        }
    }

    /// Rizos sueltos en el agua, de `from` hasta donde alcanza la vista.
    fn waves(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, from: f64, step: f64) {
        if !scene.visible(1.0, 1.5) {
            return;
        }
        part(scene, "agua");
        let reach = self.reach(scene, coast);
        let mut u = from;
        while u < reach {
            let Some(span) = self.span(scene, coast, u, SEA) else { break };
            for s in every(span, step, rng.f() * step) {
                if rng.f() < 0.45 {
                    continue;
                }
                let c = coast.at(u + (rng.f() - 0.5) * 3.0, s + (rng.f() - 0.5) * 4.0, SEA);
                let a = coast.along * (0.7 + rng.f() * 0.6);
                let o = coast.out * 0.18;
                scene.line(&[c - a, c - a * 0.35 + o, c + a * 0.35 - o, c + a]);
            }
            u += step * 0.7;
        }
    }

    /// Barcas y veleros mar adentro, separados para que no se toquen.
    fn boats(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, from: f64, count: usize) {
        let reach = self.reach(scene, coast);
        if reach < from + 6.0 {
            return;
        }
        let mut placed: Vec<V3> = Vec::new();
        for _ in 0..count * 4 {
            if placed.len() >= count {
                break;
            }
            let u = from + rng.f() * (reach - from);
            let Some(span) = self.span(scene, coast, u, SEA) else { continue };
            let at = coast.at(u, span.0 + rng.f() * (span.1 - span.0), SEA);
            if placed.iter().any(|q| (*q - at).length() < 14.0) || !self.on_paper(scene, at, -4.0) {
                continue;
            }
            let heading = coast.along * if rng.f() < 0.5 { 1.0 } else { -1.0 };
            self.sailboat(scene, at, heading, rng.f() < 0.6);
            placed.push(at);
        }
    }

    /// Barca con su origen en la popa, sobre el agua; con vela si `sail`.
    fn sailboat(&self, scene: &mut Scene, rear: V3, heading: V3, sail: bool) {
        kit::vehicle(scene, Vehicle::Boat, rear, heading);
        if !sail || !scene.visible(6.0, 3.0) {
            return;
        }
        let at = |x: f64, z: f64| rear + heading * x + v(0.0, 0.0, z);
        part(scene, "detalle");
        scene.line(&[at(3.2, 0.9), at(3.2, 7.6)]);
        let main = [at(3.15, 2.3), at(3.15, 7.4), at(0.6, 2.3)];
        let jib = [at(3.25, 6.9), at(5.8, 1.05), at(3.25, 1.3)];
        for sail in [main, jib] {
            scene.occluder(&sail);
            scene.closed(&sail);
        }
    }

    /// Paseo marítimo: enlosado, barandilla, palmeras y farolas alternas, bancos
    /// mirando al paseo, un quiosco de helados y paseantes.
    fn promenade(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast, span: (f64, f64)) {
        self.paving_strip(scene, coast, span, 3.0);
        self.railing(scene, coast, span, &stair_spots(span));
        for s in every(span, 12.0, 2.0) {
            self.palm(scene, rng, coast, s);
        }
        for s in every(span, 12.0, 8.0) {
            kit::lamp(scene, coast.at(10.6, s, 0.0), 4.5);
        }
        for s in every(span, 12.0, 5.0) {
            self.seat(scene, rng, coast, s);
        }
        let middle = (span.0 + span.1) * 0.5 + (rng.f() - 0.5) * 16.0;
        self.kiosk(scene, crowd, rng, coast, middle);
        kit::balloon_seller(scene, rng, coast.at(9.0, middle + 5.0, 0.0), 5, 1.72);
        self.strollers(scene, crowd, rng, coast, span, 6.8, (middle, 3.0));
    }

    /// Juntas del enlosado del paseo y la franja de los alcorques.
    fn paving_strip(&self, scene: &mut Scene, coast: &Coast, span: (f64, f64), step: f64) {
        if !scene.visible(step, 2.0) {
            return;
        }
        part(scene, "terreno");
        scene.line(&[coast.at(9.6, span.0, 0.0), coast.at(9.6, span.1, 0.0)]);
        for s in every(span, step, 0.0) {
            scene.line(&[coast.at(4.0, s, 0.0), coast.at(9.6, s, 0.0)]);
        }
    }

    /// Barandilla de dos tubos con postes cada 2,5 m, abierta en las escaleras.
    fn railing(&self, scene: &mut Scene, coast: &Coast, span: (f64, f64), stairs: &[f64]) {
        let u = EDGE - 0.15;
        part(scene, "detalle");
        for (a, b) in pieces(span, stairs) {
            for z in [0.55, 1.0] {
                scene.line(&[coast.at(u, a, z), coast.at(u, b, z)]);
            }
            let mut s = a;
            while s <= b + 1e-9 {
                scene.line(&[coast.at(u, s, 0.0), coast.at(u, s, 1.0)]);
                s += 2.5;
            }
        }
    }

    /// Palmera en su alcorque.
    fn palm(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, s: f64) {
        let foot = coast.at(10.6, s, 0.0);
        let height = 7.0 + rng.f() * 3.0;
        if !self.on_paper(scene, foot, 10.0) {
            return;
        }
        part(scene, "terreno");
        scene.closed(&[coast.at(10.0, s - 0.6, 0.0), coast.at(10.0, s + 0.6, 0.0), coast.at(11.2, s + 0.6, 0.0), coast.at(11.2, s - 0.6, 0.0)]);
        kit::vegetation(scene, self.style, foot, height, Plant::Palm, rng);
    }

    /// Banco del paseo y, a veces, alguien sentado.
    fn seat(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, s: f64) {
        let sitting = rng.f() < self.people();
        let mirror = rng.f() < 0.5;
        let center = coast.at(11.6, s, 0.0);
        if !self.on_paper(scene, center, 2.0) {
            return;
        }
        let along_x = coast.along.x.abs() > 0.5;
        let size = |a: f64, b: f64| if along_x { v(a, b, 0.0) } else { v(b, a, 0.0) };
        let foot = center - size(0.75, 0.3);
        kit::bench(scene, foot, along_x);
        if sitting {
            figure(scene, foot + size(0.75, -0.25), 1.7, Pose::Sit, mirror);
        }
    }

    /// Quiosco con el mostrador hacia el mar y cola delante.
    fn kiosk(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast, s: f64) {
        if !self.on_paper(scene, coast.at(7.0, s, 0.0), -4.0) {
            return;
        }
        let (a, b) = (coast.at(5.6, s - 1.6, 0.0), coast.at(8.4, s + 1.6, 3.0));
        let (min, max) = (super::lower(a, b), super::upper(a, b));
        kit::house_bare(scene, self.style, min, max, rng);
        for wall in kit::walls(min, max) {
            if wall.outward().dot(coast.out) > 0.5 {
                kit::shopfront(scene, self.style, wall, 0.2, wall.width - 0.4, 2.8, Some("HELADOS"), true);
            }
        }
        if self.people() > 0.0 {
            let length = 1.0 + 3.0 * self.people();
            crowd.queue(scene, rng, coast.at(9.2, s, 0.0), coast.at(9.2, s + length, 0.0));
        }
    }

    /// Paseantes por la franja `u` del paseo o del muelle, en el tramo visible,
    /// sin pasar por el edificio de `gap` (centro y semiancho).
    #[allow(clippy::too_many_arguments)]
    fn strollers(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast, span: (f64, f64), u: f64, gap: (f64, f64)) {
        if self.people() <= 0.0 || !self.room(scene, 0.85) {
            return;
        }
        for (a, b) in [(span.0, gap.0 - gap.1), (gap.0 + gap.1, span.1)] {
            crowd.crossing(scene, rng, coast.at(u, a, 0.0), coast.at(u, b, 0.0), 3.6, self.people() * 0.12);
        }
    }

    /// Puerto: barcas amarradas al cantil, un pantalán con más barcas, un espigón
    /// con su faro y algún velero saliendo.
    fn harbour(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast, span: (f64, f64)) {
        part(scene, "detalle");
        scene.line(&[coast.at(EDGE + 0.03, span.0, -0.4), coast.at(EDGE + 0.03, span.1, -0.4)]);
        let left = rng.f() < 0.5;
        let (mole, length) = self.mole_site(scene, coast, left);
        let pier = self.pier_site(scene, coast, mole);
        let pier = pier.and_then(|s| self.pier(scene, rng, coast, s));
        for s in every(span, 8.5, rng.f() * 8.5) {
            let clear = (s - mole).abs() > 10.0 && pier.is_none_or(|p| (s - p).abs() > 8.0);
            if clear && rng.f() < 0.75 {
                let heading = coast.along * if rng.f() < 0.5 { 1.0 } else { -1.0 };
                let rear = coast.at(EDGE + 1.6, s, SEA) - heading * 3.0;
                kit::vehicle(scene, Vehicle::Boat, rear, heading);
            }
        }
        self.mole(scene, crowd, rng, coast, mole, length);
        self.waves(scene, rng, coast, EDGE + 6.0, 5.0);
        self.boats(scene, rng, coast, EDGE + length + 8.0, 2);
    }

    /// El espigón más largo con el faro entero en el papel, hacia un extremo de
    /// los sitios donde cabe.
    fn mole_site(&self, scene: &Scene, coast: &Coast, left: bool) -> (f64, f64) {
        let Some(span) = self.span(scene, coast, EDGE, 0.0) else { return (0.0, MOLE_MIN) };
        for length in [32.0, 26.0, MOLE_MIN] {
            let fits = |s: f64| {
                let base = coast.at(EDGE + length - 2.6, s, SEA + 2.6);
                self.on_paper(scene, base, -4.0) && self.on_paper(scene, base + v(0.0, 0.0, 11.6), -4.0)
            };
            let options: Vec<f64> = every(span, 2.0, 0.0).into_iter().filter(|s| fits(*s)).collect();
            if options.len() >= 3 {
                let k = 1;
                return (if left { options[k] } else { options[options.len() - 1 - k] }, length);
            }
        }
        ((span.0 + span.1) * 0.5, MOLE_MIN)
    }

    /// Pantalán con la punta en el papel, lo más lejos posible del espigón.
    fn pier_site(&self, scene: &Scene, coast: &Coast, mole: f64) -> Option<f64> {
        let span = self.span(scene, coast, EDGE, 0.0)?;
        let fits = |s: &f64| (s - mole).abs() > 18.0 && self.on_paper(scene, coast.at(EDGE + PIER, *s, SEA), 0.0);
        every(span, 2.0, 0.0).into_iter().filter(fits).max_by(|p, q| (p - mole).abs().total_cmp(&(q - mole).abs()))
    }

    /// Pantalán flotante perpendicular al muelle con barcas a los dos lados.
    fn pier(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, s: f64) -> Option<f64> {
        let length = PIER;
        let deck = SEA + 0.55;
        let band = straight_band(coast.at(EDGE, s, deck), coast.at(EDGE + length, s, deck), 2.4);
        kit::ribbon(scene, &band, 0.3, kit::Marking::Plain);
        part(scene, "detalle");
        for k in 0..=(length / 4.0) as usize {
            let u = EDGE + 2.0 + k as f64 * 4.0;
            for side in [-1.2, 1.2] {
                scene.line(&[coast.at(u, s + side, deck - 0.3), coast.at(u, s + side, SEA)]);
            }
        }
        for side in [-1.0, 1.0] {
            let mut u = EDGE + 2.5;
            while u + 6.0 < EDGE + length {
                if rng.f() < 0.8 {
                    kit::vehicle(scene, Vehicle::Boat, coast.at(u, s + side * 2.9, SEA), coast.out);
                }
                u += 8.0;
            }
        }
        Some(s)
    }

    /// Espigón de escollera hacia el mar y faro de franjas en la punta.
    fn mole(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast, s: f64, length: f64) {
        let top = SEA + 2.6;
        let profile = [coast.at(EDGE, s - 4.0, SEA), coast.at(EDGE, s + 4.0, SEA), coast.at(EDGE, s + 2.0, top), coast.at(EDGE, s - 2.0, top)];
        kit::prism_piece(scene, self.style, &profile, coast.out * length, "contorno");
        self.rocks(scene, rng, coast, s, length);
        self.lighthouse(scene, coast.at(EDGE + length - 2.6, s, top));
        if self.people() > 0.0 {
            let from = coast.at(EDGE + 4.0, s, top);
            let to = coast.at(EDGE + length - 6.0, s, top);
            crowd.crossing(scene, rng, from, to, 1.6, self.people() * 0.12);
        }
    }

    /// Bloques de escollera dibujados sobre los dos taludes del espigón.
    fn rocks(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, s: f64, length: f64) {
        if !scene.visible(1.0, 2.0) {
            return;
        }
        part(scene, "terreno");
        for side in [-1.0, 1.0] {
            let lift = (coast.along * (side * 2.6) + v(0.0, 0.0, 2.0)).normalized() * 0.06;
            let point = |u: f64, t: f64| coast.at(u, s + side * (4.0 - 2.0 * t), SEA + 2.6 * t) + lift;
            for (row, t) in [0.16, 0.45, 0.74].into_iter().enumerate() {
                let mut u = EDGE + 0.5 + row as f64 * 0.45 + rng.f() * 0.4;
                while u < EDGE + length - 2.0 {
                    let du = 0.8 + rng.f() * 0.7;
                    let (dt, rt) = ((rng.f() - 0.5) * 0.06, 0.11 + rng.f() * 0.04);
                    let rock = boulder(rng, u + du * 0.5, t + dt, du * 0.5, rt);
                    scene.closed(&rock.iter().map(|(u, t)| point(*u, *t)).collect::<Vec<_>>());
                    u += du + 0.2 + rng.f() * 0.5;
                }
            }
        }
    }

    /// Faro troncocónico con franjas, balconada y linterna acristalada.
    fn lighthouse(&self, scene: &mut Scene, base: V3) {
        if !self.on_paper(scene, base + v(0.0, 0.0, 6.0), 8.0) {
            return;
        }
        super::decor::flag(scene, base + v(0.0, 0.0, 11.6), 2.5);
        let profile = [(1.4, 0.0), (1.05, 8.5), (1.55, 8.52), (1.55, 8.85), (0.9, 8.87), (0.9, 10.4), (1.1, 10.42), (0.55, 11.2), (0.0, 11.6)];
        part(scene, "contorno");
        scene.lathe(base, &profile, true);
        let radius = |z: f64| 1.4 - 0.35 * z / 8.5 + 0.03;
        part(scene, "acento");
        for z in [2.2, 3.4, 5.6, 6.8] {
            let ring: Vec<V3> = (0..24).map(|k| {
                let a = std::f64::consts::TAU * k as f64 / 24.0;
                base + v(a.cos() * radius(z), a.sin() * radius(z), z)
            }).collect();
            scene.closed(&ring);
        }
        part(scene, "detalle");
        for k in 0..8 {
            let a = std::f64::consts::TAU * k as f64 / 8.0;
            let (x, y) = (a.cos() * 0.93, a.sin() * 0.93);
            scene.line(&[base + v(x, y, 8.85), base + v(x, y, 10.4)]);
        }
    }

    /// Muelle: losas grandes, bolardos al borde, una lonja, pescadores con caña y
    /// paseantes.
    fn quay(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, coast: &Coast, span: (f64, f64)) {
        self.paving_strip(scene, coast, span, 5.0);
        part(scene, "detalle");
        scene.line(&[coast.at(EDGE - 0.5, span.0, 0.0), coast.at(EDGE - 0.5, span.1, 0.0)]);
        for s in every(span, 7.0, 3.0) {
            self.bollard(scene, coast.at(EDGE - 0.6, s, 0.0));
        }
        let width = span.1 - span.0;
        let market = span.0 + width * (0.3 + rng.f() * 0.4);
        self.fish_market(scene, rng, coast, market);
        for s in every(span, 9.0, rng.f() * 9.0) {
            if (s - market).abs() > 12.0 && rng.f() < 0.35 * self.people() {
                self.angler(scene, rng, coast, s);
            }
        }
        self.strollers(scene, crowd, rng, coast, span, 6.0, (market, 10.0));
    }

    fn bollard(&self, scene: &mut Scene, foot: V3) {
        if !scene.visible(0.4, 1.0) || !self.on_paper(scene, foot, 2.0) {
            return;
        }
        part(scene, "contorno");
        scene.lathe(foot, &[(0.2, 0.0), (0.16, 0.35), (0.26, 0.45), (0.0, 0.55)], true);
    }

    /// Lonja: nave de dos aguas con el rótulo hacia el agua.
    fn fish_market(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, s: f64) {
        if !self.on_paper(scene, coast.at(8.0, s, 0.0), -6.0) {
            return;
        }
        let (a, b) = (coast.at(5.0, s - 9.0, 0.0), coast.at(11.0, s + 9.0, 5.5));
        let (min, max) = (super::lower(a, b), super::upper(a, b));
        kit::nave(scene, self.style, min, max, rng);
        let along = readable(scene, coast.along);
        let face = coast.at(11.0, s, 0.0) + coast.out * 0.05;
        kit::sign(scene, face - along * 2.0 + v(0.0, 0.0, 2.6), along, "LONJA", 1.0);
    }

    /// Pescador al borde del muelle con la caña hacia el agua.
    fn angler(&self, scene: &mut Scene, rng: &mut Rng, coast: &Coast, s: f64) {
        let foot = coast.at(EDGE - 1.0, s, 0.0);
        if !self.on_paper(scene, foot, 0.0) || !figure(scene, foot, 1.72, Pose::Stand, rng.f() < 0.5) {
            return;
        }
        let hand = foot + v(0.0, 0.0, 1.1);
        let tip = coast.at(EDGE + 2.8, s + 0.6, 2.6);
        part(scene, "detalle");
        scene.line(&[hand, tip]);
        scene.line(&[tip, coast.at(EDGE + 3.4, s + 0.8, SEA)]);
    }
}
