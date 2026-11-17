//! Un solo trazo: Línea continua y Bucles.
//!
//! Las paradas son los puntos del Punteado (más juntos en las sombras). Un recorrido
//! que pasa por todas ellas, mejorado con 2-opt hasta que no se cruza, es la Línea
//! continua. Bucles hace girar la punta en pequeños lazos mientras avanza por ese
//! mismo recorrido: donde las paradas se juntan, los lazos se amontonan y oscurecen.

use crate::analysis::Analysis;
use crate::geom::{chaikin, p, simplify, Polyline, Rng, P};
use crate::stipple::{self, DotShape, StippleParams};
use crate::vectorize::{VecLayer, VecParams};
use image::RgbImage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContinuousParams {
    /// Paradas del recorrido: más paradas, más detalle y más tinta.
    pub points: u32,
    /// Exponente del tono: valores altos dejan las luces más limpias.
    pub gamma: f64,
    /// Peso de los bordes, 0–1.
    pub edges: f64,
    /// Redondeo de las esquinas, 0–3.
    pub curves: usize,
}

impl Default for ContinuousParams {
    fn default() -> Self {
        ContinuousParams { points: 10000, gamma: 2.0, edges: 0.3, curves: 1 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoopParams {
    /// Paradas del recorrido que siguen los bucles.
    pub points: u32,
    /// Radio de los bucles respecto a la distancia entre paradas, 0.2–1.5: en las
    /// sombras las paradas se juntan y los bucles se aprietan.
    pub size: f64,
    /// Exponente del tono.
    pub gamma: f64,
    /// Cuánto cambia el tamaño de los bucles por el camino, 0–1.
    pub wander: f64,
}

impl Default for LoopParams {
    fn default() -> Self {
        LoopParams { points: 3500, size: 0.5, gamma: 1.8, wander: 0.35 }
    }
}

const INK: [u8; 3] = [20, 20, 24];

pub fn continuous(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    if rgb.width() < 4 || rgb.height() < 4 {
        return vec![];
    }
    let cfg = &prm.continuous;
    let a = Analysis::new(rgb, &prm.analysis, prm.blur);
    let st = StippleParams { dots: cfg.points.clamp(50, 30_000), gamma: cfg.gamma, edges: cfg.edges, relax: 6, shape: DotShape::Point };
    let (points, spacing) = stipple::relaxed_points(&a, &st);
    if points.len() < 2 {
        return vec![];
    }
    let line = open_at_longest(two_opt(points, spacing));
    let line = if cfg.curves > 0 { chaikin(&line, cfg.curves.min(3), false) } else { line };
    vec![VecLayer::lines(INK, false, vec![line])]
}

pub fn loops(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    if rgb.width() < 4 || rgb.height() < 4 {
        return vec![];
    }
    let cfg = &prm.loops;
    let a = Analysis::new(rgb, &prm.analysis, prm.blur);
    let st = StippleParams { dots: cfg.points.clamp(50, 20_000), gamma: cfg.gamma, edges: 0.15, relax: 6, shape: DotShape::Point };
    let (points, spacing) = stipple::relaxed_points(&a, &st);
    if points.len() < 2 {
        return vec![];
    }
    let path = open_at_longest(two_opt(points, spacing));
    let size = cfg.size.clamp(0.2, 1.5);
    let wander = cfg.wander.clamp(0.0, 1.0);
    // Un salto muy largo cruza papel blanco: ahí la punta se levanta.
    let jump = spacing * 5.0;
    let mut rng = Rng::new(23);
    let mut drift = 1.0f64;
    let mut phase = 0.0f64;
    // Distancia entre paradas, suavizada a lo largo del camino: fija el radio.
    let mut local = (path[1] - path[0]).len().min(jump);
    let mut strokes = Vec::new();
    let mut current: Polyline = Vec::new();
    for seg in path.windows(2) {
        let (from, to) = (seg[0], seg[1]);
        let len = (to - from).len();
        if len > jump {
            if current.len() >= 2 {
                strokes.push(std::mem::take(&mut current));
            }
            current.clear();
            continue;
        }
        let mut t = 0.0;
        while t < len {
            local += (len - local) * 0.08;
            // El tamaño deriva despacio, como la mano que no repite dos bucles iguales.
            drift = (drift + rng.sym() * 0.04 * wander).clamp(1.0 - 0.5 * wander, 1.0 + 0.5 * wander);
            // En las luces las paradas se separan; el radio se queda corto para que
            // los bucles no se solapen y dejen el papel claro.
            let r = (size * local.min(1.2 * spacing) * drift).max(0.4 * a.scale);
            // Cada vuelta avanza algo menos que el radio; unas 20 muestras por vuelta.
            let advance = 0.7 * r;
            let dt = (advance / 20.0).min(len - t).max(1e-3);
            t += dt;
            phase += dt / advance * std::f64::consts::TAU;
            let c = from + (to - from) * (t / len);
            current.push(c + p(r * phase.cos(), r * phase.sin()));
        }
    }
    if current.len() >= 2 {
        strokes.push(current);
    }
    let lines = strokes.into_iter().map(|l| simplify(&l, 0.08 * a.scale.max(0.5))).filter(|l| l.len() >= 2).collect();
    vec![VecLayer::lines(INK, false, lines)]
}

/// Abre el recorrido cerrado por su tramo más largo, que suele cruzar papel blanco.
fn open_at_longest(tour: Vec<P>) -> Polyline {
    let n = tour.len();
    if n < 3 {
        return tour;
    }
    let longest = (0..n).max_by(|&i, &j| (tour[(i + 1) % n] - tour[i]).len().total_cmp(&(tour[(j + 1) % n] - tour[j]).len())).unwrap_or(0);
    (1..=n).map(|k| tour[(longest + k) % n]).collect()
}

/// Los `k` vecinos más cercanos de cada punto, de más cerca a más lejos.
fn neighbours(points: &[P], cell: f64, k: usize) -> Vec<Vec<usize>> {
    let key = |q: P| ((q.x / cell).floor() as i32, (q.y / cell).floor() as i32);
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, q) in points.iter().enumerate() {
        grid.entry(key(*q)).or_default().push(i);
    }
    use rayon::prelude::*;
    points
        .par_iter()
        .enumerate()
        .map(|(i, &q)| {
            let (cx, cy) = key(q);
            let mut found: Vec<(f64, usize)> = Vec::new();
            for ring in 0i32..24 {
                for dy in -ring..=ring {
                    for dx in -ring..=ring {
                        if dx.abs() != ring && dy.abs() != ring {
                            continue;
                        }
                        if let Some(list) = grid.get(&(cx + dx, cy + dy)) {
                            found.extend(list.iter().filter(|&&j| j != i).map(|&j| ((points[j] - q).len(), j)));
                        }
                    }
                }
                // Lo que quede fuera de este anillo está a más de `ring` celdas.
                if found.len() >= k {
                    found.sort_by(|a, b| a.0.total_cmp(&b.0));
                    if found[k - 1].0 <= ring as f64 * cell {
                        break;
                    }
                }
            }
            found.sort_by(|a, b| a.0.total_cmp(&b.0));
            found.into_iter().take(k).map(|(_, j)| j).collect()
        })
        .collect()
}

/// Mejora un recorrido cerrado con 2-opt (listas de vecinos y cola de puntos por
/// revisar) hasta que ningún par de tramos se puede descruzar con ganancia.
fn two_opt(points: Vec<P>, spacing: f64) -> Vec<P> {
    let n = points.len();
    if n < 5 {
        return points;
    }
    // Arranca de una curva de Hilbert: sin la dirección preferida de un barrido en
    // bandas, que 2-opt no llega a borrar.
    let points = hilbert_order(points);
    let neigh = neighbours(&points, spacing.max(0.5), 10);
    let mut tour: Vec<usize> = (0..n).collect();
    let mut pos: Vec<usize> = (0..n).collect();
    let d = |a: usize, b: usize| (points[a] - points[b]).len();
    let mut queue: VecDeque<usize> = (0..n).collect();
    let mut queued = vec![true; n];
    let mut budget = 80 * n;
    while let Some(a) = queue.pop_front() {
        queued[a] = false;
        'search: for forward in [true, false] {
            let next = |tour: &[usize], pos: &[usize], x: usize| if forward { tour[(pos[x] + 1) % n] } else { tour[(pos[x] + n - 1) % n] };
            let b = next(&tour, &pos, a);
            let dab = d(a, b);
            for &c in &neigh[a] {
                let dac = d(a, c);
                if dac >= dab {
                    break;
                }
                let e = next(&tour, &pos, c);
                if c == b || e == a {
                    continue;
                }
                let gain = dab + d(c, e) - dac - d(b, e);
                if gain > 1e-7 {
                    // Hacia delante: a b … c e → a c … b e. Hacia atrás: e c … a b
                    // vistos al revés, mismo cambio.
                    let (i, j) = if forward { (pos[b], pos[c]) } else { (pos[a], pos[e]) };
                    reverse(&mut tour, &mut pos, i, j);
                    for x in [a, b, c, e] {
                        if !queued[x] {
                            queued[x] = true;
                            queue.push_back(x);
                        }
                    }
                    break 'search;
                }
            }
        }
        budget -= 1;
        if budget == 0 {
            break;
        }
    }
    tour.into_iter().map(|i| points[i]).collect()
}

/// Ordena los puntos por su posición en una curva de Hilbert.
fn hilbert_order(points: Vec<P>) -> Vec<P> {
    let (mut lo, mut hi) = (p(f64::MAX, f64::MAX), p(f64::MIN, f64::MIN));
    for q in &points {
        lo = p(lo.x.min(q.x), lo.y.min(q.y));
        hi = p(hi.x.max(q.x), hi.y.max(q.y));
    }
    let side = (hi.x - lo.x).max(hi.y - lo.y).max(1e-9);
    const N: u32 = 1 << 16;
    let index = |q: &P| {
        let (mut x, mut y) = (((q.x - lo.x) / side * (N - 1) as f64) as u32, ((q.y - lo.y) / side * (N - 1) as f64) as u32);
        let mut d = 0u64;
        let mut s = N / 2;
        while s > 0 {
            let rx = (x & s > 0) as u32;
            let ry = (y & s > 0) as u32;
            d += s as u64 * s as u64 * ((3 * rx) ^ ry) as u64;
            if ry == 0 {
                if rx == 1 {
                    x = N - 1 - x;
                    y = N - 1 - y;
                }
                std::mem::swap(&mut x, &mut y);
            }
            s /= 2;
        }
        d
    };
    let mut keyed: Vec<(u64, P)> = points.into_iter().map(|q| (index(&q), q)).collect();
    keyed.sort_by_key(|k| k.0);
    keyed.into_iter().map(|k| k.1).collect()
}

/// Invierte el tramo del recorrido entre las posiciones `i` y `j` (incluidas, hacia
/// delante y dando la vuelta si hace falta). Si es más de la mitad, invierte el
/// resto: en un recorrido cerrado es el mismo cambio.
fn reverse(tour: &mut [usize], pos: &mut [usize], i: usize, j: usize) {
    let n = tour.len();
    let mut len = (j + n - i) % n + 1;
    let (mut i, mut j) = (i, j);
    if 2 * len > n {
        (i, j) = ((j + 1) % n, (i + n - 1) % n);
        len = n - len;
    }
    for _ in 0..len / 2 {
        tour.swap(i, j);
        pos[tour[i]] = i;
        pos[tour[j]] = j;
        i = (i + 1) % n;
        j = (j + n - 1) % n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectorize::{vectorize, VecMode};
    use image::DynamicImage;

    fn blob() -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(240, 180, |x, y| {
            let d = ((x as f64 - 120.0).powi(2) + (y as f64 - 90.0).powi(2)).sqrt();
            image::Rgb([(40.0 + 200.0 * (d / 90.0).min(1.0)) as u8; 3])
        }))
    }

    fn crossings(l: &[P]) -> usize {
        let cross = |a: P, b: P, c: P, d: P| {
            let o = |p: P, q: P, r: P| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
            o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0
        };
        let mut n = 0;
        for i in 0..l.len() - 1 {
            for j in i + 2..l.len() - 1 {
                if cross(l[i], l[i + 1], l[j], l[j + 1]) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn two_opt_untangles_a_random_tour() {
        let mut rng = Rng::new(3);
        let points: Vec<P> = (0..400).map(|_| p(rng.f() * 100.0, rng.f() * 100.0)).collect();
        let before: f64 = (0..400).map(|i| (points[(i + 1) % 400] - points[i]).len()).sum();
        let tour = two_opt(points, 5.0);
        let after: f64 = (0..400).map(|i| (tour[(i + 1) % 400] - tour[i]).len()).sum();
        assert!(after < before * 0.25, "{before} → {after}");
        assert!(crossings(&tour) < 4, "{} cruces", crossings(&tour));
    }

    #[test]
    fn continuous_line_is_one_stroke_that_gathers_in_the_dark() {
        let prm = VecParams { mode: VecMode::Continuous, max_px: 240, continuous: ContinuousParams { points: 800, curves: 0, ..Default::default() }, ..Default::default() };
        let res = vectorize(&blob(), &prm);
        assert_eq!(res.layers[0].lines.len(), 1);
        let line = &res.layers[0].lines[0];
        assert!(line.len() > 700);
        let centre = line.iter().filter(|q| (**q - p(120.0, 90.0)).len() < 45.0).count();
        let edge = line.iter().filter(|q| (**q - p(120.0, 90.0)).len() > 80.0).count();
        assert!(centre > edge, "centro {centre}, borde {edge}");
        assert!(crossings(line) < 20);
    }

    #[test]
    fn loops_circle_around_the_path() {
        let prm = VecParams { mode: VecMode::Loops, max_px: 240, loops: LoopParams { points: 300, ..Default::default() }, ..Default::default() };
        let res = vectorize(&blob(), &prm);
        let lines = &res.layers[0].lines;
        assert!(!lines.is_empty());
        let total: f64 = lines.iter().map(|l| crate::geom::length(l)).sum();
        // Mucho más largo que el recorrido: la punta da vueltas mientras avanza.
        assert!(total > 3000.0, "{total}");
    }
}
