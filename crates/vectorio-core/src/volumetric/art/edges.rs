//! Todas las aristas únicas de triangulación, agrupadas sin depender del winding.
//! Una red más abierta que la malla de calidad se apoya sobre la superficie fina.
use super::*;
use std::collections::BTreeMap;
pub(super) fn strokes(
    mesh: &Mesh,
    s: &ArtSettings,
    camera: &Camera,
    hybrid: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<MeshStroke>, String> {
    let points = mesh.points();
    let mut edges = BTreeMap::<(usize, usize), (V3, usize)>::new();
    for (i, t) in mesh.triangles.iter().enumerate() {
        if i % 256 == 0 && cancel() {
            return Err(CANCEL.into());
        }
        let normal = (points[t[1]] - points[t[0]])
            .cross(points[t[2]] - points[t[0]])
            .normalized();
        let n = V3::new(normal.x.abs(), normal.y.abs(), normal.z.abs());
        let owner = mesh.face_objects.get(i).copied().unwrap_or(0);
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let value = edges
                .entry((a.min(b), a.max(b)))
                .or_insert((V3::default(), owner));
            value.0 = value.0 + n;
            value.1 = value.1.min(owner);
        }
    }
    let count = s.mesh_inks;
    let metric = |q: V3| {
        if s.coloring == MeshColor::Depth {
            camera.project(q).1
        } else {
            q.z
        }
    };
    let (lo, hi) = points
        .iter()
        .map(|q| metric(*q))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(v), b.max(v))
        });
    let mut result = Vec::with_capacity(edges.len());
    for (i, ((a, b), (n, owner))) in edges.into_iter().enumerate() {
        if i % 256 == 0 && cancel() {
            return Err(CANCEL.into());
        }
        let class = match s.coloring {
            MeshColor::Orientation => {
                if count == 2 {
                    usize::from(n.z < n.x.max(n.y))
                } else if n.z >= n.x && n.z >= n.y {
                    0
                } else if n.y >= n.x {
                    1
                } else {
                    2
                }
            }
            MeshColor::Objects => owner % count,
            MeshColor::Height | MeshColor::Depth => (((metric((points[a] + points[b]) * 0.5) - lo)
                / (hi - lo).max(1e-12)
                * count as f64)
                .floor() as usize)
                .min(count - 1),
        };
        result.push(MeshStroke {
            points: vec![points[a], points[b]],
            role: if hybrid {
                1 + class % count
            } else {
                class % count
            },
        });
    }
    Ok(result)
}

/// Apoya cada arista recta de la red sobre la malla fina: muestras a la escala de sus
/// aristas, proyectadas al punto más cercano. Así la ocultación por la malla fina no
/// corta las cuerdas que atraviesan zonas convexas, y la red casa con las ondas.
pub(super) fn drape(
    strokes: &mut [MeshStroke],
    fine: &Mesh,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    let surface = Surface::new(fine);
    for (i, stroke) in strokes.iter_mut().enumerate() {
        if i % 128 == 0 && cancel() {
            return Err(CANCEL.into());
        }
        let (a, b) = (stroke.points[0], stroke.points[stroke.points.len() - 1]);
        let n = ((b - a).length() / surface.step).ceil().max(1.) as usize;
        let mut line = Vec::with_capacity(n + 1);
        line.push(a);
        for k in 1..n {
            let q = a + (b - a) * (k as f64 / n as f64);
            line.push(surface.closest(q).unwrap_or(q));
        }
        line.push(b);
        stroke.points = line;
    }
    Ok(())
}
/// Rejilla uniforme de triángulos para buscar el punto de superficie más cercano.
struct Surface {
    tris: Vec<[V3; 3]>,
    lo: V3,
    cell: f64,
    dims: [usize; 3],
    cells: Vec<Vec<u32>>,
    step: f64,
}
impl Surface {
    fn new(mesh: &Mesh) -> Self {
        let pts = mesh.points();
        let tris: Vec<[V3; 3]> = mesh.triangles.iter().map(|t| t.map(|i| pts[i])).collect();
        let total: f64 = tris
            .iter()
            .map(|t| (t[1] - t[0]).length() + (t[2] - t[1]).length() + (t[0] - t[2]).length())
            .sum();
        let step = (total / (3. * tris.len().max(1) as f64)).max(1e-9);
        let [lo, hi] = mesh.bounds();
        let cell = step * 2.;
        let dim = |a: f64, b: f64| (((b - a) / cell).floor() as usize + 1).min(512);
        let dims = [dim(lo.x, hi.x), dim(lo.y, hi.y), dim(lo.z, hi.z)];
        let mut grid = Self {
            tris: vec![],
            lo,
            cell,
            dims,
            cells: vec![vec![]; dims[0] * dims[1] * dims[2]],
            step,
        };
        for (i, t) in tris.iter().enumerate() {
            let min = grid.coords(V3::new(
                t[0].x.min(t[1].x).min(t[2].x),
                t[0].y.min(t[1].y).min(t[2].y),
                t[0].z.min(t[1].z).min(t[2].z),
            ));
            let max = grid.coords(V3::new(
                t[0].x.max(t[1].x).max(t[2].x),
                t[0].y.max(t[1].y).max(t[2].y),
                t[0].z.max(t[1].z).max(t[2].z),
            ));
            for x in min[0]..=max[0] {
                for y in min[1]..=max[1] {
                    for z in min[2]..=max[2] {
                        let c = grid.index([x, y, z]);
                        grid.cells[c].push(i as u32);
                    }
                }
            }
        }
        grid.tris = tris;
        grid
    }
    fn coords(&self, q: V3) -> [usize; 3] {
        let c = |v: f64, lo: f64, n: usize| {
            (((v - lo) / self.cell).floor().max(0.) as usize).min(n - 1)
        };
        [
            c(q.x, self.lo.x, self.dims[0]),
            c(q.y, self.lo.y, self.dims[1]),
            c(q.z, self.lo.z, self.dims[2]),
        ]
    }
    fn index(&self, [x, y, z]: [usize; 3]) -> usize {
        (z * self.dims[1] + y) * self.dims[0] + x
    }
    /// Busca en anillos crecientes de celdas hasta que ningún triángulo más lejano
    /// pueda mejorar el resultado.
    fn closest(&self, q: V3) -> Option<V3> {
        let c = self.coords(q);
        let mut best: Option<(f64, V3)> = None;
        let limit = self.dims.iter().copied().max().unwrap_or(1);
        for r in 0..limit {
            let range = |i: usize, n: usize| i.saturating_sub(r)..=(i + r).min(n - 1);
            for x in range(c[0], self.dims[0]) {
                for y in range(c[1], self.dims[1]) {
                    for z in range(c[2], self.dims[2]) {
                        let shell = [x.abs_diff(c[0]), y.abs_diff(c[1]), z.abs_diff(c[2])];
                        if shell.iter().copied().max() != Some(r) {
                            continue;
                        }
                        for &t in &self.cells[self.index([x, y, z])] {
                            let p = closest_on_triangle(q, &self.tris[t as usize]);
                            let d = (p - q).length();
                            if best.is_none_or(|(b, _)| d < b) {
                                best = Some((d, p));
                            }
                        }
                    }
                }
            }
            // Todo lo que queda fuera del anillo r está a más de r celdas.
            if best.is_some_and(|(d, _)| d <= r as f64 * self.cell) {
                break;
            }
        }
        best.map(|(_, p)| p)
    }
}
/// Punto más cercano de un triángulo (Ericson, «Real-Time Collision Detection»).
fn closest_on_triangle(p: V3, [a, b, c]: &[V3; 3]) -> V3 {
    let (ab, ac, ap) = (*b - *a, *c - *a, p - *a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0. && d2 <= 0. {
        return *a;
    }
    let bp = p - *b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0. && d4 <= d3 {
        return *b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0. && d1 >= 0. && d3 <= 0. {
        return *a + ab * (d1 / (d1 - d3));
    }
    let cp = p - *c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0. && d5 <= d6 {
        return *c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0. && d2 >= 0. && d6 <= 0. {
        return *a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0. && d4 - d3 >= 0. && d5 - d6 >= 0. {
        return *b + (*c - *b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1. / (va + vb + vc);
    *a + ab * (vb * denom) + ac * (vc * denom)
}
