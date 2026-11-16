//! Retrato de trazo: tensor estructural RGB suavizado, campo de líneas sin signo
//! (θ ≡ θ + π), integración por punto medio y siembra a ambos lados de cada curva.
//! Las pasadas tonales independientes permiten cruzar las sombras sin rellenarlas.

use crate::geom::{length, p, simplify, Polyline, P};
use crate::mix::MixPalette;
use crate::raster::Grid;
use crate::vectorize::{VecLayer, VecParams};
use image::{imageops, RgbImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::f64::consts::PI;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PortraitParams {
    pub color: bool,
    pub palette: MixPalette,
    /// Número aproximado de líneas a lo ancho del lado mayor, independiente de resolución.
    pub density: u32,
    /// Radio del tensor, referido a una imagen de 900 px.
    pub field_smooth: f64,
    pub follow: f64,
    pub angle_deg: f64,
    pub passes: usize,
    pub cross: bool,
    /// Longitud máxima como porcentaje del lado mayor.
    pub length_pct: f64,
    /// Exponente de cobertura: >1 conserva más papel en las luces.
    pub tone: f64,
    pub pen_width_mm: f64,
}

impl Default for PortraitParams {
    fn default() -> Self {
        Self {
            color: true,
            palette: MixPalette::Ryb,
            density: 240,
            field_smooth: 4.0,
            follow: 1.0,
            angle_deg: 45.0,
            passes: 3,
            cross: true,
            length_pct: 18.0,
            tone: 1.0,
            pen_width_mm: 0.35,
        }
    }
}

/// Genera caminos abiertos en las coordenadas de trabajo, una capa por tinta.
pub fn render(rgb: &RgbImage, prm: &VecParams) -> Vec<VecLayer> {
    if rgb.width() < 2 || rgb.height() < 2 {
        return vec![];
    }
    let cfg = &prm.portrait;
    let source = if prm.blur > 0.05 {
        imageops::blur(rgb, prm.blur)
    } else {
        rgb.clone()
    };
    let field = Field::new(&source, cfg);
    let layers = if cfg.color {
        let mut separation = prm.clone();
        separation.mix_palette = cfg.palette;
        separation.blur = 0.0;
        let separated = crate::vectorize::mix_mode(&source, &separation);
        if matches!(cfg.palette, MixPalette::Ryb | MixPalette::Cmyk) {
            color_underprint(&source, cfg.palette, separated)
        } else {
            separated
        }
    } else {
        let gray = imageops::grayscale(&source);
        let mut map = Grid::new(source.width() as usize, source.height() as usize);
        for (out, v) in map.data.iter_mut().zip(gray.as_raw()) {
            *out = 1.0 - *v as f32 / 255.0;
        }
        vec![VecLayer {
            color: [30, 30, 34],
            closed: false,
            lines: vec![],
            coverage: Some(std::sync::Arc::new(map)),
            name: Some("Negro"),
        }]
    };
    layers
        .into_par_iter()
        .enumerate()
        .map(|(ink, mut layer)| {
            let map = layer.coverage.take().unwrap();
            let mut tone = (*map).clone();
            for v in &mut tone.data {
                *v = v.clamp(0.0, 1.0).powf(cfg.tone.clamp(0.4, 2.5) as f32);
            }
            let spacing = (field.tensor.w.max(field.tensor.h) as f64
                / cfg.density.clamp(40, 600) as f64)
                .max(1.35);
            for pass in 0..cfg.passes.clamp(1, 4) {
                // El amarillo necesita aparecer antes: una cobertura pequeña de
                // tinta clara se pierde sobre el papel si usa el umbral del negro.
                let c = layer.color;
                let light_ink = cfg.color
                    && (0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64) > 150.0;
                let threshold = [if light_ink { 0.02 } else { 0.055 }, 0.32, 0.59, 0.8][pass];
                // Desfase entre tintas: evita que los trazos de todos los colores se tapen.
                let ink_angle = if cfg.color {
                    [0.0_f64, 12.0, -12.0, 5.0][ink % 4].to_radians()
                } else {
                    0.0
                };
                let rotation = ink_angle
                    + if cfg.cross {
                        [0.0, PI / 3.0, -PI / 3.0, PI / 2.0][pass]
                    } else {
                        0.0
                    };
                let pass_spacing = spacing * (1.0 - pass as f64 * 0.08);
                layer.lines.extend(streamlines(
                    &field,
                    &tone,
                    cfg,
                    threshold,
                    pass_spacing,
                    rotation,
                    ink * 4 + pass,
                ));
            }
            layer
        })
        .filter(|layer| !layer.lines.is_empty())
        .collect()
}

/// En un retrato a color, reserva el negro para sombras marcadas. Los medios
/// tonos se reconstruyen con las tintas cromáticas: evita que una piel cálida
/// quede dibujada casi toda con negro por la sustitución de gris de la mezcla.
fn color_underprint(
    rgb: &RgbImage,
    palette: MixPalette,
    separated: Vec<VecLayer>,
) -> Vec<VecLayer> {
    let mut pens = palette.pens();
    let Some((black_name, black_color)) = pens.pop() else { return separated };
    let mut black = separated
        .iter()
        .find(|l| l.color == black_color)
        .and_then(|l| l.coverage.as_ref())
        .map(|m| (**m).clone())
        .unwrap_or_else(|| Grid::new(rgb.width() as usize, rgb.height() as usize));
    for v in &mut black.data {
        *v = ((v.powf(1.0 / 1.2) - 0.2) / 0.8).clamp(0.0, 1.0);
    }
    // Retira de la foto la densidad que ya aporta el negro antes de resolver las
    // otras tintas, para no oscurecer dos veces las mismas zonas.
    let mut residual = rgb.clone();
    for (px, k) in residual.pixels_mut().zip(&black.data) {
        for c in 0..3 {
            px[c] = (px[c] as f32 / (black_color[c] as f32 / 255.0).max(0.01).powf(*k))
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
    let colors: Vec<_> = pens.iter().map(|(_, color)| *color).collect();
    let maps = crate::mix::coverage_with_cost(&residual, &colors, 0.02);
    for v in &mut black.data {
        *v = v.powf(1.2);
    }
    pens.push((black_name, black_color));
    let mut maps = maps;
    maps.push(black);
    let mut layers: Vec<_> = pens
        .into_iter()
        .zip(maps)
        .map(|((name, color), map)| VecLayer {
            color,
            closed: false,
            lines: vec![],
            coverage: Some(std::sync::Arc::new(map)),
            name: Some(name),
        })
        .collect();
    let luma = |c: [u8; 3]| 0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64;
    layers.sort_by(|a, b| luma(b.color).total_cmp(&luma(a.color)));
    layers
}

struct Field {
    /// Jxx, Jxy, Jyy. Se interpola el tensor, nunca los ángulos ni vectores con signo.
    tensor: Grid<[f32; 3]>,
    fallback: f64,
    follow: f64,
}

impl Field {
    fn new(rgb: &RgbImage, cfg: &PortraitParams) -> Self {
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
        let mut tensor = Grid::new(w, h);
        let pixel = |x: isize, y: isize, c: usize| {
            rgb.get_pixel(
                x.clamp(0, w as isize - 1) as u32,
                y.clamp(0, h as isize - 1) as u32,
            )[c] as f32
                / 255.0
        };
        for y in 0..h {
            for x in 0..w {
                let (x, y) = (x as isize, y as isize);
                let mut j = [0.0; 3];
                for c in 0..3 {
                    let gx = (pixel(x + 1, y - 1, c)
                        + 2.0 * pixel(x + 1, y, c)
                        + pixel(x + 1, y + 1, c)
                        - pixel(x - 1, y - 1, c)
                        - 2.0 * pixel(x - 1, y, c)
                        - pixel(x - 1, y + 1, c))
                        / 8.0;
                    let gy = (pixel(x - 1, y + 1, c)
                        + 2.0 * pixel(x, y + 1, c)
                        + pixel(x + 1, y + 1, c)
                        - pixel(x - 1, y - 1, c)
                        - 2.0 * pixel(x, y - 1, c)
                        - pixel(x + 1, y - 1, c))
                        / 8.0;
                    j[0] += gx * gx / 3.0;
                    j[1] += gx * gy / 3.0;
                    j[2] += gy * gy / 3.0;
                }
                tensor.set(x as usize, y as usize, j);
            }
        }
        // Tres cajas separables aproximan un gaussiano en O(píxeles), incluso con
        // radios grandes; se suavizan los productos, no el ángulo resultante.
        let sigma = cfg.field_smooth.clamp(0.5, 20.0) * w.max(h) as f64 / 900.0;
        let radius = (((1.0 + 4.0 * sigma * sigma).sqrt() - 1.0) / 2.0)
            .round()
            .max(1.0) as usize;
        for _ in 0..3 {
            tensor = box_axis(&box_axis(&tensor, radius, false), radius, true);
        }
        Self {
            tensor,
            fallback: cfg.angle_deg.to_radians(),
            follow: cfg.follow.clamp(0.0, 1.0),
        }
    }

    fn inside(&self, q: P) -> bool {
        q.x >= 0.0
            && q.y >= 0.0
            && q.x <= (self.tensor.w - 1) as f64
            && q.y <= (self.tensor.h - 1) as f64
    }

    fn direction(&self, q: P, previous: P, rotation: f64) -> P {
        let [a, b, c] = sample_tensor(&self.tensor, q).map(|v| v as f64);
        let energy = a + c;
        let anisotropy = (a - c).hypot(2.0 * b);
        let coherence = (anisotropy / (energy + 1e-12)).clamp(0.0, 1.0);
        let weight = self.follow * coherence * energy / (energy + 0.000015);
        // Tangente = autovector de menor autovalor. Mezcla en ángulo doble para
        // que dos tangentes equivalentes (θ y θ+π) nunca se cancelen.
        let tangent = p(-(a - c), -2.0 * b) * (1.0 / anisotropy.max(1e-12));
        let fixed = p((2.0 * self.fallback).cos(), (2.0 * self.fallback).sin());
        let blended = tangent * weight + fixed * (1.0 - weight);
        let angle = if blended.len() < 1e-8 {
            self.fallback
        } else {
            blended.y.atan2(blended.x) * 0.5
        } + rotation;
        let v = p(angle.cos(), angle.sin());
        if v.dot(previous) < 0.0 {
            v * -1.0
        } else {
            v
        }
    }
}

fn box_axis(src: &Grid<[f32; 3]>, radius: usize, vertical: bool) -> Grid<[f32; 3]> {
    let mut out = Grid::new(src.w, src.h);
    let (rows, cols) = if vertical {
        (src.w, src.h)
    } else {
        (src.h, src.w)
    };
    let index = |row: usize, col: usize| {
        if vertical {
            col * src.w + row
        } else {
            row * src.w + col
        }
    };
    for row in 0..rows {
        let mut sum = [0.0_f64; 3];
        let at = |col: isize| src.data[index(row, col.clamp(0, cols as isize - 1) as usize)];
        for i in -(radius as isize)..=radius as isize {
            for (s, v) in sum.iter_mut().zip(at(i)) {
                *s += v as f64;
            }
        }
        for col in 0..cols {
            out.data[index(row, col)] = sum.map(|v| (v / (2 * radius + 1) as f64).max(0.0) as f32);
            // Jxy puede ser negativo: no recortar ninguno de los componentes.
            out.data[index(row, col)][1] = (sum[1] / (2 * radius + 1) as f64) as f32;
            let lo = at(col as isize - radius as isize);
            let hi = at((col + radius + 1) as isize);
            for c in 0..3 {
                sum[c] += hi[c] as f64 - lo[c] as f64;
            }
        }
    }
    out
}

fn corners(w: usize, h: usize, q: P) -> ([usize; 4], [f64; 4]) {
    let x = q.x.clamp(0.0, (w - 1) as f64);
    let y = q.y.clamp(0.0, (h - 1) as f64);
    let (ix, iy) = (x as usize, y as usize);
    let (jx, jy) = ((ix + 1).min(w - 1), (iy + 1).min(h - 1));
    let (tx, ty) = (x - ix as f64, y - iy as f64);
    (
        [iy * w + ix, iy * w + jx, jy * w + ix, jy * w + jx],
        [
            (1.0 - tx) * (1.0 - ty),
            tx * (1.0 - ty),
            (1.0 - tx) * ty,
            tx * ty,
        ],
    )
}

fn sample_tensor(g: &Grid<[f32; 3]>, q: P) -> [f32; 3] {
    let (indices, weights) = corners(g.w, g.h, q);
    let mut v = [0.0; 3];
    for (i, w) in indices.into_iter().zip(weights) {
        for (out, value) in v.iter_mut().zip(g.data[i]) {
            *out += value * w as f32;
        }
    }
    v
}

fn sample(g: &Grid<f32>, q: P) -> f64 {
    let (indices, weights) = corners(g.w, g.h, q);
    indices
        .into_iter()
        .zip(weights)
        .map(|(i, w)| g.data[i] as f64 * w)
        .sum()
}

/// Índice espacial: distancias geométricas, no coincidencias de píxeles. El tiempo
/// de consulta queda acotado a las celdas vecinas, sin comparar con todo el dibujo.
struct Occupancy {
    cell: f64,
    points: HashMap<(i32, i32), Vec<(P, i32)>>,
}

impl Occupancy {
    fn new(cell: f64) -> Self {
        Self {
            cell,
            points: HashMap::new(),
        }
    }
    fn key(&self, q: P) -> (i32, i32) {
        (
            (q.x / self.cell).floor() as i32,
            (q.y / self.cell).floor() as i32,
        )
    }
    fn insert(&mut self, q: P, index: i32) {
        self.points.entry(self.key(q)).or_default().push((q, index));
    }
    fn near(&self, q: P, radius: f64, ignore: Option<(i32, i32)>) -> bool {
        let (x, y) = self.key(q);
        let n = (radius / self.cell).ceil() as i32;
        for dy in -n..=n {
            for dx in -n..=n {
                if let Some(points) = self.points.get(&(x + dx, y + dy)) {
                    for &(a, index) in points {
                        if ignore.is_some_and(|(at, recent)| (at - index).abs() <= recent) {
                            continue;
                        }
                        if (a - q).dot(a - q) < radius * radius {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

fn streamlines(
    field: &Field,
    tone: &Grid<f32>,
    cfg: &PortraitParams,
    threshold: f64,
    spacing: f64,
    rotation: f64,
    phase: usize,
) -> Vec<Polyline> {
    let mut occupied = Occupancy::new(spacing);
    let mut seeds = VecDeque::new();
    let step = (spacing * 0.28).clamp(0.35, 1.6);
    let max_length =
        (tone.w.max(tone.h) as f64 * cfg.length_pct.clamp(2.0, 100.0) / 100.0).max(spacing * 3.0);
    let max_steps = (max_length / step).ceil() as i32;
    let recent = (spacing * 2.0 / step).ceil() as i32;
    let mut lines = vec![];
    // Rejilla de respaldo para regiones desconectadas o muy estrechas. El desfase
    // cambia en cada pasada, también cuando se desactiva el rayado cruzado.
    let scan = spacing * 0.65;
    let offset = ((phase as f64 * 0.381966 + 0.5).fract() * 0.7 + 0.15) * scan;
    for y in 0..(tone.h as f64 / scan).ceil() as usize {
        for x in 0..(tone.w as f64 / scan).ceil() as usize {
            seeds.push_back(p(x as f64 * scan + offset, y as f64 * scan + offset));
        }
    }
    while let Some(seed) = seeds.pop_front() {
        if !field.inside(seed)
            || sample(tone, seed) < threshold
            || occupied.near(seed, spacing * 0.95, None)
        {
            continue;
        }
        let mut local = Occupancy::new(spacing);
        local.insert(seed, 0);
        let initial = field.direction(seed, p(1.0, 0.0), rotation);
        let mut halves = [vec![], vec![]];
        for (side, sign) in [-1, 1].into_iter().enumerate() {
            let (mut q, mut previous) = (seed, initial * sign as f64);
            for i in 1..=max_steps / 2 {
                let v = field.direction(q, previous, rotation);
                let mid = q + v * (step * 0.5);
                if !field.inside(mid) || sample(tone, mid) < threshold {
                    break;
                }
                let next_v = field.direction(mid, v, rotation);
                let next = q + next_v * step;
                if !field.inside(next)
                    || sample(tone, next) < threshold
                    || next_v.dot(previous) < 0.5
                    || occupied.near(next, spacing * 0.72, None)
                    || local.near(next, spacing * 0.65, Some((i * sign, recent)))
                {
                    break;
                }
                local.insert(next, i * sign);
                halves[side].push(next);
                q = next;
                previous = next_v;
            }
        }
        let mut line: Polyline = halves[0].drain(..).rev().collect();
        line.push(seed);
        line.append(&mut halves[1]);
        if line.len() < 3 || length(&line) < spacing * 1.35 {
            continue;
        }
        for &q in &line {
            occupied.insert(q, 0);
        }
        // Candidatos perpendiculares a distancia uniforme: propagan la familia de
        // curvas localmente, sin depender de la alineación de la rejilla inicial.
        let stride = (spacing / step).ceil() as usize;
        for i in (1..line.len()).step_by(stride.max(1)) {
            let d = line[i] - line[i - 1];
            let n = p(-d.y, d.x) * (spacing / d.len().max(1e-9));
            for q in [line[i] + n, line[i] - n] {
                if field.inside(q) && sample(tone, q) >= threshold {
                    seeds.push_front(q);
                }
            }
        }
        // La simplificación es pequeña respecto a la separación: conserva rasgos
        // y no redondea las curvas hacia zonas blancas tras comprobar colisiones.
        lines.push(simplify(&line, (spacing * 0.06).min(0.3)));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectorize::{vectorize, VecMode};
    use image::{DynamicImage, Rgb};

    fn mono() -> VecParams {
        VecParams {
            mode: VecMode::Portrait,
            blur: 0.0,
            portrait: PortraitParams {
                color: false,
                density: 65,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn palettes_without_fixed_pens_keep_the_original_separation() {
        let rgb = RgbImage::new(2, 2);
        for palette in [MixPalette::Photo, MixPalette::Mine] {
            let separated = vec![VecLayer { color: [120, 80, 50], lines: vec![vec![p(0.0, 0.0), p(1.0, 1.0)]], closed: false, coverage: None, name: None }];
            let result = color_underprint(&rgb, palette, separated);
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].color, [120, 80, 50]);
            assert_eq!(result[0].lines, [vec![p(0.0, 0.0), p(1.0, 1.0)]]);
        }
    }

    #[test]
    fn fixed_palette_reserves_black_for_shadows_and_keeps_warm_skin_in_color() {
        let prm = VecParams {
            mode: VecMode::Portrait,
            blur: 0.0,
            ..Default::default()
        };
        let rgb = RgbImage::from_fn(80, 40, |x, _| {
            image::Rgb(if x < 40 { [220, 180, 160] } else { [15; 3] })
        });
        let separated = crate::vectorize::mix_mode(
            &rgb,
            &VecParams {
                mix_palette: MixPalette::Ryb,
                ..prm
            },
        );
        let layers = color_underprint(&rgb, MixPalette::Ryb, separated);
        let black = layers
            .iter()
            .find(|l| l.name == Some("Negro"))
            .unwrap()
            .coverage
            .as_ref()
            .unwrap();
        assert!(black.get(20, 20) < 0.01);
        assert!(black.get(60, 20) > 0.5);
        for name in ["Rojo", "Amarillo"] {
            let map = layers
                .iter()
                .find(|l| l.name == Some(name))
                .unwrap()
                .coverage
                .as_ref()
                .unwrap();
            assert!(
                map.get(20, 20) > 0.02,
                "{name} debe dibujar la piel: {}",
                map.get(20, 20)
            );
        }
    }

    #[test]
    fn field_is_tangent_to_curved_edges_and_flat_regions_use_fallback() {
        let cfg = PortraitParams {
            angle_deg: 31.0,
            ..Default::default()
        };
        let ring = RgbImage::from_fn(120, 120, |x, y| {
            let r = (x as f64 - 60.0).hypot(y as f64 - 60.0);
            Rgb([if r < 35.0 { 40 } else { 240 }; 3])
        });
        let f = Field::new(&ring, &cfg);
        assert!(
            f.direction(p(95.0, 60.0), p(0.0, 1.0), 0.0)
                .dot(p(0.0, 1.0))
                > 0.98
        );
        assert!(
            f.direction(p(60.0, 95.0), p(1.0, 0.0), 0.0)
                .dot(p(1.0, 0.0))
                > 0.98
        );
        let flat = Field::new(&RgbImage::from_pixel(20, 20, Rgb([80; 3])), &cfg);
        let expected = p(31_f64.to_radians().cos(), 31_f64.to_radians().sin());
        assert!(flat.direction(p(10.0, 10.0), expected, 0.0).dot(expected) > 0.999999);
    }

    #[test]
    fn white_is_blank_dark_tones_get_more_lines_and_paths_stay_in_bounds() {
        let prm = mono();
        let run = |v| {
            vectorize(
                &DynamicImage::ImageRgb8(RgbImage::from_pixel(96, 120, Rgb([v; 3]))),
                &prm,
            )
        };
        assert!(run(255).layers.is_empty());
        let (light, dark) = (run(200), run(30));
        let total = |r: &crate::vectorize::VecResult| {
            r.layers
                .iter()
                .flat_map(|l| &l.lines)
                .map(|l| length(l))
                .sum::<f64>()
        };
        assert!(total(&dark) > total(&light) * 1.7);
        for layer in dark.layers {
            assert!(!layer.closed && layer.coverage.is_none());
            for line in layer.lines {
                for q in line {
                    assert!(
                        q.x.is_finite()
                            && q.y.is_finite()
                            && q.x >= 0.0
                            && q.y >= 0.0
                            && q.x <= 95.0
                            && q.y <= 119.0
                    );
                }
            }
        }
    }

    #[test]
    fn pass_spacing_is_respected_and_generation_is_deterministic() {
        let cfg = PortraitParams {
            angle_deg: 0.0,
            ..Default::default()
        };
        let field = Field::new(&RgbImage::from_pixel(80, 80, Rgb([40; 3])), &cfg);
        let mut tone = Grid::new(80, 80);
        tone.data.fill(1.0);
        let a = streamlines(&field, &tone, &cfg, 0.1, 5.0, 0.0, 0);
        assert_eq!(a, streamlines(&field, &tone, &cfg, 0.1, 5.0, 0.0, 0));
        assert!(!a.is_empty());
        for (i, line) in a.iter().enumerate() {
            for other in &a[..i] {
                for &q in line {
                    assert!(crate::geom::dist_to_lines(q, std::slice::from_ref(other)) >= 3.4);
                }
            }
        }
    }

    #[test]
    fn color_uses_only_selected_inks_and_old_documents_get_defaults() {
        let mut prm = mono();
        prm.portrait.color = true;
        prm.portrait.palette = MixPalette::Mine;
        prm.mix_pens = vec![[214, 48, 49], [9, 92, 196]];
        let rgb = RgbImage::from_fn(100, 80, |x, _| Rgb(prm.mix_pens[usize::from(x >= 50)]));
        let r = vectorize(&DynamicImage::ImageRgb8(rgb), &prm);
        assert_eq!(r.layers.len(), 2);
        assert!(r
            .layers
            .iter()
            .all(|l| prm.mix_pens.contains(&l.color) && l.coverage.is_none()));
        let mut old = serde_json::to_value(VecParams::default()).unwrap();
        old.as_object_mut().unwrap().remove("portrait");
        assert_eq!(
            serde_json::from_value::<VecParams>(old).unwrap().portrait,
            PortraitParams::default()
        );
        assert_eq!(
            serde_json::from_str::<VecParams>(&serde_json::to_string(&prm).unwrap()).unwrap(),
            prm
        );
        for (w, h) in [(1, 1), (1, 80), (80, 1)] {
            assert!(render(&RgbImage::from_pixel(w, h, Rgb([0; 3])), &prm).is_empty());
        }
    }
}
