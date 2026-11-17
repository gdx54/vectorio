//! Análisis previo de la foto, común a los estilos de dibujo más finos:
//! - Luz suavizada con un filtro guiado (alisa la piel y el cielo sin borrar bordes).
//! - Contraste local: separa los tonos cercanos dentro de cada zona.
//! - Tensor estructural suavizado: la dirección de las formas en cada punto.
//! - Importancia: dónde hay bordes y detalle que merecen más trazo.
//!
//! Todas las escalas van referidas a una imagen de 900 px de lado mayor, para que el
//! resultado no cambie de carácter al subir o bajar la resolución de trabajo.

use crate::geom::{p, P};
use crate::raster::Grid;
use image::RgbImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisParams {
    /// Suavizado que respeta los bordes, 0–1.
    pub smooth: f32,
    /// Contraste local, 0–1.
    pub local_contrast: f32,
}

impl Default for AnalysisParams {
    fn default() -> Self {
        AnalysisParams { smooth: 0.5, local_contrast: 0.35 }
    }
}

/// Peso del color frente a la luz en la dirección de las formas y en los bordes.
pub const CHROMA_WEIGHT: f32 = 0.6;

pub struct Analysis {
    pub w: usize,
    pub h: usize,
    /// Píxeles de esta imagen por cada píxel de la imagen de referencia de 900 px.
    pub scale: f64,
    /// Luz 0 (negro) – 1 (blanco), ya suavizada y con el contraste local aplicado.
    pub light: Grid<f32>,
    /// Color oponente (rojo–verde, amarillo–azul), alrededor de 0 y suavizado: separa
    /// zonas de color distinto con la misma luz, como un loro rojo sobre hojas verdes.
    pub chroma: [Grid<f32>; 2],
    /// Jxx, Jxy, Jyy de la luz y el color, suavizados.
    pub tensor: Grid<[f32; 3]>,
    /// Bordes y detalle, 0–1.
    pub importance: Grid<f32>,
}

impl Analysis {
    pub fn new(rgb: &RgbImage, prm: &AnalysisParams, blur: f32) -> Analysis {
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
        let scale = w.max(h) as f64 / 900.0;
        let mut light = Grid::<f32>::new(w, h);
        let mut chroma = [Grid::<f32>::new(w, h), Grid::<f32>::new(w, h)];
        for (i, px) in rgb.pixels().enumerate() {
            let [r, g, b] = [px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0];
            light.data[i] = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            chroma[0].data[i] = 0.5 * (r - g);
            chroma[1].data[i] = 0.5 * (0.5 * (r + g) - b);
        }
        if blur > 0.05 {
            light = gaussian(&light, blur as f64);
            chroma = chroma.map(|c| gaussian(&c, blur as f64));
        }
        // Filtro guiado por la propia imagen: dos pasadas, con radio y tolerancia que
        // crecen con la intensidad pedida.
        let smooth = prm.smooth.clamp(0.0, 1.0);
        if smooth > 0.01 {
            let radius = ((2.0 + 6.0 * smooth as f64) * scale).round().max(1.0) as usize;
            let eps = 0.0004 + 0.012 * smooth * smooth;
            for _ in 0..2 {
                light = guided(&light, radius, eps);
            }
            chroma = chroma.map(|c| guided(&c, radius, eps));
        }
        let amount = prm.local_contrast.clamp(0.0, 1.0);
        if amount > 0.01 {
            let base = gaussian(&light, 28.0 * scale);
            light.data.par_iter_mut().zip(base.data.par_iter()).for_each(|(v, b)| {
                *v = (*v + 1.6 * amount * (*v - b)).clamp(0.0, 1.0);
            });
        }
        // El color cuenta algo menos que la luz para la dirección y el detalle.
        let (mut tensor, mut magnitude) = structure(&light);
        for c in &chroma {
            let (t, m) = structure(c);
            for (a, b) in tensor.data.iter_mut().zip(&t.data) {
                for k in 0..3 {
                    a[k] += CHROMA_WEIGHT * b[k];
                }
            }
            for (a, b) in magnitude.data.iter_mut().zip(&m.data) {
                *a = (*a * *a + CHROMA_WEIGHT * b * b).sqrt();
            }
        }
        let sigma = 3.0 * scale;
        let radius = box_radius(sigma);
        for _ in 0..3 {
            tensor = box_tensor(&box_tensor(&tensor, radius, false), radius, true);
        }
        let mut importance = gaussian(&magnitude, 1.5 * scale);
        let top = percentile(&importance.data, 0.985).max(1e-4);
        importance.data.par_iter_mut().for_each(|v| *v = (*v / top).clamp(0.0, 1.0).sqrt());
        Analysis { w, h, scale, light, chroma, tensor, importance }
    }

    /// Dirección de las formas (sin signo) y cuánto se puede confiar en ella, 0–1.
    pub fn tangent(&self, x: usize, y: usize) -> (P, f32) {
        let [a, b, c] = self.tensor.get(x, y);
        let anisotropy = (a - c).hypot(2.0 * b);
        let energy = a + c;
        let coherence = if energy > 1e-9 { (anisotropy / energy).clamp(0.0, 1.0) } else { 0.0 };
        // Autovector del autovalor menor: paralelo a los bordes.
        let angle = 0.5 * (2.0 * b).atan2(a - c) + std::f32::consts::FRAC_PI_2;
        (p(angle.cos() as f64, angle.sin() as f64), coherence)
    }
}

/// Sobel de la luz: tensor sin suavizar y módulo del gradiente.
fn structure(light: &Grid<f32>) -> (Grid<[f32; 3]>, Grid<f32>) {
    let (w, h) = (light.w, light.h);
    let at = |x: isize, y: isize| light.data[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize];
    let rows: Vec<(Vec<[f32; 3]>, Vec<f32>)> = (0..h)
        .into_par_iter()
        .map(|y| {
            let y = y as isize;
            let mut t = Vec::with_capacity(w);
            let mut m = Vec::with_capacity(w);
            for x in 0..w as isize {
                let gx = (at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1) - at(x - 1, y - 1) - 2.0 * at(x - 1, y) - at(x - 1, y + 1)) / 8.0;
                let gy = (at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1) - at(x - 1, y - 1) - 2.0 * at(x, y - 1) - at(x + 1, y - 1)) / 8.0;
                t.push([gx * gx, gx * gy, gy * gy]);
                m.push(gx.hypot(gy));
            }
            (t, m)
        })
        .collect();
    let mut tensor = Grid::new(w, h);
    let mut magnitude = Grid::new(w, h);
    for (y, (t, m)) in rows.into_iter().enumerate() {
        tensor.data[y * w..(y + 1) * w].copy_from_slice(&t);
        magnitude.data[y * w..(y + 1) * w].copy_from_slice(&m);
    }
    (tensor, magnitude)
}

/// Filtro guiado de He et al. con la imagen como guía: alisa sin cruzar bordes.
fn guided(i: &Grid<f32>, radius: usize, eps: f32) -> Grid<f32> {
    let sq = Grid { w: i.w, h: i.h, data: i.data.iter().map(|v| v * v).collect() };
    let mean = box_blur(i, radius);
    let mean_sq = box_blur(&sq, radius);
    let mut a = Grid::<f32>::new(i.w, i.h);
    let mut b = Grid::<f32>::new(i.w, i.h);
    for k in 0..i.data.len() {
        let var = (mean_sq.data[k] - mean.data[k] * mean.data[k]).max(0.0);
        a.data[k] = var / (var + eps);
        b.data[k] = mean.data[k] * (1.0 - a.data[k]);
    }
    let (a, b) = (box_blur(&a, radius), box_blur(&b, radius));
    Grid { w: i.w, h: i.h, data: (0..i.data.len()).map(|k| a.data[k] * i.data[k] + b.data[k]).collect() }
}

fn box_radius(sigma: f64) -> usize {
    (((1.0 + 4.0 * sigma * sigma).sqrt() - 1.0) / 2.0).round().max(1.0) as usize
}

/// Gaussiano aproximado con tres cajas separables, en tiempo lineal.
pub fn gaussian(g: &Grid<f32>, sigma: f64) -> Grid<f32> {
    if sigma < 0.3 {
        return g.clone();
    }
    let radius = box_radius(sigma);
    let mut out = g.clone();
    for _ in 0..3 {
        out = box_blur(&out, radius);
    }
    out
}

/// Media en una caja de lado 2·radius+1, con el borde repetido.
pub fn box_blur(g: &Grid<f32>, radius: usize) -> Grid<f32> {
    let horizontal = box_pass(&g.data, g.w, g.h, radius);
    let transposed = transpose(&horizontal, g.w, g.h);
    let vertical = box_pass(&transposed, g.h, g.w, radius);
    Grid { w: g.w, h: g.h, data: transpose(&vertical, g.h, g.w) }
}

fn box_pass(data: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    let mut out = vec![0.0; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let src = &data[y * w..(y + 1) * w];
        let at = |x: isize| src[x.clamp(0, w as isize - 1) as usize] as f64;
        let r = radius as isize;
        let mut sum: f64 = (-r..=r).map(at).sum();
        let n = (2 * radius + 1) as f64;
        for (x, v) in row.iter_mut().enumerate() {
            *v = (sum / n) as f32;
            let x = x as isize;
            sum += at(x + r + 1) - at(x - r);
        }
    });
    out
}

fn transpose(data: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0; w * h];
    out.par_chunks_mut(h).enumerate().for_each(|(x, col)| {
        for (y, v) in col.iter_mut().enumerate() {
            *v = data[y * w + x];
        }
    });
    out
}

fn box_tensor(src: &Grid<[f32; 3]>, radius: usize, vertical: bool) -> Grid<[f32; 3]> {
    let channel = |c: usize| Grid { w: src.w, h: src.h, data: src.data.iter().map(|v| v[c]).collect::<Vec<f32>>() };
    let blurred: Vec<Grid<f32>> = (0..3)
        .map(|c| {
            let g = channel(c);
            if vertical {
                let t = transpose(&g.data, g.w, g.h);
                Grid { w: g.w, h: g.h, data: transpose(&box_pass(&t, g.h, g.w, radius), g.h, g.w) }
            } else {
                Grid { w: g.w, h: g.h, data: box_pass(&g.data, g.w, g.h, radius) }
            }
        })
        .collect();
    Grid { w: src.w, h: src.h, data: (0..src.data.len()).map(|k| [blurred[0].data[k], blurred[1].data[k], blurred[2].data[k]]).collect() }
}

fn percentile(data: &[f32], q: f64) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let mut sample: Vec<f32> = data.iter().step_by((data.len() / 20_000).max(1)).copied().collect();
    let k = ((sample.len() - 1) as f64 * q) as usize;
    let (_, v, _) = sample.select_nth_unstable_by(k, f32::total_cmp);
    *v
}

/// Lectura bilineal con el borde repetido.
pub fn sample(g: &Grid<f32>, q: P) -> f32 {
    let x = q.x.clamp(0.0, (g.w - 1) as f64);
    let y = q.y.clamp(0.0, (g.h - 1) as f64);
    let (ix, iy) = (x as usize, y as usize);
    let (jx, jy) = ((ix + 1).min(g.w - 1), (iy + 1).min(g.h - 1));
    let (tx, ty) = ((x - ix as f64) as f32, (y - iy as f64) as f32);
    let top = g.get(ix, iy) * (1.0 - tx) + g.get(jx, iy) * tx;
    let bottom = g.get(ix, jy) * (1.0 - tx) + g.get(jx, jy) * tx;
    top * (1.0 - ty) + bottom * ty
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step_image() -> RgbImage {
        let mut rng = crate::geom::Rng::new(3);
        RgbImage::from_fn(200, 120, |x, _| {
            let base = if x < 100 { 60.0 } else { 200.0 };
            let v = (base + rng.sym() * 25.0).clamp(0.0, 255.0) as u8;
            image::Rgb([v; 3])
        })
    }

    #[test]
    fn smoothing_removes_noise_but_keeps_the_edge() {
        let img = step_image();
        let flat = Analysis::new(&img, &AnalysisParams { smooth: 0.0, local_contrast: 0.0 }, 0.0);
        let smooth = Analysis::new(&img, &AnalysisParams { smooth: 1.0, local_contrast: 0.0 }, 0.0);
        let spread = |a: &Analysis| {
            let v: Vec<f32> = (20..80).flat_map(|x| (20..100).map(move |y| (x, y))).map(|(x, y)| a.light.get(x, y)).collect();
            let m = v.iter().sum::<f32>() / v.len() as f32;
            (v.iter().map(|x| (x - m).powi(2)).sum::<f32>() / v.len() as f32).sqrt()
        };
        assert!(spread(&smooth) < spread(&flat) * 0.5, "{} {}", spread(&smooth), spread(&flat));
        let jump = smooth.light.get(104, 60) - smooth.light.get(96, 60);
        assert!(jump > 0.35, "el borde debe seguir marcado: {jump}");
    }

    #[test]
    fn importance_peaks_on_edges_and_tangent_follows_them() {
        let img = step_image();
        let a = Analysis::new(&img, &AnalysisParams::default(), 0.0);
        assert!(a.importance.get(100, 60) > 0.8);
        assert!(a.importance.get(40, 60) < 0.5);
        let (t, coherence) = a.tangent(100, 60);
        assert!(t.y.abs() > 0.9 && coherence > 0.5, "{t:?} {coherence}");
    }
}
