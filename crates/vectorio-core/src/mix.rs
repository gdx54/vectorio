//! Mezcla óptica: la foto se reproduce superponiendo varios rotuladores, como en una
//! cuatricromía. Cada color se descompone en cuánto aporta cada rotulador (cobertura
//! 0–1) y cada cobertura se convierte en una trama: rayado con un ángulo distinto por
//! rotulador (las líneas se intercalan y se cruzan) o puntillismo.
//!
//! Modelo de color: las tintas se comportan como filtros. Trabajamos con densidad óptica
//! (D = −ln reflectancia) por canal, donde las tintas superpuestas se suman, y
//! resolvemos para cada color unas coberturas no negativas cuya suma de densidades se
//! parezca a la del píxel.

use crate::geom::{p, Polyline};
use crate::raster::Grid;
use image::RgbImage;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MixPalette {
    /// Cian, magenta, amarillo y negro.
    Cmyk,
    /// Rojo, amarillo, azul y negro: colores de rotulador habituales.
    Ryb,
    /// Los colores principales de la propia foto.
    Photo,
}

impl MixPalette {
    pub const ALL: [MixPalette; 3] = [MixPalette::Cmyk, MixPalette::Ryb, MixPalette::Photo];
    pub fn label(self) -> &'static str {
        match self {
            MixPalette::Cmyk => "Cian · magenta · amarillo · negro",
            MixPalette::Ryb => "Rojo · amarillo · azul · negro",
            MixPalette::Photo => "Colores de la foto",
        }
    }
    /// Rotuladores fijos de la paleta (vacío en `Photo`).
    pub fn pens(self) -> Vec<(&'static str, [u8; 3])> {
        match self {
            MixPalette::Cmyk => vec![("Amarillo", [250, 214, 30]), ("Magenta", [226, 0, 122]), ("Cian", [0, 160, 226]), ("Negro", [30, 30, 34])],
            MixPalette::Ryb => vec![("Amarillo", [247, 200, 30]), ("Rojo", [214, 48, 49]), ("Azul", [9, 92, 196]), ("Negro", [30, 30, 34])],
            MixPalette::Photo => vec![],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Screen {
    /// Rayado: cada rotulador con su ángulo; la cobertura decide qué líneas se dibujan.
    Hatch,
    /// Puntillismo: rejilla hexagonal con umbrales intercalados.
    Dots,
}

impl Screen {
    pub fn label(self) -> &'static str {
        match self {
            Screen::Hatch => "Rayado",
            Screen::Dots => "Puntos",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenParams {
    pub screen: Screen,
    /// Separación entre líneas o puntos con cobertura completa, mm.
    pub spacing_mm: f64,
    /// Ángulo del primer rotulador; los demás se reparten en 180°.
    pub angle_deg: f64,
    pub pen_width_mm: f64,
}

impl Default for ScreenParams {
    fn default() -> Self {
        ScreenParams { screen: Screen::Hatch, spacing_mm: 1.0, angle_deg: 15.0, pen_width_mm: 0.6 }
    }
}

const MIN_R: f64 = 0.04;

fn density(c: [u8; 3]) -> [f64; 3] {
    c.map(|v| -((v as f64 / 255.0).max(MIN_R)).ln())
}

/// Coberturas 0–1 de cada rotulador que mejor reproducen el color (mínimos cuadrados
/// con límites, por descenso por coordenadas). Una pequeña penalización evita gastar
/// tinta de más cuando dos combinaciones dan el mismo color (`cost`).
fn solve(target: [f64; 3], pens: &[[f64; 3]], norms: &[f64], cost: &[f64]) -> Vec<f64> {
    let n = pens.len();
    let mut c = vec![0.0f64; n];
    // Residuo r = Σ c·D − objetivo.
    let mut r = [-target[0], -target[1], -target[2]];
    for _ in 0..40 {
        let mut moved = 0.0;
        for i in 0..n {
            let d = &pens[i];
            let without = [r[0] - c[i] * d[0], r[1] - c[i] * d[1], r[2] - c[i] * d[2]];
            let new = (-(without[0] * d[0] + without[1] * d[1] + without[2] * d[2]) - cost[i]) / norms[i];
            let new = new.clamp(0.0, 1.0);
            moved += (new - c[i]).abs();
            c[i] = new;
            r = [without[0] + new * d[0], without[1] + new * d[1], without[2] + new * d[2]];
        }
        if moved < 1e-4 {
            break;
        }
    }
    c
}

/// Mapas de cobertura (uno por rotulador) del tamaño de la imagen.
pub fn coverage(rgb: &RgbImage, pens: &[[u8; 3]]) -> Vec<Grid<f32>> {
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let d: Vec<[f64; 3]> = pens.iter().map(|c| density(*c)).collect();
    let norms: Vec<f64> = d.iter().map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).max(1e-6)).collect();
    // Coste por unidad de tinta: los rotuladores neutros (negro, gris) salen más baratos,
    // así los tonos apagados usan negro en vez de mezclar todos los colores
    // (sustitución del componente gris).
    let cost: Vec<f64> = d
        .iter()
        .map(|v| {
            let (lo, hi) = (v[0].min(v[1]).min(v[2]), v[0].max(v[1]).max(v[2]));
            0.02 + 0.3 * (hi - lo) / hi.max(1e-6)
        })
        .collect();
    // Tabla de 32³ colores: se resuelve una vez por color cuantizado.
    const Q: usize = 32;
    let lut: Vec<Vec<f32>> = (0..Q * Q * Q)
        .into_par_iter()
        .map(|i| {
            let (r, g, b) = (i / (Q * Q), (i / Q) % Q, i % Q);
            let lvl = |v: usize| ((v * 255 + (Q - 1) / 2) / (Q - 1)) as u8;
            solve(density([lvl(r), lvl(g), lvl(b)]), &d, &norms, &cost).into_iter().map(|v| v as f32).collect()
        })
        .collect();
    let mut maps: Vec<Grid<f32>> = (0..pens.len()).map(|_| Grid::new(w, h)).collect();
    let q = |v: u8| ((v as usize * (Q - 1) + 127) / 255).min(Q - 1);
    for (i, px) in rgb.pixels().enumerate() {
        let c = &lut[q(px.0[0]) * Q * Q + q(px.0[1]) * Q + q(px.0[2])];
        for (m, v) in maps.iter_mut().zip(c) {
            // Curva suave: la tinta se extiende algo en el papel (ganancia de punto), así
            // que los tonos claros piden un poco menos.
            m.data[i] = if *v < 0.04 { 0.0 } else { v.powf(1.2) };
        }
    }
    maps
}

fn sample(g: &Grid<f32>, x: f64, y: f64) -> f64 {
    if g.w == 0 || g.h == 0 {
        return 0.0;
    }
    let x = (x - 0.5).clamp(0.0, (g.w - 1) as f64);
    let y = (y - 0.5).clamp(0.0, (g.h - 1) as f64);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(g.w - 1), (y0 + 1).min(g.h - 1));
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let a = g.get(x0, y0) as f64 * (1.0 - fx) + g.get(x1, y0) as f64 * fx;
    let b = g.get(x0, y1) as f64 * (1.0 - fx) + g.get(x1, y1) as f64 * fx;
    a * (1.0 - fy) + b * fy
}

fn frac(v: f64) -> f64 {
    v - v.floor()
}

/// Ángulo de la trama del rotulador `index` de `count`: repartidos en 180° para que las
/// líneas de cada color se crucen en vez de taparse.
pub fn screen_angle(prm: &ScreenParams, index: usize, count: usize) -> f64 {
    prm.angle_deg + 180.0 * index as f64 / count.max(1) as f64
}

/// Trama de un mapa de cobertura, en mm con origen en la esquina superior izquierda de
/// la imagen. `mm_per_px` es el tamaño en el papel de cada píxel del mapa.
pub fn render(cov: &Grid<f32>, mm_per_px: f64, prm: &ScreenParams, index: usize, count: usize) -> Vec<Polyline> {
    match prm.screen {
        Screen::Hatch => hatch(cov, mm_per_px, prm, index, count),
        Screen::Dots => dots(cov, mm_per_px, prm, index),
    }
}

fn hatch(cov: &Grid<f32>, k: f64, prm: &ScreenParams, index: usize, count: usize) -> Vec<Polyline> {
    let (wm, hm) = (cov.w as f64 * k, cov.h as f64 * k);
    let s = prm.spacing_mm.max(0.2);
    let a = screen_angle(prm, index, count).to_radians();
    let (u, n) = (p(a.cos(), a.sin()), p(-a.sin(), a.cos()));
    let corners = [p(0.0, 0.0), p(wm, 0.0), p(0.0, hm), p(wm, hm)];
    let proj = |d: crate::geom::P| corners.iter().map(|c| c.x * d.x + c.y * d.y).fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let (n0, n1) = proj(n);
    let (u0, u1) = proj(u);
    let step = (s * 0.4).min(k).max(0.08);
    let min_len = (prm.pen_width_mm * 1.2).max(0.4);
    let dash = (s * 4.0).max(2.0);
    let first = (n0 / s).floor() as i64;
    let last = (n1 / s).ceil() as i64;
    let lines: Vec<Vec<Polyline>> = (first..=last)
        .into_par_iter()
        .map(|j| {
            // Umbral por tramo (línea j, celda m a lo largo de ella) con la secuencia R2:
            // en tonos claros salen trazos cortos bien repartidos y en los oscuros se
            // juntan en líneas largas.
            let threshold = |m: i64| 0.03 + 0.94 * frac(j as f64 * 0.754_877_666 + m as f64 * 0.569_840_291 + index as f64 * 0.31);
            let o = j as f64 * s;
            let mut out = Vec::new();
            let mut cur: Vec<crate::geom::P> = Vec::new();
            let mut on = false;
            let mut x = u0;
            while x <= u1 + step {
                let q = n * o + u * x;
                let inside = q.x >= 0.0 && q.y >= 0.0 && q.x <= wm && q.y <= hm;
                let d = if inside { sample(cov, q.x / k, q.y / k) } else { 0.0 };
                let t = threshold(((x - u0) / dash).floor() as i64);
                // Histéresis para no fragmentar las líneas en el borde de un tono.
                on = if on { d > t - 0.03 } else { d > t + 0.03 };
                if on && inside {
                    cur.push(q);
                } else if !cur.is_empty() {
                    let seg = std::mem::take(&mut cur);
                    if seg.len() >= 2 && seg[0].dist(*seg.last().unwrap()) >= min_len {
                        out.push(vec![seg[0], *seg.last().unwrap()]);
                    }
                }
                x += step;
            }
            if cur.len() >= 2 && cur[0].dist(*cur.last().unwrap()) >= min_len {
                out.push(vec![cur[0], *cur.last().unwrap()]);
            }
            if j % 2 != 0 {
                out.reverse();
                for l in &mut out {
                    l.reverse();
                }
            }
            out
        })
        .collect();
    lines.into_iter().flatten().collect()
}

fn dots(cov: &Grid<f32>, k: f64, prm: &ScreenParams, index: usize) -> Vec<Polyline> {
    let (wm, hm) = (cov.w as f64 * k, cov.h as f64 * k);
    let s = prm.spacing_mm.max(0.3);
    let row_h = s * 0.866;
    let r = (prm.pen_width_mm * 0.25).max(0.1);
    let rows = (hm / row_h).ceil() as usize + 1;
    // Cada rotulador desplaza su rejilla para que los puntos de colores se intercalen.
    let shift = p(frac(index as f64 * 0.618_034) * s, frac(index as f64 * 0.414_214) * row_h);
    let out: Vec<Vec<Polyline>> = (0..rows)
        .into_par_iter()
        .map(|row| {
            let y = row as f64 * row_h + shift.y;
            let off = if row % 2 == 0 { 0.0 } else { s / 2.0 };
            let mut v = Vec::new();
            let mut i = 0usize;
            let mut x = off + shift.x;
            while x < wm {
                // Umbral R2 (secuencia de baja discrepancia en 2D).
                let t = frac(i as f64 * 0.754_877_666 + row as f64 * 0.569_840_291 + index as f64 * 0.31);
                let jx = (frac(t * 7.31) - 0.5) * s * 0.18;
                let jy = (frac(t * 3.17) - 0.5) * s * 0.18;
                let q = p(x + jx, y + jy);
                if q.x >= 0.0 && q.y >= 0.0 && q.x <= wm && q.y <= hm && sample(cov, q.x / k, q.y / k) > 0.02 + t * 0.96 {
                    v.push((0..=6).map(|a| q + p(r, 0.0).rotate(a as f64 / 6.0 * std::f64::consts::TAU)).collect());
                }
                x += s;
                i += 1;
            }
            if row % 2 == 1 {
                v.reverse();
            }
            v
        })
        .collect();
    out.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn coverage_separates_primary_colors() {
        let mut img = RgbImage::from_pixel(3, 1, Rgb([255, 255, 255]));
        img.put_pixel(0, 0, Rgb([250, 214, 30]));
        img.put_pixel(1, 0, Rgb([30, 30, 34]));
        let pens: Vec<[u8; 3]> = MixPalette::Cmyk.pens().iter().map(|p| p.1).collect();
        let m = coverage(&img, &pens);
        assert!(m[0].get(0, 0) > 0.8, "amarillo puro: {}", m[0].get(0, 0));
        assert!(m[3].get(1, 0) > 0.7, "negro: {}", m[3].get(1, 0));
        assert!(m.iter().all(|g| g.get(2, 0) < 0.05), "papel sin tinta");
    }

    #[test]
    fn hatch_density_follows_coverage() {
        let mut g = Grid::<f32>::new(100, 100);
        for y in 0..100 {
            for x in 50..100 {
                g.set(x, y, 0.8);
            }
            for x in 0..50 {
                g.set(x, y, 0.2);
            }
        }
        let prm = ScreenParams { spacing_mm: 1.0, angle_deg: 0.0, ..Default::default() };
        let lines = render(&g, 1.0, &prm, 0, 1);
        // Líneas que cruzan una sección vertical en la zona clara y en la oscura.
        let crossing = |x: f64| lines.iter().filter(|l| l[0].x.min(l[1].x) <= x && l[0].x.max(l[1].x) >= x).count() as f64;
        let (light, dark) = (crossing(25.0), crossing(75.0));
        assert!(dark > light * 2.5, "oscuro {dark} claro {light}");
    }
}
