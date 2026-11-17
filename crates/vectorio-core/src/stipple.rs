//! Punteado: puntos repartidos según el tono y el detalle de la foto.
//!
//! Los puntos se siembran con la densidad deseada (más en las sombras y en los
//! bordes importantes) y después se relajan con el método de Lloyd ponderado: cada
//! punto se mueve al centro de masas de su celda de Voronoi. El resultado es un
//! reparto regular y sin grumos, como el de un punteado hecho a mano.

use crate::analysis::Analysis;
use crate::geom::{p, Polyline, Rng, P};
use crate::raster::Grid;
use crate::vectorize::{VecLayer, VecParams};
use image::RgbImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DotShape {
    /// Un toque de la punta: el punto lo pone el grosor del rotulador.
    Point,
    /// Un círculo pequeño, más grande donde la foto es más oscura.
    Circle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StippleParams {
    /// Número de puntos.
    pub dots: u32,
    /// Exponente del tono: valores altos dejan las luces más limpias.
    pub gamma: f64,
    /// Peso de los bordes, 0–1: concentra puntos donde hay detalle.
    pub edges: f64,
    /// Iteraciones de relajación.
    pub relax: usize,
    pub shape: DotShape,
}

impl Default for StippleParams {
    fn default() -> Self {
        StippleParams { dots: 6000, gamma: 1.4, edges: 0.4, relax: 8, shape: DotShape::Point }
    }
}

pub fn render(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    if rgb.width() < 4 || rgb.height() < 4 {
        return vec![];
    }
    let cfg = &prm.stipple;
    let a = Analysis::new(rgb, &prm.analysis, prm.blur);
    let (dots, spacing) = relaxed_points(&a, cfg);
    let lines = dots
        .into_iter()
        .map(|q| match cfg.shape {
            // Un tramo diminuto: el plóter baja la punta y apenas se mueve.
            DotShape::Point => vec![q, q + p(0.12 * a.scale, 0.0)],
            DotShape::Circle => {
                let dark = 1.0 - crate::analysis::sample(&a.light, q) as f64;
                circle(q, spacing * (0.12 + 0.3 * dark.clamp(0.0, 1.0).sqrt()))
            }
        })
        .collect();
    vec![VecLayer::lines([20, 20, 24], false, lines)]
}

/// Puntos repartidos según tono y detalle, ya relajados y en recorrido de bandas,
/// con su separación media en px. También sirven de paradas a los estilos de un
/// solo trazo.
pub(crate) fn relaxed_points(a: &Analysis, cfg: &StippleParams) -> (Vec<P>, f64) {
    let density = density(a, cfg);
    let n = cfg.dots.clamp(50, 60_000) as usize;
    let mut points = seed(&density, n);
    let spacing = (a.w as f64 * a.h as f64 / n as f64).sqrt();
    for _ in 0..cfg.relax.min(40) {
        points = lloyd(&density, &points, spacing);
    }
    (order(points, spacing), spacing)
}

fn density(a: &Analysis, cfg: &StippleParams) -> Grid<f32> {
    let gamma = cfg.gamma.clamp(0.3, 4.0) as f32;
    let edges = cfg.edges.clamp(0.0, 1.0) as f32;
    let data = a
        .light
        .data
        .par_iter()
        .zip(a.importance.data.par_iter())
        .map(|(light, important)| {
            let dark = (1.0 - light).clamp(0.0, 1.0);
            dark.powf(gamma) * (1.0 - edges) + edges * important * dark.sqrt()
        })
        .collect();
    Grid { w: a.w, h: a.h, data }
}

/// Siembra por la distribución acumulada: cada punto cae en un píxel con
/// probabilidad proporcional a su densidad.
fn seed(density: &Grid<f32>, n: usize) -> Vec<P> {
    let mut cumulative = Vec::with_capacity(density.data.len());
    let mut total = 0.0f64;
    for v in &density.data {
        total += v.max(0.0) as f64;
        cumulative.push(total);
    }
    let mut rng = Rng::new(17);
    if total <= 0.0 {
        return vec![];
    }
    (0..n)
        .map(|_| {
            let target = rng.f() * total;
            let i = cumulative.partition_point(|c| *c < target).min(density.data.len() - 1);
            p((i % density.w) as f64 + rng.f(), (i / density.w) as f64 + rng.f())
        })
        .collect()
}

/// Rejilla de cubetas para buscar el punto más cercano.
struct Buckets {
    cell: f64,
    map: HashMap<(i32, i32), Vec<usize>>,
}

impl Buckets {
    fn new(points: &[P], cell: f64) -> Self {
        let mut map: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, q) in points.iter().enumerate() {
            map.entry(((q.x / cell) as i32, (q.y / cell) as i32)).or_default().push(i);
        }
        Buckets { cell, map }
    }

    fn nearest(&self, points: &[P], q: P) -> Option<usize> {
        let (cx, cy) = ((q.x / self.cell) as i32, (q.y / self.cell) as i32);
        let mut best: Option<(usize, f64)> = None;
        for ring in 0i32..4 {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    if let Some(list) = self.map.get(&(cx + dx, cy + dy)) {
                        for &i in list {
                            let d = (points[i] - q).dot(points[i] - q);
                            if best.is_none_or(|(_, b)| d < b) {
                                best = Some((i, d));
                            }
                        }
                    }
                }
            }
            // Con un candidato a menos de un anillo de distancia ya no hay otro mejor.
            if let Some((_, d)) = best {
                if d.sqrt() <= ring as f64 * self.cell {
                    break;
                }
            }
        }
        best.map(|(i, _)| i)
    }
}

/// Un paso de Lloyd ponderado: cada punto pasa al centro de masas de los píxeles que
/// tiene más cerca.
fn lloyd(density: &Grid<f32>, points: &[P], spacing: f64) -> Vec<P> {
    let buckets = Buckets::new(points, spacing.max(1.0));
    let n = points.len();
    let sums = (0..density.h)
        .into_par_iter()
        .fold(
            || vec![[0.0f64; 3]; n],
            |mut acc, y| {
                for x in 0..density.w {
                    let d = density.get(x, y) as f64;
                    if d <= 1e-6 {
                        continue;
                    }
                    let q = p(x as f64 + 0.5, y as f64 + 0.5);
                    if let Some(i) = buckets.nearest(points, q) {
                        acc[i][0] += d * q.x;
                        acc[i][1] += d * q.y;
                        acc[i][2] += d;
                    }
                }
                acc
            },
        )
        .reduce(
            || vec![[0.0f64; 3]; n],
            |mut a, b| {
                for (x, y) in a.iter_mut().zip(b) {
                    x[0] += y[0];
                    x[1] += y[1];
                    x[2] += y[2];
                }
                a
            },
        );
    points
        .iter()
        .zip(sums)
        .map(|(q, [sx, sy, m])| if m > 1e-9 { p(sx / m, sy / m) } else { *q })
        .collect()
}

/// Recorrido en bandas alternas para que el plóter no cruce el papel entre puntos.
fn order(mut points: Vec<P>, spacing: f64) -> Vec<P> {
    let band = (spacing * 2.0).max(1.0);
    points.sort_by(|a, b| {
        let (ra, rb) = ((a.y / band) as i64, (b.y / band) as i64);
        ra.cmp(&rb).then_with(|| if ra % 2 == 0 { a.x.total_cmp(&b.x) } else { b.x.total_cmp(&a.x) })
    });
    points
}

fn circle(c: P, r: f64) -> Polyline {
    let r = r.max(0.3);
    (0..=8).map(|k| k as f64 * std::f64::consts::TAU / 8.0).map(|t| c + p(r * t.cos(), r * t.sin())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectorize::{vectorize, VecMode};
    use image::DynamicImage;

    fn gradient() -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(240, 160, |x, _| image::Rgb([(x as f32 / 239.0 * 255.0) as u8; 3])))
    }

    #[test]
    fn dark_areas_get_more_dots_and_relaxation_spreads_them_evenly() {
        let prm = VecParams { mode: VecMode::Stipple, max_px: 240, stipple: StippleParams { dots: 1500, edges: 0.0, ..Default::default() }, ..Default::default() };
        let res = vectorize(&gradient(), &prm);
        let dots: Vec<P> = res.layers[0].lines.iter().map(|l| l[0]).collect();
        assert!(dots.len() > 1400);
        let dark = dots.iter().filter(|q| q.x < 80.0).count();
        let light = dots.iter().filter(|q| q.x > 160.0).count();
        assert!(dark > light * 4, "oscuro {dark}, claro {light}");
        // Tras relajar, en la zona oscura no quedan puntos casi encima unos de otros.
        let spacing = (240.0 * 160.0 / 1500.0f64).sqrt();
        let crowded = dots.iter().filter(|a| a.x < 60.0).filter(|a| dots.iter().any(|b| !std::ptr::eq(*a, b) && (**a - *b).len() < spacing * 0.08)).count();
        assert!(crowded < 10, "{crowded} puntos amontonados");
    }

    #[test]
    fn circles_grow_in_the_shadows() {
        let prm = VecParams { mode: VecMode::Stipple, max_px: 240, stipple: StippleParams { dots: 800, shape: DotShape::Circle, edges: 0.0, ..Default::default() }, ..Default::default() };
        let res = vectorize(&gradient(), &prm);
        let radius = |l: &Polyline| (l[0] - l[4]).len() / 2.0;
        let mean = |f: &dyn Fn(f64) -> bool| {
            let v: Vec<f64> = res.layers[0].lines.iter().filter(|l| f(l[0].x)).map(radius).collect();
            v.iter().sum::<f64>() / v.len().max(1) as f64
        };
        assert!(mean(&|x| x < 60.0) > mean(&|x| x > 150.0));
    }
}
