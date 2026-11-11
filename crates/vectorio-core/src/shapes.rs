//! Formas paramétricas editables: básicas y generativas. Coordenadas locales en mm
//! centradas en el origen.

use crate::geom::{p, Polyline};
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Rect { w: f64, h: f64, radius: f64 },
    Ellipse { rx: f64, ry: f64 },
    Polygon { sides: u32, r: f64 },
    Star { points: u32, outer: f64, inner: f64 },
    Heart { size: f64 },
    Spiral { turns: f64, r: f64 },
    /// Hipotrocoide (espirógrafo) con radios enteros para que cierre.
    Spirograph { big_r: u32, small_r: u32, offset: f64, size: f64 },
    Lissajous { a: u32, b: u32, phase_deg: f64, w: f64, h: f64 },
    /// Rosa polar r = cos(n/d · θ).
    Rose { n: u32, d: u32, r: f64 },
}

impl Shape {
    pub fn presets() -> Vec<Shape> {
        vec![
            Shape::Rect { w: 60.0, h: 40.0, radius: 0.0 },
            Shape::Ellipse { rx: 30.0, ry: 30.0 },
            Shape::Polygon { sides: 6, r: 30.0 },
            Shape::Star { points: 5, outer: 30.0, inner: 13.0 },
            Shape::Heart { size: 50.0 },
            Shape::Spiral { turns: 8.0, r: 30.0 },
            Shape::Spirograph { big_r: 96, small_r: 35, offset: 30.0, size: 80.0 },
            Shape::Lissajous { a: 3, b: 4, phase_deg: 90.0, w: 70.0, h: 70.0 },
            Shape::Rose { n: 5, d: 3, r: 35.0 },
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Shape::Rect { .. } => "Rectángulo",
            Shape::Ellipse { .. } => "Elipse",
            Shape::Polygon { .. } => "Polígono",
            Shape::Star { .. } => "Estrella",
            Shape::Heart { .. } => "Corazón",
            Shape::Spiral { .. } => "Espiral",
            Shape::Spirograph { .. } => "Espirógrafo",
            Shape::Lissajous { .. } => "Lissajous",
            Shape::Rose { .. } => "Rosa polar",
        }
    }

    pub fn is_closed(&self) -> bool {
        !matches!(self, Shape::Spiral { .. })
    }

    /// Ajusta la forma para que ocupe la caja `w × h` (al dibujarla arrastrando).
    pub fn fit(&mut self, w: f64, h: f64) {
        let (w, h) = (w.max(1.0), h.max(1.0));
        let m = w.min(h);
        match self {
            Shape::Rect { w: sw, h: sh, .. } => {
                *sw = w;
                *sh = h;
            }
            Shape::Ellipse { rx, ry } => {
                *rx = w / 2.0;
                *ry = h / 2.0;
            }
            Shape::Polygon { r, .. } => *r = m / 2.0,
            Shape::Star { outer, inner, .. } => {
                let k = *inner / outer.max(1e-9);
                *outer = m / 2.0;
                *inner = *outer * k;
            }
            Shape::Heart { size } => *size = m,
            Shape::Spiral { r, .. } => *r = m / 2.0,
            Shape::Spirograph { size, .. } => *size = m,
            Shape::Lissajous { w: lw, h: lh, .. } => {
                *lw = w;
                *lh = h;
            }
            Shape::Rose { r, .. } => *r = m / 2.0,
        }
    }

    pub fn outline(&self) -> Polyline {
        let mut pts: Polyline = match *self {
            Shape::Rect { w, h, radius } => {
                let (hw, hh) = (w / 2.0, h / 2.0);
                let r = radius.clamp(0.0, hw.min(hh));
                if r < 0.01 {
                    vec![p(-hw, -hh), p(hw, -hh), p(hw, hh), p(-hw, hh)]
                } else {
                    let mut v = Vec::new();
                    let corners = [(hw - r, -hh + r, -PI / 2.0), (hw - r, hh - r, 0.0), (-hw + r, hh - r, PI / 2.0), (-hw + r, -hh + r, PI)];
                    for (cx, cy, a0) in corners {
                        for k in 0..=12 {
                            let a = a0 + k as f64 / 12.0 * PI / 2.0;
                            v.push(p(cx + r * a.cos(), cy + r * a.sin()));
                        }
                    }
                    v
                }
            }
            Shape::Ellipse { rx, ry } => {
                let n = ((rx.max(ry) * 2.0) as usize).clamp(48, 720);
                (0..n).map(|k| {
                    let a = k as f64 / n as f64 * TAU;
                    p(rx * a.cos(), ry * a.sin())
                })
                .collect()
            }
            Shape::Polygon { sides, r } => {
                let n = sides.max(3);
                (0..n).map(|k| {
                    let a = k as f64 / n as f64 * TAU - PI / 2.0;
                    p(r * a.cos(), r * a.sin())
                })
                .collect()
            }
            Shape::Star { points, outer, inner } => {
                let n = points.max(2) * 2;
                (0..n).map(|k| {
                    let a = k as f64 / n as f64 * TAU - PI / 2.0;
                    let r = if k % 2 == 0 { outer } else { inner };
                    p(r * a.cos(), r * a.sin())
                })
                .collect()
            }
            Shape::Heart { size } => {
                let s = size / 34.0;
                (0..240)
                    .map(|k| {
                        let t = k as f64 / 240.0 * TAU;
                        let x = 16.0 * t.sin().powi(3);
                        let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
                        p(x * s, -y * s)
                    })
                    .collect()
            }
            Shape::Spiral { turns, r } => {
                let n = (turns * 90.0).clamp(30.0, 20_000.0) as usize;
                (0..=n)
                    .map(|k| {
                        let t = k as f64 / n as f64;
                        let a = t * turns * TAU;
                        p(r * t * a.cos(), r * t * a.sin())
                    })
                    .collect()
            }
            Shape::Spirograph { big_r, small_r, offset, size } => {
                let (rr, r) = (big_r.max(2) as f64, small_r.clamp(1, big_r.max(2) - 1) as f64);
                let g = gcd(big_r.max(2), small_r.clamp(1, big_r.max(2) - 1)) as f64;
                let loops = r / g;
                let extent = (rr - r).abs() + offset.abs();
                let s = size / (2.0 * extent.max(1e-6));
                let n = ((loops * 360.0) as usize).clamp(360, 60_000);
                (0..n)
                    .map(|k| {
                        let t = k as f64 / n as f64 * loops * TAU;
                        let x = (rr - r) * t.cos() + offset * ((rr - r) / r * t).cos();
                        let y = (rr - r) * t.sin() - offset * ((rr - r) / r * t).sin();
                        p(x * s, y * s)
                    })
                    .collect()
            }
            Shape::Lissajous { a, b, phase_deg, w, h } => {
                let n = (((a + b) as f64) * 200.0) as usize;
                let ph = phase_deg.to_radians();
                (0..n)
                    .map(|k| {
                        let t = k as f64 / n as f64 * TAU;
                        p(w / 2.0 * (a.max(1) as f64 * t + ph).sin(), h / 2.0 * (b.max(1) as f64 * t).sin())
                    })
                    .collect()
            }
            Shape::Rose { n, d, r } => {
                let (n, d) = (n.max(1), d.max(1));
                let g = gcd(n, d);
                let (n, d) = (n / g, d / g);
                let period = if (n * d) % 2 == 1 { PI * d as f64 } else { TAU * d as f64 };
                let k = n as f64 / d as f64;
                let steps = (period * 120.0) as usize;
                (0..steps)
                    .map(|i| {
                        let th = i as f64 / steps as f64 * period;
                        let rho = r * (k * th).cos();
                        p(rho * th.cos(), rho * th.sin())
                    })
                    .collect()
            }
        };
        if self.is_closed() && pts.len() > 2 {
            pts.push(pts[0]);
        }
        pts
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a.max(1)
    } else {
        gcd(b, a % b)
    }
}
