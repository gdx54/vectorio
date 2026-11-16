//! Ciudad en fiesta: manzanas de fachada continua, una plaza con verbena, calles
//! con tráfico y paseantes y, si se quiere, una carrera urbana con vallas.
use crate::{BBox, geom::Rng, scene::{Camera, Scene, V3, FACE_BUDGET, SEGMENT_BUDGET}};
use super::{choice, inks, range, GenContext, GenOutput, Generator, InkPlan, Params, ParamSpec, Show};
use super::kit::{self, v, Band, Crowd, Marking, Vehicle, Wall};
use super::style::{part, Style, Surface};

mod avenue;
mod coast;
mod events;
mod stories;
mod decor;

use avenue::{Avenue, AVENUE_SHIFT};
use coast::{Coast, COASTS};
use events::{Event, Route, EVENTS};

pub struct Festival;

/// Lado de la manzana, paso de la rejilla y retranqueo de la fachada (la acera).
const BLOCK: f64 = 24.0;
const PITCH: f64 = 32.0;
const SET: f64 = 3.0;
/// Planta baja comercial y plantas de vivienda.
const GROUND: f64 = 4.0;
const STOREY: f64 = 3.0;
const RACE_MIN: f64 = -36.0;
const RACE_MAX: f64 = 60.0;

const SIGNS: [&str; 10] = ["PAN", "LIBROS", "MERCADO", "CAFÉ", "FERIA", "FLORES", "BAR", "HELADOS", "FRUTA", "PRENSA"];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Landmark { Tower, Stage, Tank }

impl Landmark {
    /// Los altos no pueden ir entre la cámara y la plaza: la taparían.
    fn tall(self) -> bool {
        self != Landmark::Stage
    }
}

/// Anillo de la manzana respecto a la plaza.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ring {
    /// Alrededor de la plaza, a los lados o detrás.
    Core,
    /// Alrededor de la plaza y delante: edificios bajos para que se vea.
    Front,
    Outer,
}

/// Acabado de una manzana de la periferia: con todo, solo con comercios, volumen
/// sombreado o caja de contorno.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
enum Finish { Box, Bare, Shops, Full }

/// Profundidad de una manzana vecina de la plaza, hacia el fondo de la vista.
fn depth(camera: &Camera, lot: (i32, i32)) -> f64 {
    let away = camera.axes().2;
    lot.0 as f64 * away.x + lot.1 as f64 * away.y
}

fn in_front(camera: &Camera, lot: (i32, i32)) -> bool {
    depth(camera, lot) < -0.2
}

/// Manzanas del núcleo, sin la plaza.
fn core_lots() -> Vec<(i32, i32)> {
    (-1..=1).flat_map(|y| (-1..=1).map(move |x| (x, y))).filter(|lot| *lot != (0, 0)).collect()
}

/// Hitos sorteados sin repetir. Los altos van detrás o a los lados de la plaza; el
/// escenario prefiere ir delante, donde no tapa nada.
fn landmarks(count: usize, camera: &Camera, rng: &mut Rng, blocked: &[(i32, i32)]) -> Vec<((i32, i32), Landmark)> {
    let mut kinds = [Landmark::Tower, Landmark::Stage, Landmark::Tank];
    for i in 0..kinds.len() {
        let j = i + (rng.f() * (kinds.len() - i) as f64) as usize;
        kinds.swap(i, j.min(2));
    }
    let mut free: Vec<(i32, i32)> = core_lots().into_iter().filter(|lot| !blocked.contains(lot)).collect();
    let mut chosen = Vec::new();
    for kind in kinds.into_iter().take(count.min(3)) {
        let wanted = |lot: &(i32, i32)| if kind.tall() { !in_front(camera, *lot) } else { in_front(camera, *lot) };
        let mut options: Vec<usize> = (0..free.len()).filter(|i| wanted(&free[*i])).collect();
        if options.is_empty() && !kind.tall() {
            options = (0..free.len()).collect();
        }
        if options.is_empty() {
            continue;
        }
        let pick = options[((rng.f() * options.len() as f64) as usize).min(options.len() - 1)];
        chosen.push((free.remove(pick), kind));
    }
    chosen
}

fn style(ctx: &GenContext) -> Style {
    Style::new(ctx, 1.1)
}

/// Una calle recta necesita solo dos estaciones: conserva la banda y sus marcas,
/// sin miles de triángulos coplanarios en los tramos que desbordan la hoja.
fn straight_band(a: V3, b: V3, width: f64) -> Band {
    let mut band = Band::new(&[a, b], width).unwrap();
    let last = band.center.len() - 1;
    band.center = vec![band.center[0], band.center[last]];
    band.left = vec![band.left[0], band.left[last]];
    band.right = vec![band.right[0], band.right[last]];
    band.stations = vec![0.0, band.stations[last]];
    band
}

/// La misma luz relativa a la fachada que usa Pueblo blanco.
fn sunlight(yaw: f64, offset: f64) -> V3 {
    let (sy, cy) = yaw.to_radians().sin_cos();
    let facade = if sy < 0.0 { -1.0 } else { 1.0 };
    let side = if cy < 0.0 { -1.0 } else { 1.0 };
    let angle = (90.0 * facade - 25.0 * facade * side + offset).to_radians();
    let height = 35.0f64.to_radians();
    v(angle.cos() * height.cos(), angle.sin() * height.cos(), height.sin())
}

/// Reparte `total` metros en `count` tramos de anchura desigual.
fn split(rng: &mut Rng, total: f64, count: usize) -> Vec<f64> {
    let weights: Vec<f64> = (0..count).map(|_| 0.7 + rng.f() * 0.6).collect();
    let sum: f64 = weights.iter().sum();
    weights.iter().map(|w| total * w / sum).collect()
}

/// Solares de una manzana, en coordenadas locales: una sola crujía de fondo
/// entero o dos crujías con un patio en medio. Los edificios de una crujía van
/// pegados, como en un ensanche.
fn layout(rng: &mut Rng) -> Vec<(f64, f64, f64, f64)> {
    let inner = BLOCK - SET * 2.0;
    let rows = if rng.f() < 0.45 {
        let front = 6.5 + rng.f() * 1.5;
        let back = 6.5 + rng.f() * 1.5;
        vec![(SET, SET + front), (BLOCK - SET - back, BLOCK - SET)]
    } else {
        vec![(SET, BLOCK - SET)]
    };
    let mut lots = Vec::new();
    for (y0, y1) in rows {
        let count = 2 + (rng.f() < 0.5) as usize;
        let mut x = SET;
        for width in split(rng, inner, count) {
            lots.push((x, y0, x + width, y1));
            x += width;
        }
    }
    lots
}

/// La pared da a la calle: está en el borde de la fachada de la manzana.
fn on_street(wall: &Wall, origin: V3) -> bool {
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    if wall.along.x.abs() > 0.5 {
        let y = wall.origin.y - origin.y;
        near(y, SET) || near(y, BLOCK - SET)
    } else {
        let x = wall.origin.x - origin.x;
        near(x, SET) || near(x, BLOCK - SET)
    }
}

/// Guirnalda de banderines entre dos puntos, con su comba.
fn bunting(scene: &mut Scene, a: V3, b: V3, sag: f64) {
    let span = (b - a).length();
    if !span.is_finite() || span < 1.0 {
        return;
    }
    let steps = (span / 0.9).ceil() as usize;
    let point = |t: f64| a + (b - a) * t - v(0.0, 0.0, sag * 4.0 * t * (1.0 - t));
    let rope: Vec<V3> = (0..=steps).map(|k| point(k as f64 / steps as f64)).collect();
    part(scene, "detalle");
    scene.line(&rope);
    if !scene.visible(0.5, 1.2) {
        return;
    }
    part(scene, "acento");
    for k in 0..steps {
        let left = point(k as f64 / steps as f64);
        let right = point((k as f64 + 0.6) / steps as f64);
        let tip = (left + right) * 0.5 - v(0.0, 0.0, 0.55);
        scene.line(&[left, tip, right]);
    }
}

/// Enlosado en retícula que no entra en el pilón de la fuente.
fn paving_around(scene: &mut Scene, min: V3, max: V3, step: f64, center: V3, radius: f64) {
    part(scene, "terreno");
    let mut c = min.x + step;
    while c < max.x - step * 0.3 {
        for (a, b) in skip_circle(c - center.x, min.y, max.y, center.y, radius) {
            scene.line(&[v(c, a, 0.0), v(c, b, 0.0)]);
        }
        c += step;
    }
    let mut c = min.y + step;
    while c < max.y - step * 0.3 {
        for (a, b) in skip_circle(c - center.y, min.x, max.x, center.x, radius) {
            scene.line(&[v(a, c, 0.0), v(b, c, 0.0)]);
        }
        c += step;
    }
}

/// Tramos de la recta a distancia `offset` del centro que quedan fuera del círculo.
fn skip_circle(offset: f64, from: f64, to: f64, center: f64, radius: f64) -> Vec<(f64, f64)> {
    if offset.abs() >= radius {
        return vec![(from, to)];
    }
    let half = (radius * radius - offset * offset).sqrt();
    vec![(from, center - half), (center + half, to)]
}

/// Parte horizontal de un vector, de largo uno.
fn flat(d: V3) -> V3 {
    v(d.x, d.y, 0.0).normalized()
}

/// Sentido de `along` que se lee de izquierda a derecha en la hoja.
fn readable(scene: &Scene, along: V3) -> V3 {
    if along.dot(scene.camera().axes().0) < 0.0 { along * -1.0 } else { along }
}

/// Esquina mínima de la caja entre dos esquinas opuestas cualesquiera.
fn lower(a: V3, b: V3) -> V3 {
    v(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z))
}

/// Esquina máxima de la caja entre dos esquinas opuestas cualesquiera.
fn upper(a: V3, b: V3) -> V3 {
    v(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z))
}

/// Caja entre dos esquinas opuestas cualesquiera.
fn between(scene: &mut Scene, a: V3, b: V3) -> Vec<Vec<V3>> {
    scene.box3(lower(a, b), upper(a, b))
}

/// Los cuatro lados de una manzana: normal hacia fuera y sentido a lo largo.
fn sides() -> [(V3, V3); 4] {
    [
        (v(0.0, -1.0, 0.0), v(1.0, 0.0, 0.0)),
        (v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)),
        (v(0.0, 1.0, 0.0), v(-1.0, 0.0, 0.0)),
        (v(-1.0, 0.0, 0.0), v(0.0, -1.0, 0.0)),
    ]
}

struct City<'a> {
    prm: &'a Params,
    ctx: &'a GenContext,
    style: Style,
    /// Lado que da al mar, si lo hay.
    coast: Option<Coast>,
    /// Avenida diagonal, si la hay.
    avenue: Option<Avenue>,
}

/// Lo que se va llenando al recorrer las manzanas.
struct Town<'s> {
    scene: &'s mut Scene,
    crowd: &'s mut Crowd,
    rng: &'s mut Rng,
    /// Manzana en obras, si la hay.
    site: Option<(i32, i32)>,
    /// Manzana cortada por la avenida que es parque, si la hay.
    park: Option<(i32, i32)>,
}

/// Carriles libres de cruces y pasos de cebra, que se llenan al final si queda presupuesto.
struct Streets {
    lanes: Vec<Band>,
}

impl<'a> City<'a> {
    fn new(prm: &'a Params, ctx: &'a GenContext) -> Self {
        let camera = camera(prm, ctx);
        let coast = Coast::choose(prm.choice("coast"), &camera, prm.flag("circuit"), ctx.seed);
        let avenue = Avenue::choose(prm.flag("avenue") && !prm.flag("circuit"), &camera, coast.as_ref());
        City { prm, ctx, style: style(ctx).expressive(prm), coast, avenue }
    }
}

impl City<'_> {
    fn people(&self) -> f64 {
        self.prm.f64("people")
    }

    fn event(&self) -> Event {
        Event::parse(self.prm.choice("event"))
    }

    fn race_on(&self) -> bool {
        self.prm.flag("circuit")
    }

    fn build(&self, camera: Camera, paper: BBox) -> Scene {
        let mut scene = Scene::new(camera, paper);
        self.style.prepare(&mut scene);
        scene.set_sun(Some(sunlight(camera.yaw_deg, self.prm.f64("sun"))));
        let mut rng = Rng::new(self.ctx.seed);
        let mut crowd = Crowd::new(self.style);
        let (lo, hi) = self.extent(&scene);
        let cut = self.cut_lots(lo, hi);
        let mut landmarks = landmarks(self.prm.int("landmarks"), &camera, &mut rng, &cut);
        if self.prm.choice("district") == "leisure" && !landmarks.iter().any(|(_, kind)| *kind == Landmark::Stage) && rng.f() < 0.75 {
            if let Some((_, kind)) = landmarks.first_mut() {
                *kind = Landmark::Stage;
            }
        }
        let mut streets = self.streets(&mut scene, &mut crowd, &mut rng, lo, hi);
        if let Some(avenue) = &self.avenue {
            self.avenue_road(&mut scene, avenue, &mut streets.lanes);
        }
        if self.event() == Event::Parade {
            let route = Route::of(&scene, v(0.0, 0.0, 0.0));
            streets.lanes.retain(|lane| !route.blocks(lane));
        }
        // La arena y el agua quedan por debajo de la calle: tienen su propio grupo
        // de sombras, que se trama antes de abrir el de la ciudad.
        if let Some(coast) = &self.coast {
            self.coast_kerbs(&mut scene, coast, lo, hi);
            self.shore(&mut scene, &mut crowd, &mut rng, coast);
        }
        // El núcleo y la carrera reciben primero el detalle; la periferia consume lo
        // que queda.
        let cells = self.cells(&scene);
        let mut taken: Vec<(i32, i32)> = landmarks.iter().map(|(lot, _)| *lot).collect();
        taken.extend(&cut);
        let site = self.site(&scene, &mut rng, &cells, &taken);
        let park = self.park_lot(&scene, &cells, &cut);
        let (core, outer): (Vec<_>, Vec<_>) = cells.into_iter().partition(|(x, y)| x.abs() <= 1 && y.abs() <= 1);
        // Las calles ya están: a partir de aquí cada sólido arroja sombra al suelo.
        scene.begin_shadows(0.0, None, self.style.shadow_angle());
        let mut town = Town { scene: &mut scene, crowd: &mut crowd, rng: &mut rng, site, park };
        for cell in core {
            self.cell(&mut town, cell, &landmarks);
        }
        if let Some(avenue) = &self.avenue {
            self.avenue_trees(town.scene, town.rng, avenue);
            self.avenue_crossings(town.scene, town.crowd, town.rng, avenue);
        }
        if let Some(coast) = &self.coast {
            self.waterfront(town.scene, town.crowd, town.rng, coast);
        }
        if self.race_on() {
            self.race(town.scene, town.crowd, town.rng);
        }
        for cell in outer {
            self.cell(&mut town, cell, &landmarks);
        }
        let mut tall: Vec<(i32, i32)> = landmarks.iter().filter(|(_, kind)| *kind == Landmark::Tower).map(|(lot, _)| *lot).collect();
        tall.extend(site);
        self.balloon(&mut scene, &mut rng, &tall);
        self.pigeons(&mut scene, &mut rng);
        self.shadows(&mut scene);
        self.traffic(&mut scene, &mut rng, &streets);
        scene
    }

    /// Trama de las sombras reunidas, con la densidad del control, como en el pueblo.
    fn shadows(&self, scene: &mut Scene) {
        part(scene, "sombra");
        let gap = (self.style.shadow_gap() / self.prm.f64("shadow")).max(self.ctx.pen_mm * 1.3);
        scene.hatch_shadows(gap);
    }

    fn cell(&self, town: &mut Town, (col, row): (i32, i32), landmarks: &[((i32, i32), Landmark)]) {
        let (scene, crowd, rng) = (&mut *town.scene, &mut *town.crowd, &mut *town.rng);
        let origin = v(col as f64 * PITCH, row as f64 * PITCH, 0.0);
        let core = col.abs() <= 1 && row.abs() <= 1;
        let ring = match (core, in_front(scene.camera(), (col, row))) {
            (false, _) => Ring::Outer,
            (true, true) => Ring::Front,
            (true, false) => Ring::Core,
        };
        if (col, row) == (0, 0) {
            self.plaza(scene, crowd, rng, origin);
        } else if let Some((_, kind)) = landmarks.iter().find(|(lot, _)| core && *lot == (col, row)) {
            self.landmark(scene, crowd, rng, origin, *kind);
        } else if town.site == Some((col, row)) && self.room(scene, 0.7) {
            self.sidewalk(scene, crowd, rng, origin);
            self.construction(scene, crowd, rng, origin, (col, row));
        } else if let Some(avenue) = self.avenue.as_ref().filter(|a| a.cuts(origin)) {
            if town.park == Some((col, row)) {
                self.park(scene, crowd, rng, avenue, origin);
            } else {
                self.wedge(scene, crowd, rng, avenue, origin, ring);
            }
        } else {
            self.block(scene, crowd, rng, origin, ring);
        }
    }

    fn cells(&self, scene: &Scene) -> Vec<(i32, i32)> {
        let (lo, hi) = self.extent(scene);
        let mut cells: Vec<_> = (lo..=hi).flat_map(|y| (lo..=hi).map(move |x| (x, y))).filter(|(x, y)| {
            let origin = v(*x as f64 * PITCH, *y as f64 * PITCH, 0.0);
            self.visible(scene, origin) && !self.at_sea(origin + v(BLOCK * 0.5, BLOCK * 0.5, 0.0), 0.0)
        }).collect();
        cells.sort_by_key(|(x, y)| (x.abs().max(y.abs()), *y, *x));
        cells
    }

    /// Más allá de la calle de costa, hacia el mar.
    fn at_sea(&self, point: V3, margin: f64) -> bool {
        self.coast.is_some_and(|coast| coast.beyond(point, margin))
    }

    /// Tramo de calle recortado a tierra firme.
    fn land(&self, a: V3, b: V3) -> Option<(V3, V3)> {
        match &self.coast {
            Some(coast) => coast.keep(a, b),
            None => Some((a, b)),
        }
    }

    fn visible(&self, scene: &Scene, origin: V3) -> bool {
        let mut points = Vec::new();
        for x in [0.0, BLOCK] {
            for y in [0.0, BLOCK] {
                for z in [0.0, 26.0] {
                    points.push(vec![scene.camera().project(origin + v(x, y, z)).0]);
                }
            }
        }
        let bounds = BBox::of(&points);
        let area = self.ctx.area;
        bounds.max.x >= area.min.x && bounds.min.x <= area.max.x && bounds.max.y >= area.min.y && bounds.min.y <= area.max.y
    }

    /// Hay sitio para el detalle: por encima de esta fracción del presupuesto se
    /// dibujan solo volúmenes.
    fn room(&self, scene: &Scene, fraction: f64) -> bool {
        let load = scene.load();
        (load.faces as f64) < FACE_BUDGET as f64 * fraction && (load.segments as f64) < SEGMENT_BUDGET as f64 * fraction
    }

    fn landmark(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3, kind: Landmark) {
        self.sidewalk(scene, crowd, rng, origin);
        match kind {
            Landmark::Tower => kit::glass_tower(scene, self.style, origin + v(7.0, 7.0, 0.0), origin + v(17.0, 17.0, 42.0), 3.5, rng),
            Landmark::Stage => self.stage(scene, crowd, rng, origin),
            Landmark::Tank => self.tank(scene, origin),
        }
    }

    fn stage(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3) {
        part(scene, "contorno");
        let faces = scene.box3(origin + v(3.0, 2.0, 0.0), origin + v(21.0, 6.0, 2.4));
        self.style.shade(scene, &faces, Surface::Wall, "trama");
        kit::sign(scene, origin + v(7.0, 2.0, 0.8), v(1.0, 0.0, 0.0), "FERIA", 1.0);
        for x in [2.0, 14.0] {
            let base = origin + v(x, 16.0, 0.0);
            self.stand_base(scene, base);
            crowd.stands(scene, rng, base + v(0.0, 0.0, 0.5), v(8.0, 0.0, 0.0), v(0.0, 1.1, 0.0), 0.8, 4, self.people());
        }
        crowd.field(scene, rng, origin + v(3.0, 8.0, 0.0), origin + v(21.0, 14.0, 0.0), self.people() * 0.65);
    }

    /// Un solo prisma escalonado sostiene cada grada, sin cajas superpuestas.
    fn stand_base(&self, scene: &mut Scene, base: V3) {
        let mut profile = vec![base];
        for row in 0..4 {
            let y = row as f64 * 1.1;
            let z = 0.5 + row as f64 * 0.8;
            profile.push(base + v(0.0, y, z));
            profile.push(base + v(0.0, y + 1.1, z));
        }
        profile.push(base + v(0.0, 4.4, 0.0));
        part(scene, "contorno");
        scene.prism(&profile, v(8.0, 0.0, 0.0));
    }

    fn tank(&self, scene: &mut Scene, origin: V3) {
        let center = origin + v(12.0, 12.0, 0.0);
        part(scene, "contorno");
        let faces = scene.lathe(center, &[(7.0, 0.0), (7.0, 9.0), (6.5, 10.0), (0.0, 12.0)], true);
        self.style.shade(scene, &faces, Surface::Wall, "trama");
        let side = if scene.camera().yaw_deg.to_radians().sin() < 0.0 { -1.0 } else { 1.0 };
        let start = center + v(side * 3.5, side * 8.0, 3.0);
        let along = v(-side, 0.0, 0.0);
        // Rótulo genérico sobre un cartel con dos patas hasta el suelo.
        let end = start + along * 7.0;
        part(scene, "contorno");
        scene.closed(&[start, end, end + v(0.0, 0.0, 1.8), start + v(0.0, 0.0, 1.8)]);
        for foot in [start + along * 0.4, end - along * 0.4] {
            scene.line(&[v(foot.x, foot.y, 0.0), foot]);
        }
        kit::sign(scene, start + along * 0.3 + v(0.0, 0.0, 0.4), along, "DEPÓSITO", 1.0);
    }

    fn race(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng) {
        for pair in race_path().windows(2) {
            let band = straight_band(pair[0], pair[1], 8.0);
            self.fences(scene, crowd, rng, &band);
            for lane in race_lanes(&band) {
                kit::convoy(scene, &lane, &[Vehicle::Car], 0.0, 9.0, rng);
            }
        }
        let base = v(-14.0, RACE_MIN - 8.0, 0.0);
        self.stand_base(scene, base);
        crowd.stands(scene, rng, base + v(0.0, 0.0, 0.5), v(8.0, 0.0, 0.0), v(0.0, 1.1, 0.0), 0.8, 4, self.people());
        self.finish_line(scene);
    }

    /// Vallas de dos barras con postes cada 2 m y público por fuera.
    fn fences(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, band: &Band) {
        for offset in [-3.6, 3.6] {
            part(scene, "detalle");
            for z in [0.5, 1.0] {
                scene.line(&[band.at(0.0, offset).0 + v(0.0, 0.0, z), band.at(band.length(), offset).0 + v(0.0, 0.0, z)]);
            }
            let mut at = 0.0;
            while at <= band.length() {
                let foot = band.at(at, offset).0;
                scene.line(&[foot, foot + v(0.0, 0.0, 1.0)]);
                at += 2.0;
            }
        }
        // A la derecha del recorrido antihorario queda el exterior del núcleo.
        if self.people() > 0.0 {
            for k in 0..3 {
                let at = 8.0 + k as f64 * PITCH;
                let end = at + 14.0 * self.people();
                crowd.queue(scene, rng, band.at(at, -4.4).0, band.at(end, -4.4).0);
            }
        }
    }

    /// Línea a cuadros y pórtico con una pancarta de META.
    fn finish_line(&self, scene: &mut Scene) {
        part(scene, "detalle");
        for row in 0..2 {
            for k in 0..8 {
                if (row + k) % 2 != 0 {
                    continue;
                }
                let a = v(-20.0 + row as f64 * 0.6, RACE_MIN - 3.2 + k as f64 * 0.8, 0.0);
                scene.closed(&[a, a + v(0.6, 0.0, 0.0), a + v(0.6, 0.8, 0.0), a + v(0.0, 0.8, 0.0)]);
            }
        }
        let (left, right) = (RACE_MIN - 4.8, RACE_MIN + 4.8);
        part(scene, "contorno");
        for y in [left, right] {
            scene.line(&[v(-20.0, y, 0.0), v(-20.0, y, 6.0)]);
        }
        scene.closed(&[v(-20.0, left, 4.2), v(-20.0, right, 4.2), v(-20.0, right, 6.0), v(-20.0, left, 6.0)]);
        kit::sign(scene, v(-20.0, RACE_MIN - 2.6, 4.5), v(0.0, 1.0, 0.0), "META", 1.2);
    }

    /// Las horizontales poseen los cruces; las verticales llegan a sus bordes. Las
    /// calles que cruzan la carrera terminan en la valla.
    fn streets(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, lo: i32, hi: i32) -> Streets {
        let mut lanes = Vec::new();
        let start = lo as f64 * PITCH - 8.0;
        let end = (hi + 1) as f64 * PITCH;
        for row in lo..=hi + 1 {
            let y = row as f64 * PITCH - 4.0;
            for col in lo..=hi {
                let x = col as f64 * PITCH - 4.0;
                self.street_x(scene, crowd, rng, x, y, &mut lanes);
            }
            // Los extremos sobrepasan la última celda visible.
            for (a, b) in [(start, lo as f64 * PITCH - 4.0), ((hi + 1) as f64 * PITCH - 4.0, end)] {
                for (a, b) in self.pieces(v(a, y, 0.0), v(b, y, 0.0)) {
                    kit::ribbon(scene, &self.street_band(a, b), 0.0, Marking::Dashed);
                }
            }
        }
        for col in lo..=hi + 1 {
            let x = col as f64 * PITCH - 4.0;
            for row in lo..=hi {
                let y = row as f64 * PITCH;
                self.street_y(scene, crowd, rng, x, y, &mut lanes);
            }
        }
        // Sin carriles en el agua ni en la calzada de la avenida.
        lanes.retain(|lane| !self.at_sea(lane.at(lane.length() * 0.5, 0.0).0, 4.0));
        if let Some(avenue) = &self.avenue {
            lanes.retain(|lane| !avenue.crosses(lane));
        }
        Streets { lanes }
    }

    #[allow(clippy::too_many_arguments)]
    fn street_x(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, x: f64, y: f64, lanes: &mut Vec<Band>) {
        let race_line = |c: f64| self.race_on() && [RACE_MIN, RACE_MAX].contains(&c);
        let inside = y > RACE_MIN && y < RACE_MAX;
        let race = race_line(y) && (RACE_MIN..RACE_MAX).contains(&x);
        let a = if inside && race_line(x) { x + 3.6 } else { x };
        let b = if inside && race_line(x + PITCH) { x + PITCH - 3.6 } else { x + PITCH };
        let pieces = self.pieces(v(a, y, 0.0), v(b, y, 0.0));
        if pieces.is_empty() {
            return;
        }
        for (start, end) in pieces {
            let band = self.street_band(start, end);
            kit::ribbon(scene, &band, 0.0, if race { Marking::Kerbs } else { Marking::Dashed });
            if !race {
                self.crossing(scene, crowd, rng, &band, 6.3 - (start.x - x));
            }
        }
        if race {
            return;
        }
        let whole = straight_band(v(x, y, 0.0), v(x + PITCH, y, 0.0), 8.0);
        push_lanes(lanes, &whole, 10.0, 26.0);
    }

    #[allow(clippy::too_many_arguments)]
    fn street_y(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, x: f64, y: f64, lanes: &mut Vec<Band>) {
        let race = self.race_on() && [RACE_MIN, RACE_MAX].contains(&x) && (-32.0..RACE_MAX).contains(&y);
        for (start, end) in self.pieces(v(x, y, 0.0), v(x, y + BLOCK, 0.0)) {
            let band = self.street_band(start, end);
            kit::ribbon(scene, &band, 0.0, if race { Marking::Kerbs } else { Marking::Dashed });
            if race {
                continue;
            }
            self.crossing(scene, crowd, rng, &band, 2.3 - (start.y - y));
            if (end - start).length() > BLOCK - 0.1 {
                push_lanes(lanes, &band, 6.5, 22.0);
            }
        }
    }

    /// Tramos de calle en tierra firme y fuera de la calzada de la avenida.
    fn pieces(&self, a: V3, b: V3) -> Vec<(V3, V3)> {
        let Some((a, b)) = self.land(a, b) else { return Vec::new() };
        match &self.avenue {
            Some(avenue) => avenue.cut(a, b),
            None => vec![(a, b)],
        }
    }

    /// Banda de una calle de la cuadrícula; si llega a la avenida, sus bordillos
    /// acaban en oblicuo contra el de ella.
    fn street_band(&self, a: V3, b: V3) -> Band {
        let mut band = straight_band(a, b, 8.0);
        if let Some(avenue) = &self.avenue {
            avenue.trim(&mut band);
        }
        band
    }

    /// Manzanas cortadas por la avenida: ni hitos ni obra.
    fn cut_lots(&self, lo: i32, hi: i32) -> Vec<(i32, i32)> {
        let Some(avenue) = &self.avenue else { return Vec::new() };
        (lo..=hi).flat_map(|y| (lo..=hi).map(move |x| (x, y)))
            .filter(|(x, y)| (*x, *y) != (0, 0) && avenue.cuts(v(*x as f64 * PITCH, *y as f64 * PITCH, 0.0))).collect()
    }

    /// El parque va en la manzana cortada que cae más cerca del centro de la hoja.
    fn park_lot(&self, scene: &Scene, cells: &[(i32, i32)], cut: &[(i32, i32)]) -> Option<(i32, i32)> {
        let middle = self.ctx.area.center();
        cells.iter().copied().filter(|cell| cut.contains(cell)).min_by(|a, b| {
            let distance = |(x, y): (i32, i32)| {
                let p = scene.camera().project(v(x as f64 * PITCH + BLOCK * 0.5, y as f64 * PITCH + BLOCK * 0.5, 0.0)).0;
                (p.x - middle.x).hypot(p.y - middle.y)
            };
            distance(*a).total_cmp(&distance(*b))
        })
    }

    fn extent(&self, scene: &Scene) -> (i32, i32) {
        let area = self.ctx.area;
        let (mut lo, mut hi) = (-1, 1);
        for corner in [area.min, crate::p(area.max.x, area.min.y), area.max, crate::p(area.min.x, area.max.y)] {
            for z in [0.0, 48.0] {
                let ground = scene.camera().ground(corner, z);
                lo = lo.min((ground.x.min(ground.y) / PITCH).floor() as i32 - 1);
                hi = hi.max((ground.x.max(ground.y) / PITCH).floor() as i32 + 1);
            }
        }
        (lo, hi)
    }

    fn on_paper(&self, scene: &Scene, point: V3, margin: f64) -> bool {
        self.ctx.area.contains(scene.camera().project(point).0, margin)
    }

    fn crossing(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, band: &Band, at: f64) {
        if at < 0.0 || at + 2.4 > band.length() {
            return;
        }
        if !self.on_paper(scene, band.at(at, 0.0).0, 8.0) {
            return;
        }
        // Desfase de 0,3 m respecto a las estaciones de la cinta.
        kit::zebra(scene, band, at, 2.4);
        let a = band.at(at + 1.2, -3.8).0;
        let b = band.at(at + 1.2, 3.8).0;
        crowd.crossing(scene, rng, a, b, 1.8, self.people() * 0.5);
    }

    /// Coches, motos y algún autobús en los tramos entre cruces que caen en la
    /// hoja, mientras quede presupuesto.
    fn traffic(&self, scene: &mut Scene, rng: &mut Rng, streets: &Streets) {
        let kinds = [Vehicle::Car, Vehicle::Car, Vehicle::Car, Vehicle::Moto, Vehicle::Bus];
        for lane in &streets.lanes {
            let skip = rng.f() < 0.3;
            let middle = lane.at(lane.length() * 0.5, 0.0).0;
            if skip || !self.on_paper(scene, middle, 6.0) || !self.room(scene, 0.95) {
                continue;
            }
            kit::convoy(scene, lane, &kinds, 0.0, 7.0, rng);
        }
    }

    /// La plaza cambia con el evento; siempre hay vendedores de globos.
    fn plaza(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3) {
        match self.event() {
            Event::Verbena => self.verbena(scene, crowd, rng, origin),
            Event::Market => self.market(scene, crowd, rng, origin),
            Event::Fair => self.fair(scene, crowd, rng, origin),
            Event::Parade => self.parade(scene, crowd, rng, origin),
        }
        self.sellers(scene, rng, origin);
        kit::balloon_bunch(scene, rng, origin + v(1.0, 12.0, 1.0), 4);
    }

    /// Paseantes por las cuatro aceras de la manzana, entre el bordillo y los toldos.
    fn sidewalk(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3) {
        if self.people() <= 0.0 || !self.room(scene, 0.85) {
            return;
        }
        let a = 0.6;
        let b = BLOCK - 0.6;
        let corners = [v(a, a, 0.0), v(b, a, 0.0), v(b, b, 0.0), v(a, b, 0.0)];
        for k in 0..4 {
            let from = origin + corners[k];
            let to = origin + corners[(k + 1) % 4];
            crowd.crossing(scene, rng, from, to, 0.8, self.people() * 0.22);
        }
    }

    fn block(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3, ring: Ring) {
        let finish = self.finish(scene, ring);
        let residential = ring == Ring::Outer && self.prm.choice("district") == "residential";
        let leisure = ring == Ring::Outer && self.prm.choice("district") == "leisure";
        let patio = residential && rng.f() < 0.5;
        let lots = if patio { vec![(3.0, 3.0, 12.0, 9.0), (12.0, 3.0, 21.0, 9.0), (3.0, 15.0, 12.0, 21.0), (12.0, 15.0, 21.0, 21.0)] } else { layout(rng) };
        let heights = self.heights(scene, rng, ring, lots.len());
        for (k, (x0, y0, x1, y1)) in lots.into_iter().enumerate() {
            let min = origin + v(x0, y0, 0.0);
            let max = v(origin.x + x1, origin.y + y1, heights[k]);
            if finish == Finish::Box {
                part(scene, "contorno");
                scene.box3(min, max);
                continue;
            }
            kit::house_bare(scene, self.style, min, max, rng);
            if finish >= Finish::Shops {
                self.facades(scene, crowd, rng, origin, min, max, ring, finish == Finish::Full);
            }
            let roomy = x1 - x0 > 6.0 && y1 - y0 > 6.0;
            if (ring == Ring::Core || (leisure && finish == Finish::Full)) && roomy && k == 0 {
                self.rooftop(scene, crowd, rng, min, max);
            } else if finish == Finish::Full {
                let inset = self.style.quantum(scene) + 2.0 / scene.camera().scale;
                if residential && (max.x-min.x).min(max.y-min.y) > inset * 2.0 + 1.0 {
                    kit::laundry(scene, v(min.x + inset, min.y + inset, max.z), v(max.x - inset, min.y + inset, max.z), 1.8, rng);
                }
                if !residential {
                    self.laundry(scene, rng, min, max);
                }
            }
        }
        if patio && finish == Finish::Full {
            kit::vegetation(scene, self.style, origin + v(12.0, 12.0, 0.0), 3.5, kit::Plant::Olive, rng);
        }
        if finish == Finish::Full {
            self.parked_bikes(scene, origin);
            self.sidewalk(scene, crowd, rng, origin);
        }
    }

    /// El núcleo va siempre completo; la periferia pierde detalle a medida que se
    /// gasta el presupuesto, y deja margen para el tráfico, que va al final.
    fn finish(&self, scene: &Scene, ring: Ring) -> Finish {
        if ring != Ring::Outer || self.room(scene, 0.70) {
            Finish::Full
        } else if self.room(scene, 0.80) {
            Finish::Shops
        } else if self.room(scene, 0.88) {
            Finish::Bare
        } else {
            Finish::Box
        }
    }

    /// Alturas por solar: bajas delante de la plaza, más altas en el núcleo.
    fn heights(&self, scene: &Scene, rng: &mut Rng, ring: Ring, count: usize) -> Vec<f64> {
        let (low, spread) = match ring {
            Ring::Front => (2, 2),
            Ring::Core => (3, 5),
            Ring::Outer => (2, 4),
        };
        (0..count).map(|k| {
            let floors = if ring == Ring::Outer && self.prm.choice("district") == "residential" { 2 + (rng.f() < 0.2) as usize } else { low + ((rng.f() * (spread as f64*0.6)) as usize + k%2).min(spread-1) };
            self.style.snap(scene, GROUND + STOREY * (floors - 1) as f64 + 0.8)
        }).collect()
    }

    /// Fachadas a la calle: ventanas, balcones, uno o dos comercios con toldo y,
    /// en el núcleo, cola delante del primero.
    #[allow(clippy::too_many_arguments)]
    fn facades(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, origin: V3, min: V3, max: V3, ring: Ring, detailed: bool) {
        let rhythm = kit::pick(rng, &kit::RHYTHMS);
        let balconies = detailed && rhythm != kit::Rhythm::Ribbon && rng.f() < if ring == Ring::Outer && self.prm.choice("district") == "residential" { 0.8 } else { 0.35 };
        for wall in kit::walls(min, max) {
            if !wall.faces_camera(scene) || !on_street(&wall, origin) {
                continue;
            }
            if detailed {
                let floors = kit::window_grid(scene, self.style, wall, GROUND, STOREY, rhythm);
                if balconies {
                    for level in 1..floors {
                        let z = GROUND + STOREY * level as f64;
                        kit::balconies(scene, self.style, wall, z, rhythm, 0.9, crowd, rng);
                    }
                } else {
                    self.painter(scene, rng, wall, floors, max.z);
                }
            }
            let residential = ring == Ring::Outer && self.prm.choice("district") == "residential";
            if !residential || rng.f() < 0.18 {
                self.shops(scene, crowd, rng, wall, ring != Ring::Outer);
            }
            if ring == Ring::Outer && self.prm.choice("district") == "leisure" && detailed && self.room(scene, 0.8) {
                self.cafe_tables(scene, crowd, rng, wall);
            }
        }
    }

    fn shops(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, wall: Wall, queue: bool) {
        let count = if wall.width >= 12.0 { 2 } else { 1 };
        let slot = wall.width / count as f64;
        for k in 0..count {
            let width = (4.0 + rng.f() * 3.0).min(slot - 1.0);
            let u = k as f64 * slot + 0.5 + rng.f() * (slot - width - 1.0).max(0.0);
            let label = if !queue && self.prm.choice("district") == "leisure" { kit::pick(rng, &["MÚSICA", "CAFÉ", "TERRAZA"]) } else { kit::pick(rng, &SIGNS) };
            let awning = rng.f() < 0.75;
            kit::shopfront(scene, self.style, wall, u, width, 3.6, Some(label), awning);
            if queue && k == 0 && self.people() > 0.0 {
                let from = wall.at(u + 0.4, 0.0, 1.5);
                let to = wall.at(u + 0.4 + (width - 0.8) * self.people(), 0.0, 1.5);
                crowd.queue(scene, rng, from, to);
            }
        }
    }

    fn rooftop(&self, scene: &mut Scene, crowd: &mut Crowd, rng: &mut Rng, min: V3, max: V3) {
        let limit = crowd.limit;
        crowd.limit = limit.min(crowd.drawn + (11.0 * self.people()) as usize);
        kit::roof_terrace(scene, v(min.x, min.y, max.z), max, crowd, rng);
        crowd.limit = limit;
        self.roof_flag(scene, min, max);
    }
}

/// Dos carriles, uno por sentido, entre `from` y `to` metros de la banda.
fn push_lanes(lanes: &mut Vec<Band>, band: &Band, from: f64, to: f64) {
    for offset in [-2.1, 2.1] {
        let a = band.at(from, offset).0;
        let b = band.at(to, offset).0;
        let (a, b) = if offset < 0.0 { (b, a) } else { (a, b) };
        lanes.push(straight_band(a, b, 3.0));
    }
}

fn race_path() -> [V3; 5] {
    [v(RACE_MIN, RACE_MIN, 0.0), v(RACE_MAX, RACE_MIN, 0.0), v(RACE_MAX, RACE_MAX, 0.0), v(RACE_MIN, RACE_MAX, 0.0), v(RACE_MIN, RACE_MIN, 0.0)]
}

/// Cada tramo deja seis metros libres antes y después de todos los cruces.
fn race_lanes(band: &Band) -> Vec<Band> {
    (0..3).map(|k| {
        let at = k as f64 * PITCH;
        straight_band(band.at(at + 6.0, 0.0).0, band.at(at + PITCH - 6.0, 0.0).0, 8.0)
    }).collect()
}

/// Con costa, la vista se corre hacia el mar para que se vea la orilla.
const COAST_SHIFT: f64 = 30.0;

fn camera(prm: &Params, ctx: &GenContext) -> Camera {
    let mut camera = Camera {
        yaw_deg: prm.choice("yaw").parse().unwrap_or(225.0),
        scale: (ctx.area.width().min(ctx.area.height()) / 70.0).clamp(2.4, 4.0),
        world_center: v(12.0, 12.0, 0.0),
        paper_center: ctx.area.center(),
        ..Default::default()
    };
    let coast = Coast::choose(prm.choice("coast"), &camera, prm.flag("circuit"), ctx.seed);
    if let Some(coast) = &coast {
        camera.world_center = camera.world_center + coast.out * if prm.flag("avenue") && !prm.flag("circuit") { 24.0 } else { COAST_SHIFT };
    }
    if let Some(avenue) = Avenue::choose(prm.flag("avenue") && !prm.flag("circuit"), &camera, coast.as_ref()) {
        camera.world_center = camera.world_center + avenue.normal * if coast.is_some() { 12.0 } else { AVENUE_SHIFT };
    }
    if coast.is_some() && prm.flag("avenue") && !prm.flag("circuit") {
        camera.scale *= 0.9;
    }
    camera
}

fn scene(prm: &Params, ctx: &GenContext) -> Scene {
    City::new(prm, ctx).build(camera(prm, ctx), ctx.area)
}

impl Generator for Festival {
    fn id(&self) -> &'static str { "festival_city" }
    fn name(&self) -> &'static str { "Ciudad en fiesta" }
    fn category(&self) -> &'static str { "Escenas" }
    fn description(&self) -> &'static str { "Manzanas comerciales, multitudes y tráfico alrededor de una plaza en fiesta." }
    fn inks(&self) -> &'static InkPlan { &inks::DENSE }
    fn params(&self) -> Vec<ParamSpec> {
        vec![
            range("people", "Gente", 0.0, 1.0, 0.1, 0.6),
            ParamSpec::Toggle { id: "signs", label: "Mostrar rótulos", default: true, random: None, show: None,
                hint: Some("Textos de tiendas, puestos, recintos y meta") },
            choice("district", "Distrito", &[("commercial", "Comercial"), ("residential", "Residencial"), ("leisure", "Ocio")], "commercial"),
            range("landmarks", "Hitos", 1.0, 3.0, 1.0, 2.0),
            ParamSpec::Choice { id: "event", label: "Evento", options: EVENTS, default: "verbena", random: true, show: None,
                hint: Some("Lo que pasa en la plaza") },
            range("stories", "Vida en la calle", 0.0, 1.0, 0.1, 0.6),
            ParamSpec::Toggle { id: "circuit", label: "Carrera urbana", default: true, random: Some(0.7), show: None, hint: None },
            ParamSpec::Choice { id: "coast", label: "Costa", options: COASTS, default: "none", random: true, show: None,
                hint: Some("Uno de los lados delanteros da al mar") },
            ParamSpec::Toggle { id: "avenue", label: "Avenida diagonal", default: false, random: Some(0.6),
                show: Some(Show { id: "circuit", equals: "false" }), hint: Some("Cruza la cuadrícula con edificios en cuña y un parque") },
            choice("yaw", "Vista", &[("225", "Frontal"), ("315", "Lateral"), ("45", "Posterior"), ("135", "Cuarta vista")], "225"),
            ParamSpec::Range { id: "sun", label: "Giro del sol", min: -90.0, max: 90.0, step: 5.0, default: 0.0,
                random: Some((-30.0, 30.0)), show: None,
                hint: Some("Relativo a la luz de serie, que deja blancas las fachadas de la calle en cualquier vista") },
            ParamSpec::Range { id: "shadow", label: "Densidad de sombras", min: 0.3, max: 3.0, step: 0.1, default: 1.0,
                random: Some((0.5, 2.0)), show: None, hint: Some("Más alto, líneas de sombra más juntas; más bajo, más abiertas") },
            super::style::look_param(), super::style::richness_param(),
            ParamSpec::Version { current: 1 },
        ]
    }
    fn has_volume(&self)->bool{true}
    fn scene(&self,params:&Params,ctx:&GenContext)->Result<Scene,String>{
        if !ctx.valid(){return Err("Área o rotulador no válido".into());}
        let prm=params.normalized(&self.params());
        Ok(scene(&prm,ctx))
    }
    fn generate(&self, params: &Params, ctx: &GenContext) -> Result<GenOutput, String> {
        if !ctx.valid() {
            return Err("Área o rotulador no válido".into());
        }
        let prm = params.normalized(&self.params());
        let scene = scene(&prm, ctx);
        let mut output = style(ctx).expressive(&prm).finish(&scene);
        if !prm.flag("signs") {
            output.parts.retain(|part| part.name != "rótulos");
        }
        if cfg!(any(test, debug_assertions)) {
            let issues = scene.construction_issues(1.5);
            if !issues.is_empty() {
                output.warning = Some(issues.join("; "));
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{p, generate::Detail};

    fn context(seed: u64) -> GenContext {
        GenContext { area: BBox { min: p(15.0, 15.0), max: p(195.0, 282.0) }, seed, pen_mm: 0.3, detail: Detail::Final }
    }

    const YAWS: [&str; 4] = ["225", "315", "45", "135"];

    fn view(yaw: &str) -> Camera {
        let mut prm = Params::default().normalized(&Festival.params());
        prm.set("yaw", super::super::ParamValue::Text(yaw.into()));
        camera(&prm, &context(0))
    }

    #[test]
    fn hiding_signs_removes_only_lettering_in_every_event_and_coast() {
        for (event, _) in EVENTS {
            for (coast, _) in COASTS {
                let ctx = context(4);
                let mut prm = Params::default().normalized(&Festival.params());
                prm.set("event", super::super::ParamValue::Text((*event).into()));
                prm.set("coast", super::super::ParamValue::Text((*coast).into()));
                let mut expected = Festival.generate(&prm, &ctx).unwrap();
                assert!(expected.parts.iter().any(|part| part.name == "rótulos" && !part.lines.is_empty()));
                expected.parts.retain(|part| part.name != "rótulos");
                prm.set("signs", super::super::ParamValue::Bool(false));
                let hidden = Festival.generate(&prm, &ctx).unwrap();
                assert_eq!(hidden, expected, "evento {event}, costa {coast}");
                assert!(hidden.parts.iter().any(|part| part.name == "detalle" && !part.lines.is_empty()));
            }
        }
    }

    #[test]
    fn festival_is_deterministic() {
        let ctx = context(7);
        assert_eq!(Festival.generate(&Params::default(), &ctx), Festival.generate(&Params::default(), &ctx));
    }

    #[test]
    fn festival_seeds_are_clean_finite_and_within_budget() {
        for count in 1..=3 {
            for circuit in [false, true] {
                for random in [false, true] {
                    for district in ["commercial", "residential", "leisure"] {
                        check_seeds(count, circuit, random, district);
                    }
                }
            }
        }
    }

    fn check_seeds(count: usize, circuit: bool, random: bool, district: &str) {
        for seed in 0..12 {
            let mut prm = Params::default().normalized(&Festival.params());
            if random {
                Festival.randomize(&mut Rng::new(seed), &mut prm, &std::collections::BTreeSet::new());
            }
            prm.set("district", super::super::ParamValue::Text(district.into()));
            prm.set("landmarks", super::super::ParamValue::Number(count as f64));
            prm.set("circuit", super::super::ParamValue::Bool(circuit));
            let ctx = context(seed);
            let scene = scene(&prm, &ctx);
            let issues = scene.construction_issues(1.5);
            assert!(issues.is_empty(), "hitos {count}, circuito {circuit}, semilla {seed}: {issues:?}");
            assert!(scene.load().within_budget(), "hitos {count}, circuito {circuit}, semilla {seed}: {:?}", scene.load());
            let output = style(&ctx).finish(&scene);
            assert!(output.total_m() > 0.1 && output.total_m() < 80.0, "semilla {seed}: {} m", output.total_m());
            for q in output.parts.iter().flat_map(|part| &part.lines).flatten() {
                assert!(q.x.is_finite() && q.y.is_finite());
                assert!(ctx.area.contains(*q, 1e-7), "semilla {seed}: {q:?}");
            }
        }
    }

    #[test]
    fn festival_landmarks_never_repeat_nor_hide_the_square() {
        for yaw in YAWS {
            let camera = view(yaw);
            for seed in 0..24 {
                let chosen = landmarks(3, &camera, &mut Rng::new(seed), &[]);
                assert_eq!(chosen.len(), 3, "vista {yaw}, semilla {seed}");
                for (i, (lot, kind)) in chosen.iter().enumerate() {
                    assert_ne!(*lot, (0, 0));
                    assert!(lot.0.abs() <= 1 && lot.1.abs() <= 1);
                    assert!(!kind.tall() || !in_front(&camera, *lot), "vista {yaw}: {kind:?} en {lot:?}");
                    assert!(chosen[..i].iter().all(|(other, landmark)| other != lot && landmark != kind));
                }
            }
        }
    }

    #[test]
    fn festival_front_lots_are_the_three_nearest_the_camera() {
        for yaw in YAWS {
            let camera = view(yaw);
            let front: Vec<_> = core_lots().into_iter().filter(|lot| in_front(&camera, *lot)).collect();
            assert_eq!(front.len(), 3, "vista {yaw}: {front:?}");
            // La manzana de la esquina delantera es la más cercana a la cámara.
            let nearest = core_lots().into_iter().min_by(|a, b| depth(&camera, *a).total_cmp(&depth(&camera, *b))).unwrap();
            assert!(front.contains(&nearest));
            assert_eq!(nearest.0.abs() + nearest.1.abs(), 2);
        }
    }

    #[test]
    fn festival_block_layout_fills_the_frontage_without_gaps_or_overlaps() {
        for seed in 0..40 {
            let lots = layout(&mut Rng::new(seed));
            let mut rows: Vec<(f64, f64)> = lots.iter().map(|l| (l.1, l.3)).collect();
            rows.dedup();
            assert!(rows.len() == 1 || rows.len() == 2);
            for (y0, y1) in &rows {
                let mut row: Vec<_> = lots.iter().filter(|l| l.1 == *y0 && l.3 == *y1).collect();
                row.sort_by(|a, b| a.0.total_cmp(&b.0));
                assert!((row[0].0 - SET).abs() < 1e-9);
                assert!((row[row.len() - 1].2 - (BLOCK - SET)).abs() < 1e-9);
                for pair in row.windows(2) {
                    assert!((pair[0].2 - pair[1].0).abs() < 1e-9, "semilla {seed}: los edificios no están pegados");
                }
                assert!(row.iter().all(|l| l.2 - l.0 > 3.5), "semilla {seed}: solar demasiado estrecho");
            }
            if rows.len() == 2 {
                assert!(rows[1].0 - rows[0].1 >= 2.0 - 1e-9, "semilla {seed}: patio sin hueco");
            }
        }
    }

    #[test]
    fn festival_only_street_walls_count_as_frontage() {
        let origin = v(32.0, -32.0, 0.0);
        let min = origin + v(SET, SET, 0.0);
        let max = origin + v(10.0, 10.0, 12.0);
        let street: Vec<bool> = kit::walls(min, max).iter().map(|wall| on_street(wall, origin)).collect();
        // Esquina delantera: dan a la calle las paredes en x e y mínimas.
        assert_eq!(street.iter().filter(|s| **s).count(), 2);
        for wall in kit::walls(min, max) {
            let plane = if wall.along.x.abs() > 0.5 { wall.origin.y } else { wall.origin.x };
            let street_plane = if wall.along.x.abs() > 0.5 { origin.y + SET } else { origin.x + SET };
            assert_eq!(on_street(&wall, origin), (plane - street_plane).abs() < 1e-9);
        }
    }

    #[test]
    fn festival_paving_leaves_the_fountain_basin_clear() {
        assert_eq!(skip_circle(3.0, 0.0, 24.0, 12.0, 2.6), vec![(0.0, 24.0)]);
        let pieces = skip_circle(0.0, 0.0, 24.0, 12.0, 2.6);
        assert_eq!(pieces, vec![(0.0, 9.4), (14.6, 24.0)]);
    }

    #[test]
    fn festival_all_views_and_people_extremes_are_clean() {
        for yaw in YAWS {
            let mut counts = Vec::new();
            for people in [0.0, 1.0] {
                let ctx = context(0);
                let mut prm = Params::default().normalized(&Festival.params());
                prm.set("yaw", super::super::ParamValue::Text(yaw.into()));
                prm.set("people", super::super::ParamValue::Number(people));
                prm.set("landmarks", super::super::ParamValue::Number(3.0));
                let scene = scene(&prm, &ctx);
                assert!(scene.construction_issues(1.5).is_empty(), "vista {yaw}, gente {people}");
                assert!(scene.load().within_budget(), "vista {yaw}, gente {people}: {:?}", scene.load());
                let out = Festival.generate(&prm, &ctx).unwrap();
                assert!(out.warning.is_none());
                let lines = out.parts.iter().filter(|part| part.name == "gente").map(|part| part.lines.len()).sum::<usize>();
                counts.push(lines);
            }
            // Las motos conservan su piloto incluso con las calles sin peatones.
            assert!(counts[1] > counts[0], "vista {yaw}: {counts:?}");
        }
    }

    #[test]
    fn festival_avenue_has_visible_vegetation_near_its_axis_in_every_view() {
        for yaw in YAWS {
            for (coast, _) in COASTS {
                let ctx = context(4);
                let mut prm = Params::default().normalized(&Festival.params());
                prm.set("circuit", super::super::ParamValue::Bool(false));
                prm.set("avenue", super::super::ParamValue::Bool(true));
                prm.set("coast", super::super::ParamValue::Text((*coast).into()));
                prm.set("yaw", super::super::ParamValue::Text(yaw.into()));
                let city = City::new(&prm, &ctx);
                let cam = camera(&prm, &ctx);
                let mut scene = Scene::new(cam, ctx.area);
                let avenue = city.avenue.as_ref().unwrap();
                city.avenue_trees(&mut scene, &mut Rng::new(4), avenue);
                let output = scene.render();
                let near = output.parts.iter().filter(|part| part.name == "vegetación").flat_map(|part| &part.lines).flatten().filter(|q| avenue.offset(cam.ground(**q, 3.5)).abs() < 8.0).count();
                assert!(near > 4, "vista {yaw}, costa {coast}: {near}");
            }
        }
    }

    #[test]
    fn festival_coasts_and_avenue_are_clean_in_every_view() {
        for (coast, _) in COASTS {
            for avenue in [false, true] {
                for yaw in YAWS {
                    for sample in 0..9 {
                        let seed = sample / 3;
                        let district = ["commercial", "residential", "leisure"][(sample % 3) as usize];
                        let ctx = context(seed);
                        let mut prm = Params::default().normalized(&Festival.params());
                        prm.set("district", super::super::ParamValue::Text(district.into()));
                        prm.set("coast", super::super::ParamValue::Text((*coast).into()));
                        prm.set("avenue", super::super::ParamValue::Bool(avenue));
                        prm.set("circuit", super::super::ParamValue::Bool(false));
                        prm.set("yaw", super::super::ParamValue::Text(yaw.into()));
                        prm.set("people", super::super::ParamValue::Number(1.0));
                        let scene = scene(&prm, &ctx);
                        let issues = scene.construction_issues(1.5);
                        assert!(issues.is_empty(), "{coast}, avenida {avenue}, vista {yaw}, semilla {seed}: {issues:?}");
                        assert!(scene.load().within_budget(), "{coast}, avenida {avenue}, vista {yaw}, semilla {seed}: {:?}", scene.load());
                        let out = Festival.generate(&prm, &ctx).unwrap();
                        for q in out.parts.iter().flat_map(|part| &part.lines).flatten() {
                            assert!(ctx.area.contains(*q, 1e-7), "{coast}, avenida {avenue}, vista {yaw}, semilla {seed}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn festival_every_event_with_full_street_life_is_clean_in_every_view() {
        for (event, _) in EVENTS {
            for yaw in YAWS {
                for seed in 0..4 {
                    let ctx = context(seed);
                    let mut prm = Params::default().normalized(&Festival.params());
                    prm.set("event", super::super::ParamValue::Text((*event).into()));
                    prm.set("yaw", super::super::ParamValue::Text(yaw.into()));
                    prm.set("people", super::super::ParamValue::Number(1.0));
                    prm.set("stories", super::super::ParamValue::Number(1.0));
                    prm.set("landmarks", super::super::ParamValue::Number(3.0));
                    let scene = scene(&prm, &ctx);
                    let issues = scene.construction_issues(1.5);
                    assert!(issues.is_empty(), "{event}, vista {yaw}, semilla {seed}: {issues:?}");
                    assert!(scene.load().within_budget(), "{event}, vista {yaw}, semilla {seed}: {:?}", scene.load());
                }
            }
        }
    }

    #[test]
    fn festival_parade_takes_the_square_street_nearest_the_camera_and_stops_its_traffic() {
        for yaw in YAWS {
            let ctx = context(0);
            let mut scene = Scene::new(view(yaw), ctx.area);
            let route = Route::of(&scene, v(0.0, 0.0, 0.0));
            let away = flat(scene.camera().axes().2);
            assert!(route.out.dot(away) < 0.0, "vista {yaw}");
            let mut prm = Params::default().normalized(&Festival.params());
            prm.set("yaw", super::super::ParamValue::Text(yaw.into()));
            let city = City::new(&prm, &ctx);
            let mut crowd = Crowd::new(city.style);
            let streets = city.streets(&mut scene, &mut crowd, &mut Rng::new(0), -2, 2);
            let blocked = streets.lanes.iter().filter(|lane| route.blocks(lane)).count();
            assert_eq!(blocked, 2, "vista {yaw}: los dos carriles del tramo");
        }
    }

    #[test]
    fn festival_full_street_life_finds_a_free_building_site_that_does_not_hide_the_square() {
        for yaw in YAWS {
            let ctx = context(5);
            let mut prm = Params::default().normalized(&Festival.params());
            prm.set("yaw", super::super::ParamValue::Text(yaw.into()));
            prm.set("stories", super::super::ParamValue::Number(1.0));
            let city = City::new(&prm, &ctx);
            let scene = Scene::new(view(yaw), ctx.area);
            let cells = city.cells(&scene);
            let taken = [(1, 0), (0, 1)];
            let site = city.site(&scene, &mut Rng::new(1), &cells, &taken).expect("obra");
            assert_eq!(site.0.abs().max(site.1.abs()), 1);
            assert!(!taken.contains(&site) && depth(scene.camera(), site) > -0.9, "vista {yaw}: {site:?}");
        }
    }

    #[test]
    fn festival_square_has_bunting_and_traffic_reaches_the_outskirts() {
        let ctx = context(3);
        let prm = Params::default().normalized(&Festival.params());
        let out = Festival.generate(&prm, &ctx).unwrap();
        let accent = out.parts.iter().filter(|part| part.name == "acento").map(|part| part.lines.len()).sum::<usize>();
        assert!(accent > 50, "banderines: {accent}");
        let city = City::new(&prm, &ctx);
        let mut scene = Scene::new(camera(&prm, &ctx), ctx.area);
        let (lo, hi) = city.extent(&scene);
        let streets = city.streets(&mut scene, &mut Crowd::new(city.style), &mut Rng::new(0), lo, hi);
        // Hay carriles fuera del núcleo, no solo en las dos calles de la plaza.
        let far = streets.lanes.iter().filter(|lane| lane.center[0].x.abs().max(lane.center[0].y.abs()) > 50.0).count();
        assert!(far > 10, "carriles de la periferia: {far}");
    }

    #[test]
    fn festival_camera_and_populated_cells_cover_the_paper() {
        for yaw in YAWS {
            for area in [context(0).area, BBox { min: p(5.0, 5.0), max: p(195.0, 195.0) }] {
                let mut ctx = context(0);
                ctx.area = area;
                let mut prm = Params::default().normalized(&Festival.params());
                prm.set("yaw", super::super::ParamValue::Text(yaw.into()));
                let cam = camera(&prm, &ctx);
                assert!((2.4..=4.0).contains(&cam.scale));
                assert_eq!(cam.world_center, v(12.0, 12.0, 0.0));
                let scene = Scene::new(cam, area);
                let city = City::new(&prm, &ctx);
                let cells = city.cells(&scene);
                for row in 0..20 {
                    for col in 0..20 {
                        let paper = p(area.min.x + area.width() * (col as f64 + 0.5) / 20.0, area.min.y + area.height() * (row as f64 + 0.5) / 20.0);
                        let ground = cam.ground(paper, 0.0);
                        let street = ground.x.rem_euclid(PITCH) >= BLOCK || ground.y.rem_euclid(PITCH) >= BLOCK;
                        let cell = ((ground.x / PITCH).floor() as i32, (ground.y / PITCH).floor() as i32);
                        assert!(street || cells.contains(&cell), "vista {yaw}, papel {paper:?}, celda {cell:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn festival_race_follows_streets_and_cars_keep_crossings_clear() {
        let path = race_path();
        assert_eq!(path.first(), path.last());
        for side in path.windows(2) {
            assert!(side[0].z == 0.0 && side[1].z == 0.0);
            assert!(side[0].x == side[1].x || side[0].y == side[1].y);
            for q in side {
                assert!(((q.x + 4.0) / PITCH).fract().abs() < 1e-8);
                assert!(((q.y + 4.0) / PITCH).fract().abs() < 1e-8);
            }
            let band = straight_band(side[0], side[1], 8.0);
            for lane in race_lanes(&band) {
                for k in 0..=20 {
                    let q = lane.at(lane.length() * k as f64 / 20.0, 0.0).0;
                    for cross in 0..=3 {
                        assert!((q - band.at(cross as f64 * PITCH, 0.0).0).length() >= 6.0 - 1e-8);
                    }
                }
            }
        }
    }

    #[test]
    #[ignore]
    fn festival_probe_loads() {
        for seed in 0..12 {
            let ctx = GenContext { area: BBox { min: p(5.0, 5.0), max: p(195.0, 195.0) }, seed, pen_mm: 0.3, detail: Detail::Final };
            let mut prm = Params::default().normalized(&Festival.params());
            Festival.randomize(&mut Rng::new(seed), &mut prm, &std::collections::BTreeSet::new());
            let s = scene(&prm, &ctx);
            eprintln!("PROBE {seed} {:?} yaw {}", s.load(), prm.choice("yaw"));
        }
    }
}
