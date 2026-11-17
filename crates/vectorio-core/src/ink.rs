//! Tinta: contornos coherentes al estilo de un dibujo a plumilla.
//!
//! Sigue la idea de las «líneas coherentes» (diferencia de gaussianas guiada por el
//! flujo): en cada punto se mide el contraste a través del borde (perpendicular a la
//! dirección de las formas) y luego se acumula a lo largo de esa dirección. Así los
//! bordes débiles pero continuos sobreviven y el ruido suelto desaparece. La máscara
//! resultante se adelgaza hasta su eje y se recorre como trazos abiertos.
//!
//! Opcionalmente se añaden los contornos de las zonas de sombra (niveles de luz),
//! como las manchas de negro que delimita un ilustrador.

use crate::analysis::{self, Analysis};
use crate::geom::{p, Polyline, P};
use crate::raster::{self, Grid};
use crate::vectorize::{VecLayer, VecParams};
use image::RgbImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InkParams {
    /// Escala de los bordes que se buscan, px referidos a 900 px: valores altos
    /// ignoran el detalle fino y dejan solo las formas grandes.
    pub scale: f64,
    /// Sensibilidad 0–1: más alta encuentra bordes más suaves.
    pub sensitivity: f64,
    /// Longitud con que se acumula el borde a lo largo de las formas, px referidos a
    /// 900 px. Une los trazos y alisa su recorrido.
    pub continuity: f64,
    /// Contornos de sombra: 0 = ninguno, hasta 4 niveles de luz.
    pub shadow_levels: usize,
    /// Trazos más cortos que esto (px referidos a 900 px) se descartan.
    pub min_length: f64,
}

impl Default for InkParams {
    fn default() -> Self {
        InkParams { scale: 1.2, sensitivity: 0.7, continuity: 4.0, shadow_levels: 0, min_length: 8.0 }
    }
}

pub fn render(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    if rgb.width() < 8 || rgb.height() < 8 {
        return vec![];
    }
    let cfg = &prm.ink;
    let a = Analysis::new(rgb, &prm.analysis, prm.blur);
    let mask = edge_mask(&a, cfg);
    let mut lines = trace_mask(mask, a.scale, cfg);
    if cfg.shadow_levels > 0 {
        // Cada nivel se recorta contra lo ya dibujado, bordes y niveles anteriores.
        for level in shadow_contours(&a, cfg) {
            let kept = away_from(level, &lines, a.w, a.h, (2.5 * a.scale).max(1.5), (cfg.min_length * a.scale).max(3.0));
            lines.extend(kept);
        }
    }
    vec![VecLayer::lines([20, 20, 24], false, lines)]
}

/// Contornos coherentes ya recorridos como trazos, en px de la imagen.
pub(crate) fn contour_lines(a: &Analysis, cfg: &InkParams) -> Vec<Polyline> {
    trace_mask(edge_mask(a, cfg), a.scale, cfg)
}

/// Respuesta de diferencia de gaussianas a través del borde, acumulada a lo largo de
/// las formas. Negativa en el lado oscuro de cada borde.
fn flow_dog(a: &Analysis, cfg: &InkParams) -> Grid<f32> {
    let (w, h) = (a.w, a.h);
    let sigma_c = (cfg.scale.clamp(0.3, 6.0) * a.scale).max(0.6);
    let sigma_s = 1.6 * sigma_c;
    let rho = 0.99;
    let reach = (3.0 * sigma_s).ceil() as i32;
    let gauss = |t: f64, s: f64| (-(t * t) / (2.0 * s * s)).exp() / s;
    // (desplazamiento, peso para la luz, peso para el color). El color usa una
    // diferencia exacta, sin el pequeño sesgo de la luz: una zona roja uniforme no
    // debe parecer borde.
    let kernel: Vec<(f64, f64, f64)> = (-reach..=reach)
        .map(|t| t as f64)
        .map(|t| (t, gauss(t, sigma_c) - rho * gauss(t, sigma_s), gauss(t, sigma_c) - gauss(t, sigma_s)))
        .collect();
    let norm_c: f64 = (-reach..=reach).map(|t| gauss(t as f64, sigma_c)).sum();
    let tangents: Vec<(P, f32)> = (0..w * h).into_par_iter().map(|i| a.tangent(i % w, i / w)).collect();
    // 1. A través del borde: la normal es la tangente girada 90°. La luz da el lado
    // oscuro de cada borde; el color añade los bordes entre zonas de igual luz.
    let chroma_weight = analysis::CHROMA_WEIGHT.sqrt() as f64;
    let mut across = Grid::<f32>::new(w, h);
    across.data.par_iter_mut().enumerate().for_each(|(i, out)| {
        let (x, y) = ((i % w) as f64, (i / w) as f64);
        let t = tangents[i].0;
        let n = p(-t.y, t.x);
        let mut sums = [0.0f64; 3];
        for &(s, k, kc) in &kernel {
            let q = p(x + n.x * s, y + n.y * s);
            sums[0] += k * analysis::sample(&a.light, q) as f64;
            sums[1] += kc * analysis::sample(&a.chroma[0], q) as f64;
            sums[2] += kc * analysis::sample(&a.chroma[1], q) as f64;
        }
        // En el color no hay lado oscuro: cada canal marca su lado negativo, así cada
        // borde de color sale una sola vez.
        let color = sums[1].min(sums[2]) * chroma_weight;
        *out = (sums[0].min(color) / norm_c) as f32;
    });
    // 2. A lo largo de las formas, siguiendo el campo de direcciones en ambos sentidos.
    let sigma_m = (cfg.continuity.clamp(0.0, 20.0) * a.scale).max(0.5);
    let steps = (2.5 * sigma_m).ceil() as usize;
    let mut along = Grid::<f32>::new(w, h);
    along.data.par_iter_mut().enumerate().for_each(|(i, out)| {
        let start = p((i % w) as f64, (i / w) as f64);
        let mut sum = across.data[i] as f64;
        let mut weight = 1.0;
        for sign in [1.0, -1.0] {
            let mut q = start;
            let mut dir = tangents[i].0 * sign;
            for k in 1..=steps {
                let (ix, iy) = (q.x.round() as isize, q.y.round() as isize);
                if ix < 0 || iy < 0 || ix >= w as isize || iy >= h as isize {
                    break;
                }
                let mut t = tangents[iy as usize * w + ix as usize].0;
                if t.dot(dir) < 0.0 {
                    t = t * -1.0;
                }
                dir = t;
                q = q + dir;
                if q.x < 0.0 || q.y < 0.0 || q.x > (w - 1) as f64 || q.y > (h - 1) as f64 {
                    break;
                }
                let g = (-((k * k) as f64) / (2.0 * sigma_m * sigma_m)).exp();
                sum += g * analysis::sample(&across, q) as f64;
                weight += g;
            }
        }
        *out = (sum / weight) as f32;
    });
    along
}

/// Umbral con histéresis: un borde fuerte se queda con todos los débiles que le
/// tocan, así un contorno no se corta donde la foto pierde un poco de contraste.
fn edge_mask(a: &Analysis, cfg: &InkParams) -> Grid<bool> {
    let response = flow_dog(a, cfg);
    let strong = -(0.002 + 0.03 * (1.0 - cfg.sensitivity.clamp(0.0, 1.0)).powi(2)) as f32;
    let weak = strong * 0.4;
    let (w, h) = (a.w, a.h);
    let mut mask = Grid { w, h, data: vec![false; w * h] };
    let mut stack: Vec<usize> = (0..w * h).filter(|&i| response.data[i] < strong).collect();
    for &i in &stack {
        mask.data[i] = true;
    }
    while let Some(i) = stack.pop() {
        let (x, y) = ((i % w) as isize, (i / w) as isize);
        for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                continue;
            }
            let j = ny as usize * w + nx as usize;
            if !mask.data[j] && response.data[j] < weak {
                mask.data[j] = true;
                stack.push(j);
            }
        }
    }
    mask
}

fn trace_mask(mut mask: Grid<bool>, scale: f64, cfg: &InkParams) -> Vec<Polyline> {
    crate::vectorize::remove_specks(&mut mask, (4.0 * scale * scale).ceil() as usize);
    raster::thin(&mut mask);
    let min_length = cfg.min_length.max(0.0) * scale;
    let strokes = join_strokes(raster::trace_skeleton(&mask), (3.0 * scale).max(2.0));
    strokes
        .into_par_iter()
        .map(|l| crate::geom::chaikin(&crate::geom::simplify(&l, 0.6 * scale.max(0.5)), 2, false))
        .filter(|l| l.len() >= 2 && crate::geom::length(l) >= min_length.max(2.0))
        .collect()
}

/// Dirección con que sale un trazo por su principio, mirando unos píxeles adentro.
fn heading(l: &[P]) -> P {
    let k = l.len().min(6) - 1;
    let d = l[0] - l[k];
    let n = d.len();
    if n > 1e-9 { d * (1.0 / n) } else { p(0.0, 0.0) }
}

/// Une trazos cuyos extremos quedan cerca y apuntan uno hacia el otro: el
/// esqueleto parte los contornos en cruces y huecos pequeños, y un dibujante los
/// habría hecho de un solo gesto.
fn join_strokes(mut lines: Vec<Polyline>, gap: f64) -> Vec<Polyline> {
    lines.retain(|l| l.len() >= 2);
    let n = lines.len();
    // Extremo `e`: trazo e / 2, lado e % 2 (0 = principio, 1 = final). Cada uno con
    // su posición y la dirección hacia la que sale.
    let ends: Vec<(P, P)> = (0..2 * n)
        .map(|e| {
            let l = &lines[e / 2];
            if e % 2 == 0 {
                (l[0], heading(l))
            } else {
                let rev: Polyline = l.iter().rev().take(6).copied().collect();
                (*l.last().unwrap(), heading(&rev))
            }
        })
        .collect();
    let cell = |q: P| ((q.x / gap).floor() as i64, (q.y / gap).floor() as i64);
    let mut buckets: std::collections::HashMap<(i64, i64), Vec<usize>> = std::collections::HashMap::new();
    for (e, (q, _)) in ends.iter().enumerate() {
        buckets.entry(cell(*q)).or_default().push(e);
    }
    let mut candidates: Vec<(f64, usize, usize)> = Vec::new();
    for (e, &(qi, di)) in ends.iter().enumerate() {
        let (cx, cy) = cell(qi);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let Some(list) = buckets.get(&(cx + dx, cy + dy)) else { continue };
                for &f in list {
                    if f <= e || f / 2 == e / 2 {
                        continue;
                    }
                    let (qj, dj) = ends[f];
                    let d = (qj - qi).len();
                    if d > gap {
                        continue;
                    }
                    // Los dos extremos deben mirarse: salir uno hacia el otro.
                    let straight = -di.dot(dj);
                    if d > 1.5 {
                        let towards = (qj - qi) * (1.0 / d);
                        if di.dot(towards) - dj.dot(towards) < 0.6 || straight < 0.2 {
                            continue;
                        }
                    }
                    candidates.push((d / gap - 0.5 * straight, e, f));
                }
            }
        }
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Uniones sin ciclos: cada extremo se usa una vez y nunca se cierra una cadena.
    let mut link = vec![usize::MAX; 2 * n];
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (_, e, f) in candidates {
        if link[e] != usize::MAX || link[f] != usize::MAX {
            continue;
        }
        let (re, rf) = (root(&mut parent, e / 2), root(&mut parent, f / 2));
        if re == rf {
            continue;
        }
        parent[re] = rf;
        link[e] = f;
        link[f] = e;
    }
    // Cada cadena empieza en un extremo suelto y se recorre de trazo en trazo.
    let mut used = vec![false; n];
    let mut out = Vec::new();
    for start in 0..2 * n {
        if link[start] != usize::MAX || used[start / 2] {
            continue;
        }
        let mut chain: Polyline = Vec::new();
        let mut enter = start;
        loop {
            let s = enter / 2;
            used[s] = true;
            if enter % 2 == 0 {
                chain.extend(lines[s].iter().copied());
            } else {
                chain.extend(lines[s].iter().rev().copied());
            }
            let exit = enter ^ 1;
            if link[exit] == usize::MAX {
                break;
            }
            enter = link[exit];
        }
        out.push(chain);
    }
    out
}

/// Recorta las líneas donde pasan pegadas a otros trazos: un contorno de sombra que
/// coincide con un borde ya dibujado saldría doble.
fn away_from(lines: Vec<Polyline>, drawn: &[Polyline], w: usize, h: usize, reach: f64, min_length: f64) -> Vec<Polyline> {
    let mut near = Grid::<bool>::new(w, h);
    let r = reach.ceil() as isize;
    for l in drawn {
        for seg in l.windows(2) {
            let steps = ((seg[1] - seg[0]).len().ceil() as usize).max(1);
            for k in 0..=steps {
                let q = seg[0] + (seg[1] - seg[0]) * (k as f64 / steps as f64);
                for dy in -r..=r {
                    for dx in -r..=r {
                        let (x, y) = (q.x.round() as isize + dx, q.y.round() as isize + dy);
                        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && ((dx * dx + dy * dy) as f64) <= reach * reach {
                            near.set(x as usize, y as usize, true);
                        }
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for l in lines {
        let mut run: Polyline = Vec::new();
        for q in l {
            if near.at(q.x.round() as isize, q.y.round() as isize) {
                if run.len() >= 2 && crate::geom::length(&run) >= min_length {
                    out.push(std::mem::take(&mut run));
                }
                run.clear();
            } else {
                run.push(q);
            }
        }
        if run.len() >= 2 && crate::geom::length(&run) >= min_length {
            out.push(run);
        }
    }
    out
}

/// Contornos de la luz a varios niveles: delimitan las sombras sin rellenarlas.
fn shadow_contours(a: &Analysis, cfg: &InkParams) -> Vec<Vec<Polyline>> {
    let levels = cfg.shadow_levels.min(4);
    let field = analysis::gaussian(&a.light, 2.0 * a.scale);
    let min_length = (cfg.min_length.max(0.0) * 2.0 * a.scale).max(4.0);
    (1..=levels)
        .map(|k| {
            // Los niveles se reparten por los tonos oscuros y medios.
            let level = 0.12 + 0.48 * k as f32 / (levels + 1) as f32;
            raster::contours(&field, level)
                .into_iter()
                .map(|l| crate::geom::chaikin(&crate::geom::simplify(&l, 0.8 * a.scale.max(0.5)), 2, false))
                .filter(|l| l.len() >= 2 && crate::geom::length(l) >= min_length)
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectorize::{vectorize, VecMode};
    use image::DynamicImage;

    fn face() -> DynamicImage {
        let mut rng = crate::geom::Rng::new(9);
        DynamicImage::ImageRgb8(RgbImage::from_fn(300, 360, |x, y| {
            let (dx, dy) = (x as f64 - 150.0, y as f64 - 170.0);
            let r = (dx * dx / 90.0f64.powi(2) + dy * dy / 120.0f64.powi(2)).sqrt();
            let base = if r < 1.0 { 215.0 - 70.0 * (dx / 90.0).max(0.0) } else { 70.0 };
            image::Rgb([(base + rng.sym() * 12.0).clamp(0.0, 255.0) as u8; 3])
        }))
    }

    #[test]
    fn ink_draws_the_outline_as_few_long_strokes_and_ignores_noise() {
        let prm = VecParams { mode: VecMode::Ink, max_px: 360, ..Default::default() };
        let res = vectorize(&face(), &prm);
        let lines = &res.layers[0].lines;
        assert!(!lines.is_empty());
        let total: f64 = lines.iter().map(|l| crate::geom::length(l)).sum();
        let longest = lines.iter().map(|l| crate::geom::length(l)).fold(0.0, f64::max);
        // El contorno de la elipse mide unos 660 px: casi todo debe salir en trazos largos.
        assert!(longest > 150.0, "trazo más largo {longest}");
        assert!(total > 450.0 && total < 2500.0, "longitud total {total}");
        // Todo lo dibujado queda cerca del borde de la cara, no en el ruido del fondo.
        let near_edge = lines.iter().flatten().filter(|q| {
            let r = ((q.x - 150.0).powi(2) / 90.0f64.powi(2) + (q.y - 170.0).powi(2) / 120.0f64.powi(2)).sqrt();
            (r - 1.0).abs() < 0.12
        }).count();
        let all = lines.iter().map(|l| l.len()).sum::<usize>();
        assert!(near_edge as f64 > all as f64 * 0.85, "{near_edge} de {all}");
    }

    #[test]
    fn shadow_levels_add_contours() {
        let base = VecParams { mode: VecMode::Ink, max_px: 300, ..Default::default() };
        let shaded = VecParams { ink: InkParams { shadow_levels: 3, ..Default::default() }, ..base.clone() };
        let n = |prm: &VecParams| vectorize(&face(), prm).layers[0].lines.len();
        assert!(n(&shaded) > n(&base));
    }
}
