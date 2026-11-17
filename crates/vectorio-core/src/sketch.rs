//! Boceto: un dibujo a mano que se va construyendo mirando el papel.
//!
//! No reparte trazos con una regla fija: simula la tinta que ya hay y pone el
//! siguiente trazo donde más falta tono, como hace quien dibuja.
//! - **Tono físico.** La oscuridad que pide la foto se pasa a densidad de tinta
//!   (`-ln(1 - oscuridad)`). Así las rayas que se cruzan se suman como en el papel y
//!   las sombras no se empastan.
//! - **De lo grande a lo pequeño.** Primero se encajan las masas con rayas largas
//!   mirando la foto desenfocada; luego se modela con trazos medios y al final se
//!   repasa el detalle con trazos cortos, sobre todo donde hay bordes.
//! - **Cada trazo es un gesto.** Nace en el punto que más tinta pide y avanza
//!   eligiendo en cada paso la dirección que más tono le falta. Se inclina hacia la
//!   forma de la foto (pelo, plumas, pliegues) o hacia un ángulo de rayado, y se
//!   detiene cuando ya no hace falta.
//! - **Rayado cruzado natural.** Donde ya hay rayas en una dirección y aún falta
//!   oscuridad, el siguiente trazo prefiere cruzarlas.
//! - **Pulso.** Las rayas se suavizan y tiemblan un poco, como hechas a mano.
//! - **Contornos opcionales**, los mismos del estilo Tinta, como encaje previo.
//! - **Dos tintas**, como una sanguina con carbón: la clara construye los medios
//!   tonos y la oscura las sombras y los contornos.

use crate::analysis::{self, Analysis};
use crate::geom::{chaikin, length, p, Polyline, Rng, P};
use crate::raster::Grid;
use crate::vectorize::{VecLayer, VecParams};
use image::RgbImage;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SketchInks {
    /// Un solo rotulador oscuro.
    One,
    /// Sanguina para los medios tonos y oscuro para sombras y contornos.
    Two,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SketchParams {
    /// Anchos de punta que caben en el lado mayor: con más, trazos más finos y
    /// apretados.
    pub density: u32,
    /// Tope de trazos.
    pub max_strokes: u32,
    /// Exponente del tono: más alto aclara los medios tonos.
    pub tone: f64,
    /// Luces que se quedan en papel, 0–0.6.
    pub paper: f64,
    /// Contornos de encaje, 0 = ninguno.
    pub contours: f64,
    /// Cuánto siguen los trazos las formas de la foto, 0 = rayado recto.
    pub follow: f64,
    /// Ángulo del rayado donde la foto no marca dirección.
    pub angle_deg: f64,
    /// Ganas de cruzar las rayas en las sombras, 0–1.
    pub cross: f64,
    /// Soltura de la mano, 0–1: arcos y contornos repetidos.
    pub hand: f64,
    /// Largo máximo del trazo, % del lado mayor.
    pub length_pct: f64,
    /// Rayas por haz, 0–1: con 0 cada trazo va suelto; con más, se agrupan en haces
    /// paralelos como al sombrear a mano.
    pub bundle: f64,
    /// Foco, 0–1: el centro de interés lleva todo el tono y los bordes se disuelven.
    pub focus: f64,
    pub inks: SketchInks,
}

impl Default for SketchParams {
    fn default() -> Self {
        SketchParams {
            density: 380,
            max_strokes: 24_000,
            tone: 1.3,
            paper: 0.2,
            contours: 0.5,
            follow: 0.7,
            angle_deg: 40.0,
            cross: 0.6,
            hand: 0.35,
            length_pct: 9.0,
            bundle: 0.6,
            focus: 0.5,
            inks: SketchInks::One,
        }
    }
}

const DARK: [u8; 3] = [28, 26, 30];
const SANGUINE: [u8; 3] = [172, 74, 48];
/// Densidad de una raya de un ancho de punta sobre papel, por tinta: el oscuro deja
/// el papel al ~10 %, la sanguina al ~45 %.
const INK_DENSITY: [f32; 2] = [1.1, 0.45];
/// Densidad por debajo de la cual una zona es una luz que se deja en papel.
const LIGHT: f32 = 0.12;
/// Oscuridad máxima que se pide: nunca negro absoluto.
const MAX_DARK: f32 = 0.94;

pub fn render(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    if rgb.width() < 16 || rgb.height() < 16 {
        return vec![];
    }
    let cfg = &prm.sketch;
    let a = Analysis::new(rgb, &prm.analysis, prm.blur);
    let mut sketch = Sketch::new(&a, cfg);
    let two = cfg.inks == SketchInks::Two;
    if cfg.contours > 0.01 {
        sketch.contours(&a, cfg);
    }
    let base = (cfg.length_pct.clamp(1.0, 40.0) / 100.0 * sketch.n as f64).max(4.0);
    // Pasadas de lo grande a lo pequeño. Con dos tintas, la sanguina encaja y modela
    // los medios tonos y el oscuro entra después a por las sombras.
    let passes: Vec<Pass> = if two {
        vec![
            Pass { level: 0, ink: 1, fill: 0.7, cap: Some(0.4), length: base * 1.6, step: 2.0, turn_deg: 6.0, follow: 0.6, detail: 0.0, share: 0.25 },
            Pass { level: 1, ink: 1, fill: 1.0, cap: Some(0.4), length: base, step: 1.5, turn_deg: 10.0, follow: 1.0, detail: 0.2, share: 0.3 },
            Pass { level: 1, ink: 0, fill: 0.9, cap: None, length: base, step: 1.5, turn_deg: 10.0, follow: 1.0, detail: 0.3, share: 0.25 },
            Pass { level: 2, ink: 0, fill: 1.0, cap: None, length: base * 0.5, step: 1.0, turn_deg: 15.0, follow: 1.3, detail: 1.0, share: 0.2 },
        ]
    } else {
        vec![
            Pass { level: 0, ink: 0, fill: 0.55, cap: None, length: base * 1.6, step: 2.0, turn_deg: 6.0, follow: 0.6, detail: 0.0, share: 0.3 },
            Pass { level: 1, ink: 0, fill: 0.85, cap: None, length: base, step: 1.5, turn_deg: 10.0, follow: 1.0, detail: 0.3, share: 0.4 },
            Pass { level: 2, ink: 0, fill: 1.0, cap: None, length: base * 0.5, step: 1.0, turn_deg: 15.0, follow: 1.3, detail: 1.0, share: 0.3 },
        ]
    };
    let total = cfg.max_strokes.clamp(100, 60_000) as usize;
    for pass in &passes {
        let before = sketch.strokes.len();
        sketch.run(pass, cfg, (total as f64 * pass.share) as usize);
        if std::env::var("VECTORIO_DEBUG").is_ok() {
            let made = &sketch.strokes[before..];
            let mean = made.iter().map(|s| length(&s.1)).sum::<f64>() / made.len().max(1) as f64;
            eprintln!("pasada nivel {} tinta {}: {} trazos, largo medio {:.1} (máx {:.0})", pass.level, pass.ink, made.len(), mean, pass.length);
        }
    }
    // De vuelta a px de la imagen.
    let to_px = 1.0 / sketch.k;
    let mut layers = Vec::new();
    for (ink, color) in [(1usize, SANGUINE), (0, DARK)] {
        let lines: Vec<Polyline> = sketch.strokes.iter().filter(|s| s.0 == ink).map(|s| s.1.iter().map(|q| *q * to_px).collect()).collect();
        if !lines.is_empty() {
            layers.push(VecLayer::lines(color, false, lines));
        }
    }
    layers
}

struct Pass {
    level: usize,
    ink: usize,
    /// Parte del tono final que se busca en esta pasada.
    fill: f32,
    /// Tope de densidad que atiende esta tinta (la sanguina no llega al negro).
    cap: Option<f32>,
    /// Largo máximo del trazo, en anchos de punta.
    length: f64,
    step: f64,
    turn_deg: f64,
    /// Cuánto pesa la forma de la foto, por encima del ajuste general.
    follow: f64,
    /// Cuánto se prefiere el detalle (bordes) al elegir dónde dibujar.
    detail: f32,
    /// Parte del tope de trazos de esta pasada.
    share: f64,
}

/// Un nivel de la pirámide: tono pedido y tinta puesta, vistos con un desenfoque de
/// un tamaño. Las celdas miden `cell` anchos de punta.
struct Level {
    cell: f64,
    w: usize,
    h: usize,
    target: Vec<f32>,
    ink: Vec<f32>,
    importance: Vec<f32>,
    /// Puntos donde no ha salido trazo: no se vuelve a intentar enseguida.
    tried: Vec<f32>,
    /// Ruido fijo para que las semillas no sigan una rejilla.
    jitter: Vec<f32>,
    tiles: Vec<(f32, usize)>,
}

const TILE: usize = 8;
/// Desenfoque de la huella en celdas propias.
const RADIUS: f64 = 2.6;

impl Level {
    fn index(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    fn sample(&self, v: &[f32], q: P) -> f32 {
        let x = (q.x - 0.5).clamp(0.0, (self.w - 1) as f64);
        let y = (q.y - 0.5).clamp(0.0, (self.h - 1) as f64);
        let (ix, iy) = (x as usize, y as usize);
        let (jx, jy) = ((ix + 1).min(self.w - 1), (iy + 1).min(self.h - 1));
        let (tx, ty) = ((x - ix as f64) as f32, (y - iy as f64) as f32);
        let top = v[self.index(ix, iy)] * (1.0 - tx) + v[self.index(jx, iy)] * tx;
        let bottom = v[self.index(ix, jy)] * (1.0 - tx) + v[self.index(jx, jy)] * tx;
        top * (1.0 - ty) + bottom * ty
    }

    fn tile_dims(&self) -> (usize, usize) {
        (self.w.div_ceil(TILE), self.h.div_ceil(TILE))
    }
}

struct Sketch {
    /// Anchos de punta en el lado mayor y anchos de punta por px de la imagen.
    n: usize,
    k: f64,
    w: usize,
    h: usize,
    levels: Vec<Level>,
    /// Dirección de las formas (ángulo sin signo) y fiabilidad, por celda de punta.
    field: Vec<(f32, f32)>,
    /// Ángulo de rayado, con una deriva suave para que no parezca una trama.
    hatch: Vec<f32>,
    /// Orientación de la tinta ya puesta (cos 2θ, sen 2θ, peso), en el nivel grueso.
    oriented: Vec<[f32; 3]>,
    strokes: Vec<(usize, Polyline)>,
    rng: Rng,
    /// Lo que deja media raya en el nivel fino.
    fine_thr: f32,
}

impl Sketch {
    fn new(a: &Analysis, cfg: &SketchParams) -> Sketch {
        let n = cfg.density.clamp(80, 1400) as usize;
        let k = n as f64 / a.w.max(a.h) as f64;
        let (w, h) = (((a.w as f64 * k).ceil() as usize).max(2), ((a.h as f64 * k).ceil() as usize).max(2));
        // Luz media en cada celda de punta.
        let light = analysis::gaussian(&a.light, (0.45 / k).max(0.0));
        // Niveles automáticos: la foto ocupa toda la escala, de la sombra más honda a
        // la luz más limpia, sin dejarse llevar por unos pocos píxeles extremos.
        let mut sorted: Vec<f32> = light.data.iter().step_by(7).copied().collect();
        sorted.sort_by(f32::total_cmp);
        let at = |f: f64| sorted[((sorted.len() - 1) as f64 * f) as usize];
        let (lo, hi) = (at(0.01), at(0.99));
        let span = (hi - lo).max(0.35);
        let lo = lo.min(1.0 - span);
        let paper = cfg.paper.clamp(0.0, 0.6) as f32;
        let tone = cfg.tone.clamp(0.3, 3.0) as f32;
        let at_px = |x: usize, y: usize| p((x as f64 + 0.5) / k, (y as f64 + 0.5) / k);
        let focus = Focus::new(a, cfg.focus);
        let mut density = Grid::<f32>::new(w, h);
        let mut important = Grid::<f32>::new(w, h);
        let mut field = vec![(0.0f32, 0.0f32); w * h];
        for y in 0..h {
            for x in 0..w {
                let q = at_px(x, y);
                let dark = 1.0 - ((analysis::sample(&light, q) - lo) / span).clamp(0.0, 1.0);
                let t = ((dark - paper) / (1.0 - paper)).clamp(0.0, 1.0).powf(tone) * focus.at(q);
                density.data[y * w + x] = -(1.0 - MAX_DARK * t).ln();
                important.data[y * w + x] = analysis::sample(&a.importance, q);
                let (ix, iy) = ((q.x as usize).min(a.w - 1), (q.y as usize).min(a.h - 1));
                let (t, c) = a.tangent(ix, iy);
                field[y * w + x] = (t.y.atan2(t.x) as f32, c);
            }
        }
        // Fiabilidad relativa: la foto manda donde su dirección destaca sobre el resto.
        let mut coherent: Vec<f32> = field.iter().map(|f| f.1).collect();
        coherent.sort_by(f32::total_cmp);
        let high = coherent[coherent.len() * 4 / 5].max(1e-4);
        for f in &mut field {
            f.1 = (f.1 / high).clamp(0.0, 1.0).powf(0.8);
        }
        let mut rng = Rng::new(29);
        let levels = [4.0, 2.0, 1.0].iter().map(|&cell| level(&density, &important, cell, &mut rng)).collect::<Vec<_>>();
        // Deriva del rayado: ruido de valor muy suave, ±14°.
        let base = cfg.angle_deg.to_radians() as f32;
        let drift = value_noise(w, h, 40.0, &mut rng);
        let hatch = drift.iter().map(|v| base + v * 0.25).collect();
        let coarse = &levels[0];
        let oriented = vec![[0.0f32; 3]; coarse.w * coarse.h];
        Sketch { n, k, w, h, levels, field, hatch, oriented, strokes: vec![], rng, fine_thr: 0.5 * INK_DENSITY[0] * gauss(0.0) as f32 }
    }

    /// Contornos de encaje, en tinta oscura.
    fn contours(&mut self, a: &Analysis, cfg: &SketchParams) {
        let c = cfg.contours.clamp(0.0, 1.0);
        let ink = crate::ink::InkParams {
            scale: 1.4,
            sensitivity: 0.35 + 0.5 * c,
            continuity: 5.0,
            shadow_levels: 0,
            min_length: 14.0 + 26.0 * (1.0 - c),
        };
        let lines = crate::ink::contour_lines(a, &ink);
        let hand = cfg.hand.clamp(0.0, 1.0);
        for line in lines {
            let pts: Polyline = line.iter().map(|q| *q * self.k).collect();
            // Encontrado y perdido: el contorno se marca donde linda con una sombra y se
            // pierde en las luces.
            for run in self.found(&pts) {
                if length(&run) < 4.0 {
                    continue;
                }
                let first = self.hand_line(&run, hand * 0.5);
                self.deposit(&first, 0, 0.8);
                self.strokes.push((0, first));
                // Una segunda pasada que no coincide del todo, como al buscar la forma.
                let len = length(&run);
                if hand > 0.05 && len > 12.0 && self.rng.f() < 0.35 + 0.6 * hand {
                    let n = run.len();
                    let a0 = (self.rng.f() * 0.35 * n as f64) as usize;
                    let a1 = n - (self.rng.f() * 0.35 * n as f64) as usize;
                    if a1 > a0 + 2 {
                        let shift = (0.5 + 0.7 * self.rng.f()) * if self.rng.f() < 0.5 { -1.0 } else { 1.0 };
                        let second = offset(&run[a0..a1], shift);
                        let second = self.hand_line(&second, hand);
                        self.deposit(&second, 0, 0.6);
                        self.strokes.push((0, second));
                    }
                }
            }
        }
    }

    /// Tramos de un contorno que lindan con algo oscuro.
    fn found(&self, pts: &[P]) -> Vec<Polyline> {
        let lvl = self.levels.last().unwrap();
        let dark = |q: P| lvl.sample(&lvl.target, q);
        let mut runs = vec![];
        let mut run: Polyline = vec![];
        let mut gap = 0;
        for i in 0..pts.len() {
            let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(pts.len() - 1)]);
            let d = b - a;
            let n = d.len().max(1e-9);
            let normal = p(-d.y / n, d.x / n) * 2.0;
            let side = dark(pts[i] + normal).max(dark(pts[i] - normal));
            if side > 0.3 {
                run.push(pts[i]);
                gap = 0;
            } else if !run.is_empty() && gap < 4 {
                // Un respiro corto no rompe el contorno.
                run.push(pts[i]);
                gap += 1;
            } else {
                run.truncate(run.len() - gap.min(run.len()));
                if run.len() > 1 {
                    runs.push(std::mem::take(&mut run));
                }
                run.clear();
                gap = 0;
            }
        }
        run.truncate(run.len() - gap.min(run.len()));
        if run.len() > 1 {
            runs.push(run);
        }
        runs
    }

    fn residual(&self, lvl: &Level, i: usize, pass: &Pass) -> f32 {
        let mut target = lvl.target[i];
        if let Some(cap) = pass.cap {
            target = target.min(cap);
        }
        let r = pass.fill * target - lvl.ink[i];
        r * (1.0 - pass.detail * 0.6 + pass.detail * 1.2 * lvl.importance[i]) - lvl.tried[i]
    }

    fn residual_at(&self, lvl: &Level, q: P, pass: &Pass) -> f32 {
        let mut target = lvl.sample(&lvl.target, q);
        if let Some(cap) = pass.cap {
            target = target.min(cap);
        }
        let r = pass.fill * target - lvl.sample(&lvl.ink, q);
        r * (1.0 - pass.detail * 0.6 + pass.detail * 1.2 * lvl.sample(&lvl.importance, q))
    }

    /// Residuo en el nivel más fino, con el reparto de la pasada.
    fn fine_residual(&self, q: P, pass: &Pass) -> f32 {
        let lvl = self.levels.last().unwrap();
        let mut target = lvl.sample(&lvl.target, q * (1.0 / lvl.cell));
        if let Some(cap) = pass.cap {
            target = target.min(cap);
        }
        pass.fill * target - lvl.sample(&lvl.ink, q * (1.0 / lvl.cell))
    }

    /// Tono que pide la foto de cerca.
    fn fine_target(&self, q: P) -> f32 {
        let lvl = self.levels.last().unwrap();
        lvl.sample(&lvl.target, q * (1.0 / lvl.cell))
    }

    /// Umbral de una pasada: lo que deja una raya en su nivel, a medias.
    fn threshold(&self, pass: &Pass) -> f32 {
        let cell = self.levels[pass.level].cell as f32;
        0.5 * INK_DENSITY[pass.ink] / cell * gauss(0.0) as f32
    }

    fn run(&mut self, pass: &Pass, cfg: &SketchParams, budget: usize) {
        let thr = self.threshold(pass);
        self.levels[pass.level].tried.iter_mut().for_each(|v| *v = 0.0);
        self.refresh(pass, None);
        let mut made = 0;
        let mut misses = 0;
        let hand = cfg.hand.clamp(0.0, 1.0);
        let bundle = 1 + (cfg.bundle.clamp(0.0, 1.0) * 7.0).round() as usize;
        while made < budget && misses < 4000 {
            let Some((value, seed)) = self.best(pass) else { break };
            if value < thr {
                break;
            }
            let lvl = &self.levels[pass.level];
            let start = p((seed % lvl.w) as f64 + 0.5, (seed / lvl.w) as f64 + 0.5) * lvl.cell;
            let line = self.gesture(start, pass, cfg, thr);
            if line.len() < 2 || length(&line) < pass.step * 2.0 + 2.0 {
                misses += 1;
                let lvl = &mut self.levels[pass.level];
                lvl.tried[seed] += thr * 2.0;
                let bounds = (seed % lvl.w, seed / lvl.w, seed % lvl.w, seed / lvl.w);
                self.refresh(pass, Some(bounds));
                continue;
            }
            let lead = self.hand_line(&chaikin(&line, 2, false), hand);
            // El haz: rayas paralelas que copian el gesto, tan juntas como pide el tono
            // y cada vez algo más cortas, hasta que la zona ya no pide más.
            let fine = self.levels.last().unwrap();
            let mid = lead[lead.len() / 2];
            let want = (pass.fill * fine.sample(&fine.target, mid)).max(0.05);
            let spacing = (INK_DENSITY[pass.ink] / want).clamp(1.25, 6.0) as f64;
            let lead_len = length(&lead);
            let mut group = vec![lead.clone()];
            self.lay(&lead, pass, &mut made);
            for side in [1.0, -1.0] {
                let mut prev = lead.clone();
                while group.len() < bundle && made < budget {
                    let jitter = 1.0 + 0.25 * self.rng.sym();
                    let next = offset(&prev, side * spacing * jitter);
                    let Some(next) = self.trim(&next, pass, thr, lead_len * 0.3) else { break };
                    self.lay(&next, pass, &mut made);
                    group.push(next.clone());
                    prev = next;
                }
            }
        }
    }

    /// Pone un trazo en el papel y al día las semillas de su zona.
    fn lay(&mut self, line: &[P], pass: &Pass, made: &mut usize) {
        let bounds = self.deposit(line, pass.ink, 1.0)[pass.level];
        self.strokes.push((pass.ink, line.to_vec()));
        if bounds.0 != usize::MAX {
            self.refresh(pass, Some(bounds));
        }
        *made += 1;
    }

    /// Recorta una raya del haz al tramo que aún pide tinta, con los extremos algo
    /// desiguales para que el haz se afile como hecho a mano.
    fn trim(&mut self, line: &[P], pass: &Pass, thr: f32, min_len: f64) -> Option<Polyline> {
        let lvl = &self.levels[pass.level];
        let ok: Vec<bool> = line
            .iter()
            .map(|q| {
                q.x >= 0.0
                    && q.y >= 0.0
                    && q.x < self.w as f64
                    && q.y < self.h as f64
                    && self.residual_at(lvl, *q * (1.0 / lvl.cell), pass) > thr * 0.15
                    && self.fine_residual(*q, pass) > self.fine_thr * 0.3
            })
            .collect();
        let (mut best, mut start) = ((0usize, 0usize), None);
        for i in 0..=ok.len() {
            match (i < ok.len() && ok[i], start) {
                (true, None) => start = Some(i),
                (false, Some(a)) => {
                    if i - a > best.1 - best.0 {
                        best = (a, i);
                    }
                    start = None;
                }
                _ => {}
            }
        }
        let n = best.1 - best.0;
        if n < 3 {
            return None;
        }
        let cut = |r: f64| (r * 0.22 * n as f64) as usize;
        let (a, b) = (best.0 + cut(self.rng.f()), best.1 - cut(self.rng.f()));
        let run = line[a..b.max(a + 2)].to_vec();
        (length(&run) >= min_len.max(pass.step * 2.0)).then_some(run)
    }

    /// Recalcula los máximos por mosaico del nivel de la pasada.
    fn refresh(&mut self, pass: &Pass, bounds: Option<(usize, usize, usize, usize)>) {
        let lvl_i = pass.level;
        let lvl = &self.levels[lvl_i];
        let (tw, th) = lvl.tile_dims();
        let (tx0, ty0, tx1, ty1) = match bounds {
            Some((x0, y0, x1, y1)) => (x0 / TILE, y0 / TILE, (x1 / TILE).min(tw - 1), (y1 / TILE).min(th - 1)),
            None => (0, 0, tw - 1, th - 1),
        };
        let mut updates = Vec::new();
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let mut best = (f32::NEG_INFINITY, 0usize);
                for y in ty * TILE..((ty + 1) * TILE).min(lvl.h) {
                    for x in tx * TILE..((tx + 1) * TILE).min(lvl.w) {
                        let i = lvl.index(x, y);
                        let v = self.residual(lvl, i, pass) + lvl.jitter[i] * 0.02;
                        if v > best.0 {
                            best = (v, i);
                        }
                    }
                }
                updates.push((ty * tw + tx, best));
            }
        }
        let lvl = &mut self.levels[lvl_i];
        for (t, best) in updates {
            lvl.tiles[t] = best;
        }
    }

    fn best(&self, pass: &Pass) -> Option<(f32, usize)> {
        self.levels[pass.level].tiles.iter().copied().max_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// Dirección que se busca en un punto (en anchos de punta): la forma de la foto,
    /// el rayado o una mezcla según su fiabilidad.
    fn wanted(&self, q: P) -> (f32, f32, f32) {
        let (x, y) = ((q.x as usize).min(self.w - 1), (q.y as usize).min(self.h - 1));
        let (angle, coherence) = self.field[y * self.w + x];
        (angle, coherence, self.hatch[y * self.w + x])
    }

    fn score(&self, q: P, theta: f64, pass: &Pass, cfg: &SketchParams, thr: f32) -> (f64, f32, f32) {
        let lvl = &self.levels[pass.level];
        let dir = p(theta.cos(), theta.sin());
        // Residuo a lo largo del paso y un poco más allá.
        let mut sum = 0.0f32;
        let mut first = 0.0f32;
        let mut fine = 0.0f32;
        for s in 1..=4 {
            let at = q + dir * (pass.step * 0.5 * s as f64);
            if at.x < 0.0 || at.y < 0.0 || at.x >= self.w as f64 || at.y >= self.h as f64 {
                return (f64::NEG_INFINITY, 0.0, 0.0);
            }
            let r = self.residual_at(lvl, at * (1.0 / lvl.cell), pass);
            sum += r;
            if s <= 2 {
                first += r * 0.5;
                // Lo que pide la foto de cerca: una raya de encaje no cruza una luz.
                fine += self.fine_target(at) * 0.5;
            }
        }
        let tone = (sum / 4.0 / thr).clamp(-2.0, 3.0) as f64;
        let (angle, coherence, hatch) = self.wanted(q);
        let follow = (cfg.follow.clamp(0.0, 1.0) * pass.follow).min(1.0) * coherence as f64;
        let along = (theta - angle as f64).cos().abs().powi(6);
        // Tinta ya puesta aquí y hacia dónde va.
        let coarse = &self.levels[0];
        let c = q * (1.0 / coarse.cell);
        let (cx, cy) = ((c.x as usize).min(coarse.w - 1), (c.y as usize).min(coarse.h - 1));
        let [oc, os, weight] = self.oriented[cy * coarse.w + cx];
        // Rayado por capas, como a mano: la primera dirección siempre; la segunda y la
        // tercera se abren cuando la zona ya lleva tinta y sigue pidiendo más.
        let cross = cfg.cross.clamp(0.0, 1.0);
        let gate = |lo: f32, hi: f32| (((weight - lo) / (hi - lo)).clamp(0.0, 1.0) as f64) * cross;
        let layers = [(0.0, 1.0), (1.3, gate(0.9, 1.4)), (-0.75, gate(1.8, 2.4))];
        let ruled = layers.iter().map(|&(offset, g)| g * (theta - hatch as f64 - offset).cos().abs().powi(6)).fold(0.0, f64::max);
        let aligned = follow * along + (1.0 - follow) * ruled;
        // No repetir la dirección que ya domina donde aún falta oscuridad.
        let crossing = if weight > 1e-4 {
            let strength = (oc.hypot(os) / weight).min(1.0) as f64;
            let existing = 0.5 * (os as f64).atan2(oc as f64);
            strength * (theta - existing).cos().powi(4) * ((weight as f64 - 0.9) / 0.6).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let light = ((fine - LIGHT) / LIGHT).min(0.0) as f64;
        (tone + 2.4 * aligned - 1.2 * cross * crossing + 1.5 * light, first, fine)
    }

    /// Un trazo desde una semilla: primero elige hacia dónde apunta y luego crece
    /// por los dos lados.
    fn gesture(&mut self, start: P, pass: &Pass, cfg: &SketchParams, thr: f32) -> Polyline {
        let mut best = (f64::NEG_INFINITY, 0.0);
        for i in 0..18 {
            let theta = i as f64 * PI / 18.0 + self.rng.f() * 0.05;
            let (a, _, _) = self.score(start, theta, pass, cfg, thr);
            let (b, _, _) = self.score(start, theta + PI, pass, cfg, thr);
            let s = a.max(-5.0) + b.max(-5.0);
            if s > best.0 {
                best = (s, theta);
            }
        }
        let half = pass.length * (0.55 + 0.45 * self.rng.f());
        let mut forward = self.grow(start, best.1, half, pass, cfg, thr);
        let backward = self.grow(start, best.1 + PI, half, pass, cfg, thr);
        let mut line: Polyline = backward.into_iter().rev().collect();
        line.pop();
        line.append(&mut forward);
        line
    }

    fn grow(&self, start: P, theta: f64, max_len: f64, pass: &Pass, cfg: &SketchParams, thr: f32) -> Polyline {
        let mut pts = vec![start];
        let mut q = start;
        let mut heading = theta;
        let mut travelled = 0.0;
        let turn = pass.turn_deg.to_radians();
        let keep = thr * 0.55;
        // Un gesto seguro no se para en la primera celda clara: sigue mientras, de
        // media, el tramo aún pide tinta, y solo frena en seco al entrar en una luz.
        let sure = max_len * 0.35;
        let mut mean = keep * 2.0;
        let mut turning = 0.0;
        let mut lit = 0;
        while travelled < max_len {
            let mut best = (f64::NEG_INFINITY, heading, 0.0f32, 0.0f32);
            for j in -3i32..=3 {
                let t = heading + turn * j as f64 / 3.0;
                let (s, first, fine) = self.score(q, t, pass, cfg, thr);
                let s = s - 0.15 * (j as f64 / 3.0).powi(2);
                if s > best.0 {
                    best = (s, t, first, fine);
                }
            }
            mean = 0.75 * mean + 0.25 * best.2;
            lit = if best.3 < LIGHT { lit + 1 } else { 0 };
            if best.0 == f64::NEG_INFINITY || lit >= 2 || (mean < keep && travelled >= sure) || mean < keep * 0.2 {
                break;
            }
            // La muñeca gira con inercia: curvas amplias, sin quiebros.
            turning = 0.6 * turning + 0.4 * (best.1 - heading);
            heading += turning;
            q = q + p(heading.cos(), heading.sin()) * pass.step;
            pts.push(q);
            travelled += pass.step;
        }
        pts
    }

    /// La mano: el trazo se arquea un poco, como al girar la muñeca, y se desvía
    /// despacio. Nada de temblor rápido.
    fn hand_line(&mut self, line: &[P], hand: f64) -> Polyline {
        let hand = hand.clamp(0.0, 1.0);
        let total = length(line);
        if hand < 1e-3 || line.len() < 3 || total < 1e-6 {
            return line.to_vec();
        }
        let bow = hand * 0.06 * total.min(60.0) * self.rng.sym();
        let (phase, freq) = (self.rng.f() * 100.0, 1.0 / (18.0 + 14.0 * self.rng.f()));
        let drift = hand * 0.35;
        let mut s = 0.0;
        let mut out = Vec::with_capacity(line.len());
        for i in 0..line.len() {
            if i > 0 {
                s += line[i].dist(line[i - 1]);
            }
            let (a, b) = (line[i.saturating_sub(1)], line[(i + 1).min(line.len() - 1)]);
            let d = b - a;
            let n = d.len();
            if n < 1e-9 {
                out.push(line[i]);
                continue;
            }
            let normal = p(-d.y / n, d.x / n);
            let t = s / total;
            let shift = bow * 4.0 * t * (1.0 - t) + drift * (s * freq * std::f64::consts::TAU + phase).sin();
            out.push(line[i] + normal * shift);
        }
        out
    }

    /// Pone la tinta de un trazo en todos los niveles. Devuelve la caja afectada en
    /// celdas del nivel más fino que mira la pasada actual (las de todos los niveles
    /// se recalculan por su cuenta en la siguiente consulta).
    fn deposit(&mut self, line: &[P], ink: usize, strength: f32) -> Vec<(usize, usize, usize, usize)> {
        let density = INK_DENSITY[ink] * strength;
        let mut boxes = Vec::new();
        for li in 0..self.levels.len() {
            let lvl = &mut self.levels[li];
            let scale = 1.0 / lvl.cell;
            let amount = density / lvl.cell as f32;
            let mut bounds = (usize::MAX, usize::MAX, 0usize, 0usize);
            for pair in line.windows(2) {
                let (a, b) = (pair[0] * scale, pair[1] * scale);
                let r = RADIUS;
                let x0 = ((a.x.min(b.x) - r).floor().max(0.0)) as usize;
                let y0 = ((a.y.min(b.y) - r).floor().max(0.0)) as usize;
                let x1 = ((a.x.max(b.x) + r).ceil() as usize).min(lvl.w - 1);
                let y1 = ((a.y.max(b.y) + r).ceil() as usize).min(lvl.h - 1);
                if x0 > x1 || y0 > y1 {
                    continue;
                }
                bounds = (bounds.0.min(x0), bounds.1.min(y0), bounds.2.max(x1), bounds.3.max(y1));
                let d = b - a;
                let len2 = d.dot(d);
                if len2 < 1e-12 {
                    continue;
                }
                let theta = d.y.atan2(d.x);
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let c = p(x as f64 + 0.5, y as f64 + 0.5);
                        let t = (c - a).dot(d) / len2;
                        if !(0.0..1.0).contains(&t) {
                            continue;
                        }
                        let dist = (c - (a + d * t)).len();
                        if dist > r {
                            continue;
                        }
                        // Densidad por celda: la raya, desenfocada a lo ancho.
                        let add = amount * gauss(dist) as f32;
                        let i = y * lvl.w + x;
                        lvl.ink[i] += add;
                        if li == 0 {
                            let o = &mut self.oriented[i];
                            o[0] += add * (2.0 * theta).cos() as f32;
                            o[1] += add * (2.0 * theta).sin() as f32;
                            o[2] += add;
                        }
                    }
                }
            }
            boxes.push(bounds);
        }
        boxes
    }
}

/// La misma línea desplazada de lado, en anchos de punta.
pub(crate) fn offset(line: &[P], by: f64) -> Polyline {
    (0..line.len())
        .map(|i| {
            let (a, b) = (line[i.saturating_sub(1)], line[(i + 1).min(line.len() - 1)]);
            let d = b - a;
            let n = d.len().max(1e-9);
            line[i] + p(-d.y / n, d.x / n) * by
        })
        .collect()
}

/// Dónde mira el dibujo: cerca de lo importante y del centro de interés lleva todo
/// el tono; hacia los bordes y en las zonas sin detalle se disuelve.
pub(crate) struct Focus {
    amount: f32,
    near: Grid<f32>,
    center: P,
    radius: f64,
}

impl Focus {
    pub(crate) fn new(a: &Analysis, amount: f64) -> Focus {
        let side = a.w.max(a.h) as f64;
        let near = analysis::gaussian(&a.importance, side * 0.05);
        let mut sorted: Vec<f32> = near.data.iter().step_by(5).copied().collect();
        sorted.sort_by(f32::total_cmp);
        let high = sorted[sorted.len() * 19 / 20].max(1e-4);
        let near = Grid { w: near.w, h: near.h, data: near.data.iter().map(|v| (v / high).min(1.0)).collect() };
        let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
        for y in (0..near.h).step_by(3) {
            for x in (0..near.w).step_by(3) {
                let v = near.data[y * near.w + x].powi(2) as f64;
                sx += v * x as f64;
                sy += v * y as f64;
                sw += v;
            }
        }
        let center = if sw > 1e-9 { p(sx / sw, sy / sw) } else { p(a.w as f64 / 2.0, a.h as f64 / 2.0) };
        // El centro de interés tira hacia el de la foto: casi nunca está en un borde.
        let center = center.lerp(p(a.w as f64 / 2.0, a.h as f64 / 2.0), 0.3);
        Focus { amount: amount.clamp(0.0, 1.0) as f32, near, center, radius: (a.w as f64).hypot(a.h as f64) * 0.5 }
    }

    /// Parte del tono que se dibuja en un punto, en px de la imagen.
    pub(crate) fn at(&self, q: P) -> f32 {
        if self.amount <= 0.0 {
            return 1.0;
        }
        let r = (q.dist(self.center) / self.radius).min(1.5);
        let radial = (1.0 - (r - 0.35).max(0.0) / 0.75).clamp(0.0, 1.0);
        let radial = radial * radial * (3.0 - 2.0 * radial);
        let near = analysis::sample(&self.near, q);
        let keep = ((0.3 + 0.8 * near) * (0.15 + 0.85 * radial as f32)).clamp(0.0, 1.0);
        keep.powf(2.0 * self.amount)
    }
}

/// Gaussiana normalizada de sigma 1 (celdas): la tinta de una raya repartida a
/// través.
pub(crate) fn gauss(d: f64) -> f64 {
    (-0.5 * d * d).exp() / (2.0 * PI).sqrt()
}

fn level(density: &Grid<f32>, important: &Grid<f32>, cell: f64, rng: &mut Rng) -> Level {
    let (w, h) = (((density.w as f64 / cell).ceil() as usize).max(1), ((density.h as f64 / cell).ceil() as usize).max(1));
    let down = |g: &Grid<f32>| {
        let mut out = Grid::<f32>::new(w, h);
        let mut count = vec![0.0f32; w * h];
        for y in 0..g.h {
            for x in 0..g.w {
                let (cx, cy) = (((x as f64 / cell) as usize).min(w - 1), ((y as f64 / cell) as usize).min(h - 1));
                out.data[cy * w + cx] += g.data[y * g.w + x];
                count[cy * w + cx] += 1.0;
            }
        }
        for (v, n) in out.data.iter_mut().zip(count) {
            *v /= n.max(1.0);
        }
        // Mismo desenfoque que la huella de las rayas.
        analysis::gaussian(&out, 1.0)
    };
    let target = down(density).data;
    let importance = down(important).data;
    let jitter = (0..w * h).map(|_| rng.f() as f32).collect();
    let (tw, th) = (w.div_ceil(TILE), h.div_ceil(TILE));
    Level { cell, w, h, ink: vec![0.0; w * h], tried: vec![0.0; w * h], target, importance, jitter, tiles: vec![(f32::NEG_INFINITY, 0); tw * th] }
}


/// Ruido de valor suave, −1–1, con celdas de `cell` px.
pub(crate) fn value_noise(w: usize, h: usize, cell: f64, rng: &mut Rng) -> Vec<f32> {
    let (gw, gh) = ((w as f64 / cell).ceil() as usize + 2, (h as f64 / cell).ceil() as usize + 2);
    let knots: Vec<f64> = (0..gw * gh).map(|_| rng.sym()).collect();
    let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f64 / cell, y as f64 / cell);
            let (ix, iy) = (fx as usize, fy as usize);
            let (tx, ty) = (smooth(fx - ix as f64), smooth(fy - iy as f64));
            let k = |a: usize, b: usize| knots[b * gw + a];
            let top = k(ix, iy) * (1.0 - tx) + k(ix + 1, iy) * tx;
            let bottom = k(ix, iy + 1) * (1.0 - tx) + k(ix + 1, iy + 1) * tx;
            out[y * w + x] = (top * (1.0 - ty) + bottom * ty) as f32;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectorize::{vectorize, VecMode};
    use image::DynamicImage;

    fn run(img: RgbImage, sketch: SketchParams) -> Vec<VecLayer> {
        let prm = VecParams { mode: VecMode::Sketch, max_px: 240, sketch, ..Default::default() };
        vectorize(&DynamicImage::ImageRgb8(img), &prm).layers
    }

    /// Tinta trazada (largo de las líneas) en la franja x ∈ [a, b).
    fn ink_in(layers: &[VecLayer], a: f64, b: f64) -> f64 {
        layers
            .iter()
            .flat_map(|l| &l.lines)
            .flat_map(|l| l.windows(2))
            .filter(|s| (s[0].x + s[1].x) / 2.0 >= a && (s[0].x + s[1].x) / 2.0 < b)
            .map(|s| (s[1] - s[0]).len())
            .sum()
    }

    #[test]
    fn shadows_get_more_ink_and_highlights_stay_paper() {
        let img = RgbImage::from_fn(240, 160, |x, _| image::Rgb([(x as f32 / 239.0 * 255.0) as u8; 3]));
        let layers = run(img, SketchParams { focus: 0.0, contours: 0.0, ..Default::default() });
        assert_eq!(layers.len(), 1);
        let (dark, mid, light) = (ink_in(&layers, 0.0, 60.0), ink_in(&layers, 90.0, 150.0), ink_in(&layers, 200.0, 240.0));
        assert!(dark > mid * 1.3, "oscuro {dark:.0}, medio {mid:.0}");
        assert!(mid > light * 2.0, "medio {mid:.0}, claro {light:.0}");
        assert!(light < dark * 0.1, "las luces deben quedar casi en papel: {light:.0} frente a {dark:.0}");
    }

    #[test]
    fn strokes_are_long_gestures_not_dots() {
        let img = RgbImage::from_fn(240, 160, |x, y| image::Rgb([if (x as i32 - 120).pow(2) + (y as i32 - 80).pow(2) < 3600 { 30 } else { 230 }; 3]));
        let layers = run(img, SketchParams::default());
        let lines: Vec<&Polyline> = layers.iter().flat_map(|l| &l.lines).collect();
        assert!(lines.len() > 50, "{} trazos", lines.len());
        let mean = lines.iter().map(|l| crate::geom::length(l)).sum::<f64>() / lines.len() as f64;
        // Un ancho de punta es 240/380 px: los trazos medios deben ser decenas de veces más largos.
        assert!(mean > 6.0, "largo medio {mean:.1} px");
    }

    #[test]
    fn focus_dissolves_the_edges() {
        let img = RgbImage::from_pixel(240, 160, image::Rgb([90, 90, 90]));
        let flat = run(img.clone(), SketchParams { focus: 0.0, contours: 0.0, ..Default::default() });
        let focused = run(img, SketchParams { focus: 1.0, contours: 0.0, ..Default::default() });
        let edge = |l: &[VecLayer]| ink_in(l, 0.0, 30.0) + ink_in(l, 210.0, 240.0);
        let centre = |l: &[VecLayer]| ink_in(l, 90.0, 150.0);
        let ratio = |l: &[VecLayer]| edge(l) / centre(l).max(1.0);
        assert!(ratio(&focused) < ratio(&flat) * 0.7, "bordes/centro: con foco {:.2}, sin foco {:.2}", ratio(&focused), ratio(&flat));
    }

    #[test]
    fn two_inks_lay_sanguine_first_and_charcoal_in_the_shadows() {
        let img = RgbImage::from_fn(240, 160, |x, _| image::Rgb([(x as f32 / 239.0 * 255.0) as u8; 3]));
        let layers = run(img, SketchParams { inks: SketchInks::Two, focus: 0.0, contours: 0.0, ..Default::default() });
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].color, SANGUINE);
        assert_eq!(layers[1].color, DARK);
        let dark = &layers[1..];
        assert!(ink_in(dark, 0.0, 60.0) > ink_in(dark, 120.0, 180.0) * 2.0, "el carbón debe ir a las sombras");
    }

    #[test]
    fn the_same_photo_gives_the_same_drawing() {
        let img = RgbImage::from_fn(120, 90, |x, y| image::Rgb([((x * y) % 255) as u8, (x * 2) as u8, (y * 2) as u8]));
        let a = run(img.clone(), SketchParams::default());
        let b = run(img, SketchParams::default());
        assert_eq!(a.iter().map(|l| l.lines.clone()).collect::<Vec<_>>(), b.iter().map(|l| l.lines.clone()).collect::<Vec<_>>());
    }
}
