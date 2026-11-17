//! Foto → dibujo:
//! - Líneas: esqueleto de los trazos oscuros (dibujos a tinta, bocetos).
//! - Siluetas: contornos cerrados de las zonas oscuras, listos para rellenar.
//! - Colores: la imagen se reduce a N colores y cada uno se convierte en formas
//!   cerradas que se asignan a plumillas y se rellenan.
//! - Grabado: líneas onduladas cuya amplitud y frecuencia siguen la oscuridad.
//! - Mezcla: coberturas de tintas para el motor de tramas.
//! - Retrato: líneas de corriente que siguen el tensor estructural de la foto.
//! - Topografía: curvas de tiempo de una onda Fast Marching guiada por la luz.
//! - Tinta: contornos coherentes guiados por el flujo de la imagen ([`crate::ink`]).
//! - Punteado: puntos relajados según tono y detalle ([`crate::stipple`]).
//! - Sombreado cruzado y Buril: rayas a mano ([`crate::hatching`]).
//! - Línea continua y Bucles: un solo trazo por toda la foto ([`crate::tour`]).
//!
//! Los resultados están en píxeles de la imagen de trabajo; la app los escala a mm.

use crate::geom::{chaikin, length, p, signed_area, simplify, Polyline, Rng};
use crate::mix::{self, MixPalette};
use crate::raster::{self, Grid};
use image::{imageops, DynamicImage, GrayImage, RgbImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VecMode {
    Lines,
    Silhouette,
    Colors,
    Engrave,
    /// Varios rotuladores superpuestos con tramas que se intercalan (mezcla óptica).
    Mix,
    /// Líneas de corriente guiadas por el tensor estructural de la imagen.
    Portrait,
    Topography,
    /// Contornos coherentes, como un dibujo a plumilla.
    Ink,
    /// Puntos repartidos por tono y detalle.
    Stipple,
    /// Capas de rayas que se cruzan en las sombras.
    CrossHatch,
    /// Rayas que se curvan con el relieve y se ensanchan en las sombras.
    Burin,
    /// Un único trazo que recorre toda la foto.
    Continuous,
    /// Un trazo de bucles enlazados.
    Loops,
    /// Dibujo a mano construido trazo a trazo mirando la tinta ya puesta.
    Sketch,
    /// Toques de color que se construyen mirando la pintura ya puesta.
    Paint,
}

impl VecMode {
    pub const ALL: [VecMode; 15] = [
        VecMode::Colors,
        VecMode::Mix,
        VecMode::Sketch,
        VecMode::Paint,
        VecMode::Portrait,
        VecMode::Topography,
        VecMode::Stipple,
        VecMode::CrossHatch,
        VecMode::Burin,
        VecMode::Continuous,
        VecMode::Loops,
        VecMode::Ink,
        VecMode::Lines,
        VecMode::Silhouette,
        VecMode::Engrave,
    ];
    pub fn label(self) -> &'static str {
        match self {
            VecMode::Lines => "Líneas",
            VecMode::Silhouette => "Siluetas",
            VecMode::Colors => "Colores",
            VecMode::Engrave => "Grabado",
            VecMode::Mix => "Mezcla",
            VecMode::Portrait => "Retrato",
            VecMode::Topography => "Topografía",
            VecMode::Ink => "Tinta",
            VecMode::Stipple => "Punteado",
            VecMode::CrossHatch => "Sombreado cruzado",
            VecMode::Burin => "Buril",
            VecMode::Continuous => "Línea continua",
            VecMode::Loops => "Bucles",
            VecMode::Sketch => "Boceto",
            VecMode::Paint => "Pinceladas",
        }
    }
    pub fn help(self) -> &'static str {
        match self {
            VecMode::Lines => "Sigue el centro de los trazos oscuros. Ideal para dibujos a tinta y bocetos.",
            VecMode::Silhouette => "Contorna las zonas oscuras como formas cerradas que luego puedes rellenar.",
            VecMode::Colors => "Reduce la foto a pocos colores; cada uno se convierte en formas con relleno de su rotulador.",
            VecMode::Engrave => "Líneas onduladas que se agitan más donde la imagen es más oscura.",
            VecMode::Mix => "Superpone varios rotuladores con tramas cruzadas o puntos intercalados: los colores se mezclan en el papel, como en una cuatricromía.",
            VecMode::Portrait => "Líneas curvas que siguen las formas de la foto, con pasadas extra en las sombras. En negro o con varias tintas.",
            VecMode::Topography => "Una onda recorre la foto y dibuja curvas de nivel: se juntan en las sombras y se separan en las luces.",
            VecMode::Ink => "Contornos limpios y continuos que siguen las formas de la foto, como un dibujo a plumilla.",
            VecMode::Stipple => "Miles de puntos bien repartidos: se juntan en las sombras y en los bordes importantes.",
            VecMode::CrossHatch => "Capas de rayas en varias direcciones: cuanto más oscura la zona, más capas se cruzan.",
            VecMode::Burin => "Rayas paralelas que se curvan con el relieve de la foto y se ensanchan en las sombras, como un grabado a buril.",
            VecMode::Continuous => "Un único trazo, sin levantar la punta, que recorre la foto y se apretuja en las sombras.",
            VecMode::Loops => "Un trazo de bucles enlazados que se amontonan en las sombras, como un garabato a mano.",
            VecMode::Sketch => "Un dibujo a mano que se construye mirando el papel: encaja las masas, modela con rayas que siguen las formas, cruza en las sombras y repasa el detalle. Con una tinta o sanguina y carbón.",
            VecMode::Paint => "Una pintura de toques cortos de color, entre puntos y rayas, que se construye mirando el papel: cada toque elige el rotulador que más acerca el color a la foto, sigue las formas y alterna colores vecinos que se mezclan en el ojo.",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VecParams {
    pub mode: VecMode,
    /// Lado mayor de la imagen de trabajo, px.
    pub max_px: u32,
    /// Umbral 0–255; negativo = automático (Otsu).
    pub threshold: i32,
    pub invert: bool,
    /// Desenfoque previo (sigma en px).
    pub blur: f32,
    /// Tolerancia de simplificación, px.
    pub detail: f64,
    /// Iteraciones de suavizado.
    pub smooth: usize,
    /// Descarta manchas o trazos menores que esto (px o px²).
    pub min_size: f64,
    pub colors: usize,
    /// En Colores, omite el color más claro (el papel).
    pub skip_paper: bool,
    pub engrave_rows: u32,
    pub engrave_amp: f64,
    /// Rotuladores de la mezcla óptica.
    #[serde(default = "default_palette")]
    pub mix_palette: MixPalette,
    /// Ajustes de color aplicados a la foto antes de interpretarla.
    #[serde(default)]
    pub adjust: crate::adjust::ColorAdjust,
    /// Colores de los rotuladores elegidos para la mezcla con «Mis rotuladores».
    #[serde(default)]
    pub mix_pens: Vec<[u8; 3]>,
    #[serde(default)]
    pub portrait: crate::portrait::PortraitParams,
    #[serde(default)]
    pub topo: crate::topo::TopoParams,
    /// Análisis previo de los estilos de dibujo (Tinta, Punteado, rayas, un solo trazo).
    #[serde(default)]
    pub analysis: crate::analysis::AnalysisParams,
    #[serde(default)]
    pub ink: crate::ink::InkParams,
    #[serde(default)]
    pub stipple: crate::stipple::StippleParams,
    #[serde(default)]
    pub hatch: crate::hatching::HatchParams,
    #[serde(default)]
    pub burin: crate::hatching::BurinParams,
    #[serde(default)]
    pub continuous: crate::tour::ContinuousParams,
    #[serde(default)]
    pub loops: crate::tour::LoopParams,
    #[serde(default)]
    pub sketch: crate::sketch::SketchParams,
    #[serde(default)]
    pub paint: crate::paint::PaintParams,
}

fn default_palette() -> MixPalette {
    MixPalette::Cmyk
}

impl Default for VecParams {
    fn default() -> Self {
        VecParams {
            mode: VecMode::Colors,
            max_px: 900,
            threshold: -1,
            invert: false,
            blur: 1.0,
            detail: 0.8,
            smooth: 2,
            min_size: 12.0,
            colors: 5,
            skip_paper: true,
            engrave_rows: 90,
            engrave_amp: 0.9,
            mix_palette: MixPalette::Cmyk,
            adjust: Default::default(),
            mix_pens: vec![],
            portrait: Default::default(),
            topo: Default::default(),
            analysis: Default::default(),
            ink: Default::default(),
            stipple: Default::default(),
            hatch: Default::default(),
            burin: Default::default(),
            continuous: Default::default(),
            loops: Default::default(),
            sketch: Default::default(),
            paint: Default::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VecLayer {
    pub color: [u8; 3],
    pub closed: bool,
    pub lines: Vec<Polyline>,
    /// Mezcla: cobertura 0–1 por píxel; las líneas se generan al tamaño final con
    /// [`mix::render`].
    pub coverage: Option<std::sync::Arc<Grid<f32>>>,
    /// Nombre del rotulador (en la mezcla, el de la paleta).
    pub name: Option<&'static str>,
}

impl VecLayer {
    pub(crate) fn lines(color: [u8; 3], closed: bool, lines: Vec<Polyline>) -> Self {
        VecLayer { color, closed, lines, coverage: None, name: None }
    }
}

#[derive(Clone, Debug)]
pub struct VecResult {
    pub width: f64,
    pub height: f64,
    pub layers: Vec<VecLayer>,
}

pub fn working_image(img: &DynamicImage, max_px: u32) -> RgbImage {
    if img.color().has_alpha() {
        return flatten_white(&working_rgba(img, max_px));
    }
    let (w, h) = (img.width(), img.height());
    let scale = (max_px as f64 / w.max(h) as f64).min(1.0);
    let rgb = img.to_rgb8();
    if scale >= 1.0 {
        return rgb;
    }
    let (nw, nh) = (((w as f64 * scale).round() as u32).max(1), ((h as f64 * scale).round() as u32).max(1));
    imageops::resize(&rgb, nw, nh, imageops::FilterType::Triangle)
}

/// Imagen de trabajo con su transparencia, necesaria para recortar Topografía y
/// conservar la foto al guardarla. El resto de motores recibe papel blanco.
pub fn working_rgba(img: &DynamicImage, max_px: u32) -> image::RgbaImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let scale = (max_px.max(1) as f64 / w.max(h).max(1) as f64).min(1.0);
    if scale >= 1.0 { return rgba; }
    // Filtrar con alfa premultiplicado impide que el RGB oculto de los píxeles
    // transparentes cree halos oscuros o de otro color alrededor del recorte.
    let mut premultiplied = rgba;
    for px in premultiplied.pixels_mut() {
        let a = px[3] as u32;
        for c in 0..3 { px[c] = ((px[c] as u32*a+127)/255) as u8; }
    }
    let mut resized = imageops::resize(&premultiplied, (w as f64*scale).round().max(1.0) as u32, (h as f64*scale).round().max(1.0) as u32, imageops::FilterType::Triangle);
    for px in resized.pixels_mut() {
        let a = px[3] as u32;
        for c in 0..3 { px[c] = (px[c] as u32*255+a/2).checked_div(a).unwrap_or(0).min(255) as u8; }
    }
    resized
}

pub(crate) fn flatten_white(rgba: &image::RgbaImage) -> RgbImage {
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let px = rgba.get_pixel(x, y);
        let a = px[3] as u32;
        image::Rgb([0, 1, 2].map(|c| ((px[c] as u32*a + 255*(255-a) + 127)/255) as u8))
    })
}

pub(crate) fn luma(c: [u8; 3]) -> f64 {
    0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64
}

fn gray_of(rgb: &RgbImage, blur: f32) -> GrayImage {
    let g = imageops::grayscale(rgb);
    if blur > 0.05 {
        imageops::blur(&g, blur)
    } else {
        g
    }
}

pub fn vectorize(img: &DynamicImage, prm: &VecParams) -> VecResult {
    if prm.mode == VecMode::Topography { return crate::topo::render(img, prm); }
    let rgb = prm.adjust.apply(&working_image(img, prm.max_px.clamp(64, 4000)));
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let layers = match prm.mode {
        VecMode::Lines => lines_mode(&rgb, prm),
        VecMode::Silhouette => silhouette_mode(&rgb, prm),
        VecMode::Colors => colors_mode(&rgb, prm),
        VecMode::Engrave => engrave_mode(&rgb, prm),
        VecMode::Mix => mix_mode(&rgb, prm),
        VecMode::Portrait => crate::portrait::render(&rgb, prm),
        VecMode::Ink => crate::ink::render(&rgb, prm),
        VecMode::Stipple => crate::stipple::render(&rgb, prm),
        VecMode::CrossHatch => crate::hatching::cross_hatch(&rgb, prm),
        VecMode::Burin => crate::hatching::burin(&rgb, prm),
        VecMode::Continuous => crate::tour::continuous(&rgb, prm),
        VecMode::Loops => crate::tour::loops(&rgb, prm),
        VecMode::Sketch => crate::sketch::render(&rgb, prm),
        VecMode::Paint => crate::paint::render(&rgb, prm),
        VecMode::Topography => unreachable!("Topografía conserva el canal alfa antes de convertir a RGB"),
    };
    VecResult { width: w as f64, height: h as f64, layers }
}

fn ink_mask(rgb: &RgbImage, prm: &VecParams) -> Grid<bool> {
    let g = gray_of(rgb, prm.blur);
    let thr = if prm.threshold < 0 { raster::otsu(g.as_raw()) } else { prm.threshold.clamp(0, 255) as u8 };
    let (w, h) = (g.width() as usize, g.height() as usize);
    let mut m = Grid::<bool>::new(w, h);
    for (i, &v) in g.as_raw().iter().enumerate() {
        m.data[i] = (v <= thr) != prm.invert;
    }
    remove_specks(&mut m, prm.min_size as usize);
    m
}

/// Elimina componentes conexas menores que `min` píxeles.
pub(crate) fn remove_specks(m: &mut Grid<bool>, min: usize) {
    if min == 0 {
        return;
    }
    let (w, h) = (m.w, m.h);
    let mut seen = vec![false; w * h];
    let mut stack = Vec::new();
    let mut comp = Vec::new();
    for start in 0..w * h {
        if !m.data[start] || seen[start] {
            continue;
        }
        comp.clear();
        stack.push(start);
        seen[start] = true;
        while let Some(i) = stack.pop() {
            comp.push(i);
            let (x, y) = (i % w, i / w);
            let mut push = |j: usize| {
                if m.data[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < w {
                push(i + 1);
            }
            if y > 0 {
                push(i - w);
            }
            if y + 1 < h {
                push(i + w);
            }
        }
        if comp.len() < min {
            for &i in &comp {
                m.data[i] = false;
            }
        }
    }
}

fn finish_open(lines: Vec<Polyline>, prm: &VecParams) -> Vec<Polyline> {
    lines
        .into_par_iter()
        .map(|l| chaikin(&simplify(&l, prm.detail), prm.smooth, false))
        .filter(|l| l.len() >= 2 && length(l) >= prm.min_size.sqrt().max(2.0))
        .collect()
}

fn finish_closed(lines: Vec<Polyline>, prm: &VecParams) -> Vec<Polyline> {
    lines
        .into_par_iter()
        .filter(|l| l.len() >= 4 && l[0].dist(l[l.len() - 1]) < 1.5)
        .map(|l| chaikin(&simplify(&l, prm.detail), prm.smooth, true))
        .filter(|l| l.len() >= 4 && signed_area(l).abs() >= prm.min_size)
        .collect()
}

fn lines_mode(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    let mut m = ink_mask(rgb, prm);
    raster::thin(&mut m);
    let lines = finish_open(raster::trace_skeleton(&m), prm);
    vec![VecLayer::lines([20, 20, 24], false, lines)]
}

fn mask_contours(m: &Grid<bool>) -> Vec<Polyline> {
    // Campo con borde vacío para que todos los contornos cierren.
    let (w, h) = (m.w + 2, m.h + 2);
    let mut f = Grid::<f32>::new(w, h);
    for y in 0..m.h {
        for x in 0..m.w {
            if m.get(x, y) {
                f.set(x + 1, y + 1, 1.0);
            }
        }
    }
    // Suavizado 3×3 para contornos menos escalonados.
    let src = f.clone();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let mut s = 0.0;
            for dy in 0..3 {
                for dx in 0..3 {
                    s += src.get(x + dx - 1, y + dy - 1);
                }
            }
            f.set(x, y, s / 9.0);
        }
    }
    raster::contours(&f, 0.5).into_iter().map(|l| l.into_iter().map(|q| q - p(1.0, 1.0)).collect()).collect()
}

fn silhouette_mode(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    let m = ink_mask(rgb, prm);
    let lines = finish_closed(mask_contours(&m), prm);
    vec![VecLayer::lines([20, 20, 24], true, lines)]
}

/// k-means en RGB con inicialización k-means++ determinista.
pub fn kmeans(pixels: &[[u8; 3]], k: usize, seed: u64) -> Vec<[f64; 3]> {
    let k = k.clamp(1, 16);
    let mut rng = Rng::new(seed);
    let step = (pixels.len() / 20_000).max(1);
    let sample: Vec<[f64; 3]> = pixels.iter().step_by(step).map(|c| [c[0] as f64, c[1] as f64, c[2] as f64]).collect();
    if sample.is_empty() {
        return vec![[0.0; 3]];
    }
    let d2 = |a: &[f64; 3], b: &[f64; 3]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
    let mut centers = vec![sample[(rng.f() * sample.len() as f64) as usize % sample.len()]];
    while centers.len() < k {
        let dists: Vec<f64> = sample.iter().map(|s| centers.iter().map(|c| d2(s, c)).fold(f64::MAX, f64::min)).collect();
        let total: f64 = dists.iter().sum();
        if total <= 0.0 {
            break;
        }
        let mut target = rng.f() * total;
        let mut idx = 0;
        for (i, d) in dists.iter().enumerate() {
            target -= d;
            if target <= 0.0 {
                idx = i;
                break;
            }
        }
        centers.push(sample[idx]);
    }
    for _ in 0..16 {
        let acc = sample
            .par_iter()
            .fold(
                || vec![([0.0f64; 3], 0usize); centers.len()],
                |mut acc, s| {
                    let (bi, _) = centers.iter().enumerate().map(|(i, c)| (i, d2(s, c))).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
                    for (sum, value) in acc[bi].0.iter_mut().zip(s) {
                        *sum += value;
                    }
                    acc[bi].1 += 1;
                    acc
                },
            )
            .reduce(
                || vec![([0.0f64; 3], 0usize); centers.len()],
                |mut a, b| {
                    for (x, y) in a.iter_mut().zip(b) {
                        for j in 0..3 {
                            x.0[j] += y.0[j];
                        }
                        x.1 += y.1;
                    }
                    a
                },
            );
        let mut moved = 0.0;
        for (c, (sum, n)) in centers.iter_mut().zip(acc) {
            if n > 0 {
                let new = [sum[0] / n as f64, sum[1] / n as f64, sum[2] / n as f64];
                moved += d2(c, &new);
                *c = new;
            }
        }
        if moved < 0.5 {
            break;
        }
    }
    centers
}

fn colors_mode(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    let blurred = if prm.blur > 0.05 { imageops::blur(rgb, prm.blur) } else { rgb.clone() };
    let (w, h) = (blurred.width() as usize, blurred.height() as usize);
    let pixels: Vec<[u8; 3]> = blurred.pixels().map(|p| p.0).collect();
    let centers = kmeans(&pixels, prm.colors, 7);
    let d2 = |a: &[u8; 3], b: &[f64; 3]| (a[0] as f64 - b[0]).powi(2) + (a[1] as f64 - b[1]).powi(2) + (a[2] as f64 - b[2]).powi(2);
    let mut labels: Vec<u8> = pixels
        .par_iter()
        .map(|c| centers.iter().enumerate().min_by(|a, b| d2(c, a.1).total_cmp(&d2(c, b.1))).unwrap().0 as u8)
        .collect();
    // Filtro de moda 3×3 para quitar ruido entre regiones.
    for _ in 0..2 {
        let src = labels.clone();
        labels = (0..w * h)
            .into_par_iter()
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let mut counts = [0u8; 16];
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                            counts[src[ny as usize * w + nx as usize] as usize] += 1;
                        }
                    }
                }
                let (best, n) = counts.iter().enumerate().max_by_key(|(_, n)| **n).unwrap();
                if *n >= 5 { best as u8 } else { src[i] }
            })
            .collect();
    }
    let lightest = (0..centers.len())
        .max_by(|&a, &b| luma(to_u8(centers[a])).total_cmp(&luma(to_u8(centers[b]))))
        .unwrap_or(0);
    let mut layers: Vec<VecLayer> = (0..centers.len())
        .into_par_iter()
        .filter(|&c| !(prm.skip_paper && c == lightest && luma(to_u8(centers[c])) > 150.0))
        .map(|c| {
            let mut m = Grid::<bool>::new(w, h);
            for (i, &l) in labels.iter().enumerate() {
                m.data[i] = l as usize == c;
            }
            remove_specks(&mut m, prm.min_size as usize);
            VecLayer::lines(to_u8(centers[c]), true, finish_closed(mask_contours(&m), prm))
        })
        .filter(|l| !l.lines.is_empty())
        .collect();
    // Del más claro al más oscuro: el plotter pinta primero los colores claros.
    layers.sort_by(|a, b| luma(b.color).total_cmp(&luma(a.color)));
    layers
}

pub(crate) fn to_u8(c: [f64; 3]) -> [u8; 3] {
    [c[0].round().clamp(0.0, 255.0) as u8, c[1].round().clamp(0.0, 255.0) as u8, c[2].round().clamp(0.0, 255.0) as u8]
}

fn engrave_mode(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    let g = gray_of(rgb, prm.blur);
    let (w, h) = (g.width() as usize, g.height() as usize);
    let rows = prm.engrave_rows.clamp(5, 600) as usize;
    let row_h = h as f64 / rows as f64;
    let lines: Vec<Polyline> = (0..rows)
        .into_par_iter()
        .map(|r| {
            let y0 = (r as f64 + 0.5) * row_h;
            let mut phase = 0.0f64;
            let mut line = Vec::with_capacity(w * 2);
            let step = 0.5;
            let mut x = 0.0;
            while x < w as f64 {
                let (sx, sy) = ((x as usize).min(w - 1), (y0 as usize).min(h - 1));
                let v = g.get_pixel(sx as u32, sy as u32).0[0] as f64 / 255.0;
                let dark = if prm.invert { v } else { 1.0 - v };
                phase += (0.15 + dark * 1.1) * step;
                let y = y0 + (row_h * 0.5 * prm.engrave_amp) * dark * phase.sin();
                line.push(p(x, y));
                x += step;
            }
            simplify(&line, 0.08)
        })
        .collect();
    let row_dir: Vec<Polyline> = lines
        .into_iter()
        .enumerate()
        .map(|(i, mut l)| {
            if i % 2 == 1 {
                l.reverse();
            }
            l
        })
        .collect();
    vec![VecLayer::lines([20, 20, 24], false, row_dir)]
}

pub(crate) fn mix_mode(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    let blurred = if prm.blur > 0.05 { imageops::blur(rgb, prm.blur) } else { rgb.clone() };
    let mut pens: Vec<(Option<&'static str>, [u8; 3])> = prm.mix_palette.pens().into_iter().map(|(n, c)| (Some(n), c)).collect();
    if prm.mix_palette == MixPalette::Mine {
        pens = prm.mix_pens.iter().map(|c| (None, *c)).collect();
        pens.dedup_by_key(|p| p.1);
        if pens.is_empty() {
            pens = MixPalette::Cmyk.pens().into_iter().map(|(n, c)| (Some(n), c)).collect();
        }
    }
    if pens.is_empty() {
        let pixels: Vec<[u8; 3]> = blurred.pixels().map(|p| p.0).collect();
        pens = kmeans(&pixels, prm.colors, 7).into_iter().map(to_u8).filter(|c| luma(*c) < 235.0).map(|c| (None, c)).collect();
    }
    // Del más claro al más oscuro: el amarillo primero y el negro al final.
    pens.sort_by(|a, b| luma(b.1).total_cmp(&luma(a.1)));
    let colors: Vec<[u8; 3]> = pens.iter().map(|p| p.1).collect();
    let maps = mix::coverage(&blurred, &colors);
    pens.into_iter()
        .zip(maps)
        .filter(|(_, m)| m.data.iter().any(|v| *v > 0.03))
        .map(|((name, color), m)| VecLayer { color, closed: false, lines: vec![], coverage: Some(std::sync::Arc::new(m)), name })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn transparent_pixels_are_white_and_resizing_does_not_add_colored_fringes() {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(4, 1, |x, _| {
            image::Rgba(match x { 0 | 1 => [255, 0, 0, 255], 2 => [0, 0, 255, 0], _ => [0, 0, 0, 0] })
        }));
        let full = working_image(&img, 4);
        assert_eq!(full.get_pixel(3, 0).0, [255; 3]);
        let resized = working_image(&img, 2);
        assert_eq!(resized.dimensions(), (2, 1));
        assert!(resized.pixels().all(|px| px[0] == 255 && px[1] == px[2]));
    }

    fn test_image() -> DynamicImage {
        let mut img = RgbImage::from_pixel(200, 120, Rgb([250, 250, 250]));
        for y in 20..100 {
            for x in 20..90 {
                img.put_pixel(x, y, Rgb([220, 30, 30]));
            }
            for x in 110..180 {
                img.put_pixel(x, y, Rgb([20, 40, 200]));
            }
        }
        for x in 10..190 {
            img.put_pixel(x, 110, Rgb([0, 0, 0]));
            img.put_pixel(x, 111, Rgb([0, 0, 0]));
        }
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn colors_finds_two_shapes_without_paper() {
        let r = vectorize(&test_image(), &VecParams { colors: 4, ..Default::default() });
        assert!(r.layers.len() >= 2);
        assert!(r.layers.iter().all(|l| l.closed));
    }

    #[test]
    fn mix_with_my_pens_uses_exactly_those_colours() {
        let mine = vec![[214, 48, 49], [9, 92, 196]];
        let r = vectorize(&test_image(), &VecParams { mode: VecMode::Mix, mix_palette: MixPalette::Mine, mix_pens: mine.clone(), ..Default::default() });
        assert!(!r.layers.is_empty());
        assert!(r.layers.iter().all(|l| mine.contains(&l.color)), "{:?}", r.layers.iter().map(|l| l.color).collect::<Vec<_>>());
    }

    #[test]
    fn every_mode_runs() {
        for mode in VecMode::ALL {
            let r = vectorize(&test_image(), &VecParams { mode, ..Default::default() });
            assert!(r.layers.iter().any(|l| !l.lines.is_empty() || l.coverage.is_some()), "{:?}", mode);
        }
    }
}
