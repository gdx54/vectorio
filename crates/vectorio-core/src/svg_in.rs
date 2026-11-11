//! Importación de SVG (por ejemplo, salidas de Linear) como trazados editables.

use crate::geom::{p, Polyline, P};
use usvg::tiny_skia_path::PathSegment;

/// Trazado importado: líneas en mm, si es cerrado y su color.
pub struct Imported {
    pub lines: Vec<Polyline>,
    pub closed: bool,
    pub color: [u8; 3],
}

const PX_TO_MM: f64 = 25.4 / 96.0;

pub fn import(data: &[u8]) -> Result<Vec<Imported>, String> {
    let tree = usvg::Tree::from_data(data, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    walk(tree.root(), &mut out);
    Ok(out)
}

fn color_of(paint: &usvg::Paint) -> Option<[u8; 3]> {
    match paint {
        usvg::Paint::Color(c) => Some([c.red, c.green, c.blue]),
        _ => None,
    }
}

fn walk(g: &usvg::Group, out: &mut Vec<Imported>) {
    for node in g.children() {
        match node {
            usvg::Node::Group(child) => walk(child, out),
            usvg::Node::Path(path) => {
                let ts = path.abs_transform();
                let map = |x: f32, y: f32| -> P {
                    let (x, y) = (x as f64, y as f64);
                    p(
                        (ts.sx as f64 * x + ts.kx as f64 * y + ts.tx as f64) * PX_TO_MM,
                        (ts.ky as f64 * x + ts.sy as f64 * y + ts.ty as f64) * PX_TO_MM,
                    )
                };
                let mut lines: Vec<Polyline> = Vec::new();
                let mut cur: Polyline = Vec::new();
                let mut all_closed = true;
                let mut last = P::default();
                let mut start = P::default();
                for seg in path.data().segments() {
                    match seg {
                        PathSegment::MoveTo(a) => {
                            if cur.len() >= 2 {
                                all_closed = false;
                                lines.push(std::mem::take(&mut cur));
                            }
                            cur.clear();
                            last = map(a.x, a.y);
                            start = last;
                            cur.push(last);
                        }
                        PathSegment::LineTo(a) => {
                            last = map(a.x, a.y);
                            cur.push(last);
                        }
                        PathSegment::QuadTo(c, a) => {
                            let (c, a) = (map(c.x, c.y), map(a.x, a.y));
                            let n = steps(last.dist(c) + c.dist(a));
                            for k in 1..=n {
                                let t = k as f64 / n as f64;
                                cur.push(last.lerp(c, t).lerp(c.lerp(a, t), t));
                            }
                            last = a;
                        }
                        PathSegment::CubicTo(c1, c2, a) => {
                            let (c1, c2, a) = (map(c1.x, c1.y), map(c2.x, c2.y), map(a.x, a.y));
                            let n = steps(last.dist(c1) + c1.dist(c2) + c2.dist(a));
                            for k in 1..=n {
                                let t = k as f64 / n as f64;
                                let u = 1.0 - t;
                                cur.push(last * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + a * (t * t * t));
                            }
                            last = a;
                        }
                        PathSegment::Close => {
                            if cur.first() != Some(&start) || cur.len() < 2 {
                                cur.push(start);
                            } else if cur.last() != Some(&start) {
                                cur.push(start);
                            }
                            if cur.len() >= 2 {
                                lines.push(std::mem::take(&mut cur));
                            }
                            last = start;
                        }
                    }
                }
                if cur.len() >= 2 {
                    all_closed = false;
                    lines.push(cur);
                }
                if lines.is_empty() {
                    continue;
                }
                let color = path
                    .stroke()
                    .and_then(|s| color_of(s.paint()))
                    .or_else(|| path.fill().and_then(|f| color_of(f.paint())))
                    .unwrap_or([0, 0, 0]);
                out.push(Imported { lines, closed: all_closed, color });
            }
            _ => {}
        }
    }
}

fn steps(len_mm: f64) -> usize {
    ((len_mm / 0.3) as usize).clamp(4, 256)
}
