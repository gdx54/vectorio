//! Preparación de salida sin driver: huella física, secado local y recorrido temporal.
//! El mapa acumulado sigue el criterio de Linear (centro de celda y reentradas).
//! El plan de secado es conservador: reserva cada huella hasta secar desde el final
//! del trazo. Conserva todos los segmentos y el orden de las tintas.

use crate::machine::plan::{self, Kinematics, Limits};
use crate::{geom::length, p, Paper, Pen, PlotLayer, PlotSettings, Polyline, P};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};

/// Cómo se mueve la máquina (sale de sus ajustes): con esto la estimación cuenta lo que
/// tarda de verdad en acelerar, frenar y tomar esquinas, no solo la velocidad de crucero.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dynamics {
    /// Aceleración con la pluma abajo y arriba, mm/s².
    pub accel: f64,
    pub travel_accel: f64,
    pub junction_mm: f64,
    pub max_speed: f64,
    pub steps_per_mm: f64,
}

impl Default for Dynamics {
    fn default() -> Self { crate::machine::MachineSettings::default().dynamics() }
}

impl Dynamics {
    fn valid(&self) -> bool {
        [self.accel, self.travel_accel, self.max_speed, self.steps_per_mm].iter().all(|v| v.is_finite() && *v > 0.0)
            && self.junction_mm.is_finite() && self.junction_mm >= 0.0
    }
}

/// Tiempos de la máquina, con el mismo planificador de movimiento que usa al trazar.
pub(crate) struct Timing { draw: Limits, travel: Limits, kin: Kinematics, pub lift: f64 }

impl Timing {
    pub(crate) fn new(motion: &PlotSettings, d: &Dynamics) -> Self {
        let draw = Limits { speed: motion.draw_speed.min(d.max_speed), accel: d.accel, junction_mm: d.junction_mm };
        Timing { draw, travel: Limits { speed: motion.travel_speed.min(d.max_speed), accel: d.travel_accel, ..draw },
            kin: Kinematics { steps_per_mm: d.steps_per_mm }, lift: motion.lift_time }
    }
    /// Viaje de `a` a `b`, s; 0 si los motores ya están ahí (la máquina ni levanta la pluma).
    pub(crate) fn hop(&self, a: P, b: P) -> f64 {
        if self.kin.motors(a) == self.kin.motors(b) { 0.0 } else { plan::duration(&[a, b], &self.travel) }
    }
    pub(crate) fn draw(&self, path: &[P]) -> f64 { plan::duration(path, &self.draw) }
}

/// Motivo por el que no se pudo preparar la salida. El texto para el usuario lo pone la interfaz.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputError {
    InvalidSettings,
    MissingPen,
    InvalidPenWidth,
    InvalidCoordinates,
    InvalidLength,
    /// Se pidió interrumpir el cálculo porque ya no hace falta.
    Cancelled,
}

fn check(cancel: &AtomicBool) -> Result<(), OutputError> {
    if cancel.load(Ordering::Relaxed) { Err(OutputError::Cancelled) } else { Ok(()) }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputSettings {
    pub drying: bool,
    pub reorder: bool,
    pub dry_seconds: f64,
    pub max_wet_passes: u16,
    pub soak_mm: f64,
    pub cell_mm: f64,
    pub chunk_mm: f64,
    pub pen_change_seconds: f64,
    /// Trazar los objetos en el orden del lienzo (de abajo arriba) en vez de una
    /// tinta tras otra en el orden de la paleta.
    pub canvas_order: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self { drying: true, reorder: true, dry_seconds: 20.0, max_wet_passes: 3,
            soak_mm: 0.3, cell_mm: 0.5, chunk_mm: 30.0, pen_change_seconds: 30.0, canvas_order: false }
    }
}

pub const HEAT_COLORS: [[u8; 4]; 6] = [
    [150, 150, 150, 90], [250, 204, 21, 255], [249, 140, 22, 255],
    [234, 72, 30, 255], [190, 18, 60, 255], [88, 28, 135, 255],
];
pub const HEAT_LABELS: [&str; 6] = ["1", "2", "3", "4–5", "6–9", "10+"];
pub fn heat_level(n: u16) -> usize {
    match n { 0..=1 => 0, 2 => 1, 3 => 2, 4..=5 => 3, 6..=9 => 4, _ => 5 }
}

#[derive(Clone, Debug, Default)]
pub struct HeatGrid {
    pub cell_mm: f64,
    pub cols: usize,
    pub rows: usize,
    pub counts: Vec<u16>,
}

impl HeatGrid {
    fn new(paper: &Paper, cell: f64) -> Self {
        let mut cell = cell.max((paper.width_mm * paper.height_mm / 1_500_000.0).sqrt())
            .max(paper.width_mm.max(paper.height_mm) / 1_500_000.0);
        let (cols, rows) = loop {
            let cols = (paper.width_mm / cell).ceil() as usize;
            let rows = (paper.height_mm / cell).ceil() as usize;
            if cols.saturating_mul(rows) <= 1_600_000 { break (cols, rows); }
            cell *= 1.05;
        };
        Self { cell_mm: cell, cols, rows, counts: vec![0; cols * rows] }
    }
    pub fn max(&self) -> u16 { self.counts.iter().copied().max().unwrap_or(0) }
    pub fn inked(&self) -> usize { self.counts.iter().filter(|&&v| v > 0).count() }
    pub fn at_least(&self, n: u16) -> usize { self.counts.iter().filter(|&&v| v >= n).count() }
    pub fn levels(&self) -> [usize; 6] {
        let mut levels = [0; 6];
        for &n in &self.counts { if n > 0 { levels[heat_level(n)] += 1; } }
        levels
    }
    /// Lejos del papel se conserva el máximo, como en Linear: los focos no se diluyen.
    pub fn pixels(&self, max_side: usize) -> (usize, usize, Vec<[u8; 4]>) {
        let f = self.cols.max(self.rows).div_ceil(max_side.max(1)).max(1);
        let (w, h) = (self.cols.div_ceil(f), self.rows.div_ceil(f));
        let mut bins = vec![0; w * h];
        for y in 0..self.rows { for x in 0..self.cols {
            let i = (y / f) * w + x / f;
            bins[i] = bins[i].max(self.counts[y * self.cols + x]);
        } }
        (w, h, bins.into_iter().map(|n| if n == 0 { [0; 4] } else { HEAT_COLORS[heat_level(n)] }).collect())
    }
}

/// Celdas que pisa un trazo y cuántas veces: las esperas de secado salen de aquí.
pub type Footprint = Vec<(usize, u16)>;

/// Pasadas físicas por celda. Permanecer dentro no suma; salir y volver sí.
fn footprint(path: &[P], grid: &HeatGrid, radius: f64) -> Footprint {
    let cell = grid.cell_mm;
    let r = radius.max(cell * 0.5);
    let reach = r + cell * 0.25;
    let gap = 3.0 * cell + 2.0 * r;
    let mut touched: HashMap<usize, (u16, f64)> = HashMap::new();
    let mut pos = 0.0;
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let delta = b - a;
        let len = delta.len();
        // Recorta el muestreo al papel incluso si un trazo tiene extremos muy lejos.
        let mut lo: f64 = 0.0;
        let mut hi: f64 = 1.0;
        for (v, d, size) in [(a.x, delta.x, grid.cols as f64 * cell), (a.y, delta.y, grid.rows as f64 * cell)] {
            if d.abs() < 1e-12 {
                if v < -reach || v > size + reach { hi = -1.0; }
            } else {
                let (u, v) = ((-reach - v) / d, (size + reach - v) / d);
                lo = lo.max(u.min(v)); hi = hi.min(u.max(v));
            }
        }
        if lo <= hi {
            let n = ((hi - lo) * len / (cell * 0.5)).ceil().max(1.0) as usize;
            for k in 0..=n {
                let q = a.lerp(b, lo + (hi - lo) * k as f64 / n as f64);
                let x0 = ((q.x - reach) / cell - 0.5).ceil().max(0.0) as usize;
                let y0 = ((q.y - reach) / cell - 0.5).ceil().max(0.0) as usize;
                let x1 = (((q.x + reach) / cell - 0.5).floor() as isize).min(grid.cols as isize - 1);
                let y1 = (((q.y + reach) / cell - 0.5).floor() as isize).min(grid.rows as isize - 1);
                for y in y0 as isize..=y1 { for x in x0 as isize..=x1 {
                    let center = p((x as f64 + 0.5) * cell, (y as f64 + 0.5) * cell);
                    let u = if len > 0.0 { ((center - a).dot(delta) / (len * len)).clamp(0.0, 1.0) } else { 0.0 };
                    if a.lerp(b, u).dist(center) > r + 1e-10 { continue; }
                    let i = y as usize * grid.cols + x as usize;
                    let distance = pos + len * u;
                    let entry = touched.entry(i).or_insert((0, f64::NEG_INFINITY));
                    if (distance - entry.1).abs() > gap { entry.0 = entry.0.saturating_add(1); }
                    entry.1 = distance;
                } }
            }
        }
        pos += len;
    }
    let mut out: Footprint = touched.into_iter().map(|(i, (count, _))| (i, count)).collect();
    out.sort_unstable_by_key(|x| x.0);
    out
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputStroke {
    pub pen: usize,
    pub path: Polyline,
    #[serde(skip)]
    footprint: Footprint,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum OutputStep {
    ChangePen { pen: usize, start_s: f64, end_s: f64 },
    Travel { from: P, to: P, start_s: f64, end_s: f64 },
    Wait { at: P, start_s: f64, end_s: f64 },
    Draw { stroke: usize, start_s: f64, end_s: f64 },
}

impl OutputStep {
    pub fn times(&self) -> (f64, f64) {
        match *self {
            Self::ChangePen { start_s, end_s, .. } | Self::Travel { start_s, end_s, .. }
            | Self::Wait { start_s, end_s, .. } | Self::Draw { start_s, end_s, .. } => (start_s, end_s),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OutputStats {
    pub draw_mm: f64,
    pub travel_mm: f64,
    pub draw_seconds: f64,
    pub travel_seconds: f64,
    pub lift_seconds: f64,
    pub wait_seconds: f64,
    pub change_seconds: f64,
    pub waits: usize,
    pub pen_changes: usize,
    pub strokes: usize,
    pub total_seconds: f64,
    pub outside_points: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputPlan {
    pub format: String,
    pub units: String,
    pub generator: String,
    /// El origen (0, 0) está en la esquina superior izquierda del papel.
    pub origin: String,
    /// Las Y crecen hacia abajo, como en SVG.
    pub y_axis: String,
    pub paper: Paper,
    pub pens: Vec<Pen>,
    pub motion: PlotSettings,
    pub settings: OutputSettings,
    pub strokes: Vec<OutputStroke>,
    pub steps: Vec<OutputStep>,
    pub stats: OutputStats,
    pub baseline_stats: OutputStats,
    #[serde(skip)]
    pub total_heat: HeatGrid,
    #[serde(skip)]
    pub wet_before: HeatGrid,
    #[serde(skip)]
    pub wet_after: HeatGrid,
}

impl OutputPlan {
    /// Huellas de secado de cada trazo. El JSON del plan no las incluye; la máquina por
    /// Wi-Fi las recibe aparte para esperar lo mismo que por USB.
    pub fn footprints(&self) -> Vec<&Footprint> { self.strokes.iter().map(|s| &s.footprint).collect() }

    /// Devuelve las huellas a un plan leído de JSON. Falla si no casan con los trazos.
    pub fn set_footprints(&mut self, footprints: Vec<Footprint>) -> bool {
        if footprints.len() != self.strokes.len() { return false; }
        for (stroke, cells) in self.strokes.iter_mut().zip(footprints) { stroke.footprint = cells; }
        true
    }

    /// SVG del orden preparado; las esperas solo existen en el plan JSON.
    pub fn layers(&self) -> Vec<PlotLayer> {
        let mut layers: Vec<PlotLayer> = Vec::new();
        for step in &self.steps {
            if let OutputStep::Draw { stroke, .. } = *step {
                let s = &self.strokes[stroke];
                if layers.last().is_none_or(|l| l.pen != s.pen) { layers.push(PlotLayer { pen: s.pen, paths: vec![] }); }
                layers.last_mut().unwrap().paths.push(s.path.clone());
            }
        }
        layers
    }
}

/// Divide a distancia exacta sin simplificar ni unir puntos que no estaban unidos.
fn split_path(path: Polyline, max_length: f64) -> Vec<Polyline> {
    if path.len() < 2 { return vec![]; }
    let mut out = Vec::new();
    let mut current = vec![path[0]];
    let mut remaining = max_length;
    for pair in path.windows(2) {
        let (mut a, b) = (pair[0], pair[1]);
        while a.dist(b) > remaining + 1e-8 {
            let cut = a.lerp(b, remaining / a.dist(b));
            current.push(cut);
            out.push(std::mem::replace(&mut current, vec![cut]));
            a = cut; remaining = max_length;
        }
        remaining -= a.dist(b);
        current.push(b);
        if remaining < 1e-8 {
            out.push(std::mem::replace(&mut current, vec![b]));
            remaining = max_length;
        }
    }
    if current.len() > 1 { out.push(current); }
    out
}

fn add_stroke(out: &mut Vec<OutputStroke>, path: Polyline, pen: usize, grid: &HeatGrid, radius: f64, cap: u16) {
    let cells = footprint(&path, grid, radius);
    let len = length(&path);
    // Por debajo de la tolerancia de corte se conserva la geometría. Cada
    // subdivisión reduce la longitud hasta este límite y termina también con puntos.
    if len > 1e-6 && cells.iter().any(|&(_, n)| n > cap) {
        // También interrumpe una polilínea que se pisa a sí misma repetidamente.
        for part in split_path(path, len / 2.0) { add_stroke(out, part, pen, grid, radius, cap); }
    } else { out.push(OutputStroke { pen, path, footprint: cells }); }
}

#[derive(Default)]
struct WetState { cells: HashMap<usize, VecDeque<(f64, u16)>> }
impl WetState {
    fn wait(&self, cells: &Footprint, arrival: f64, dry: f64, cap: u16) -> f64 {
        let mut ready = arrival;
        for &(i, added) in cells {
            if let Some(events) = self.cells.get(&i) {
                let mut total: u64 = events.iter().filter(|&&(end, _)| end + dry > arrival + 1e-8).map(|&(_, n)| n as u64).sum();
                for &(end, count) in events {
                    if total + added as u64 <= cap as u64 { break; }
                    if end + dry <= arrival + 1e-8 { continue; }
                    total -= count as u64;
                    ready = ready.max(end + dry);
                }
            }
        }
        (ready - arrival).max(0.0)
    }
    fn deposit(&mut self, cells: &Footprint, start: f64, end: f64, dry: f64, mut peak: Option<&mut HeatGrid>) {
        for &(i, added) in cells {
            let events = self.cells.entry(i).or_default();
            while events.front().is_some_and(|&(t, _)| t + dry <= start + 1e-8) { events.pop_front(); }
            if let Some(peak) = peak.as_deref_mut() {
                let total: u64 = events.iter().map(|&(_, n)| n as u64).sum::<u64>() + added as u64;
                peak.counts[i] = peak.counts[i].max(total.min(u16::MAX as u64) as u16);
            }
            events.push_back((end, added));
        }
    }
}

/// Secado mientras traza la máquina. El plan calcula las esperas con tiempos estimados
/// sin rampas ni esquinas, y con el cambio de tinta a tiempo fijo; en la máquina el
/// dibujo suele ir más lento, y esas esperas sobrarían. Esto aplica el mismo criterio
/// con el tiempo real: cada trazo espera solo lo que falte para que se seque lo que pisa.
pub struct Drying { wet: WetState, dry: f64, cap: u16 }

impl Drying {
    /// `None` si el plan no protege el secado.
    pub fn new(cfg: &OutputSettings) -> Option<Self> {
        (cfg.drying && cfg.dry_seconds.is_finite() && cfg.dry_seconds > 0.0 && cfg.max_wet_passes > 0)
            .then(|| Drying { wet: WetState::default(), dry: cfg.dry_seconds, cap: cfg.max_wet_passes })
    }

    /// Segundos que tiene que esperar `stroke` si empieza en el instante `now`.
    pub fn wait(&self, stroke: &OutputStroke, now: f64) -> f64 { self.wet.wait(&stroke.footprint, now, self.dry, self.cap) }

    /// Anota que `stroke` se dibujó entre `start` y `end`.
    pub fn deposit(&mut self, stroke: &OutputStroke, start: f64, end: f64) {
        self.wet.deposit(&stroke.footprint, start, end, self.dry, None);
    }
}

/// Recorre los trazos en orden con los tiempos de la máquina: sube la pluma antes de cada
/// viaje y la baja antes de cada trazo (dos movimientos por trazo), salvo si el trazo
/// empieza donde acabó el anterior.
fn schedule(strokes: &mut [OutputStroke], grid: &HeatGrid, cfg: &OutputSettings, timing: &Timing,
    protect: bool, cancel: &AtomicBool) -> Result<(Vec<OutputStep>, OutputStats, HeatGrid), OutputError> {
    let mut steps = Vec::new();
    let mut stats = OutputStats::default();
    let mut peak = grid.clone(); peak.counts.fill(0);
    let mut wet = WetState::default();
    let mut now = 0.0;
    let mut cur = P::default();
    let mut layer_start = 0;
    let mut previous_pen = None;
    let mut pen_up = true;
    let lift = timing.lift;
    while layer_start < strokes.len() {
        let pen = strokes[layer_start].pen;
        let layer_end = strokes[layer_start..].iter().position(|s| s.pen != pen).map_or(strokes.len(), |n| layer_start + n);
        if previous_pen.is_some() {
            if !pen_up { now += lift; stats.lift_seconds += lift; pen_up = true; }
            steps.push(OutputStep::ChangePen { pen, start_s: now, end_s: now + cfg.pen_change_seconds });
            now += cfg.pen_change_seconds;
            stats.pen_changes += 1; stats.change_seconds += cfg.pen_change_seconds;
        }
        previous_pen = Some(pen);
        // Ventana móvil acotada: no hay búsqueda cuadrática en fotos grandes.
        let mut pending: VecDeque<usize> = (layer_start..layer_end).collect();
        while !pending.is_empty() {
            check(cancel)?;
            let mut best = (f64::INFINITY, 0, false, 0.0);
            let candidates = if protect && cfg.reorder { pending.len().min(96) } else { 1 };
            for (slot, &id) in pending.iter().take(candidates).enumerate() {
                let s = &strokes[id];
                let reverse = protect && cfg.reorder && cur.dist(*s.path.last().unwrap()) < cur.dist(s.path[0]);
                let start = if reverse { *s.path.last().unwrap() } else { s.path[0] };
                let hop = timing.hop(cur, start);
                let travel = hop + if hop > 0.0 { lift * (if pen_up { 1.0 } else { 2.0 }) } else { 0.0 };
                let wait = if protect { wet.wait(&s.footprint, now + travel, cfg.dry_seconds, cfg.max_wet_passes) } else { 0.0 };
                let cost = travel + wait;
                if cost < best.0 { best = (cost, slot, reverse, wait); }
            }
            let id = pending.remove(best.1).unwrap();
            let s = &mut strokes[id];
            if best.2 { s.path.reverse(); }
            let start = s.path[0];
            let travel_mm = cur.dist(start);
            let hop = timing.hop(cur, start);
            // Para viajar, y para esperar a que seque, la pluma va arriba.
            let up = if (hop > 0.0 || best.3 > 1e-7) && !pen_up { lift } else { 0.0 };
            if up > 0.0 || hop > 0.0 || best.3 > 1e-7 { pen_up = true; }
            steps.push(OutputStep::Travel { from: cur, to: start, start_s: now, end_s: now + up + hop });
            now += up + hop;
            stats.travel_mm += travel_mm; stats.travel_seconds += hop;
            stats.lift_seconds += up;
            if best.3 > 1e-7 {
                // Tanto el viaje como la espera mantienen la pluma levantada.
                steps.push(OutputStep::Wait { at: start, start_s: now, end_s: now + best.3 });
                now += best.3; stats.waits += 1; stats.wait_seconds += best.3;
            }
            let len = length(&s.path);
            let down = if pen_up { lift } else { 0.0 };
            pen_up = false;
            let draw_s = timing.draw(&s.path);
            let end = now + down + draw_s;
            wet.deposit(&s.footprint, now + down, end, cfg.dry_seconds, Some(&mut peak));
            steps.push(OutputStep::Draw { stroke: id, start_s: now, end_s: end });
            stats.draw_mm += len; stats.draw_seconds += draw_s; stats.lift_seconds += down; stats.strokes += 1;
            now = end; cur = *s.path.last().unwrap();
        }
        let back = cur.dist(P::default());
        let hop = timing.hop(cur, P::default());
        let up = if !pen_up { lift } else { 0.0 };
        pen_up = true;
        steps.push(OutputStep::Travel { from: cur, to: P::default(), start_s: now, end_s: now + up + hop });
        now += up + hop;
        stats.travel_mm += back; stats.travel_seconds += hop; stats.lift_seconds += up;
        cur = P::default(); layer_start = layer_end;
    }
    stats.total_seconds = now;
    Ok((steps, stats, peak))
}

/// Plan con los tiempos de una máquina con los ajustes de serie.
pub fn prepare(layers: &[PlotLayer], paper: &Paper, pens: &[Pen], motion: &PlotSettings, cfg: &OutputSettings) -> Result<OutputPlan, OutputError> {
    prepare_cancellable(layers, paper, pens, motion, cfg, &Dynamics::default(), &AtomicBool::new(false))
}

/// Igual que `prepare`, con los tiempos de esta máquina (`dynamics`); se interrumpe con
/// `Err(Cancelled)` en cuanto `cancel` se activa.
pub fn prepare_cancellable(layers: &[PlotLayer], paper: &Paper, pens: &[Pen], motion: &PlotSettings, cfg: &OutputSettings,
    dynamics: &Dynamics, cancel: &AtomicBool) -> Result<OutputPlan, OutputError> {
    if !dynamics.valid() { return Err(OutputError::InvalidSettings); }
    let timing = Timing::new(motion, dynamics);
    if ![paper.width_mm, paper.height_mm, motion.draw_speed, motion.travel_speed, cfg.cell_mm, cfg.chunk_mm, cfg.dry_seconds]
        .iter().all(|v| v.is_finite() && *v > 0.0)
        || ![motion.lift_time, cfg.soak_mm, cfg.pen_change_seconds].iter().all(|v| v.is_finite() && *v >= 0.0)
        || !(paper.width_mm * paper.height_mm).is_finite()
        || cfg.max_wet_passes == 0 || cfg.cell_mm < 0.05 || cfg.chunk_mm < 1.0 {
        return Err(OutputError::InvalidSettings);
    }
    let max_radius = pens.iter().map(|pen| pen.width_mm / 2.0 + cfg.soak_mm).fold(0.0_f64, f64::max);
    let mut total_heat = HeatGrid::new(paper, cfg.cell_mm.max(max_radius / 3.0));
    let mut original = Vec::new();
    let mut outside = 0;
    for layer in layers {
        let pen = pens.get(layer.pen).ok_or(OutputError::MissingPen)?;
        if !pen.width_mm.is_finite() || pen.width_mm <= 0.0 { return Err(OutputError::InvalidPenWidth); }
        let radius = pen.width_mm / 2.0 + cfg.soak_mm;
        for path in &layer.paths {
            if path.len() < 2 { continue; }
            check(cancel)?;
            if path.iter().any(|q| !q.x.is_finite() || !q.y.is_finite()) { return Err(OutputError::InvalidCoordinates); }
            if !length(path).is_finite() { return Err(OutputError::InvalidLength); }
            outside += path.iter().filter(|q| q.x < 0.0 || q.y < 0.0 || q.x > paper.width_mm || q.y > paper.height_mm).count();
            let cells = footprint(path, &total_heat, radius);
            for &(i, n) in &cells { total_heat.counts[i] = total_heat.counts[i].saturating_add(n); }
            original.push(OutputStroke { pen: layer.pen, path: path.clone(), footprint: cells });
        }
    }
    let (baseline_steps, mut baseline_stats, wet_before) = schedule(&mut original, &total_heat, cfg, &timing, false, cancel)?;
    // Una ruta que ya cumple el límite se conserva entera: no se fragmenta ni se
    // reordena por proximidad si eso solo añadiría viajes o nuevas zonas húmedas.
    let needs_protection = cfg.drying && wet_before.max() > cfg.max_wet_passes;
    let mut strokes = if needs_protection {
        let mut strokes = Vec::new();
        for s in original {
            check(cancel)?;
            // Una celda que en total no supera el límite nunca puede superarlo en húmedo:
            // el trazo que solo pasa por celdas así se conserva entero.
            if s.footprint.iter().all(|&(i, _)| total_heat.counts[i] <= cfg.max_wet_passes) { strokes.push(s); continue; }
            let radius = pens[s.pen].width_mm / 2.0 + cfg.soak_mm;
            for part in split_path(s.path, cfg.chunk_mm) { add_stroke(&mut strokes, part, s.pen, &total_heat, radius, cfg.max_wet_passes); }
        }
        strokes
    } else { original };
    let (steps, mut stats, wet_after) = if needs_protection { schedule(&mut strokes, &total_heat, cfg, &timing, true, cancel)? }
        else { (baseline_steps, baseline_stats.clone(), wet_before.clone()) };
    stats.outside_points = outside; baseline_stats.outside_points = outside;
    Ok(OutputPlan { format: "vectorio-output-v1".into(), units: "mm, s".into(),
        generator: format!("Vectorio {}", env!("CARGO_PKG_VERSION")), origin: "top-left".into(), y_axis: "down".into(),
        paper: paper.clone(), pens: pens.to_vec(), motion: motion.clone(), settings: cfg.clone(),
        strokes, steps, stats, baseline_stats, total_heat, wet_before, wet_after })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup(paths: Vec<Polyline>, cfg: OutputSettings) -> OutputPlan {
        prepare(&[PlotLayer { pen: 0, paths }], &Paper { name: "Prueba".into(), width_mm: 30.0, height_mm: 20.0, margin_mm: 0.0 },
            &[Pen::new("Negro", [0, 0, 0], 0.5)], &PlotSettings::default(), &cfg).unwrap()
    }
    fn line(y: f64) -> Polyline { vec![p(2.0, y), p(12.0, y)] }
    #[test]
    fn crossing_counts_physical_passes_and_sampling_does_not_multiply_them() {
        let cfg = OutputSettings { soak_mm: 0.0, cell_mm: 0.25, ..Default::default() };
        let plan = setup(vec![line(5.125), vec![p(7.125, 2.0), p(7.125, 12.0)]], cfg);
        assert_eq!(plan.total_heat.max(), 2);
        assert!(plan.total_heat.at_least(2) > 0);
    }
    #[test]
    fn soak_and_different_pen_widths_affect_the_same_physical_grid() {
        let paths = vec![line(5.125), line(6.125)];
        let a = setup(paths.clone(), OutputSettings { soak_mm: 0.0, cell_mm: 0.25, ..Default::default() });
        let b = setup(paths, OutputSettings { soak_mm: 0.7, cell_mm: 0.25, ..Default::default() });
        assert_eq!(a.total_heat.max(), 1); assert_eq!(b.total_heat.max(), 2);
    }
    #[test]
    fn waits_obey_the_wet_limit_and_survive_export() {
        let cfg = OutputSettings { max_wet_passes: 1, dry_seconds: 15.0, ..Default::default() };
        let plan = setup(vec![line(5.0); 5], cfg);
        assert_eq!(plan.total_heat.max(), 5);
        assert_eq!(plan.wet_after.max(), 1);
        assert!(plan.wet_before.max() > 1);
        assert!(plan.stats.wait_seconds > 40.0);
        assert_eq!(plan.stats.strokes, 5);
        let json = serde_json::to_value(&plan).unwrap();
        assert!(json["steps"].as_array().unwrap().iter().any(|s| s["action"] == "wait"));
    }
    #[test]
    fn reorders_to_use_other_zones_before_waiting() {
        let paths = vec![line(5.0), line(5.0), line(15.0)];
        let plan = setup(paths, OutputSettings { max_wet_passes: 1, ..Default::default() });
        let drawn: Vec<_> = plan.steps.iter().filter_map(|s| match *s { OutputStep::Draw { stroke, .. } => Some(stroke), _ => None }).collect();
        assert_eq!(drawn, vec![0, 2, 1]);
        assert_eq!(plan.wet_after.max(), 1);
    }
    #[test]
    fn splitting_preserves_length_and_self_crossing_lines_get_safe_breaks() {
        let path = vec![p(2.0, 5.0), p(12.0, 5.0), p(2.0, 5.0), p(12.0, 5.0), p(2.0, 5.0)];
        let plan = setup(vec![path.clone()], OutputSettings { max_wet_passes: 1, chunk_mm: 100.0, ..Default::default() });
        assert!(plan.stats.strokes > 1);
        assert!((plan.stats.draw_mm - length(&path)).abs() < 1e-8);
        assert_eq!(plan.wet_after.max(), 1);
    }
    #[test]
    fn disabled_drying_has_no_waits_and_keeps_paths_intact() {
        let paths = vec![line(5.0); 4];
        let plan = setup(paths.clone(), OutputSettings { drying: false, ..Default::default() });
        assert_eq!(plan.stats.waits, 0);
        assert_eq!(plan.layers()[0].paths, paths);
        assert_eq!(plan.wet_after.counts, plan.wet_before.counts);
    }
    #[test]
    fn microscopic_loops_and_points_count_as_one_pass_and_keep_their_geometry() {
        let grid = HeatGrid::new(&Paper::default(), 0.05);
        let point = p(grid.cell_mm * 0.5, grid.cell_mm * 0.5);
        for path in [vec![point; 5], (0..100).map(|i| point + p((i % 2) as f64 * 1e-9, 0.0)).collect()] {
            let mut strokes = Vec::new();
            add_stroke(&mut strokes, path.clone(), 0, &grid, 0.025, 1);
            assert_eq!(strokes.len(), 1);
            assert_eq!(strokes[0].path, path);
            assert_eq!(strokes[0].footprint.iter().map(|c| c.1).max(), Some(1));
        }
        let invalid = OutputSettings { max_wet_passes: 0, ..Default::default() };
        assert_eq!(prepare(&[], &Paper::default(), &[], &PlotSettings::default(), &invalid).unwrap_err(), OutputError::InvalidSettings);
    }
    #[test]
    fn already_safe_routes_do_not_gain_fragments_or_waits() {
        let paths = vec![line(5.0), line(15.0)];
        let plan = setup(paths.clone(), OutputSettings { chunk_mm: 5.0, ..Default::default() });
        assert_eq!(plan.layers()[0].paths, paths);
        assert_eq!(plan.stats.strokes, 2);
        assert_eq!(plan.stats.total_seconds, plan.baseline_stats.total_seconds);
    }
    #[test]
    fn old_documents_get_drying_defaults_and_new_settings_round_trip() {
        let mut doc = crate::Document::default();
        doc.output.dry_seconds = 45.0;
        assert_eq!(crate::Document::from_json(&doc.to_json()).unwrap().output.dry_seconds, 45.0);
        let mut json = serde_json::to_value(&doc).unwrap(); json.as_object_mut().unwrap().remove("output");
        let restored: crate::Document = serde_json::from_value(json).unwrap();
        assert_eq!(restored.output, OutputSettings::default());
    }
    #[test]
    fn pen_order_and_wet_history_survive_a_manual_change() {
        let paper = Paper::default();
        let pens = vec![Pen::new("Fino", [0, 0, 0], 0.3), Pen::new("Grueso", [255, 0, 0], 1.0)];
        let cfg = OutputSettings { max_wet_passes: 1, pen_change_seconds: 0.0, ..Default::default() };
        let plan = prepare(&[PlotLayer { pen: 1, paths: vec![line(5.0)] }, PlotLayer { pen: 0, paths: vec![line(5.0)] }],
            &paper, &pens, &PlotSettings::default(), &cfg).unwrap();
        assert_eq!(plan.layers().iter().map(|l| l.pen).collect::<Vec<_>>(), vec![1, 0]);
        assert_eq!(plan.stats.pen_changes, 1);
        assert!(plan.stats.wait_seconds > 0.0);
        assert_eq!(plan.wet_after.max(), 1);
        let sum = plan.stats.draw_seconds + plan.stats.travel_seconds + plan.stats.lift_seconds
            + plan.stats.wait_seconds + plan.stats.change_seconds;
        assert!((sum - plan.stats.total_seconds).abs() < 1e-8);
        for pair in plan.steps.windows(2) { assert!((pair[0].times().1 - pair[1].times().0).abs() < 1e-8); }
    }
    #[test]
    fn the_estimate_uses_the_machine_motion_and_two_pen_moves_per_stroke() {
        // Dos trazos separados y uno que empieza donde acaba el anterior.
        let paths = vec![vec![p(2.0, 5.0), p(12.0, 5.0), p(12.0, 15.0)], vec![p(20.0, 5.0), p(30.0, 5.0)], vec![p(30.0, 5.0), p(30.0, 9.0)]];
        let motion = PlotSettings::default();
        let dynamics = Dynamics::default();
        let cfg = OutputSettings { drying: false, ..Default::default() };
        let plan = prepare_cancellable(&[PlotLayer { pen: 0, paths: paths.clone() }], &Paper::default(),
            &[Pen::new("Negro", [0, 0, 0], 0.5)], &motion, &cfg, &dynamics, &AtomicBool::new(false)).unwrap();
        let timing = Timing::new(&motion, &dynamics);
        let draw: f64 = paths.iter().map(|path| timing.draw(path)).sum();
        // Acelerar y frenar (y la esquina) cuesta más que ir siempre a velocidad de crucero.
        assert!(draw > paths.iter().map(|path| length(path)).sum::<f64>() / motion.draw_speed);
        assert!((plan.stats.draw_seconds - draw).abs() < 1e-9);
        // Baja, sube para viajar, baja; el tercero sigue abajo; sube para volver al origen.
        let lifts = 4.0;
        assert!((plan.stats.lift_seconds - lifts * motion.lift_time).abs() < 1e-9, "{}", plan.stats.lift_seconds);
        let travel = timing.hop(P::default(), p(2.0, 5.0)) + timing.hop(p(12.0, 15.0), p(20.0, 5.0))
            + timing.hop(p(30.0, 9.0), P::default());
        assert!((plan.stats.travel_seconds - travel).abs() < 1e-9);
        let sum = plan.stats.draw_seconds + plan.stats.travel_seconds + plan.stats.lift_seconds
            + plan.stats.wait_seconds + plan.stats.change_seconds;
        assert!((sum - plan.stats.total_seconds).abs() < 1e-8);
        // Una máquina más lenta en aceleración tarda más, y la estimación lo refleja.
        let slow = Dynamics { accel: 100.0, travel_accel: 100.0, ..dynamics };
        let slower = prepare_cancellable(&[PlotLayer { pen: 0, paths }], &Paper::default(),
            &[Pen::new("Negro", [0, 0, 0], 0.5)], &motion, &cfg, &slow, &AtomicBool::new(false)).unwrap();
        assert!(slower.stats.total_seconds > plan.stats.total_seconds);
    }
    #[test]
    fn thin_lines_on_cell_boundaries_and_reentries_are_not_lost() {
        let grid = HeatGrid::new(&Paper::default(), 0.5);
        let cells = footprint(&line(5.0), &grid, 0.05);
        assert!(!cells.is_empty());
        assert!(cells.iter().all(|c| c.1 == 1));
        let path = vec![p(2.0, 5.0), p(12.0, 5.0), p(2.0, 5.0), p(12.0, 5.0)];
        assert!(footprint(&path, &grid, 0.3).iter().any(|c| c.1 >= 3));
    }
    #[test]
    fn many_crossings_keep_all_geometry_and_respect_each_selected_limit() {
        let paths: Vec<_> = (0..48).map(|i| {
            let y = 2.0 + (i % 12) as f64;
            vec![p(2.0, y), p(14.0, 18.0 - y), p(25.0, y)]
        }).collect();
        let len: f64 = paths.iter().map(|l| length(l)).sum();
        for cap in 1..=4 {
            let plan = setup(paths.clone(), OutputSettings { max_wet_passes: cap, chunk_mm: 7.0, ..Default::default() });
            assert!(plan.wet_after.max() <= cap);
            assert!((plan.stats.draw_mm - len).abs() < 1e-6);
            let drawn = plan.steps.iter().filter(|s| matches!(s, OutputStep::Draw { .. })).count();
            assert_eq!(drawn, plan.strokes.len());
        }
    }
    #[test]
    fn off_paper_geometry_is_reported_and_invalid_settings_fail_explicitly() {
        let plan = setup(vec![vec![p(-10.0, 5.0), p(40.0, 5.0)]], OutputSettings::default());
        assert_eq!(plan.stats.outside_points, 2);
        assert_eq!(plan.stats.draw_mm, 50.0);
        let bad = OutputSettings { dry_seconds: f64::NAN, ..Default::default() };
        assert_eq!(prepare(&[], &Paper::default(), &[], &PlotSettings::default(), &bad).unwrap_err(), OutputError::InvalidSettings);
    }
    #[test]
    fn only_strokes_through_hot_cells_are_split() {
        let long = vec![p(10.0, 200.0), p(210.0, 200.0)];
        let mut paths = vec![line(5.0); 5];
        paths.push(long.clone());
        let plan = prepare(&[PlotLayer { pen: 0, paths }], &Paper::default(), &[Pen::new("Negro", [0, 0, 0], 0.5)],
            &PlotSettings::default(), &OutputSettings { max_wet_passes: 1, chunk_mm: 30.0, ..Default::default() }).unwrap();
        assert_eq!(plan.wet_after.max(), 1);
        assert_eq!(plan.stats.draw_mm, 5.0 * length(&line(5.0)) + length(&long));
        // La línea larga y lejana no se trocea: sigue siendo un solo trazo.
        assert_eq!(plan.strokes.iter().filter(|s| (length(&s.path) - 200.0).abs() < 1e-6).count(), 1);
    }
    #[test]
    fn cancelling_stops_the_calculation() {
        let paths = vec![line(5.0); 200];
        let layers = [PlotLayer { pen: 0, paths }];
        let cancel = AtomicBool::new(true);
        let result = prepare_cancellable(&layers, &Paper::default(), &[Pen::new("Negro", [0, 0, 0], 0.5)],
            &PlotSettings::default(), &OutputSettings::default(), &Dynamics::default(), &cancel);
        assert_eq!(result.unwrap_err(), OutputError::Cancelled);
    }
    #[test]
    fn plan_json_keeps_the_documented_schema() {
        let plan = setup(vec![line(5.0); 3], OutputSettings { max_wet_passes: 1, ..Default::default() });
        let json = serde_json::to_value(&plan).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["baseline_stats", "format", "generator", "motion", "origin", "paper", "pens", "settings", "stats", "steps", "strokes", "units", "y_axis"]);
        assert_eq!(json["format"], "vectorio-output-v1");
        assert_eq!((json["origin"].as_str(), json["y_axis"].as_str()), (Some("top-left"), Some("down")));
        let mut seen = std::collections::BTreeMap::new();
        for step in json["steps"].as_array().unwrap() {
            let mut keys: Vec<_> = step.as_object().unwrap().keys().cloned().collect();
            keys.sort();
            seen.insert(step["action"].as_str().unwrap().to_string(), keys);
        }
        assert_eq!(seen["draw"], ["action", "end_s", "start_s", "stroke"]);
        assert_eq!(seen["travel"], ["action", "end_s", "from", "start_s", "to"]);
        assert_eq!(seen["wait"], ["action", "at", "end_s", "start_s"]);
        let mut stroke: Vec<_> = json["strokes"][0].as_object().unwrap().keys().cloned().collect();
        stroke.sort();
        assert_eq!(stroke, ["path", "pen"]);
    }
}
