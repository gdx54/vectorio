//! Ajustes de color de una foto antes de separarla en tintas: brillo, contraste,
//! tonos medios, saturación, tono y temperatura. Cambian cómo se interpretan los
//! colores (qué es oscuro, qué colores salen, cuánto se distinguen), no la foto
//! guardada.

use image::RgbImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorAdjust {
    /// −1 (negro) … 1 (blanco).
    pub brightness: f32,
    /// −1 (gris plano) … 1 (muy contrastado).
    pub contrast: f32,
    /// −1 (sombras más oscuras) … 1 (sombras abiertas). Curva gamma.
    pub midtones: f32,
    /// 0 (blanco y negro) … 2 (colores muy vivos).
    pub saturation: f32,
    /// Giro del tono, grados −180 … 180.
    pub hue_deg: f32,
    /// −1 (frío, azulado) … 1 (cálido, anaranjado).
    pub warmth: f32,
    /// Estira los niveles para que lo más oscuro sea negro y lo más claro blanco.
    pub auto_levels: bool,
}

impl Default for ColorAdjust {
    fn default() -> Self {
        ColorAdjust { brightness: 0.0, contrast: 0.0, midtones: 0.0, saturation: 1.0, hue_deg: 0.0, warmth: 0.0, auto_levels: false }
    }
}

impl ColorAdjust {
    pub fn is_identity(&self) -> bool {
        *self == ColorAdjust::default()
    }

    /// Devuelve la imagen ajustada (o la misma si no hay ajustes).
    pub fn apply(&self, img: &RgbImage) -> RgbImage {
        if self.is_identity() {
            return img.clone();
        }
        let (lo, hi) = if self.auto_levels { levels(img) } else { (0.0, 1.0) };
        let lut = self.tone_lut(lo, hi);
        let sat = self.saturation.max(0.0);
        let rot = hue_matrix(self.hue_deg);
        let warm = [1.0 + 0.12 * self.warmth, 1.0, 1.0 - 0.12 * self.warmth];
        let colour = sat != 1.0 || self.hue_deg != 0.0 || self.warmth != 0.0;
        let mut out = img.clone();
        out.par_chunks_mut(3).for_each(|px| {
            // Primero la luz (curva común a los tres canales), luego el color.
            let mut c = [lut[px[0] as usize], lut[px[1] as usize], lut[px[2] as usize]];
            if colour {
                for (v, w) in c.iter_mut().zip(warm) {
                    *v *= w;
                }
                if self.hue_deg != 0.0 {
                    c = mul(&rot, c);
                }
                if sat != 1.0 {
                    let y = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
                    for v in &mut c {
                        *v = y + (*v - y) * sat;
                    }
                }
            }
            for (o, v) in px.iter_mut().zip(c) {
                *o = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        });
        out
    }

    /// Curva de luz: niveles, brillo, contraste (en S alrededor del gris medio) y gamma.
    fn tone_lut(&self, lo: f32, hi: f32) -> [f32; 256] {
        let gamma = 2f32.powf(-self.midtones * 1.5);
        let k = (self.contrast.clamp(-1.0, 1.0) * 0.99 + 1.0) / (1.0 - self.contrast.clamp(-1.0, 1.0) * 0.99).max(0.01);
        let mut lut = [0.0; 256];
        for (i, v) in lut.iter_mut().enumerate() {
            let mut x = ((i as f32 / 255.0 - lo) / (hi - lo).max(1e-3)).clamp(0.0, 1.0);
            x = x.powf(gamma);
            x = (x - 0.5) * k.sqrt() + 0.5;
            x += self.brightness * 0.5;
            *v = x;
        }
        lut
    }
}

/// Percentiles 0,5 % y 99,5 % de la luminancia, 0–1.
fn levels(img: &RgbImage) -> (f32, f32) {
    let mut hist = [0usize; 256];
    for px in img.pixels() {
        let y = 0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32;
        hist[y.round() as usize] += 1;
    }
    let n = hist.iter().sum::<usize>().max(1);
    let cut = n / 200;
    let (mut acc, mut lo) = (0, 0);
    while lo < 255 && acc + hist[lo] <= cut {
        acc += hist[lo];
        lo += 1;
    }
    let (mut acc, mut hi) = (0, 255);
    while hi > lo && acc + hist[hi] <= cut {
        acc += hist[hi];
        hi -= 1;
    }
    if hi <= lo {
        return (0.0, 1.0);
    }
    (lo as f32 / 255.0, hi as f32 / 255.0)
}

/// Giro de tono que conserva la luminancia (rotación alrededor del eje gris).
fn hue_matrix(deg: f32) -> [[f32; 3]; 3] {
    let (s, c) = deg.to_radians().sin_cos();
    let (lr, lg, lb) = (0.213, 0.715, 0.072);
    [
        [lr + c * (1.0 - lr) - s * lr, lg - c * lg - s * lg, lb - c * lb + s * (1.0 - lb)],
        [lr - c * lr + s * 0.143, lg + c * (1.0 - lg) + s * 0.140, lb - c * lb - s * 0.283],
        [lr - c * lr - s * (1.0 - lr), lg - c * lg + s * lg, lb + c * (1.0 - lb) + s * lb],
    ]
}

fn mul(m: &[[f32; 3]; 3], c: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|r| m[r][0] * c[0] + m[r][1] * c[1] + m[r][2] * c[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(colors: &[[u8; 3]]) -> RgbImage {
        let mut i = RgbImage::new(colors.len() as u32, 1);
        for (x, c) in colors.iter().enumerate() {
            i.put_pixel(x as u32, 0, image::Rgb(*c));
        }
        i
    }

    fn px(i: &RgbImage, x: u32) -> [u8; 3] {
        i.get_pixel(x, 0).0
    }

    #[test]
    fn default_leaves_the_photo_untouched() {
        let i = img(&[[10, 120, 240], [200, 30, 90]]);
        assert_eq!(ColorAdjust::default().apply(&i), i);
        assert!(ColorAdjust::default().is_identity());
    }

    #[test]
    fn each_control_moves_colours_the_expected_way() {
        let i = img(&[[60, 60, 60], [200, 200, 200], [200, 60, 60]]);
        let bright = ColorAdjust { brightness: 0.3, ..Default::default() }.apply(&i);
        assert!(px(&bright, 0)[0] > 60 && px(&bright, 1)[0] > 200);
        let contrast = ColorAdjust { contrast: 0.6, ..Default::default() }.apply(&i);
        assert!(px(&contrast, 0)[0] < 60 && px(&contrast, 1)[0] > 200, "{contrast:?}");
        let flat = ColorAdjust { contrast: -0.6, ..Default::default() }.apply(&i);
        assert!(px(&flat, 0)[0] > 60 && px(&flat, 1)[0] < 200);
        let open = ColorAdjust { midtones: 0.6, ..Default::default() }.apply(&i);
        assert!(px(&open, 0)[0] > 60);
        let gray = ColorAdjust { saturation: 0.0, ..Default::default() }.apply(&i);
        let g = px(&gray, 2);
        assert!(g[0] == g[1] && g[1] == g[2], "{g:?}");
        let vivid = ColorAdjust { saturation: 1.8, ..Default::default() }.apply(&i);
        assert!(px(&vivid, 2)[0] as i32 - px(&vivid, 2)[1] as i32 > 140);
        let warm = ColorAdjust { warmth: 1.0, ..Default::default() }.apply(&i);
        assert!(px(&warm, 0)[0] > px(&warm, 0)[2]);
        // Girar 120° lleva el rojo hacia el verde; los grises no cambian.
        let hue = ColorAdjust { hue_deg: 120.0, ..Default::default() }.apply(&i);
        let h = px(&hue, 2);
        assert!(h[1] > h[0] && h[1] > h[2], "{h:?}");
        assert_eq!(px(&hue, 0), [60, 60, 60]);
    }

    #[test]
    fn auto_levels_stretch_a_dull_photo_to_black_and_white() {
        let colors: Vec<[u8; 3]> = (0..400).map(|i| { let v = 90 + (i % 80) as u8; [v, v, v] }).collect();
        let out = ColorAdjust { auto_levels: true, ..Default::default() }.apply(&img(&colors));
        let ys: Vec<u8> = out.pixels().map(|p| p[0]).collect();
        assert!(*ys.iter().min().unwrap() < 10 && *ys.iter().max().unwrap() > 245);
    }

    #[test]
    fn old_params_open_without_adjustments() {
        let a: ColorAdjust = serde_json::from_str("{}").unwrap();
        assert!(a.is_identity());
    }

    #[test]
    fn the_separation_reads_the_adjusted_colours() {
        let photo = image::DynamicImage::ImageRgb8(RgbImage::from_fn(60, 40, |x, _| if x < 30 { image::Rgb([220, 40, 40]) } else { image::Rgb([40, 60, 220]) }));
        let mut prm = crate::vectorize::VecParams { colors: 2, skip_paper: false, ..Default::default() };
        let vivid = crate::vectorize::vectorize(&photo, &prm);
        assert!(vivid.layers.iter().any(|l| l.color[0] as i32 - l.color[2] as i32 > 100));
        prm.adjust.saturation = 0.0;
        let gray = crate::vectorize::vectorize(&photo, &prm);
        assert!(gray.layers.iter().all(|l| l.color.iter().max().unwrap() - l.color.iter().min().unwrap() <= 2), "{:?}", gray.layers.iter().map(|l| l.color).collect::<Vec<_>>());
    }
}
