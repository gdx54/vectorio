//! BVH de triángulos para oclusión hemisférica y espesor, independiente de cámara.
use super::*;
#[derive(Clone, Copy)]
struct Bounds {
    min: V3,
    max: V3,
}
impl Bounds {
    fn empty() -> Self {
        Self {
            min: V3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
            max: V3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        }
    }
    fn add(&mut self, p: V3) {
        self.min = V3::new(
            self.min.x.min(p.x),
            self.min.y.min(p.y),
            self.min.z.min(p.z),
        );
        self.max = V3::new(
            self.max.x.max(p.x),
            self.max.y.max(p.y),
            self.max.z.max(p.z),
        );
    }
    fn hit(&self, origin: V3, dir: V3, limit: f64) -> bool {
        let (mut lo, mut hi) = (0f64, limit);
        for (o, d, a, b) in [
            (origin.x, dir.x, self.min.x, self.max.x),
            (origin.y, dir.y, self.min.y, self.max.y),
            (origin.z, dir.z, self.min.z, self.max.z),
        ] {
            if d.abs() < 1e-14 {
                if o < a || o > b {
                    return false;
                }
            } else {
                let x = (a - o) / d;
                let y = (b - o) / d;
                lo = lo.max(x.min(y));
                hi = hi.min(x.max(y));
                if hi < lo {
                    return false;
                }
            }
        }
        true
    }
}
struct Node {
    bounds: Bounds,
    start: usize,
    end: usize,
    children: Option<(usize, usize)>,
}
pub(super) struct Bvh {
    triangles: Vec<[V3; 3]>,
    ids: Vec<usize>,
    nodes: Vec<Node>,
}
impl Bvh {
    pub fn new(mesh: &Mesh, cancel: &dyn Fn() -> bool) -> Result<Self, String> {
        let p = mesh.points();
        let triangles = mesh
            .triangles
            .iter()
            .map(|t| t.map(|i| p[i]))
            .collect::<Vec<_>>();
        let ids = (0..triangles.len()).collect();
        let mut tree = Self {
            triangles,
            ids,
            nodes: vec![],
        };
        if !tree.ids.is_empty() {
            let centers: Vec<_> = tree.triangles.iter().map(|t| t[0] + t[1] + t[2]).collect();
            tree.build(0, tree.ids.len(), &centers, cancel)?;
        }
        Ok(tree)
    }
    fn build(
        &mut self,
        start: usize,
        end: usize,
        centers: &[V3],
        cancel: &dyn Fn() -> bool,
    ) -> Result<usize, String> {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let mut bounds = Bounds::empty();
        for &i in &self.ids[start..end] {
            for p in self.triangles[i] {
                bounds.add(p);
            }
        }
        let id = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            start,
            end,
            children: None,
        });
        if end - start > 8 {
            let size = bounds.max - bounds.min;
            let axis = if size.x >= size.y.max(size.z) {
                0
            } else if size.y >= size.z {
                1
            } else {
                2
            };
            let coord = |v: V3| [v.x, v.y, v.z][axis];
            let mid = (start + end) / 2;
            self.ids[start..end].select_nth_unstable_by(mid - start, |a, b| {
                coord(centers[*a])
                    .total_cmp(&coord(centers[*b]))
                    .then(a.cmp(b))
            });
            let a = self.build(start, mid, centers, cancel)?;
            let b = self.build(mid, end, centers, cancel)?;
            self.nodes[id].children = Some((a, b));
        }
        Ok(id)
    }
    pub fn nearest(&self, origin: V3, dir: V3, skip: &[usize], limit: f64) -> Option<f64> {
        if self.nodes.is_empty() {
            return None;
        }
        // El árbol parte por la mediana: cinco millones de caras no alcanzan
        // 24 niveles. Evita una asignación por cada rayo de AO/espesor.
        let mut stack = [0usize; 64];
        let mut top = 1;
        let mut nearest = limit;
        let mut found = false;
        while top > 0 {
            top -= 1;
            let id = stack[top];
            let node = &self.nodes[id];
            if !node.bounds.hit(origin, dir, nearest) {
                continue;
            }
            if let Some((a, b)) = node.children {
                stack[top] = a;
                stack[top + 1] = b;
                top += 2;
            } else {
                for &i in &self.ids[node.start..node.end] {
                    if skip.contains(&i) {
                        continue;
                    }
                    if let Some(t) = hit(origin, dir, self.triangles[i]) {
                        if t < nearest {
                            nearest = t;
                            found = true;
                        }
                    }
                }
            }
        }
        found.then_some(nearest)
    }
}
fn hit(o: V3, d: V3, [a, b, c]: [V3; 3]) -> Option<f64> {
    let e = b - a;
    let f = c - a;
    let h = d.cross(f);
    let det = e.dot(h);
    if det.abs() < 1e-13 {
        return None;
    }
    let s = o - a;
    let u = s.dot(h) / det;
    if !(-1e-10..=1. + 1e-10).contains(&u) {
        return None;
    }
    let q = s.cross(e);
    let v = d.dot(q) / det;
    if v < -1e-10 || u + v > 1. + 1e-10 {
        return None;
    }
    let t = f.dot(q) / det;
    (t > 1e-7).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn median_bvh_agrees_with_exhaustive_intersections_and_skip_limits() {
        let mesh = Primitive::Sphere.mesh();
        let bvh = Bvh::new(&mesh, &|| false).unwrap();
        let points = mesh.points();
        for i in 0..200 {
            let angle = i as f64 * 2.399963229728653;
            let origin = V3::new(
                angle.cos() * 3.,
                angle.sin() * 3.,
                (i as f64 * 0.1).sin() * 2.,
            );
            let dir = (V3::new(0.2 * (i as f64).sin(), 0., 0.) - origin).normalized();
            for limit in [1., 5., 10.] {
                let skip = [i % mesh.triangles.len(), (i + 1) % mesh.triangles.len()];
                let brute = mesh
                    .triangles
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| !skip.contains(j))
                    .filter_map(|(_, t)| hit(origin, dir, t.map(|j| points[j])))
                    .filter(|t| *t < limit)
                    .min_by(f64::total_cmp);
                assert_eq!(brute, bvh.nearest(origin, dir, &skip, limit));
            }
        }
        assert!(Bvh::new(&mesh, &|| true).is_err());
    }
}
