//! Topografía por Fast Marching, adaptación nativa de FastMarchingTopoPlot
//! de Roland Blok (Unlicense): https://github.com/rolandblok/FastMarchingTopoPlot.
//! Véanse `third_party/FastMarchingTopoPlot` y `docs/referencia/topografia.md`.
//!
//! Luz → velocidad F → solución de |∇T| F = 1 → isolíneas del tiempo T.
//! El alfa recorta los trazos; no impide que la onda atraviese el fondo blanco.

use crate::geom::{simplify, Polyline, P};
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
    pub lines: usize,
    pub gamma: f64,
    pub speed_offset: f64,
    /// Coordenadas normalizadas, de izquierda a derecha y de arriba abajo.
    pub seed: [f64; 2],
    pub skip_transparent: bool,
    pub pen_width_mm: f64,
    /// Tolerancia de simplificación, en píxeles de trabajo. Menor que la usada
    /// en siluetas para conservar la curvatura de las isolíneas muy próximas.
    pub detail: f64,
}

impl Default for TopoParams {
    fn default() -> Self {
        Self {
            lines: 200,
            gamma: 1.0,
            speed_offset: 0.1,
            seed: [0.5, 0.5],
            skip_transparent: true,
            pen_width_mm: 0.15,
            detail: 0.1,
        }
    }
}

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
    let mut gray = Grid::new(w, h);
    for (v, px) in gray.data.iter_mut().zip(rgb.pixels()) {
        // Mismos coeficientes de gris que el proyecto original.
        *v = (0.299 * px[0] as f64 + 0.587 * px[1] as f64 + 0.114 * px[2] as f64) as f32 / 255.0;
    }
    let mut speed = gaussian_blur(&gray, prm.blur.clamp(0.0, 20.0) as f64);
    let gamma = cfg.gamma.clamp(0.2, 4.0);
    let offset = cfg.speed_offset.clamp(0.01, 1.0);
    for v in &mut speed.data {
        *v = (v.clamp(0.0, 1.0) as f64).powf(gamma) as f32 + offset as f32;
    }
    let time = travel_time(&speed, cfg.seed);
    let (lo, hi) = time
        .data
        .iter()
        .filter(|t| t.is_finite())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &t| {
            (lo.min(t), hi.max(t))
        });
    if !lo.is_finite() || hi <= lo {
        return out;
    }
    let count = cfg.lines.clamp(2, 600);
    let mask = (cfg.skip_transparent && alpha.data.iter().any(|a| *a < 0.5)).then_some(&alpha);
    let lines: Vec<Polyline> = (0..count)
        .into_par_iter()
        .flat_map_iter(|i| {
            let level = lo + (hi - lo) * (i + 1) as f32 / (count + 1) as f32;
            raster::contours(&time, level)
                .into_iter()
                .flat_map(|line| {
                    let line = simplify(&line, cfg.detail.clamp(0.0, 2.0));
                    match mask {
                        Some(alpha) => clip_to_alpha(&line, alpha),
                        None => vec![line],
                    }
                })
                .filter(|line| line.len() >= 2)
                .collect::<Vec<_>>()
        })
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
    out
}

/// Gaussiano separable sobre valores flotantes, con extensión constante del borde.
/// Se conserva la precisión de las luces antes de aplicar gamma.
fn gaussian_blur(src: &Grid<f32>, sigma: f64) -> Grid<f32> {
    if sigma <= 0.0 || src.w == 0 || src.h == 0 {
        return src.clone();
    }
    let r = (sigma * 3.0).ceil() as isize;
    let mut kernel: Vec<_> = (-r..=r)
        .map(|x| (-(x * x) as f64 / (2.0 * sigma * sigma)).exp())
        .collect();
    let total: f64 = kernel.iter().sum();
    for v in &mut kernel {
        *v /= total;
    }
    let mut tmp = Grid::new(src.w, src.h);
    let mut dst = Grid::new(src.w, src.h);
    tmp.data
        .par_chunks_mut(src.w)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, out) in row.iter_mut().enumerate() {
                *out = kernel
                    .iter()
                    .enumerate()
                    .map(|(i, weight)| {
                        let xx =
                            (x as isize + i as isize - r).clamp(0, src.w as isize - 1) as usize;
                        src.get(xx, y) as f64 * weight
                    })
                    .sum::<f64>() as f32;
            }
        });
    dst.data
        .par_chunks_mut(src.w)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, out) in row.iter_mut().enumerate() {
                *out = kernel
                    .iter()
                    .enumerate()
                    .map(|(i, weight)| {
                        let yy =
                            (y as isize + i as isize - r).clamp(0, src.h as isize - 1) as usize;
                        tmp.get(x, yy) as f64 * weight
                    })
                    .sum::<f64>() as f32;
            }
        });
    dst
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

/// Resuelve el mapa de tiempos desde un origen. Las velocidades no positivas o
/// no finitas son obstáculos; los píxeles inaccesibles quedan en infinito.
pub fn travel_time(speed: &Grid<f32>, seed: [f64; 2]) -> Grid<f32> {
    let (w, h) = (speed.w, speed.h);
    let mut time = Grid::new(w, h);
    time.data.fill(f32::INFINITY);
    if w == 0 || h == 0 || speed.data.len() != w * h {
        return time;
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
        return time;
    }
    let mut frozen = vec![false; w * h];
    let mut heap = BinaryHeap::new();
    time.data[seed] = 0.0;
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
            let mut tx = f64::INFINITY;
            let mut ty = f64::INFINITY;
            if nx > 0 && frozen[i - 1] {
                tx = tx.min(time.data[i - 1] as f64);
            }
            if nx + 1 < w && frozen[i + 1] {
                tx = tx.min(time.data[i + 1] as f64);
            }
            if ny > 0 && frozen[i - w] {
                ty = ty.min(time.data[i - w] as f64);
            }
            if ny + 1 < h && frozen[i + w] {
                ty = ty.min(time.data[i + w] as f64);
            }
            let next = arrival(tx, ty, 1.0 / speed.data[i] as f64);
            if next < time.data[i] as f64 {
                time.data[i] = next as f32;
                heap.push(Trial {
                    time: next,
                    index: i,
                });
            }
        }
    }
    time
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
fn clip_to_alpha(line: &Polyline, alpha: &Grid<f32>) -> Vec<Polyline> {
    let Some(&first) = line.first() else {
        return vec![];
    };
    let mut chunks = vec![];
    let mut current = vec![];
    let (mut previous, mut was_visible) = (first, visible(alpha, first));
    if was_visible {
        current.push(first);
    }
    for edge in line.windows(2) {
        let steps = (edge[0].dist(edge[1]) / 0.25).ceil().max(1.0) as usize;
        for i in 1..=steps {
            let q = edge[0].lerp(edge[1], i as f64 / steps as f64);
            let is_visible = visible(alpha, q);
            if was_visible != is_visible {
                let (mut lo, mut hi) = (previous, q);
                for _ in 0..12 {
                    let mid = lo.lerp(hi, 0.5);
                    if visible(alpha, mid) == was_visible {
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
        let t = travel_time(&speed, [0.5, 0.5]);
        for i in 0..=20 {
            assert_eq!(t.get(20 + i, 20), i as f32);
            assert!((t.get(20 + i, 20 + i) - t.get(20 - i, 20 - i)).abs() < 1e-5);
        }
        assert!((t.get(40, 40) - 20.0_f32 * 2.0_f32.sqrt()).abs() < 1.1);
        speed.data.fill(0.5);
        let slow = travel_time(&speed, [0.5, 0.5]);
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
        for (a, b) in travel_time(&speed, [0.5, 0.25])
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
        let t = travel_time(&speed, [0.0, 0.0]);
        assert!((t.get(60, 0) - t.get(59, 0)) > 9.0 * (t.get(20, 0) - t.get(19, 0)));
        let mut blocked = Grid::new(4, 1);
        blocked.data = vec![1.0, 0.0, f32::NAN, 1.0];
        let t = travel_time(&blocked, [0.0, 0.0]);
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
                lines: 25,
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
}
