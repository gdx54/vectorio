//! Importación de SVG (por ejemplo, salidas de Linear) como trazados editables.

use crate::geom::{p, Polyline, P};
use usvg::tiny_skia_path::{PathSegment, Transform};

mod clip;

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
    walk(tree.root(), &[], &mut out)?;
    Ok(out)
}

fn color_of(paint: &usvg::Paint) -> Option<[u8; 3]> {
    match paint {
        usvg::Paint::Color(c) => Some([c.red, c.green, c.blue]),
        _ => None,
    }
}

fn walk(g: &usvg::Group, inherited: &[clip::Region], out: &mut Vec<Imported>) -> Result<(), String> {
    if g.mask().is_some() { return Err("El SVG contiene máscaras que no se pueden convertir fielmente en trazos. Convierte las máscaras en trazados antes de importarlo.".into()); }
    let mut clips = inherited.to_vec();
    if let Some(path) = g.clip_path() { clips.push(clip::from_clip(path, g.abs_transform())?); }
    for node in g.children() {
        match node {
            usvg::Node::Group(child) => walk(child, &clips, out)?,
            usvg::Node::Path(path) if path.is_visible() => {
                let (mut lines, mut closed) = path_lines(path, path.abs_transform());
                if !clips.is_empty() {
                    lines = clip::apply(&lines, &clips);
                    closed &= lines.iter().all(|line| line.first() == line.last());
                }
                if lines.is_empty() { continue; }
                let color = path.stroke().and_then(|s| color_of(s.paint()))
                    .or_else(|| path.fill().and_then(|f| color_of(f.paint()))).unwrap_or([0, 0, 0]);
                out.push(Imported { lines, closed, color });
            }
            _ => {}
        }
    }
    Ok(())
}

/// Aplana curvas en coordenadas del documento, tanto para trazos como para recortes.
fn path_lines(path: &usvg::Path, ts: Transform) -> (Vec<Polyline>, bool) {
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
                if cur.len() >= 2 {
                    if cur.last() != Some(&start) { cur.push(start); }
                    lines.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
                last = start;
            }
        }
    }
    if cur.len() >= 2 {
        all_closed = false;
        lines.push(cur);
    }
    (lines, all_closed)
}

fn steps(len_mm: f64) -> usize {
    ((len_mm / 0.3) as usize).clamp(4, 256)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(q: f64) -> f64 { q / PX_TO_MM }

    #[test]
    fn clip_path_cuts_segments_instead_of_importing_hidden_geometry() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <defs><clipPath id="c"><rect width="10" height="10"/></clipPath></defs>
            <path clip-path="url(#c)" d="M0 0L100 100" stroke="black" fill="none"/>
        </svg>"##;
        let imported = import(svg).unwrap();
        assert_eq!(imported.len(), 1);
        let end = *imported[0].lines[0].last().unwrap();
        assert!((px(end.x) - 10.0).abs() < 1e-4 && (px(end.y) - 10.0).abs() < 1e-4);
        assert!(!imported[0].closed);
    }

    #[test]
    fn transformed_clips_union_their_children_and_intersect_their_parents() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <defs><clipPath id="c"><rect width="10" height="10"/><rect x="20" width="10" height="10"/></clipPath>
            <clipPath id="outer"><rect x="15" width="20" height="80"/></clipPath></defs>
            <g clip-path="url(#outer)"><g transform="translate(10 20)" clip-path="url(#c)">
            <path d="M-5 5L40 5" stroke="black" fill="none"/></g></g>
        </svg>"##;
        let imported = import(svg).unwrap();
        let lines = &imported[0].lines;
        assert_eq!(lines.len(), 2);
        for (line, (start, end)) in lines.iter().zip([(15.0, 20.0), (30.0, 35.0)]) {
            assert!((px(line[0].x) - start).abs() < 1e-4);
            assert!((px(line.last().unwrap().x) - end).abs() < 1e-4);
            assert!(line.iter().all(|p| (px(p.y) - 25.0).abs() < 1e-4));
        }
    }

    #[test]
    fn evenodd_holes_do_not_get_bridged_and_object_bounding_box_clips_scale() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <defs><clipPath id="c"><path clip-rule="evenodd" d="M0 0H20V20H0Z M5 5H15V15H5Z"/></clipPath></defs>
            <path clip-path="url(#c)" d="M-5 10H25" stroke="black" fill="none"/>
        </svg>"##;
        let imported = import(svg).unwrap();
        assert_eq!(imported[0].lines.len(), 2);
        assert!((px(imported[0].lines[0].last().unwrap().x) - 5.0).abs() < 1e-4);
        assert!((px(imported[0].lines[1][0].x) - 15.0).abs() < 1e-4);
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <defs><clipPath id="c" clipPathUnits="objectBoundingBox"><rect width="0.5" height="1"/></clipPath></defs>
            <path clip-path="url(#c)" d="M10 10L50 50" stroke="black" fill="none"/>
        </svg>"##;
        let imported = import(svg).unwrap();
        assert!((px(imported[0].lines[0].last().unwrap().x) - 30.0).abs() < 1e-4);
    }

    #[test]
    fn unsupported_masks_are_reported_instead_of_silently_ignored() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <defs><mask id="m"><rect width="10" height="10" fill="white"/></mask></defs>
            <path mask="url(#m)" d="M0 0L100 100" stroke="black" fill="none"/>
        </svg>"##;
        assert!(import(svg).err().unwrap().contains("máscaras"));
    }

    #[test]
    fn empty_closed_subpaths_do_not_add_phantom_strokes() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <path d="M10 10Z M20 20L30 20L30 30Z M50 50Z" stroke="black" fill="none"/>
        </svg>"#;
        let imported = import(svg).unwrap();
        let lines: Vec<_> = imported.iter().flat_map(|item| &item.lines).collect();
        assert_eq!(lines.len(), 1);
        assert!(imported.iter().all(|item| item.closed));
        assert_eq!(lines[0].first(), lines[0].last());
        assert!(crate::geom::length(lines[0]) > 0.0);
    }

    #[test]
    fn mixed_open_and_closed_subpaths_keep_their_segments() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <path d="M10 10L20 10 M30 30L40 30L40 40Z" stroke="black" fill="none"/>
        </svg>"#;
        let imported = import(svg).unwrap();
        assert_eq!(imported.len(), 1);
        assert!(!imported[0].closed);
        assert_eq!(imported[0].lines.iter().map(Vec::len).collect::<Vec<_>>(), [2, 4]);
    }
}
