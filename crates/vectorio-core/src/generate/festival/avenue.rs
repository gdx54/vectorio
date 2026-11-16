//! Avenida diagonal: cruza la cuadrícula a 45° por un costado de la plaza. Las
//! manzanas de este lado quedan en medio triángulo —edificios plancha con la proa
//! en el cruce—, las de más allá en chaflán, y una de las cortadas es un parque
//! con pinos, olivos y cipreses.
use std::f64::consts::FRAC_1_SQRT_2;

use crate::{p, BBox, geom::Rng, scene::{Camera, Scene, V3}};
use super::super::kit::{self, v, Band, Crowd, Plant, Wall};
use super::super::style::part;
use super::coast::{Coast, Shore};
use super::{flat, split, straight_band, City, Finish, Ring, BLOCK, GROUND, PITCH, SET, STOREY};

/// Media calzada, acera delante de las cuñas y mediana, en metros.
const HALF: f64 = 6.0;
const WALK: f64 = 2.5;
const MEDIAN: f64 = 2.0;
/// Eje de la avenida en metros diagonales (√2 veces la distancia) desde el centro
/// de la plaza: deja las manzanas vecinas de la plaza en medio triángulo justo.
const AXIS: f64 = 44.0;
/// La vista se corre hacia la avenida para que se vean las dos aceras.
pub(super) const AVENUE_SHIFT: f64 = 8.0;
/// Media anchura de las calles de la cuadrícula.
const STREET: f64 = 4.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Avenue {
    /// Del centro de la plaza hacia la avenida.
    pub normal: V3,
    pub along: V3,
}

fn center() -> V3 {
    v(BLOCK * 0.5, BLOCK * 0.5, 0.0)
}

impl Avenue {
    /// La diagonal de delante de la plaza, que se ve de frente; con costa, que
    /// ocupa un lado delantero, la de detrás. Una diagonal lateral iría en la línea
    /// de la mirada y sus fachadas se verían de canto.
    pub fn choose(on: bool, camera: &Camera, coast: Option<&Coast>) -> Option<Self> {
        if !on {
            return None;
        }
        let away = flat(camera.axes().2);
        let s = FRAC_1_SQRT_2;
        let options = [v(s, s, 0.0), v(s, -s, 0.0), v(-s, s, 0.0), v(-s, -s, 0.0)];
        let toward = |n: &V3| -n.dot(away);
        let normal = options.into_iter()
            .filter(|n| coast.is_none_or(|c| n.dot(c.out) < 0.0) && toward(n).abs() > 0.5)
            .max_by(|a, b| toward(a).total_cmp(&toward(b)))?;
        Some(Self { normal, along: v(-normal.y, normal.x, 0.0) })
    }

    /// La manzana queda entre la avenida y la cámara: sus edificios van bajos para
    /// no tapar la calzada.
    fn in_front(&self, camera: &Camera, origin: V3) -> bool {
        self.side(origin) > 0.0 && self.normal.dot(flat(camera.axes().2)) < -0.3
    }

    /// Distancia con signo al eje, en metros: positiva más allá de la avenida.
    pub fn offset(&self, p: V3) -> f64 {
        self.normal.x * (p.x - center().x) + self.normal.y * (p.y - center().y) - AXIS * FRAC_1_SQRT_2
    }

    /// Punto a `s` metros a lo largo del eje y `off` hacia fuera.
    pub fn point(&self, s: f64, off: f64) -> V3 {
        center() + self.normal * (AXIS * FRAC_1_SQRT_2 + off) + self.along * s
    }

    /// Abscisa a lo largo del eje.
    fn station(&self, p: V3) -> f64 {
        self.along.dot(v(p.x - center().x, p.y - center().y, 0.0))
    }

    /// Tramos del segmento que quedan fuera de la calzada; nada de menos de un metro.
    pub fn cut(&self, a: V3, b: V3) -> Vec<(V3, V3)> {
        let (da, db) = (self.offset(a), self.offset(b));
        let pieces = if (db - da).abs() < 1e-9 {
            if da.abs() >= HALF { vec![(a, b)] } else { Vec::new() }
        } else {
            let (p, q) = ((-HALF - da) / (db - da), (HALF - da) / (db - da));
            let (enter, leave) = (p.min(q), p.max(q));
            if leave <= 0.0 || enter >= 1.0 {
                vec![(a, b)]
            } else {
                let at = |t: f64| a + (b - a) * t;
                let mut pieces = Vec::new();
                if enter > 0.0 {
                    pieces.push((a, at(enter)));
                }
                if leave < 1.0 {
                    pieces.push((at(leave), b));
                }
                pieces
            }
        };
        pieces.into_iter().filter(|(p, q)| (*q - *p).length() >= 1.0).collect()
    }

    /// Los bordillos de una calle cortada acaban en el de la avenida, en oblicuo.
    pub fn trim(&self, band: &mut Band) {
        let last = band.center.len() - 1;
        let dir = (band.center[last] - band.center[0]).normalized();
        let rate = self.normal.dot(dir);
        if rate.abs() < 1e-6 {
            return;
        }
        for i in [0, last] {
            let edge = self.offset(band.center[i]);
            if (edge.abs() - HALF).abs() > 1e-6 {
                continue;
            }
            for side in [&mut band.left, &mut band.right] {
                let q = side[i];
                side[i] = q + dir * ((edge - self.offset(q)) / rate);
            }
        }
    }

    /// El carril pisa la calzada de la avenida.
    pub fn crosses(&self, lane: &Band) -> bool {
        let length = lane.length();
        (0..=6).any(|k| self.offset(lane.at(length * k as f64 / 6.0, 0.0).0).abs() < HALF + 2.5)
    }

    /// La avenida corta los solares de la manzana o su acera.
    pub fn cuts(&self, origin: V3) -> bool {
        let reach = HALF + WALK;
        let offsets: Vec<f64> = [v(SET, SET, 0.0), v(BLOCK - SET, SET, 0.0), v(BLOCK - SET, BLOCK - SET, 0.0), v(SET, BLOCK - SET, 0.0)]
            .iter().map(|q| self.offset(origin + *q)).collect();
        let (lo, hi) = offsets.iter().fold((f64::MAX, f64::MIN), |(lo, hi), d| (lo.min(*d), hi.max(*d)));
        lo < reach && hi > -reach
    }

    /// Lado de la avenida en que queda la manzana: +1 más allá, -1 hacia la plaza.
    fn side(&self, origin: V3) -> f64 {
        self.offset(origin + center()).signum()
    }

    /// Planta de la manzana entre `inset` metros de su borde y `gap` de la calzada.
    fn plot(&self, origin: V3, inset: f64, gap: f64) -> Vec<V3> {
        let side = self.side(origin);
        let rect = vec![origin + v(inset, inset, 0.0), origin + v(BLOCK - inset, inset, 0.0),
            origin + v(BLOCK - inset, BLOCK - inset, 0.0), origin + v(inset, BLOCK - inset, 0.0)];
        clip(&rect, |p| side * self.offset(p) - HALF - gap)
    }

    /// Fondo desde la línea de fachada de la avenida.
    fn depth(&self, origin: V3, p: V3) -> f64 {
        self.side(origin) * self.offset(p) - HALF - WALK
    }

    /// Llegadas de las calles al eje; solo aquí se interrumpe la mediana.
    fn crossings(&self, span: (f64, f64)) -> Vec<f64> {
        let origin = self.point(0.0, 0.0);
        let mut stations = Vec::new();
        for k in -20..=20 {
            let street = k as f64 * PITCH - STREET;
            for (c, rate) in [(origin.x, self.along.x), (origin.y, self.along.y)] {
                let at = (street - c) / rate;
                if at > span.0 + 2.0 && at < span.1 - 2.0 {
                    stations.push(at);
                }
            }
        }
        stations.sort_by(f64::total_cmp);
        stations.dedup_by(|a, b| (*a - *b).abs() < 0.1);
        stations
    }

    /// La copa más ancha deja libre el paso, no solo el pie del tronco.
    fn tree_clear(&self, station: f64, span: (f64, f64)) -> bool {
        self.crossings(span).iter().all(|crossing| (station - crossing).abs() >= 5.5)
    }

    /// También se reserva la vista de los cruces: una copa del parque puede
    /// taparlos en el papel aunque su tronco esté fuera de la calzada.
    fn canopy_clear(&self, scene: &Scene, foot: V3, height: f64) -> bool {
        let center = scene.camera().project(foot + v(0.0, 0.0, height * 0.78)).0;
        let radius = scene.paper_mm(height * 0.5);
        let canopy = BBox { min: center - p(radius, radius), max: center + p(radius, radius) };
        self.crossings((-300.0, 300.0)).iter().all(|station| {
            let corners: Vec<_> = [-1.8, 1.8].iter().flat_map(|s| {
                [-6.5, 6.5].iter().map(move |off| vec![scene.camera().project(self.point(station + s, *off)).0])
            }).collect();
            let crossing = BBox::of(&corners);
            canopy.max.x < crossing.min.x || canopy.min.x > crossing.max.x || canopy.max.y < crossing.min.y || canopy.min.y > crossing.max.y
        })
    }

    fn median_runs(&self, span: (f64, f64)) -> Vec<(f64, f64)> {
        let mut start = span.0;
        let mut runs = Vec::new();
        for at in self.crossings(span) {
            if at - 1.8 > start {
                runs.push((start, at - 1.8));
            }
            start = at + 1.8;
        }
        if span.1 > start {
            runs.push((start, span.1));
        }
        runs
    }

    /// Tramos del eje, desplazado `off`, que no pisan las calles de la cuadrícula.
    fn free_runs(&self, (from, to): (f64, f64), off: f64, margin: f64, least: f64) -> Vec<(f64, f64)> {
        let mut runs = Vec::new();
        let mut start = None;
        let mut s = from;
        while s <= to {
            let free = !in_street(self.point(s, off), margin);
            match (free, start) {
                (true, None) => start = Some(s),
                (false, Some(a)) => {
                    runs.push((a, s - 0.5));
                    start = None;
                }
                _ => {}
            }
            s += 0.5;
        }
        if let Some(a) = start {
            runs.push((a, to));
        }
        runs.into_iter().filter(|(a, b)| b - a >= least).collect()
    }
}

/// El punto cae en una calle de la cuadrícula, con `margin` metros de holgura.
fn in_street(p: V3, margin: f64) -> bool {
    let near = |c: f64| {
        let r = (c + STREET).rem_euclid(PITCH);
        r.min(PITCH - r) < STREET + margin
    };
    near(p.x) || near(p.y)
}

/// Recorta un polígono convexo al semiplano `keep(p) >= 0`.
fn clip(poly: &[V3], keep: impl Fn(V3) -> f64) -> Vec<V3> {
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (ka, kb) = (keep(a), keep(b));
        if ka >= 0.0 {
            out.push(a);
        }
        if (ka >= 0.0) != (kb >= 0.0) {
            out.push(a + (b - a) * (ka / (ka - kb)));
        }
    }
    out.dedup_by(|p, q| (*p - *q).length() < 1e-6);
    if out.len() > 1 && (out[0] - out[out.len() - 1]).length() < 1e-6 {
        out.pop();
    }
    if out.len() < 3 { Vec::new() } else { out }
}

/// Área con signo en planta: positiva si el giro es antihorario.
fn area(poly: &[V3]) -> f64 {
    (0..poly.len()).map(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        a.x * b.y - b.x * a.y
    }).sum::<f64>() * 0.5
}

fn centroid(poly: &[V3]) -> V3 {
    poly.iter().fold(v(0.0, 0.0, 0.0), |sum, p| sum + *p) * (1.0 / poly.len() as f64)
}

/// Polígono convexo encogido `d` metros por todos sus lados.
fn inset(poly: &[V3], d: f64) -> Vec<V3> {
    let mut out = poly.to_vec();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let inward = v(-(b - a).y, (b - a).x, 0.0).normalized();
        out = clip(&out, |p| (p - a).dot(inward) - d);
        if out.is_empty() {
            break;
        }
    }
    out
}

/// Corta las esquinas agudas `cut` metros: la proa estrecha de un edificio plancha.
fn chamfer(poly: &[V3], cut: f64) -> Vec<V3> {
    let n = poly.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (prev, here, next) = (poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]);
        let (a, b) = ((prev - here).normalized(), (next - here).normalized());
        if a.dot(b) > 0.5 && (prev - here).length() > cut * 2.0 && (next - here).length() > cut * 2.0 {
            out.push(here + a * cut);
            out.push(here + b * cut);
        } else {
            out.push(here);
        }
    }
    out
}

/// Tramo `[from, to]` de la recta `p + dir·t` dentro del polígono convexo antihorario.
fn chord(poly: &[V3], p: V3, dir: V3) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (f64::MIN, f64::MAX);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let inward = v(-(b - a).y, (b - a).x, 0.0);
        let (num, den) = ((p - a).dot(inward), dir.dot(inward));
        if den.abs() < 1e-12 {
            if num < 0.0 {
                return None;
            }
        } else if den > 0.0 {
            lo = lo.max(-num / den);
        } else {
            hi = hi.min(-num / den);
        }
    }
    (hi - lo > 0.5).then_some((lo, hi))
}

/// Antihorario, para que la normal de cada pared mire hacia fuera.
fn ccw(mut poly: Vec<V3>) -> Vec<V3> {
    if area(&poly) < 0.0 {
        poly.reverse();
    }
    poly
}

/// El lado `a-b` está sobre el contorno del polígono `outer`.
fn on_boundary(outer: &[V3], a: V3, b: V3) -> bool {
    let mid = (a + b) * 0.5;
    (0..outer.len()).any(|i| {
        let (p, q) = (outer[i], outer[(i + 1) % outer.len()]);
        let d = (q - p).normalized();
        let rel = mid - p;
        let along = rel.dot(d);
        along > -0.05 && along < (q - p).length() + 0.05 && (rel - d * along).length() < 0.05
    })
}

impl City<'_> {
    /// Calzada de dos sentidos con mediana arbolada; deja carriles para el tráfico.
    pub(super) fn avenue_road(&self, scene: &mut Scene, avenue: &Avenue, lanes: &mut Vec<Band>) {
        let Some(span) = self.avenue_span(scene, avenue) else { return };
        let Some((a, b)) = self.land(avenue.point(span.0, 0.0), avenue.point(span.1, 0.0)) else { return };
        let span = (avenue.station(a), avenue.station(b));
        kit::ribbon(scene, &straight_band(a, b, HALF * 2.0), 0.0, kit::Marking::Plain);
        for (s0, s1) in avenue.median_runs(span) {
            kit::ribbon(scene, &straight_band(avenue.point(s0, 0.0), avenue.point(s1, 0.0), MEDIAN), 0.15, kit::Marking::Plain);
        }
        part(scene, "detalle");
        for side in [-1.0, 1.0] {
            let off = side * (MEDIAN * 0.5 + 2.5);
            for (s0, s1) in avenue.free_runs(span, off, 0.5, 4.0) {
                let mut s = s0 + 1.5;
                while s + 3.0 <= s1 {
                    scene.line(&[avenue.point(s, off), avenue.point(s + 3.0, off)]);
                    s += 6.0;
                }
            }
            for lane in [MEDIAN * 0.5 + 1.25, MEDIAN * 0.5 + 3.75] {
                let off = side * lane;
                for (s0, s1) in avenue.free_runs(span, off, 1.5, 12.0) {
                    let (p, q) = (avenue.point(s0, off), avenue.point(s1, off));
                    let (p, q) = if side < 0.0 { (q, p) } else { (p, q) };
                    lanes.push(straight_band(p, q, 3.0));
                }
            }
        }
    }

    /// Tramo del eje que cae en la hoja.
    fn avenue_span(&self, scene: &Scene, avenue: &Avenue) -> Option<(f64, f64)> {
        let found: Vec<f64> = (-150..=150).map(|k| k as f64 * 2.0).filter(|s| self.on_paper(scene, avenue.point(*s, 0.0), 40.0)).collect();
        Some((*found.first()? - 20.0, *found.last()? + 20.0))
    }

    /// Árboles y farolas en la mediana: palmeras junto al mar, árboles de copa
    /// redonda tierra adentro.
    pub(super) fn avenue_trees(&self, scene: &mut Scene, rng: &mut Rng, avenue: &Avenue) {
        let Some(span) = self.avenue_span(scene, avenue) else { return };
        let plant = if self.coast.is_some_and(|c| c.kind == Shore::Beach) { Plant::Palm } else { Plant::Olive };
        for (s0, s1) in avenue.median_runs(span) {
            let count = ((s1 - s0 - 2.0) / 7.0).floor().max(0.0) as usize + 1;
            let step = (s1 - s0) / count as f64;
            for k in 0..count {
                let station = s0 + step * (k as f64 + 0.5);
                if !avenue.tree_clear(station, span) {
                    continue;
                }
                let foot = avenue.point(station, 0.0) + v(0.0, 0.0, 0.15);
                if self.at_sea(foot, 0.0) || !self.on_paper(scene, foot, 8.0) {
                    continue;
                }
                if k % 2 == 1 {
                    part(scene, "contorno");
                    let pole = foot + avenue.along * (step * 0.5).min(2.0);
                    kit::lamp(scene, pole, 6.0);
                    if k % 3 == 1 {
                        super::decor::banner(scene, pole, 6.0);
                    }
                }
                let height = if plant == Plant::Palm { 7.5 + rng.f() * 1.5 } else { 5.5 + rng.f() * 1.2 };
                kit::vegetation(scene, self.style, foot, height, plant, rng);
            }
        }
    }

    pub(super) fn avenue_crossings(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, avenue: &Avenue) {
        let Some(span) = self.avenue_span(scene, avenue) else { return };
        for at in avenue.crossings(span) {
            let center = avenue.point(at, 0.0);
            if self.at_sea(center, 0.0) || !self.on_paper(scene, center, 4.0) {
                continue;
            }
            let band = straight_band(avenue.point(at - 1.5, 0.0), avenue.point(at + 1.5, 0.0), HALF * 2.0);
            kit::zebra(scene, &band, 0.3, 2.4);
            crowd.crossing(scene, rng, center - avenue.normal * (HALF + 0.5), center + avenue.normal * (HALF + 0.5), 1.8, self.people() * 0.35);
        }
    }

    /// Manzana cortada: edificios plancha en una franja a lo largo de la avenida y
    /// uno o dos detrás, con comercios y paseantes en la acera de la avenida.
    pub(super) fn wedge(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, avenue: &Avenue, origin: V3, ring: Ring) {
        let plot = ccw(avenue.plot(origin, SET, WALK));
        if plot.is_empty() || area(&plot) < 30.0 {
            return;
        }
        let finish = self.finish(scene, ring);
        let front = 7.5 + rng.f() * 2.0;
        let mut strip = clip(&plot, |p| front - avenue.depth(origin, p));
        let back = clip(&plot, |p| avenue.depth(origin, p) - front);
        let mut regions = Vec::new();
        if back.is_empty() || area(&ccw(back.clone())) < 30.0 {
            strip = plot.clone();
        } else {
            regions.push((back, false));
        }
        regions.insert(0, (strip, true));
        let mut lots: Vec<(Vec<V3>, bool)> = Vec::new();
        for (region, on_avenue) in regions {
            for piece in self.slices(rng, avenue, &region) {
                lots.push((piece, on_avenue));
            }
        }
        let low = if avenue.in_front(scene.camera(), origin) { Ring::Front } else { ring };
        let mut heights = self.heights(scene, rng, low, lots.len());
        // Las proas suben un piso más que el resto de la franja.
        let prows = lots.iter().filter(|(_, on)| *on).count();
        if prows >= 2 {
            for k in [0, prows - 1] {
                heights[k] = self.style.snap(scene, heights[k] + STOREY);
            }
        }
        for ((lot, _), height) in lots.into_iter().zip(heights) {
            let lot = ccw(chamfer(&lot, 3.0));
            self.flatiron(scene, crowd, rng, &plot, &lot, height, ring, finish);
        }
        if finish == Finish::Full && self.people() > 0.0 && self.room(scene, 0.85) {
            let edge: Vec<V3> = plot.iter().copied().filter(|p| avenue.depth(origin, *p).abs() < 0.05).collect();
            if edge.len() == 2 {
                let out = avenue.normal * (-avenue.side(origin) * 1.2);
                crowd.crossing(scene, rng, edge[0] + out, edge[1] + out, 0.8, self.people() * 0.25);
            }
        }
    }

    /// Divide una región a lo largo de la avenida en solares de 8 a 12 metros.
    fn slices(&self, rng: &mut Rng, avenue: &Avenue, region: &[V3]) -> Vec<Vec<V3>> {
        let stations: Vec<f64> = region.iter().map(|p| avenue.station(*p)).collect();
        let (lo, hi) = stations.iter().fold((f64::MAX, f64::MIN), |(lo, hi), s| (lo.min(*s), hi.max(*s)));
        let count = ((hi - lo) / 10.0).round().clamp(1.0, 3.0) as usize;
        let mut pieces = Vec::new();
        let mut start = lo;
        for width in split(rng, hi - lo, count) {
            let end = start + width;
            let piece = clip(&clip(region, |p| avenue.station(p) - start), |p| end - avenue.station(p));
            if !piece.is_empty() && area(&ccw(piece.clone())).abs() > 6.0 {
                pieces.push(ccw(piece));
            }
            start = end;
        }
        pieces
    }

    /// Un edificio de planta poligonal: volumen, ventanas en las fachadas a la calle
    /// y comercios en la planta baja.
    #[allow(clippy::too_many_arguments)]
    fn flatiron(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, plot: &[V3], lot: &[V3], height: f64, ring: Ring, finish: Finish) {
        if finish == Finish::Box {
            part(scene, "contorno");
            scene.prism(lot, v(0.0, 0.0, height));
            return;
        }
        kit::prism_piece(scene, self.style, lot, v(0.0, 0.0, height), "contorno");
        if finish < Finish::Shops {
            return;
        }
        let rhythm = kit::pick(rng, &kit::RHYTHMS);
        for i in 0..lot.len() {
            let (a, b) = (lot[i], lot[(i + 1) % lot.len()]);
            let width = (b - a).length();
            let wall = Wall { origin: a, along: (b - a).normalized(), width, height };
            let prow = width <= 5.0 && on_boundary(plot, a, a) && on_boundary(plot, b, b);
            if width < 3.5 || !wall.faces_camera(scene) || (!on_boundary(plot, a, b) && !prow) {
                continue;
            }
            if finish == Finish::Full {
                kit::window_grid(scene, self.style, wall, GROUND, STOREY, rhythm);
            }
            if prow {
                kit::shopfront(scene, self.style, wall, 0.2, width - 0.4, 3.6, Some("CAFÉ"), false);
            } else if width >= 5.5 {
                self.shops(scene, crowd, rng, wall, ring != Ring::Outer);
            }
        }
    }

    /// Parque en la manzana cortada: dos paseos que se cruzan en una fuente,
    /// cipreses en hilera por el borde, pinos al fondo y olivos cerca de la avenida.
    pub(super) fn park(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, avenue: &Avenue, origin: V3) {
        let plot = ccw(avenue.plot(origin, 1.0, 1.0));
        if plot.is_empty() || area(&plot) < 60.0 {
            return;
        }
        part(scene, "terreno");
        scene.closed(&plot);
        let lawn = inset(&plot, 0.6);
        if lawn.is_empty() {
            return;
        }
        scene.closed(&lawn);
        let middle = centroid(&lawn);
        let paths = [avenue.along, avenue.normal];
        self.park_paths(scene, crowd, rng, &lawn, middle, &paths);
        kit::fountain(scene, self.style, middle, 1.8);
        self.cypresses(scene, rng, avenue, origin, &lawn);
        self.park_trees(scene, rng, &lawn, middle, &paths);
    }

    /// Paseos de dos metros con paseantes, interrumpidos por la fuente.
    fn park_paths(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, lawn: &[V3], middle: V3, paths: &[V3]) {
        for dir in paths {
            let Some((lo, hi)) = chord(lawn, middle, *dir) else { continue };
            let across = v(-dir.y, dir.x, 0.0);
            for (from, to) in [(lo, -2.4), (2.4, hi)] {
                if to - from < 1.0 {
                    continue;
                }
                part(scene, "terreno");
                for side in [-1.0, 1.0] {
                    let start = middle + across * side;
                    let Some((a, b)) = chord(lawn, start, *dir) else { continue };
                    let (a, b) = (a.max(from), b.min(to));
                    if b - a > 0.5 {
                        scene.line(&[start + *dir * a, start + *dir * b]);
                    }
                }
                if self.people() > 0.0 {
                    crowd.crossing(scene, rng, middle + *dir * from, middle + *dir * to, 1.2, self.people() * 0.3);
                }
            }
        }
    }

    /// Cipreses cada tres metros por los bordes que dan a las calles.
    fn cypresses(&self, scene: &mut Scene, rng: &mut Rng, avenue: &Avenue, origin: V3, lawn: &[V3]) {
        let row = inset(lawn, 1.0);
        for i in 0..row.len() {
            let (a, b) = (row[i], row[(i + 1) % row.len()]);
            let on_avenue = avenue.depth(origin, a) < 1.0 && avenue.depth(origin, b) < 1.0;
            let length = (b - a).length();
            if on_avenue || length < 3.0 {
                continue;
            }
            let count = (length / 3.2).floor() as usize;
            for k in 0..count {
                let foot = a + (b - a) * ((k as f64 + 0.5) / count as f64);
                kit::vegetation(scene, self.style, foot, 4.5 + rng.f() * 1.0, Plant::Cypress, rng);
            }
        }
    }

    /// Pinos y olivos en una malla irregular, lejos de los paseos y de la fuente.
    #[allow(clippy::too_many_arguments)]
    /// Pinos en la mitad del fondo y olivos, más bajos, en la de delante: así las
    /// copas grandes no tapan la fuente.
    fn park_trees(&self, scene: &mut Scene, rng: &mut Rng, lawn: &[V3], middle: V3, paths: &[V3]) {
        let away = flat(scene.camera().axes().2);
        let inner = inset(lawn, 2.2);
        if inner.is_empty() {
            return;
        }
        let fair = self.event() == super::events::Event::Fair;
        let step = if fair { 6.0 } else { 4.6 };
        for i in -4..=4 {
            for j in -4..=4 {
                let (ji, jj) = ((rng.f() - 0.5) * 1.2, (rng.f() - 0.5) * 1.2);
                let foot = middle + paths[0] * (i as f64 * step + ji) + paths[1] * (j as f64 * step + jj);
                let clear = paths.iter().all(|dir| (foot - middle).dot(v(-dir.y, dir.x, 0.0)).abs() > 2.2);
                if !clear || (foot - middle).length() < 3.8 || chord(&inner, foot, paths[0]).is_none_or(|(lo, hi)| lo > 0.0 || hi < 0.0) {
                    continue;
                }
                if (foot - middle).dot(away) > 0.0 {
                    let height = if fair { 5.5 + rng.f() } else { 7.0 + rng.f() * 1.5 };
                    if self.avenue.as_ref().is_none_or(|avenue| avenue.canopy_clear(scene, foot, height)) {
                        kit::vegetation(scene, self.style, foot, height, Plant::Pine, rng);
                    }
                } else {
                    let height = if fair { 3.5 + rng.f() * 0.5 } else { 4.0 + rng.f() };
                    if self.avenue.as_ref().is_none_or(|avenue| avenue.canopy_clear(scene, foot, height)) {
                        kit::vegetation(scene, self.style, foot, height, Plant::Olive, rng);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_crowns_keep_the_avenue_crossings_clear() {
        let s = FRAC_1_SQRT_2;
        let avenue = Avenue { normal: v(s, s, 0.0), along: v(-s, s, 0.0) };
        let span = (-90.0, 90.0);
        let crossings = avenue.crossings(span);
        assert!(!crossings.is_empty());
        for crossing in crossings {
            for offset in [-5.4, 0.0, 5.4] {
                assert!(!avenue.tree_clear(crossing + offset, span));
            }
        }
        assert!((-90..90).any(|station| avenue.tree_clear(station as f64, span)));
    }

    #[test]
    fn clip_keeps_the_half_plane_and_chamfer_trims_acute_corners() {
        let square = vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(10.0, 10.0, 0.0), v(0.0, 10.0, 0.0)];
        let triangle = clip(&square, |p| p.x - p.y);
        assert!((area(&triangle) - 50.0).abs() < 1e-9);
        assert_eq!(chamfer(&triangle, 1.0).len(), 5);
        assert!((area(&inset(&square, 1.0)) - 64.0).abs() < 1e-9);
        assert_eq!(chord(&square, v(5.0, 5.0, 0.0), v(1.0, 0.0, 0.0)), Some((-5.0, 5.0)));
    }

    #[test]
    fn avenue_cut_leaves_the_roadway_free_and_half_triangles_beyond() {
        let s = FRAC_1_SQRT_2;
        let avenue = Avenue { normal: v(s, -s, 0.0), along: v(s, s, 0.0) };
        for (a, b) in avenue.cut(v(28.0, -4.0, 0.0), v(60.0, -4.0, 0.0)) {
            for q in [a, b] {
                assert!(avenue.offset(q).abs() >= HALF - 1e-6);
            }
        }
        // Manzana (1, 0): queda medio triángulo del lado de la plaza; la (2, 0),
        // más allá, en chaflán; la plaza no se toca.
        let plot = ccw(avenue.plot(v(32.0, 0.0, 0.0), SET, WALK));
        assert_eq!(plot.len(), 3);
        assert!(area(&plot) > 150.0);
        assert_eq!(ccw(avenue.plot(v(64.0, 0.0, 0.0), SET, WALK)).len(), 5);
        assert!(avenue.cuts(v(64.0, 0.0, 0.0)) && !avenue.cuts(v(0.0, 0.0, 0.0)));
    }
}
