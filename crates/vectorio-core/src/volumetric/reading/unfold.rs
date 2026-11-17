//! Bosque de desarrollos isométricos: corta una unión cuando produciría solape.
use super::*;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
/// Unión candidata: primero la más plana y, a igualdad, la más antigua.
#[derive(PartialEq)]
struct Join {
    flatness: f64,
    order: usize,
    face: usize,
    edge: usize,
    other: usize,
}
impl Eq for Join {}
impl PartialOrd for Join {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Join {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.flatness
            .total_cmp(&other.flatness)
            .then(self.order.cmp(&other.order))
    }
}
/// Envolvente convexa (cadena monótona) en sentido antihorario.
fn hull(mut points: Vec<P>) -> Vec<P> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| a.dist(*b) < 1e-12);
    if points.len() < 3 {
        return points;
    }
    let mut out: Vec<P> = Vec::with_capacity(points.len() * 2);
    for pass in 0..2 {
        let start = out.len();
        let iter: Box<dyn Iterator<Item = &P>> = if pass == 0 {
            Box::new(points.iter())
        } else {
            Box::new(points.iter().rev())
        };
        for &q in iter {
            while out.len() >= start + 2
                && cross(out[out.len() - 1] - out[out.len() - 2], q - out[out.len() - 2]) <= 0.
            {
                out.pop();
            }
            out.push(q);
        }
        out.pop();
    }
    out
}
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
    let area_of = |t: [usize; 3]| {
        (points[t[1]] - points[t[0]])
            .cross(points[t[2]] - points[t[0]])
            .length()
    };
    // Las islas crecen como un árbol de Prim: siempre por la unión más plana de todo el
    // borde, no siguiendo una sola cadena. Salen piezas compactas y con menos cortes.
    let mut seeds: Vec<usize> = (0..mesh.triangles.len()).collect();
    seeds.sort_by(|a, b| {
        area_of(mesh.triangles[*b])
            .total_cmp(&area_of(mesh.triangles[*a]))
            .then(a.cmp(b))
    });
    let mut placed: Vec<Option<(usize, [P; 3])>> = vec![None; mesh.triangles.len()];
    let mut islands: Vec<Vec<usize>> = vec![];
    let mut frontier = BinaryHeap::new();
    let push = |frontier: &mut BinaryHeap<Join>, face: usize, order: &mut usize| {
        let t = mesh.triangles[face];
        for j in 0..3 {
            let (a, b) = (t[j], t[(j + 1) % 3]);
            let owners = &edges[&(a.min(b), a.max(b))];
            if owners.len() != 2 {
                continue;
            }
            let other = if owners[0] == face { owners[1] } else { owners[0] };
            *order += 1;
            frontier.push(Join {
                flatness: normals[face].dot(normals[other]),
                order: usize::MAX - *order,
                face,
                edge: j,
                other,
            });
        }
    };
    let mut order = 0;
    for seed in seeds {
        if placed[seed].is_some() {
            continue;
        }
        let island = islands.len();
        islands.push(vec![seed]);
        placed[seed] = Some((island, initial(mesh.triangles[seed].map(|i| points[i]))));
        frontier.clear();
        push(&mut frontier, seed, &mut order);
        while let Some(Join { face, edge: j, other, .. }) = frontier.pop() {
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            if placed[other].is_some() {
                continue;
            }
            let t = mesh.triangles[face];
            let (_, xy) = placed[face].unwrap();
            let (a, b) = (t[j], t[(j + 1) % 3]);
            let other_t = mesh.triangles[other];
            let c = *other_t.iter().find(|i| **i != a && **i != b).unwrap();
            let start = xy[j];
            let end = xy[(j + 1) % 3];
            let direction = end - start;
            let len = direction.len();
            if len < 1e-12 {
                continue;
            }
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
            push(&mut frontier, other, &mut order);
        }
    }
    // Una arista queda unida si sus dos caras coinciden sobre el papel en la misma isla.
    let mut joined = BTreeSet::new();
    for faces in edges.values() {
        if let [f, g] = faces[..] {
            let ((fi, fxy), (gi, gxy)) = (placed[f].unwrap(), placed[g].unwrap());
            if fi != gi {
                continue;
            }
            let (ft, gt) = (mesh.triangles[f], mesh.triangles[g]);
            let shared = ft.iter().filter(|v| gt.contains(v));
            if shared.clone().all(|v| {
                let at = |t: [usize; 3], xy: [P; 3]| xy[t.iter().position(|i| i == v).unwrap()];
                at(ft, fxy).dist(at(gt, gxy)) < 1e-7
            }) {
                joined.insert((f.min(g), f.max(g)));
            }
        }
    }
    // Cada isla se gira a su caja mínima y en apaisado: aprovecha el papel en filas.
    for faces in &islands {
        let hull = hull(faces.iter().flat_map(|i| placed[*i].unwrap().1).collect());
        let size = |angle: f64| {
            let mut b = BBox::empty();
            for q in &hull {
                b.add(q.rotate(angle));
            }
            (b.width(), b.height())
        };
        let mut best = (f64::INFINITY, 0.);
        for k in 0..hull.len() {
            let edge = hull[(k + 1) % hull.len()] - hull[k];
            let angle = -edge.y.atan2(edge.x);
            let (w, h) = size(angle);
            if w * h < best.0 - 1e-12 {
                best = (w * h, if h > w { angle + std::f64::consts::FRAC_PI_2 } else { angle });
            }
        }
        for i in faces {
            let entry = placed[*i].as_mut().unwrap();
            entry.1 = entry.1.map(|q| q.rotate(best.1));
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
    let mut tallest: Vec<usize> = (0..bounds.len()).collect();
    tallest.sort_by(|a, b| bounds[*b].height().total_cmp(&bounds[*a].height()).then(a.cmp(b)));
    let area = effects::area(paper);
    let pack = |scale: f64| -> Option<Vec<P>> {
        let mut positions = vec![p(0., 0.); bounds.len()];
        let (mut x, mut y, mut row) = (area.min.x, area.min.y, 0f64);
        for &island in &tallest {
            let b = &bounds[island];
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
            positions[island] = p(x + tabs + 1., y + tabs + 1.) - b.min * scale;
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
