//! Topografía por Fast Marching, adaptación nativa de FastMarchingTopoPlot
//! de Roland Blok (Unlicense): https://github.com/rolandblok/FastMarchingTopoPlot.
//! Véanse `third_party/FastMarchingTopoPlot` y `docs/referencia/topografia.md`.
//!
//! Luz → velocidad F → solución de |∇T| F = 1 → isolíneas del tiempo T.
//! El alfa recorta los trazos; no impide que la onda atraviese el fondo blanco.

use crate::analysis::{gaussian, sample, Analysis};
use crate::geom::{p, simplify, Polyline, P};
use crate::raster::{self, Grid};
use crate::vectorize::{flatten_white, working_rgba, VecLayer, VecParams, VecResult};
use image::DynamicImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TopoParams {
    /// Anchos de punta que caben en el lado mayor: fija la escala de todo el dibujo.
    pub fineness: u32,
    /// La interfaz calcula la finura con la punta y el tamaño en el papel.
    pub fit: bool,
    /// Parte del papel que cubre la tinta en el negro, 0.3–0.95: por debajo de 1 las
    /// curvas de las sombras se juntan sin llegar a empastarse.
    pub black: f64,
    /// Separación máxima entre curvas en las luces, en anchos de punta.
    pub gap: f64,
    /// Exponente del tono: más alto aclara los medios tonos.
    pub tone: f64,
    /// Coordenadas normalizadas, de izquierda a derecha y de arriba abajo.
    pub seed: [f64; 2],
    pub skip_transparent: bool,
    pub pen_width_mm: f64,
    /// Tolerancia de simplificación, en píxeles de trabajo.
    pub detail: f64,
    /// Esquirla: una ventana donde las curvas se multiplican y se trazan con una
    /// punta más fina, como un fragmento de la foto visto de más cerca.
    pub shard: bool,
    /// Centro de la esquirla, en coordenadas normalizadas.
    pub shard_center: [f64; 2],
    /// Alto de la esquirla respecto al lado mayor.
    pub shard_size: f64,
    pub shard_angle_deg: f64,
    /// Cuántas veces más curvas, y más fina la punta, dentro de la esquirla.
    pub shard_fine: u32,
}

/// Nombre de la capa de la esquirla: lleva su propia punta, más fina.
pub const FINE_LAYER: &str = "Negro fino";

impl Default for TopoParams {
    fn default() -> Self {
        Self {
            fineness: 500,
            fit: true,
            black: 0.75,
            gap: 8.0,
            tone: 1.0,
            seed: [0.5, 0.5],
            skip_transparent: true,
            pen_width_mm: 0.3,
            detail: 0.1,
            shard: false,
            shard_center: [0.42, 0.45],
            shard_size: 0.55,
            shard_angle_deg: 0.0,
            shard_fine: 3,
        }
    }
}

/// Píxeles de cálculo por ancho de punta: bastan para que las curvas más juntas,
/// a algo más de una punta, salgan suaves de los cuadrados de marcha.
const SUB: f64 = 3.0;
/// Suavizado del tono antes de la onda, en anchos de punta: el ruido más fino que
/// la separación entre curvas solo las haría temblar.
const SOFT: f64 = 0.7;

pub fn render(img: &DynamicImage, prm: &VecParams) -> VecResult {
    let rgba = working_rgba(img, prm.max_px.clamp(64, 4000));
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    let mut out = VecResult {
        width: w as f64,
        height: h as f64,
        layers: vec![],
    };
    if w < 2 || h < 2 {
        return out;
    }
    let cfg = &prm.topo;
    let mut alpha = Grid::new(w, h);
    for (a, px) in alpha.data.iter_mut().zip(rgba.pixels()) {
        *a = px[3] as f32 / 255.0;
    }
    if cfg.skip_transparent && alpha.data.iter().all(|a| *a < 0.5) {
        return out;
    }
    let rgb = prm.adjust.apply(&flatten_white(&rgba));
    let analysis = Analysis::new(&rgb, &prm.analysis, prm.blur.clamp(0.0, 20.0));
    // La onda se resuelve en su propia rejilla, con SUB píxeles por ancho de punta.
    let fine = cfg.fineness.clamp(60, 2400) as f64;
    let ratio = if cfg.shard { cfg.shard_fine.clamp(2, 5) } else { 1 };
    // Con esquirla, la rejilla se afina para que sus curvas, más juntas, sigan suaves.
    let sub = SUB.max(2.0 * ratio as f64);
    let k = (fine * sub / w.max(h) as f64).clamp(0.25, 8.0);
    let (sw, sh) = (((w as f64 * k).round() as usize).max(2), ((h as f64 * k).round() as usize).max(2));
    let pen = sw.max(sh) as f64 / fine;
    let to_work = |q: P| p((q.x + 0.5) / k - 0.5, (q.y + 0.5) / k - 0.5);
    let mut light = Grid::new(sw, sh);
    light.data.par_chunks_mut(sw).enumerate().for_each(|(y, row)| {
        for (x, v) in row.iter_mut().enumerate() {
            *v = sample(&analysis.light, to_work(p(x as f64, y as f64)));
        }
    });
    let light = gaussian(&light, SOFT * pen);
    // Cobertura de tinta pedida → separación entre curvas → velocidad de la onda.
    // Con una curva por unidad de tiempo, la separación es la propia velocidad.
    let black = cfg.black.clamp(0.3, 0.95);
    let white = (1.0 / cfg.gap.clamp(2.0, 40.0)).min(black);
    let tone = cfg.tone.clamp(0.3, 4.0);
    let mut speed = Grid::new(sw, sh);
    for (s, l) in speed.data.iter_mut().zip(&light.data) {
        let dark = (1.0 - *l as f64).clamp(0.0, 1.0).powf(tone);
        *s = (pen / (white + (black - white) * dark)) as f32;
    }
    let time = travel_time(&speed, cfg.seed, true);
    let top = time.data.iter().filter(|t| t.is_finite()).fold(0.0f32, |a, &t| a.max(t));
    let count = (top.floor() as usize).min(5000);
    if count == 0 {
        return out;
    }
    let tolerance = cfg.detail.clamp(0.0, 2.0) * k;
    let alpha = (cfg.skip_transparent && alpha.data.iter().any(|a| *a < 0.5)).then_some(&alpha);
    // La esquirla en píxeles de trabajo; un hueco blanco de punta y media la separa.
    let shard = cfg.shard.then(|| shard_polygon(cfg, w, h));
    let margin = 0.75 * w.max(h) as f64 / fine;
    let outside = |q: P| shard.as_ref().is_none_or(|poly| signed_distance(poly, q) > margin);
    let inside = |q: P| shard.as_ref().is_some_and(|poly| signed_distance(poly, q) < -margin);
    let extract = |field: &Grid<f32>, offset: P, level: f32, keep: &(dyn Fn(P) -> bool + Sync)| -> Vec<Polyline> {
        raster::contours(field, level)
            .into_iter()
            .flat_map(|line| {
                let line: Polyline = simplify(&line, tolerance).into_iter().map(|q| to_work(q + offset)).collect();
                clip_by(&line, |q| alpha.is_none_or(|a| visible(a, q)) && keep(q))
            })
            .filter(|line| line.len() >= 2)
            .collect()
    };
    let lines: Vec<Polyline> = (1..=count)
        .into_par_iter()
        .flat_map_iter(|i| extract(&time, p(0.0, 0.0), i as f32, &outside))
        .collect();
    if !lines.is_empty() {
        out.layers.push(VecLayer {
            color: [30, 30, 34],
            name: Some("Negro"),
            closed: false,
            lines,
            coverage: None,
        });
    }
    if let Some(poly) = &shard {
        // Solo se recorre el recuadro de la esquirla, con `ratio` curvas por cada una.
        let to_solve = |q: P| p((q.x + 0.5) * k - 0.5, (q.y + 0.5) * k - 0.5);
        let (lo, hi) = poly.iter().fold((p(f64::MAX, f64::MAX), p(f64::MIN, f64::MIN)), |(lo, hi), &q| {
            let q = to_solve(q);
            (p(lo.x.min(q.x), lo.y.min(q.y)), p(hi.x.max(q.x), hi.y.max(q.y)))
        });
        let (x0, y0) = ((lo.x.floor().max(0.0)) as usize, (lo.y.floor().max(0.0)) as usize);
        let (x1, y1) = ((hi.x.ceil() as usize + 1).min(sw), (hi.y.ceil() as usize + 1).min(sh));
        if x1 > x0 + 1 && y1 > y0 + 1 {
            let mut crop = Grid::new(x1 - x0, y1 - y0);
            for y in y0..y1 {
                crop.data[(y - y0) * crop.w..(y - y0 + 1) * crop.w].copy_from_slice(&time.data[y * sw + x0..y * sw + x1]);
            }
            let offset = p(x0 as f64, y0 as f64);
            let fine_lines: Vec<Polyline> = (1..=count * ratio as usize)
                .into_par_iter()
                .flat_map_iter(|j| extract(&crop, offset, j as f32 / ratio as f32, &inside))
                .collect();
            if !fine_lines.is_empty() {
                out.layers.push(VecLayer {
                    color: [62, 62, 72],
                    name: Some(FINE_LAYER),
                    closed: false,
                    lines: fine_lines,
                    coverage: None,
                });
            }
        }
    }
    out
}

/// Contorno de la esquirla, en píxeles de trabajo: un fragmento alargado e
/// irregular, como un trozo de cristal.
pub fn shard_polygon(cfg: &TopoParams, w: usize, h: usize) -> Vec<P> {
    const SHAPE: [(f64, f64); 4] = [(0.02, -0.5), (0.13, -0.36), (0.11, 0.5), (-0.17, 0.08)];
    let size = cfg.shard_size.clamp(0.1, 1.5) * w.max(h) as f64;
    let center = p(cfg.shard_center[0].clamp(0.0, 1.0) * w as f64, cfg.shard_center[1].clamp(0.0, 1.0) * h as f64);
    let (sin, cos) = cfg.shard_angle_deg.to_radians().sin_cos();
    SHAPE.iter().map(|&(x, y)| center + p(x * cos - y * sin, x * sin + y * cos) * size).collect()
}

/// Distancia al borde de un polígono, negativa dentro.
fn signed_distance(poly: &[P], q: P) -> f64 {
    let mut best = f64::INFINITY;
    let mut inside = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let ab = b - a;
        let t = ((q - a).dot(ab) / ab.dot(ab).max(1e-12)).clamp(0.0, 1.0);
        best = best.min(q.dist(a + ab * t));
        if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) / (b.y - a.y) * ab.x {
            inside = !inside;
        }
    }
    if inside { -best } else { best }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Trial {
    time: f64,
    index: usize,
}
impl Eq for Trial {}
impl PartialOrd for Trial {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Trial {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap extrae el mayor: invertir tiempo e índice crea un min-heap
        // determinista, también cuando hay píxeles con tiempos idénticos.
        other
            .time
            .total_cmp(&self.time)
            .then_with(|| other.index.cmp(&self.index))
    }
}

/// Actualización upwind de primer orden. Se usa una sola dimensión si la raíz
/// cuadrática quedaría por debajo del mayor vecino (condición de causalidad).
fn arrival(a: f64, b: f64, inv_speed: f64) -> f64 {
    let (lo, hi) = (a.min(b), a.max(b));
    if !lo.is_finite() {
        return f64::INFINITY;
    }
    if !hi.is_finite() || hi - lo >= inv_speed {
        return lo + inv_speed;
    }
    (lo + hi
        + (2.0 * inv_speed * inv_speed - (hi - lo) * (hi - lo))
            .max(0.0)
            .sqrt())
        * 0.5
}

/// Vecino aguas arriba en un eje: tiempo efectivo y peso de la diferencia. Con dos
/// vecinos congelados y en orden, la diferencia de segundo orden pesa 9/4.
fn upwind(time: &[f64], frozen: &[bool], back: Option<(usize, Option<usize>)>, fwd: Option<(usize, Option<usize>)>, second: bool) -> Option<(f64, f64, f64)> {
    let side = |s: Option<(usize, Option<usize>)>| {
        let (a, b) = s?;
        if !frozen[a] {
            return None;
        }
        let t1 = time[a];
        if second {
            if let Some(b) = b.filter(|&b| frozen[b] && time[b] <= t1) {
                return Some(((4.0 * t1 - time[b]) / 3.0, 2.25, t1));
            }
        }
        Some((t1, 1.0, t1))
    };
    match (side(back), side(fwd)) {
        (Some(a), Some(b)) => Some(if a.2 <= b.2 { a } else { b }),
        (a, b) => a.or(b),
    }
}

/// Resuelve el mapa de tiempos desde un origen. Las velocidades no positivas o
/// no finitas son obstáculos; los píxeles inaccesibles quedan en infinito.
/// `second` usa diferencias de segundo orden donde caben: frentes más redondos y
/// sin el rombo de las cuatro direcciones.
pub fn travel_time(speed: &Grid<f32>, seed: [f64; 2], second: bool) -> Grid<f32> {
    let (w, h) = (speed.w, speed.h);
    let mut out = Grid::new(w, h);
    out.data.fill(f32::INFINITY);
    if w == 0 || h == 0 || speed.data.len() != w * h {
        return out;
    }
    let coord = |v: f64, n: usize| {
        (if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.5
        } * (n - 1) as f64)
            .round() as usize
    };
    let seed = coord(seed[1], h) * w + coord(seed[0], w);
    if !speed.data[seed].is_finite() || speed.data[seed] <= 0.0 {
        return out;
    }
    let mut time = vec![f64::INFINITY; w * h];
    let mut frozen = vec![false; w * h];
    let mut heap = BinaryHeap::new();
    time[seed] = 0.0;
    heap.push(Trial {
        time: 0.0,
        index: seed,
    });
    while let Some(Trial { index, .. }) = heap.pop() {
        if frozen[index] {
            continue;
        }
        frozen[index] = true;
        let (x, y) = (index % w, index / w);
        let neighbors = [
            y.checked_sub(1).map(|y| y * w + x),
            (y + 1 < h).then_some((y + 1) * w + x),
            x.checked_sub(1).map(|x| y * w + x),
            (x + 1 < w).then_some(y * w + x + 1),
        ];
        for i in neighbors.into_iter().flatten() {
            if frozen[i] || !speed.data[i].is_finite() || speed.data[i] <= 0.0 {
                continue;
            }
            let (nx, ny) = (i % w, i / w);
            let inv = 1.0 / speed.data[i] as f64;
            let left = (nx > 0).then(|| (i - 1, (nx > 1).then(|| i - 2)));
            let right = (nx + 1 < w).then(|| (i + 1, (nx + 2 < w).then(|| i + 2)));
            let up = (ny > 0).then(|| (i - w, (ny > 1).then(|| i - 2 * w)));
            let down = (ny + 1 < h).then(|| (i + w, (ny + 2 < h).then(|| i + 2 * w)));
            let ax = upwind(&time, &frozen, left, right, second);
            let ay = upwind(&time, &frozen, up, down, second);
            let next = if second { solve(ax, ay, inv) } else {
                arrival(ax.map_or(f64::INFINITY, |a| a.0), ay.map_or(f64::INFINITY, |a| a.0), inv)
            };
            if next < time[i] {
                time[i] = next;
                heap.push(Trial {
                    time: next,
                    index: i,
                });
            }
        }
    }
    for (o, t) in out.data.iter_mut().zip(time) {
        *o = t as f32;
    }
    out
}

/// Σ α (T − t)² = 1/F² con los ejes disponibles. Si la raíz no respeta la
/// causalidad (queda por debajo de un vecino usado), se resuelve con un solo eje.
fn solve(ax: Option<(f64, f64, f64)>, ay: Option<(f64, f64, f64)>, inv: f64) -> f64 {
    let single = |a: (f64, f64, f64)| a.0 + inv / a.1.sqrt();
    match (ax, ay) {
        (None, None) => f64::INFINITY,
        (Some(a), None) | (None, Some(a)) => single(a),
        (Some(a), Some(b)) => {
            let qa = a.1 + b.1;
            let qb = -2.0 * (a.1 * a.0 + b.1 * b.0);
            let qc = a.1 * a.0 * a.0 + b.1 * b.0 * b.0 - inv * inv;
            let disc = qb * qb - 4.0 * qa * qc;
            if disc >= 0.0 {
                let t = (-qb + disc.sqrt()) / (2.0 * qa);
                if t >= a.2 && t >= b.2 {
                    return t;
                }
            }
            single(a).min(single(b))
        }
    }
}

fn visible(alpha: &Grid<f32>, q: P) -> bool {
    q.x >= 0.0
        && q.y >= 0.0
        && q.x < alpha.w as f64
        && q.y < alpha.h as f64
        && alpha.get(q.x as usize, q.y as usize) >= 0.5
}

/// Recorta también segmentos largos después de simplificar: no une por error
/// ambos lados de un agujero transparente ni pierde los extremos de un anillo.
#[cfg(test)]
fn clip_to_alpha(line: &Polyline, alpha: &Grid<f32>) -> Vec<Polyline> {
    clip_by(line, |q| visible(alpha, q))
}

/// Trozos de la línea donde `keep` se cumple, con los cortes afinados por bisección.
fn clip_by(line: &Polyline, keep: impl Fn(P) -> bool) -> Vec<Polyline> {
    let Some(&first) = line.first() else {
        return vec![];
    };
    let mut chunks = vec![];
    let mut current = vec![];
    let (mut previous, mut was_visible) = (first, keep(first));
    if was_visible {
        current.push(first);
    }
    for edge in line.windows(2) {
        let steps = (edge[0].dist(edge[1]) / 0.25).ceil().max(1.0) as usize;
        for i in 1..=steps {
            let q = edge[0].lerp(edge[1], i as f64 / steps as f64);
            let is_visible = keep(q);
            if was_visible != is_visible {
                let (mut lo, mut hi) = (previous, q);
                for _ in 0..12 {
                    let mid = lo.lerp(hi, 0.5);
                    if keep(mid) == was_visible {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                // Cada extremo queda dentro del área opaca, no en su lado vacío.
                if was_visible {
                    current.push(lo);
                    if current.len() >= 2 {
                        chunks.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                } else {
                    current.push(hi);
                }
            }
            if is_visible && (i == steps || !was_visible) {
                current.push(q);
            }
            previous = q;
            was_visible = is_visible;
        }
    }
    if current.len() >= 2 {
        chunks.push(current);
    }
    if first.dist(*line.last().unwrap()) < 1e-8
        && chunks.len() > 1
        && chunks
            .first()
            .unwrap()
            .first()
            .unwrap()
            .dist(*chunks.last().unwrap().last().unwrap())
            < 1e-8
    {
        let mut tail = chunks.pop().unwrap();
        tail.extend(chunks[0].drain(1..));
        chunks[0] = tail;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::p;
    use crate::vectorize::{vectorize, VecMode};
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    #[test]
    fn wave_has_the_right_speed_symmetry_and_causal_update() {
        let mut speed = Grid::new(41, 41);
        speed.data.fill(1.0);
        let t = travel_time(&speed, [0.5, 0.5], false);
        for i in 0..=20 {
            assert_eq!(t.get(20 + i, 20), i as f32);
            assert!((t.get(20 + i, 20 + i) - t.get(20 - i, 20 - i)).abs() < 1e-5);
        }
        assert!((t.get(40, 40) - 20.0_f32 * 2.0_f32.sqrt()).abs() < 1.1);
        speed.data.fill(0.5);
        let slow = travel_time(&speed, [0.5, 0.5], false);
        for (a, b) in t.data.iter().zip(slow.data) {
            assert!((b - 2.0 * a).abs() < 1e-4);
        }
        assert_eq!(arrival(1.0, 2.2, 1.0), 2.0);
    }

    #[test]
    fn solver_matches_the_original_javascript_on_a_variable_speed_fixture() {
        // worker.js blob 71fd1c43dbeb48cbf0bdcf9d7bc30f363f5e946b.
        // Valores producidos ejecutando fmmTravelTime original con Node, no Rust.
        let mut speed = Grid::new(7, 5);
        for (i, v) in speed.data.iter_mut().enumerate() {
            *v = (0.3 + 0.05 * (i % 7) as f64 + 0.02 * (i / 7) as f64) as f32;
        }
        let expected: [f64; 35] = [
            9.5725336, 6.5309973, 4.0675716, 2.2222223, 3.4789312, 4.8600078, 6.2338872, 8.2086554,
            5.0836549, 2.3809524, 0.0, 1.923077, 3.677463, 5.2903662, 8.9814863, 6.143661,
            3.8089211, 2.0408163, 3.2900796, 4.6665025, 6.0383406, 10.0457001, 7.4798675,
            5.4394279, 4.0016007, 4.8573771, 5.9171958, 7.0474286, 11.2659655, 8.9344082,
            7.1198454, 5.8883934, 6.4776826, 7.2842817, 8.1989527,
        ];
        for (a, b) in travel_time(&speed, [0.5, 0.25], false)
            .data
            .into_iter()
            .zip(expected)
        {
            assert!((a as f64 - b).abs() < 2e-5, "{a} != {b}");
        }
    }

    #[test]
    fn slow_regions_compress_contours_and_obstacles_stay_unreachable() {
        let mut speed = Grid::new(81, 1);
        for x in 0..81 {
            speed.set(x, 0, if x < 40 { 1.0 } else { 0.1 });
        }
        let t = travel_time(&speed, [0.0, 0.0], false);
        assert!((t.get(60, 0) - t.get(59, 0)) > 9.0 * (t.get(20, 0) - t.get(19, 0)));
        let mut blocked = Grid::new(4, 1);
        blocked.data = vec![1.0, 0.0, f32::NAN, 1.0];
        let t = travel_time(&blocked, [0.0, 0.0], true);
        assert_eq!(t.data[0], 0.0);
        assert!(t.data[1..].iter().all(|t| t.is_infinite()));
    }

    #[test]
    fn alpha_clipping_does_not_bridge_holes_even_after_simplification() {
        let mut alpha = Grid::new(12, 3);
        alpha.data.fill(1.0);
        for y in 0..3 {
            for x in 4..7 {
                alpha.set(x, y, 0.0);
            }
        }
        let chunks = clip_to_alpha(&vec![p(0.5, 1.5), p(11.5, 1.5)], &alpha);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].last().unwrap().x < 4.0 && chunks[1][0].x >= 7.0);
        for line in chunks {
            for edge in line.windows(2) {
                for i in 0..=100 {
                    assert!(visible(&alpha, edge[0].lerp(edge[1], i as f64 / 100.0)));
                }
            }
        }
    }

    #[test]
    fn native_paths_are_finite_in_bounds_deterministic_and_transparency_survives_save() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_fn(64, 48, |x, y| {
            if (8..56).contains(&x) && (7..41).contains(&y) {
                Rgba([(x * 3) as u8, (y * 4) as u8, 50, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        }));
        let prm = VecParams {
            mode: VecMode::Topography,
            blur: 0.0,
            topo: TopoParams {
                fineness: 120,
                ..Default::default()
            },
            ..Default::default()
        };
        let r = vectorize(&img, &prm);
        assert!(!r.layers.is_empty());
        assert_eq!(r.layers[0].lines, vectorize(&img, &prm).layers[0].lines);
        assert!(r.layers[0].coverage.is_none() && !r.layers[0].closed);
        for line in &r.layers[0].lines {
            for q in line {
                assert!(
                    q.x.is_finite()
                        && q.y.is_finite()
                        && q.x >= 8.0
                        && q.x < 56.0
                        && q.y >= 7.0
                        && q.y < 41.0
                );
            }
        }
        let saved = crate::photo::EncodedImage::from_image(&img)
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(saved.to_rgba8(), img.to_rgba8());
        assert_eq!(vectorize(&saved, &prm).layers[0].lines, r.layers[0].lines);
        let transparent =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(12, 12, Rgba([0, 0, 0, 0])));
        assert!(vectorize(&transparent, &prm).layers.is_empty());
        let drawn = vectorize(
            &transparent,
            &VecParams {
                topo: TopoParams {
                    skip_transparent: false,
                    ..prm.topo.clone()
                },
                ..prm.clone()
            },
        );
        assert!(!drawn.layers.is_empty());
        for (w, h) in [(1, 1), (1, 25), (25, 1)] {
            assert!(vectorize(
                &DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([80; 3]))),
                &prm
            )
            .layers
            .is_empty());
        }
        let mut old = serde_json::to_value(&prm).unwrap();
        old.as_object_mut().unwrap().remove("topo");
        assert_eq!(
            serde_json::from_value::<VecParams>(old).unwrap().topo,
            TopoParams::default()
        );
        assert_eq!(
            serde_json::from_str::<VecParams>(&serde_json::to_string(&prm).unwrap()).unwrap(),
            prm
        );
    }

    #[test]
    fn second_order_fronts_are_rounder_than_first_order() {
        let mut speed = Grid::new(121, 121);
        speed.data.fill(1.0);
        let first = travel_time(&speed, [0.5, 0.5], false);
        let second = travel_time(&speed, [0.5, 0.5], true);
        let error = |t: &Grid<f32>| {
            (0..=12).map(|a| {
                let angle = a as f64 / 12.0 * std::f64::consts::FRAC_PI_2;
                let (x, y) = (60.0 + 50.0 * angle.cos(), 60.0 + 50.0 * angle.sin());
                let d = (x.round() - 60.0).hypot(y.round() - 60.0);
                (t.get(x.round() as usize, y.round() as usize) as f64 - d).abs() / d
            }).fold(0.0, f64::max)
        };
        assert!(error(&second) < 0.5 * error(&first), "{} {}", error(&second), error(&first));
        assert!(error(&second) < 0.04);
    }

    fn flat(value: u8) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_pixel(200, 100, Rgb([value; 3])))
    }

    #[test]
    fn shadows_pack_curves_to_the_ink_limit_and_highlights_spread_them() {
        let prm = VecParams {
            mode: VecMode::Topography,
            blur: 0.0,
            topo: TopoParams { fineness: 200, seed: [0.0, 0.5], ..Default::default() },
            ..Default::default()
        };
        let count = |v| vectorize(&flat(v), &prm).layers[0].lines.len() as f64;
        let (dark, light) = (count(0), count(255));
        // Un ancho de punta por píxel: en el negro, una curva cada 1/0,75 px; en el blanco, cada 8.
        let far = 200f64.hypot(50.0);
        assert!((dark - far * 0.75).abs() < 0.08 * far * 0.75, "{dark}");
        assert!((light - far / 8.0).abs() < 2.5, "{light}");
    }

    #[test]
    fn the_shard_draws_finer_curves_with_its_own_pen_inside_its_outline() {
        let topo = TopoParams { fineness: 200, shard: true, shard_center: [0.5, 0.5], shard_size: 0.6, ..Default::default() };
        let prm = VecParams { mode: VecMode::Topography, blur: 0.0, topo: topo.clone(), ..Default::default() };
        let r = vectorize(&flat(128), &prm);
        assert_eq!(r.layers.len(), 2);
        assert_eq!(r.layers[1].name, Some(FINE_LAYER));
        let poly = shard_polygon(&topo, 200, 100);
        for q in r.layers[1].lines.iter().flatten() {
            assert!(signed_distance(&poly, *q) < 0.0);
        }
        for q in r.layers[0].lines.iter().flatten() {
            assert!(signed_distance(&poly, *q) > 0.0);
        }
        // Tres veces más curvas: el triple de tinta por área, con una punta tres veces más fina.
        let ink = |lines: &[Polyline]| lines.iter().map(|l| crate::geom::length(l)).sum::<f64>();
        let area = |poly: &[P]| 0.5 * (0..poly.len()).map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            a.x * b.y - b.x * a.y
        }).sum::<f64>().abs();
        let inside = ink(&r.layers[1].lines) / area(&poly);
        let outside = ink(&r.layers[0].lines) / (200.0 * 100.0 - area(&poly));
        assert!((inside / outside - 3.0).abs() < 0.6, "{}", inside / outside);
    }

}
