//! Rayas a mano: Sombreado cruzado y Buril.
//!
//! - Sombreado cruzado: capas de rayas paralelas, cada una en otra dirección y solo
//!   donde la foto supera su nivel de oscuridad. Las luces quedan con una capa o
//!   ninguna; las sombras, con todas cruzadas.
//! - Buril: rayas paralelas que se desvían con la luz de la foto, como si el papel
//!   tuviera relieve, y se ensanchan en las sombras. El ensanche se dibuja con varias
//!   pasadas de ida y vuelta que se separan del eje, como el surco de un buril.

use crate::analysis::{self, Analysis};
use crate::geom::{p, simplify, Polyline, Rng, P};
use crate::raster::{self, Grid};
use crate::vectorize::{VecLayer, VecParams};
use image::RgbImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

const INK: [u8; 3] = [20, 20, 24];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HatchParams {
    /// Separación entre rayas, px referidos a 900 px.
    pub spacing: f64,
    /// Capas cruzadas, 1–4.
    pub layers: usize,
    /// Dirección de la primera capa, grados.
    pub angle: f64,
    /// Exponente del tono: valores altos dejan las luces más limpias.
    pub gamma: f64,
    /// Temblor de la mano, 0–1.
    pub tremble: f64,
}

impl Default for HatchParams {
    fn default() -> Self {
        HatchParams { spacing: 6.0, layers: 4, angle: 45.0, gamma: 1.2, tremble: 0.3 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BurinParams {
    /// Separación entre rayas, px referidos a 900 px.
    pub spacing: f64,
    /// Dirección de las rayas, grados.
    pub angle: f64,
    /// Cuánto se desvían las rayas con la luz, 0–3.
    pub relief: f64,
    /// Ancho máximo del surco respecto a la separación, 0–1.
    pub swell: f64,
    /// Pasadas del surco más ancho, 2–8: en las sombras la punta va y vuelve.
    pub strands: usize,
    /// Exponente del tono.
    pub gamma: f64,
}

impl Default for BurinParams {
    fn default() -> Self {
        BurinParams { spacing: 6.0, angle: 20.0, relief: 1.0, swell: 0.8, strands: 4, gamma: 1.0 }
    }
}

/// Oscuridad 0–1, apenas suavizada y con su curva de tono.
fn darkness(a: &Analysis, gamma: f64) -> Grid<f32> {
    let mut g = analysis::gaussian(&a.light, (1.0 * a.scale).max(0.5));
    let gamma = gamma.clamp(0.3, 4.0) as f32;
    g.data.par_iter_mut().for_each(|v| *v = (1.0 - *v).clamp(0.0, 1.0).powf(gamma));
    g
}

pub fn cross_hatch(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    if rgb.width() < 4 || rgb.height() < 4 {
        return vec![];
    }
    let cfg = &prm.hatch;
    let a = Analysis::new(rgb, &prm.analysis, prm.blur);
    let dark = darkness(&a, cfg.gamma);
    let spacing = (cfg.spacing.clamp(1.0, 60.0) * a.scale).max(1.2);
    let layers = cfg.layers.clamp(1, 4);
    let tremble = cfg.tremble.clamp(0.0, 1.0) * spacing * 0.3;
    let (w, h) = (a.w as f64, a.h as f64);
    let mut lines = Vec::new();
    // Direcciones: la base, la perpendicular y las dos diagonales entre ellas.
    for (k, offset) in [0.0, 90.0, -45.0, 45.0].into_iter().take(layers).enumerate() {
        let level = (0.15 + 0.75 * k as f64 / layers as f64) as f32;
        let angle = (cfg.angle + offset).to_radians();
        let dir = p(angle.cos(), angle.sin());
        let normal = p(-dir.y, dir.x);
        let corners = [p(0.0, 0.0), p(w, 0.0), p(0.0, h), p(w, h)];
        let (n0, n1) = corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(c.dot(normal)), hi.max(c.dot(normal))));
        let (t0, t1) = corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), c| (lo.min(c.dot(dir)), hi.max(c.dot(dir))));
        // Cada capa empieza desplazada para que sus rayas no coincidan con las de otra.
        let shift = (k as f64 * 0.381966).fract() * spacing;
        let count = ((n1 - n0) / spacing).ceil() as usize + 1;
        let layer: Vec<Vec<Polyline>> = (0..count)
            .into_par_iter()
            .map(|m| {
                let offset = n0 + shift + m as f64 * spacing;
                let mut rng = Rng::new((k * 100_003 + m) as u64 + 7);
                let waves = [(rng.f() * 6.3, 1.0 / (25.0 + 30.0 * rng.f())), (rng.f() * 6.3, 1.0 / (7.0 + 9.0 * rng.f()))];
                let mut runs = Vec::new();
                let mut run: Polyline = Vec::new();
                let steps = ((t1 - t0) / 0.5).ceil() as usize;
                for i in 0..=steps {
                    let t = t0 + i as f64 * 0.5;
                    let wobble = tremble * (0.7 * (t * waves[0].1 / a.scale + waves[0].0).sin() + 0.3 * (t * waves[1].1 / a.scale + waves[1].0).sin());
                    let q = normal * (offset + wobble) + dir * t;
                    let inside = q.x >= 0.0 && q.y >= 0.0 && q.x < w - 1.0 && q.y < h - 1.0;
                    if inside && analysis::sample(&dark, q) > level {
                        run.push(q);
                    } else if !run.is_empty() {
                        runs.push(std::mem::take(&mut run));
                    }
                }
                if !run.is_empty() {
                    runs.push(run);
                }
                let mut kept: Vec<Polyline> = runs
                    .into_iter()
                    .filter(|r| r.len() >= 2 && crate::geom::length(r) >= spacing.max(2.0))
                    .map(|r| simplify(&r, 0.15 * a.scale.max(0.5)))
                    .collect();
                // Rayas alternas en sentido contrario: la punta va y vuelve.
                if m % 2 == 1 {
                    kept.reverse();
                    for r in &mut kept {
                        r.reverse();
                    }
                }
                kept
            })
            .collect();
        lines.extend(layer.into_iter().flatten());
    }
    vec![VecLayer::lines(INK, false, lines)]
}

pub fn burin(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    if rgb.width() < 4 || rgb.height() < 4 {
        return vec![];
    }
    let cfg = &prm.burin;
    let a = Analysis::new(rgb, &prm.analysis, prm.blur);
    let dark = darkness(&a, cfg.gamma);
    let spacing = (cfg.spacing.clamp(1.5, 60.0) * a.scale).max(1.5);
    let angle = cfg.angle.to_radians();
    let normal = p(-angle.sin(), angle.cos());
    // Campo de fase: cada raya es una curva de nivel entera. La luz suavizada empuja
    // las rayas a un lado y las curva sobre las formas.
    let soft = analysis::gaussian(&a.light, 5.0 * a.scale);
    let push = cfg.relief.clamp(0.0, 3.0) * 30.0 * a.scale;
    let mut phase = Grid::<f32>::new(a.w, a.h);
    phase.data.par_iter_mut().enumerate().for_each(|(i, v)| {
        let q = p((i % a.w) as f64 + 0.5, (i / a.w) as f64 + 0.5);
        *v = ((q.dot(normal) + push * (soft.data[i] as f64 - 0.5)) / spacing) as f32;
    });
    let (lo, hi) = phase.data.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    let strands = cfg.strands.clamp(2, 8);
    let max_half = cfg.swell.clamp(0.0, 1.0) * 0.45 * spacing;
    let lines: Vec<Polyline> = ((lo.floor() as i64)..=(hi.ceil() as i64))
        .into_par_iter()
        .flat_map_iter(|k| raster::contours(&phase, k as f32 + 0.5))
        .flat_map_iter(|l| groove(&simplify(&l, 0.2 * a.scale.max(0.5)), &dark, max_half, strands, spacing))
        .collect();
    vec![VecLayer::lines(INK, false, lines)]
}

/// Convierte una raya en surco: se corta en las luces más blancas y, según la
/// oscuridad, se dibuja con más pasadas de ida y vuelta que se separan del eje. Más
/// oscuro: surco más ancho y más pasadas, siempre a la misma distancia entre ellas.
fn groove(line: &[P], dark: &Grid<f32>, max_half: f64, strands: usize, spacing: f64) -> Vec<Polyline> {
    if line.len() < 2 {
        return vec![];
    }
    // Remuestreo a pasos cortos: el ancho cambia a lo largo de la raya.
    let step = (spacing * 0.25).clamp(0.4, 2.0);
    let mut pts: Vec<P> = vec![line[0]];
    for seg in line.windows(2) {
        let n = ((seg[1] - seg[0]).len() / step).ceil().max(1.0) as usize;
        pts.extend((1..=n).map(|k| seg[0] + (seg[1] - seg[0]) * (k as f64 / n as f64)));
    }
    let n = pts.len();
    let normal_at = |i: usize| {
        let d = pts[(i + 1).min(n - 1)] - pts[i.saturating_sub(1)];
        let l = d.len().max(1e-9);
        p(-d.y / l, d.x / l)
    };
    // (pasadas, medio ancho) en cada punto; 0 pasadas = papel en blanco.
    let info: Vec<(usize, f64)> = pts
        .iter()
        .map(|q| {
            let d = (analysis::sample(dark, *q) as f64).clamp(0.0, 1.0);
            if d < 0.04 {
                return (0, 0.0);
            }
            let passes = 1 + ((strands - 1) as f64 * d).round() as usize;
            (passes, max_half * (passes - 1) as f64 / (strands - 1).max(1) as f64)
        })
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let passes = info[i].0;
        let mut j = i;
        while j + 1 < n && info[j + 1].0 == passes {
            j += 1;
        }
        // Cada tramo comparte su último punto con el siguiente para no dejar huecos.
        let end = (j + 1).min(n - 1);
        if passes == 1 && end > i {
            out.push(pts[i..=end].to_vec());
        } else if passes > 1 && end > i {
            // El ancho se suaviza en los extremos del tramo hacia el de sus vecinos.
            let half_at = |t: usize| {
                let edge = (t - i).min(end - t) as f64;
                let ramp = (edge / 3.0).min(1.0);
                info[t.min(j)].1 * (0.6 + 0.4 * ramp)
            };
            let mut groove: Polyline = Vec::new();
            for s in 0..passes {
                let f = -1.0 + 2.0 * s as f64 / (passes - 1) as f64;
                let pass: Vec<P> = (i..=end).map(|t| pts[t] + normal_at(t) * (half_at(t) * f)).collect();
                if s % 2 == 0 {
                    groove.extend(pass);
                } else {
                    groove.extend(pass.into_iter().rev());
                }
            }
            out.push(groove);
        }
        i = j + 1;
    }
    out.retain(|l| l.len() >= 2 && crate::geom::length(l) >= 1.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectorize::{vectorize, VecMode};
    use image::DynamicImage;

    fn steps() -> DynamicImage {
        // Cuatro franjas verticales, de claro a oscuro.
        DynamicImage::ImageRgb8(RgbImage::from_fn(320, 160, |x, _| image::Rgb([[245u8, 170, 100, 25][(x / 80) as usize]; 3])))
    }

    fn ink_in(lines: &[Polyline], x0: f64, x1: f64) -> f64 {
        lines.iter().flat_map(|l| l.windows(2)).filter(|s| s[0].x >= x0 && s[0].x < x1).map(|s| (s[1] - s[0]).len()).sum()
    }

    #[test]
    fn darker_bands_get_more_crossing_layers() {
        let prm = VecParams { mode: VecMode::CrossHatch, max_px: 320, ..Default::default() };
        let res = vectorize(&steps(), &prm);
        let lines = &res.layers[0].lines;
        let bands: Vec<f64> = (0..4).map(|b| ink_in(lines, b as f64 * 80.0 + 8.0, b as f64 * 80.0 + 72.0)).collect();
        assert!(bands[0] < bands[1] && bands[1] < bands[2] && bands[2] < bands[3], "{bands:?}");
        assert!(bands[0] < bands[3] * 0.2, "{bands:?}");
    }

    #[test]
    fn burin_grooves_widen_in_the_shadows() {
        let prm = VecParams { mode: VecMode::Burin, max_px: 320, burin: BurinParams { relief: 0.0, angle: 0.0, ..Default::default() }, ..Default::default() };
        let res = vectorize(&steps(), &prm);
        let lines = &res.layers[0].lines;
        let bands: Vec<f64> = (0..4).map(|b| ink_in(lines, b as f64 * 80.0 + 8.0, b as f64 * 80.0 + 72.0)).collect();
        // Con rayas horizontales a la misma separación, el surco ancho multiplica la tinta.
        assert!(bands[0] < bands[1] && bands[1] < bands[2] && bands[2] < bands[3], "{bands:?}");
        assert!(bands[3] > bands[1] * 1.6, "{bands:?}");
    }
}
