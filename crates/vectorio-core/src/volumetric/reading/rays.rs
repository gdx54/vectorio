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
            tree.build(0, tree.ids.len(), cancel)?;
        }
        Ok(tree)
    }
    fn build(
        &mut self,
        start: usize,
        end: usize,
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
            self.ids[start..end].sort_unstable_by(|a, b| {
                let c = |i: usize| self.triangles[i].iter().map(|v| coord(*v)).sum::<f64>();
                c(*a).total_cmp(&c(*b)).then(a.cmp(b))
            });
            let mid = (start + end) / 2;
            let a = self.build(start, mid, cancel)?;
            let b = self.build(mid, end, cancel)?;
            self.nodes[id].children = Some((a, b));
        }
        Ok(id)
    }
    pub fn nearest(&self, origin: V3, dir: V3, skip: &[usize], limit: f64) -> Option<f64> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut stack = vec![0];
        let mut nearest = limit;
        let mut found = false;
        while let Some(id) = stack.pop() {
            let node = &self.nodes[id];
            if !node.bounds.hit(origin, dir, nearest) {
                continue;
            }
            if let Some((a, b)) = node.children {
                stack.extend([a, b]);
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
