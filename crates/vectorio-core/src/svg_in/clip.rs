//! Recorte vectorial de polilíneas: unión dentro de un clipPath e intersección entre clips.
use super::{path_lines, Polyline, P};
use usvg::tiny_skia_path::Transform;

#[derive(Clone)]
pub(super) enum Region {
    Shape(Vec<Polyline>, usvg::FillRule),
    Union(Vec<Region>),
    Intersection(Vec<Region>),
}

pub(super) fn from_clip(clip: &usvg::ClipPath, parent: Transform) -> Result<Region, String> {
    let base = parent.pre_concat(clip.transform());
    let region = from_group(clip.root(), base)?;
    Ok(if let Some(extra) = clip.clip_path() {
        Region::Intersection(vec![region, from_clip(extra, parent)?])
    } else { region })
}

fn from_group(group: &usvg::Group, base: Transform) -> Result<Region, String> {
    let mut children = Vec::new();
    for node in group.children() {
        match node {
            usvg::Node::Group(child) => children.push(from_group(child, base)?),
            usvg::Node::Path(path) if path.is_visible() => {
                let (rings, _) = path_lines(path, base.pre_concat(path.abs_transform()));
                children.push(Region::Shape(rings, path.fill().map_or(usvg::FillRule::NonZero, |f| f.rule())));
            }
            usvg::Node::Path(_) => {}
            _ => return Err("El recorte SVG contiene elementos que no se pueden convertir en trazados.".into()),
        }
    }
    let region = Region::Union(children);
    Ok(if let Some(clip) = group.clip_path() {
        Region::Intersection(vec![region, from_clip(clip, base.pre_concat(group.abs_transform()))?])
    } else { region })
}

fn cross(a: P, b: P) -> f64 { a.x * b.y - a.y * b.x }

impl Region {
    fn edges(&self, visit: &mut impl FnMut(P, P)) {
        match self {
            Self::Shape(rings, _) => for ring in rings.iter().filter(|r| r.len() >= 3) {
                for i in 0..ring.len() { visit(ring[i], ring[(i + 1) % ring.len()]); }
            },
            Self::Union(regions) | Self::Intersection(regions) => for region in regions { region.edges(visit); },
        }
    }

    fn contains(&self, q: P) -> bool {
        match self {
            Self::Union(regions) => regions.iter().any(|r| r.contains(q)),
            Self::Intersection(regions) => regions.iter().all(|r| r.contains(q)),
            Self::Shape(rings, rule) => {
                let mut winding = 0i32;
                let mut boundary = false;
                for ring in rings.iter().filter(|r| r.len() >= 3) {
                    for i in 0..ring.len() {
                        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                        let edge = b - a;
                        let side = cross(edge, q - a);
                        if edge.len() > 1e-12 && side.abs() <= 1e-8 * edge.len()
                            && (q - a).dot(q - b) <= 1e-12 { boundary = true; }
                        if a.y <= q.y && b.y > q.y && side > 0.0 { winding += 1; }
                        if a.y > q.y && b.y <= q.y && side < 0.0 { winding -= 1; }
                    }
                }
                boundary || match rule { usvg::FillRule::EvenOdd => winding % 2 != 0, usvg::FillRule::NonZero => winding != 0 }
            }
        }
    }
}

pub(super) fn apply(lines: &[Polyline], clips: &[Region]) -> Vec<Polyline> {
    let mut out = Vec::new();
    for line in lines {
        let mut current: Polyline = Vec::new();
        for pair in line.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let delta = b - a;
            if delta.len() <= 1e-12 { continue; }
            let mut cuts = vec![0.0, 1.0];
            for clip in clips {
                clip.edges(&mut |c, d| {
                    let edge = d - c;
                    let denominator = cross(delta, edge);
                    if denominator.abs() > 1e-12 {
                        let t = cross(c - a, edge) / denominator;
                        let u = cross(c - a, delta) / denominator;
                        if t > 0.0 && t < 1.0 && (-1e-10..=1.0 + 1e-10).contains(&u) { cuts.push(t); }
                    } else if cross(c - a, delta).abs() <= 1e-10 {
                        // Un tramo sobre el borde también debe cortarse en sus extremos.
                        for q in [c, d] {
                            let t = (q - a).dot(delta) / delta.dot(delta);
                            if t > 0.0 && t < 1.0 { cuts.push(t); }
                        }
                    }
                });
            }
            cuts.sort_by(f64::total_cmp);
            cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
            for interval in cuts.windows(2) {
                let (start, end) = (a.lerp(b, interval[0]), a.lerp(b, interval[1]));
                if clips.iter().all(|r| r.contains(start.lerp(end, 0.5))) {
                    if current.last().is_some_and(|p| p.dist(start) > 1e-8) { finish(&mut current, &mut out); }
                    if current.is_empty() { current.push(start); }
                    current.push(end);
                } else { finish(&mut current, &mut out); }
            }
        }
        finish(&mut current, &mut out);
    }
    out
}

fn finish(current: &mut Polyline, out: &mut Vec<Polyline>) {
    if current.len() >= 2 { out.push(std::mem::take(current)); }
    else { current.clear(); }
}
