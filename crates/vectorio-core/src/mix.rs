//! Mezcla óptica: la foto se reproduce superponiendo varios rotuladores, como en una
//! cuatricromía. Cada color se descompone en cuánto aporta cada rotulador (cobertura
//! 0–1) y cada cobertura se convierte en una trama: rayado con un ángulo distinto por
//! rotulador (las líneas se intercalan y se cruzan) o puntillismo, con el motor de
//! tramas de [`crate::trace`].
//!
//! Modelo de color: las tintas se comportan como filtros. Trabajamos con densidad óptica
//! (D = −ln reflectancia) por canal, donde las tintas superpuestas se suman, y
//! resolvemos para cada color unas coberturas no negativas cuya suma de densidades se
//! parezca a la del píxel.

use crate::geom::Polyline;
use crate::trace::{self, TraceParams};
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

/// Tipo de trama de la mezcla (el mismo que usa el motor de tramas).
pub use crate::trace::TraceStyle as Screen;

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

/// Ángulo de la trama del rotulador `index` de `count`: repartidos en 180° para que las
/// líneas de cada color se crucen en vez de taparse.
pub fn screen_angle(prm: &ScreenParams, index: usize, count: usize) -> f64 {
    prm.angle_deg + 180.0 * index as f64 / count.max(1) as f64
}

/// Ajustes de trama del rotulador `index` de `count` para el motor de tramas.
pub fn trace_params(prm: &ScreenParams, index: usize, count: usize) -> TraceParams {
    TraceParams { style: prm.screen, spacing_mm: prm.spacing_mm, angle_deg: screen_angle(prm, index, count).rem_euclid(360.0), ..Default::default() }
}

/// Trama de un mapa de cobertura, en mm con origen en la esquina superior izquierda de
/// la imagen. `mm_per_px` es el tamaño en el papel de cada píxel del mapa.
pub fn render(cov: &Grid<f32>, mm_per_px: f64, prm: &ScreenParams, index: usize, count: usize) -> Vec<Polyline> {
    trace::render(cov, mm_per_px, &trace_params(prm, index, count), prm.pen_width_mm, index)
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
