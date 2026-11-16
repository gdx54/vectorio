//! Distancias sobre el grafo de aristas: aproximación geodésica, por componente.
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
        let mut visits = 0;
        while let Some(Node(d, a)) = heap.pop() {
            visits += 1;
            if visits % 256 == 0 && cancel() {
                return Err(CANCEL.into());
            }
            if d > values[a] {
                continue;
            }
            for &(b, w) in &self.adjacency[a] {
                let next = d + w;
                if next < values[b] {
                    values[b] = next;
                    heap.push(Node(next, b));
                }
            }
        }
        Ok(values)
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
