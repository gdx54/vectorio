use std::collections::BTreeMap;
use super::{normal, Scene, V3};

impl Scene {
    /// Sólido convexo cerrado. Cada arista debe pertenecer a dos caras.
    /// Acepta giros mezclados; orienta las caras con el centroide y devuelve esas
    /// caras exteriores para poder tramarlas o clasificarlas por tono.
    /// `smoothing` está vacío (todo facetado) o contiene un grupo por cara.
    /// `quiet` oculta y arroja sombra, pero no añade aristas.
    pub fn solid(&mut self, vertices: &[V3], faces: &[Vec<usize>], smoothing: &[u16], quiet: bool) -> Vec<Vec<V3>> {
        self.build_solid(vertices, faces, smoothing, quiet, false)
    }
    /// Respeta el giro exterior de las caras. Admite sólidos no convexos cerrados.
    pub fn solid_oriented(&mut self, vertices: &[V3], faces: &[Vec<usize>], smoothing: &[u16], quiet: bool) -> Vec<Vec<V3>> {
        self.build_solid(vertices, faces, smoothing, quiet, true)
    }
    fn build_solid(&mut self, vertices: &[V3], faces: &[Vec<usize>], smoothing: &[u16], quiet: bool, keep_winding: bool) -> Vec<Vec<V3>> {
        if !self.enabled || vertices.len() < 4 || faces.len() < 4 || (!smoothing.is_empty() && smoothing.len() != faces.len()) ||
            !vertices.iter().all(|v| v.finite()) { return Vec::new(); }
        let center = vertices.iter().copied().fold(V3::default(), |sum, v| sum + v) * (1.0 / vertices.len() as f64);
        let mut oriented = Vec::with_capacity(faces.len());
        let mut front = Vec::with_capacity(faces.len());
        let mut edges: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (i, indices) in faces.iter().enumerate() {
            if indices.len() < 3 || !indices.iter().all(|&j| j < vertices.len()) { return Vec::new(); }
            let mut points: Vec<_> = indices.iter().map(|&j| vertices[j]).collect();
            let n = normal(&points);
            if !super::valid_face(&points) { return Vec::new(); }
            let outward = if !keep_winding && n.dot(points[0] - center) < 0.0 { points.reverse(); n * -1.0 } else { n };
            front.push(self.camera.facing(outward));
            for (&a, &b) in indices.iter().zip(indices.iter().cycle().skip(1)).take(indices.len()) {
                if a == b { return Vec::new(); }
                edges.entry((a.min(b), a.max(b))).or_default().push(i);
            }
            oriented.push(points);
        }
        // Validar antes de añadir nada impide dejar media geometría si falta una cara.
        if !edges.values().all(|neighbors| neighbors.len() == 2) { return Vec::new(); }
        for (i, points) in oriented.iter().enumerate() { if keep_winding || front[i] { self.occluder(points); } }
        self.collect_shadow(vertices);
        if !quiet {
            for ((a, b), neighbors) in edges {
                let (i, j) = (neighbors[0], neighbors[1]);
                let smooth = !smoothing.is_empty() && smoothing[i] > 0 && smoothing[i] == smoothing[j];
                if (front[i] || front[j]) && (!smooth || front[i] != front[j]) { self.line(&[vertices[a], vertices[b]]); }
            }
        }
        oriented
    }

    /// Caja alineada con los ejes; devuelve las seis caras exteriores.
    pub fn box3(&mut self, min: V3, max: V3) -> Vec<Vec<V3>> { self.rotated_box(min, max, 0.0) }

    /// Gira la caja alrededor de su centro, sobre la vertical, en grados.
    pub fn rotated_box(&mut self, min: V3, max: V3, yaw_deg: f64) -> Vec<Vec<V3>> {
        if !min.finite() || !max.finite() || min.x >= max.x || min.y >= max.y || min.z >= max.z || !yaw_deg.is_finite() { return Vec::new(); }
        let center = (min + max) * 0.5;
        let (s, c) = yaw_deg.to_radians().sin_cos();
        let vertices: Vec<_> = [
            V3::new(min.x, min.y, min.z), V3::new(max.x, min.y, min.z),
            V3::new(max.x, max.y, min.z), V3::new(min.x, max.y, min.z),
            V3::new(min.x, min.y, max.z), V3::new(max.x, min.y, max.z),
            V3::new(max.x, max.y, max.z), V3::new(min.x, max.y, max.z),
        ].into_iter().map(|v| {
            let q = v - center;
            center + V3::new(q.x * c - q.y * s, q.x * s + q.y * c, q.z)
        }).collect();
        let faces = vec![vec![0, 3, 2, 1], vec![4, 5, 6, 7], vec![0, 1, 5, 4],
            vec![1, 2, 6, 5], vec![2, 3, 7, 6], vec![3, 0, 4, 7]];
        self.solid(&vertices, &faces, &[], self.quiet)
    }

    /// Prisma de base plana convexa. El vector no puede ser tangente a la base.
    pub fn extrude(&mut self, polygon: &[V3], vector: V3) -> Vec<Vec<V3>> {
        if !super::valid_face(polygon) || !vector.finite() || normal(polygon).dot(vector).abs() <= 1e-12 { return Vec::new(); }
        let n = polygon.len();
        let vertices: Vec<_> = polygon.iter().copied().chain(polygon.iter().map(|v| *v + vector)).collect();
        let mut faces: Vec<Vec<usize>> = vec![(0..n).collect(), (n..2 * n).collect()];
        for i in 0..n { let j = (i + 1) % n; faces.push(vec![i, j, j + n, i + n]); }
        self.solid(&vertices, &faces, &[], self.quiet)
    }

    /// Revolución vertical de un perfil que puede tener cuellos (radio, altura relativa al centro).
    /// Las alturas crecen estrictamente; los radios nulos son puntas únicas, solo
    /// en los extremos. Cierra automáticamente las tapas de radio distinto de cero.
    pub fn lathe(&mut self, center: V3, profile: &[(f64, f64)], smooth: bool) -> Vec<Vec<V3>> {
        if !center.finite() || profile.len() < 2 || profile.len() > 4096 ||
            !profile.iter().all(|&(r, h)| r.is_finite() && h.is_finite() && r >= 0.0) ||
            !profile.windows(2).all(|w| w[0].1 < w[1].1) ||
            !profile[1..profile.len() - 1].iter().all(|v| v.0 > 0.0) { return Vec::new(); }
        let radius = profile.iter().map(|v| v.0).fold(0.0, f64::max);
        if radius <= 0.0 { return Vec::new(); }
        let segments = self.circle_segments(radius);
        if profile.len().saturating_mul(segments) > 200_000 { return Vec::new(); }
        let mut vertices = Vec::new();
        let mut rings = Vec::new();
        for &(r, h) in profile {
            let mut ring = Vec::new();
            for i in 0..if r == 0.0 { 1 } else { segments } {
                let (s, c) = (std::f64::consts::TAU * i as f64 / segments as f64).sin_cos();
                ring.push(vertices.len());
                vertices.push(center + V3::new(r * c, r * s, h));
            }
            rings.push(ring);
        }
        let mut faces = Vec::new();
        let mut groups = Vec::new();
        for pair in rings.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            for i in 0..segments {
                let j = (i + 1) % segments;
                faces.push(if a.len() == 1 { vec![a[0], b[j], b[i]] }
                    else if b.len() == 1 { vec![a[i], a[j], b[0]] }
                    else { vec![a[i], a[j], b[j], b[i]] });
                groups.push(u16::from(smooth));
            }
        }
        for (i, ring) in [&rings[0], &rings[rings.len() - 1]].into_iter().enumerate() {
            if ring.len() > 1 {
                let mut cap = ring.clone(); if i == 0 { cap.reverse(); }
                faces.push(cap); groups.push(0);
            }
        }
        self.solid_oriented(&vertices, &faces, &groups, self.quiet)
    }
}
