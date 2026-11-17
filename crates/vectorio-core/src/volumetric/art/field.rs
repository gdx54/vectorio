//! Distancias geodésicas por componente: Dijkstra sobre aristas, refinado dentro de cada
//! triángulo con el desplegado plano (como Fast Marching) para que los frentes no zigzagueen.
use super::*;
use std::{cmp::Ordering, collections::BinaryHeap};
#[derive(Clone, Copy)]
struct Node(f64, usize);
impl PartialEq for Node {
    fn eq(&self, b: &Self) -> bool {
        self.0 == b.0 && self.1 == b.1
    }
}
impl Eq for Node {}
impl PartialOrd for Node {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
impl Ord for Node {
    fn cmp(&self, b: &Self) -> Ordering {
        b.0.total_cmp(&self.0).then_with(|| b.1.cmp(&self.1))
    }
}
pub(crate) struct Graph {
    points: Vec<V3>,
    adjacency: Vec<Vec<(usize, f64)>>,
    faces: Vec<[usize; 3]>,
    incident: Vec<Vec<usize>>,
    pub(super) components: Vec<Vec<usize>>,
}
impl Graph {
    pub fn new(mesh: &Mesh, cancel: &dyn Fn() -> bool) -> Result<Self, String> {
        let points = mesh.points();
        let mut edges = std::collections::BTreeSet::new();
        for (i, t) in mesh.triangles.iter().enumerate() {
            if i % 256 == 0 && cancel() {
                return Err(CANCEL.into());
            }
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                edges.insert((a.min(b), a.max(b)));
            }
        }
        let mut faces = Vec::with_capacity(mesh.triangles.len());
        let mut incident = vec![vec![]; points.len()];
        for t in &mesh.triangles {
            let [a, b, c] = t.map(|i| points[i]);
            if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] || (b - a).cross(c - a).length() < 1e-14 {
                continue;
            }
            for v in *t {
                incident[v].push(faces.len());
            }
            faces.push(*t);
        }
        let mut adjacency = vec![vec![]; points.len()];
        for (a, b) in edges {
            let length = (points[a] - points[b]).length();
            adjacency[a].push((b, length));
            adjacency[b].push((a, length));
        }
        let mut seen = vec![false; points.len()];
        let mut components = Vec::new();
        for first in 0..points.len() {
            if seen[first] {
                continue;
            }
            let mut component = vec![first];
            seen[first] = true;
            let mut at = 0;
            while at < component.len() {
                if at % 256 == 0 && cancel() {
                    return Err(CANCEL.into());
                }
                let a = component[at];
                at += 1;
                for &(b, _) in &adjacency[a] {
                    if !seen[b] {
                        seen[b] = true;
                        component.push(b);
                    }
                }
            }
            components.push(component);
        }
        Ok(Self {
            points,
            adjacency,
            faces,
            incident,
            components,
        })
    }
    pub(crate) fn distance(
        &self,
        direction: V3,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<f64>, String> {
        let mut values = vec![f64::INFINITY; self.points.len()];
        let mut heap = BinaryHeap::new();
        for component in &self.components {
            let seed = *component
                .iter()
                .max_by(|a, b| {
                    self.points[**a]
                        .dot(direction)
                        .total_cmp(&self.points[**b].dot(direction))
                        .then_with(|| b.cmp(a))
                })
                .unwrap();
            values[seed] = 0.;
            heap.push(Node(0., seed));
        }
        let mut done = vec![false; self.points.len()];
        let mut visits = 0;
        while let Some(Node(d, a)) = heap.pop() {
            visits += 1;
            if visits % 256 == 0 && cancel() {
                return Err(CANCEL.into());
            }
            if d > values[a] || done[a] {
                continue;
            }
            done[a] = true;
            fn relax(values: &mut [f64], heap: &mut BinaryHeap<Node>, b: usize, next: f64) {
                if next < values[b] {
                    values[b] = next;
                    heap.push(Node(next, b));
                }
            }
            for &(b, w) in &self.adjacency[a] {
                if !done[b] {
                    relax(&mut values, &mut heap, b, d + w);
                }
            }
            // Con dos vértices ya fijados, el tercero se alcanza en línea recta a través
            // del triángulo desplegado, no por el camino quebrado de sus aristas.
            for &f in &self.incident[a] {
                let t = self.faces[f];
                for &c in &t {
                    if c == a || !done[c] {
                        continue;
                    }
                    let Some(&b) = t.iter().find(|v| **v != a && **v != c) else {
                        continue;
                    };
                    if done[b] {
                        continue;
                    }
                    let (p, q) = (a.min(c), a.max(c));
                    if let Some(next) = self.unfold(p, values[p], q, values[q], b) {
                        relax(&mut values, &mut heap, b, next);
                    }
                }
            }
        }
        Ok(values)
    }
    /// Fuente virtual a distancia `dp` de `p` y `dq` de `q`, al otro lado de la arista;
    /// solo vale si el rayo hasta `b` cruza la arista dentro del triángulo.
    fn unfold(&self, p: usize, dp: f64, q: usize, dq: f64, b: usize) -> Option<f64> {
        let (pp, pq, pb) = (self.points[p], self.points[q], self.points[b]);
        let axis = pq - pp;
        let length = axis.length();
        if length < 1e-14 {
            return None;
        }
        let x_axis = axis * (1. / length);
        let bx = (pb - pp).dot(x_axis);
        let by = ((pb - pp) - x_axis * bx).length();
        if by < 1e-14 {
            return None;
        }
        let sx = (dp * dp - dq * dq + length * length) / (2. * length);
        let sy2 = dp * dp - sx * sx;
        if sy2 <= 0. {
            return None;
        }
        let sy = -sy2.sqrt();
        let t = -sy / (by - sy);
        let cross = sx + t * (bx - sx);
        if !(0. ..=length).contains(&cross) {
            return None;
        }
        Some(((bx - sx).powi(2) + (by - sy).powi(2)).sqrt())
    }
    pub fn values(
        &self,
        s: &ArtSettings,
        seed: u64,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<f64>, String> {
        let (sy, cy) = s.origin_azimuth.to_radians().sin_cos();
        let (se, ce) = s.origin_elevation.to_radians().sin_cos();
        let direction = V3::new(cy * ce, sy * ce, se);
        let mut distance = self.distance(direction, cancel)?;
        if s.pattern == WavePattern::Interference {
            let other = self.distance(direction * -1., cancel)?;
            for (a, b) in distance.iter_mut().zip(other) {
                *a -= b;
            }
        }
        // Armónicos espaciales coherentes: nunca desplazan los puntos fuera de la superficie.
        let phase = |i: u64| {
            let mut x = seed.wrapping_add(i.wrapping_mul(0x9e3779b97f4a7c15));
            x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
            ((x ^ (x >> 31)) >> 11) as f64 / ((1u64 << 53) as f64) * std::f64::consts::TAU
        };
        let phases = [phase(1), phase(2), phase(3)];
        for (i, (q, value)) in self.points.iter().zip(&mut distance).enumerate() {
            if i % 256 == 0 && cancel() {
                return Err(CANCEL.into());
            }
            let f = s.frequency;
            *value += s.deformation
                * ((q.x * f + 0.8 * (q.z * f * 0.8 + phases[0]).sin() + phases[1]).sin()
                    * (q.y * f * 0.8 - q.z * f * 0.4 + phases[2]).cos());
        }
        Ok(distance)
    }
}
