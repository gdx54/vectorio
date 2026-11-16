//! Bosque de desarrollos isométricos: corta una unión cuando produciría solape.
use super::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
fn cross(a: P, b: P) -> f64 {
    a.x * b.y - a.y * b.x
}
fn overlaps(a: [P; 3], b: [P; 3]) -> bool {
    let inside = |p: P, t: [P; 3]| {
        let sign = cross(t[1] - t[0], t[2] - t[0]).signum();
        (0..3).all(|i| cross(t[(i + 1) % 3] - t[i], p - t[i]) * sign > 1e-9)
    };
    if inside((a[0] + a[1] + a[2]) * (1. / 3.), b)
        || inside((b[0] + b[1] + b[2]) * (1. / 3.), a)
        || a.iter().any(|p| inside(*p, b))
        || b.iter().any(|p| inside(*p, a))
    {
        return true;
    }
    for i in 0..3 {
        for j in 0..3 {
            let p = a[i];
            let q = a[(i + 1) % 3];
            let r = b[j];
            let s = b[(j + 1) % 3];
            if cross(q - p, r - p) * cross(q - p, s - p) < -1e-12
                && cross(s - r, p - r) * cross(s - r, q - r) < -1e-12
            {
                return true;
            }
        }
    }
    false
}
fn initial(t: [V3; 3]) -> [P; 3] {
    let len = (t[1] - t[0]).length();
    let x = (t[2] - t[0]).dot(t[1] - t[0]) / len;
    let y = ((t[2] - t[0]).length().powi(2) - x * x).max(0.).sqrt();
    [p(0., 0.), p(len, 0.), p(x, y)]
}
pub(super) fn draw(
    d: &Drawing,
    config: &ReadingMap,
    tabs: f64,
    paper: &crate::Paper,
    cancel: &dyn Fn() -> bool,
) -> Result<(Vec<Vec<Polyline>>, f64), String> {
    let mesh = &d.mesh;
    if mesh.triangles.len() > 1024 {
        return Err("El desarrollo supera 1.024 caras; reduce las facetas.".into());
    }
    let points = mesh.points();
    let normals = super::super::shadow::normals(mesh, cancel)?;
    let mut edges = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (i, t) in mesh.triangles.iter().enumerate() {
        for j in 0..3 {
            let (a, b) = (t[j], t[(j + 1) % 3]);
            edges.entry((a.min(b), a.max(b))).or_default().push(i);
        }
    }
    if edges.values().any(|faces| faces.len() > 2) {
        return Err("El desarrollo requiere una malla manifold; revisa las aristas compartidas por más de dos caras.".into());
    }
    let mut placed = vec![None; mesh.triangles.len()];
    let mut islands: Vec<Vec<usize>> = vec![];
    let mut joined = BTreeSet::new();
    for seed in 0..mesh.triangles.len() {
        if placed[seed].is_some() {
            continue;
        }
        let island = islands.len();
        islands.push(vec![seed]);
        placed[seed] = Some((island, initial(mesh.triangles[seed].map(|i| points[i]))));
        let mut queue = VecDeque::from([seed]);
        while let Some(face) = queue.pop_front() {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let t = mesh.triangles[face];
            let (_, xy) = placed[face].unwrap();
            let mut order = [0, 1, 2];
            order.sort_by(|a, b| {
                let score = |j: usize| {
                    let (a, b) = (t[j], t[(j + 1) % 3]);
                    edges[&(a.min(b), a.max(b))]
                        .iter()
                        .filter(|i| **i != face)
                        .map(|i| normals[face].dot(normals[*i]))
                        .fold(-2f64, f64::max)
                };
                score(*b).total_cmp(&score(*a))
            });
            for j in order {
                let (a, b) = (t[j], t[(j + 1) % 3]);
                let pair = (a.min(b), a.max(b));
                let owners = &edges[&pair];
                if owners.len() != 2 {
                    continue;
                }
                let other = if owners[0] == face {
                    owners[1]
                } else {
                    owners[0]
                };
                if let Some((other_island, other_xy)) = placed[other] {
                    if other_island == island {
                        let other_t = mesh.triangles[other];
                        let oa = other_xy[other_t.iter().position(|v| *v == a).unwrap()];
                        let ob = other_xy[other_t.iter().position(|v| *v == b).unwrap()];
                        if oa.dist(xy[j]) < 1e-7 && ob.dist(xy[(j + 1) % 3]) < 1e-7 {
                            joined.insert((face.min(other), face.max(other)));
                        }
                    }
                    continue;
                }
                let other_t = mesh.triangles[other];
                let c = *other_t.iter().find(|i| **i != a && **i != b).unwrap();
                let start = xy[j];
                let end = xy[(j + 1) % 3];
                let direction = end - start;
                let len = direction.len();
                let unit = direction * (1. / len);
                let ca = (points[c] - points[a]).length();
                let cb = (points[c] - points[b]).length();
                let x = (ca * ca + len * len - cb * cb) / (2. * len);
                let y = (ca * ca - x * x).max(0.).sqrt();
                let sign = -cross(direction, xy[(j + 2) % 3] - start).signum();
                let third = start + unit * x + p(-unit.y, unit.x) * (y * sign);
                let candidate = other_t.map(|v| {
                    if v == a {
                        start
                    } else if v == b {
                        end
                    } else {
                        third
                    }
                });
                if islands[island]
                    .iter()
                    .any(|i| overlaps(candidate, placed[*i].unwrap().1))
                {
                    continue;
                }
                placed[other] = Some((island, candidate));
                islands[island].push(other);
                joined.insert((face.min(other), face.max(other)));
                queue.push_front(face);
                queue.push_front(other);
                break;
            }
        }
    }
    let bounds = islands
        .iter()
        .map(|faces| {
            let mut b = BBox::empty();
            for i in faces {
                for p in placed[*i].unwrap().1 {
                    b.add(p);
                }
            }
            b
        })
        .collect::<Vec<_>>();
    let area = effects::area(paper);
    let pack = |scale: f64| -> Option<Vec<P>> {
        let mut positions = Vec::new();
        let (mut x, mut y, mut row) = (area.min.x, area.min.y, 0f64);
        for b in &bounds {
            let w = b.width() * scale + tabs * 2. + 2.;
            let h = b.height() * scale + tabs * 2. + 2.;
            if w > area.width() || h > area.height() {
                return None;
            }
            if x + w > area.max.x {
                x = area.min.x;
                y += row;
                row = 0.;
            }
            if y + h > area.max.y {
                return None;
            }
            positions.push(p(x + tabs + 1., y + tabs + 1.) - b.min * scale);
            x += w;
            row = row.max(h);
        }
        Some(positions)
    };
    let (mut lo, mut hi) = (0.01, 500.);
    if pack(lo).is_none() {
        return Err("El desarrollo no cabe con estas pestañas; reduce las facetas o el tamaño de las pestañas.".into());
    }
    for _ in 0..28 {
        let mid = (lo + hi) * 0.5;
        if pack(mid).is_some() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let positions = pack(lo).unwrap();
    let mut out = vec![vec![]; config.pens.len()];
    let mut label = 0;
    for ((a, b), faces) in edges {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let connected =
            faces.len() == 2 && joined.contains(&(faces[0].min(faces[1]), faces[0].max(faces[1])));
        for (owner, &face) in faces.iter().enumerate() {
            if connected && owner > 0 {
                continue;
            }
            let (island, xy) = placed[face].unwrap();
            let t = mesh.triangles[face];
            let local = |v| xy[t.iter().position(|i| *i == v).unwrap()] * lo + positions[island];
            let start = local(a);
            let end = local(b);
            if connected {
                if normals[faces[0]].dot(normals[faces[1]]) > 1. - 1e-8 {
                    continue;
                }
                let mut axis = (points[b] - points[a]).normalized();
                let oriented = t.windows(2).any(|w| w == [a, b]) || (t[2] == a && t[0] == b);
                if !oriented {
                    axis = axis * (-1.);
                }
                let raw = (points[t[1]] - points[t[0]]).cross(points[t[2]] - points[t[0]]);
                if raw.dot(normals[face]) < 0. {
                    axis = axis * (-1.);
                }
                let sign = normals[faces[0]].cross(normals[faces[1]]).dot(axis);
                let slot = if sign >= 0. { 1 } else { 2 };
                out[slot % config.pens.len()].push(vec![start, end]);
            } else {
                let center = (start + end) * 0.5;
                let third = local(*t.iter().find(|i| **i != a && **i != b).unwrap());
                let edge = end - start;
                let unit = edge * (1. / edge.len());
                let outside = p(-unit.y, unit.x) * (-cross(edge, third - start).signum());
                if faces.len() == 2 && owner == 0 && edge.len() > tabs * 1.5 {
                    let shift = outside * tabs;
                    let inset = unit * tabs.min(edge.len() * 0.2);
                    out[0].push(vec![start, start + shift + inset, end + shift - inset, end]);
                    out[1 % config.pens.len()].push(vec![start, end]);
                } else {
                    out[0].push(vec![start, end]);
                }
                if faces.len() == 2 {
                    if owner == 0 {
                        label += 1;
                    }
                    let inward = outside * (-1.);
                    legend::text(
                        &mut out[0],
                        &format!("{}", label),
                        center + inward * 1.2,
                        1.2,
                        8.,
                    );
                }
            }
        }
    }
    for (slot, lines) in out.iter_mut().enumerate() {
        if slot > 0 {
            *lines = super::super::shadow::tone::dashed(
                lines,
                DashPattern {
                    dash_mm: 1.5,
                    gap_mm: 0.6,
                },
                cancel,
            )?;
        }
        *lines = encode::clip(lines, area);
    }
    Ok((out, lo))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coincident_triangles_overlap_but_a_shared_fold_does_not() {
        let a = [p(0., 0.), p(2., 0.), p(0., 2.)];
        assert!(overlaps(a, a));
        assert!(!overlaps(a, [p(0., 0.), p(2., 0.), p(0., -2.)]));
    }
}
