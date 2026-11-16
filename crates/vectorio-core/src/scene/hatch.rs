use super::{normal, Scene, V3};
use super::hidden::{merge_intervals, Face, Vertex};
use crate::geom::{p, BBox, P};

/// Trama global en la hoja: el origen no depende del polígono ni del grupo.
/// Por eso dos sombras vecinas tienen exactamente la misma fase.
pub(super) fn hatch_polygons(polygons: &[Face], paper: BBox, angle_deg: f64, spacing: f64, inset: f64) -> Vec<[P; 2]> {
    if polygons.is_empty() || !angle_deg.is_finite() || !spacing.is_finite() || spacing <= 0.0 { return Vec::new(); }
    let (s, c) = angle_deg.to_radians().sin_cos();
    let direction = p(c, s);
    let normal = p(-s, c);
    let corners = [paper.min, p(paper.max.x, paper.min.y), paper.max, p(paper.min.x, paper.max.y)];
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for face in polygons { for q in &face.points { low = low.min(q.dot(normal)); high = high.max(q.dot(normal)); } }
    let paper_low = corners.iter().map(|q| q.dot(normal)).fold(f64::INFINITY, f64::min);
    let paper_high = corners.iter().map(|q| q.dot(normal)).fold(f64::NEG_INFINITY, f64::max);
    low = low.max(paper_low); high = high.min(paper_high);
    let along_low = corners.iter().map(|q| q.dot(direction)).fold(f64::INFINITY, f64::min) - 1.0;
    let along_high = corners.iter().map(|q| q.dot(direction)).fold(f64::NEG_INFINITY, f64::max) + 1.0;
    if (high - low) / spacing > 200_000.0 { return Vec::new(); }
    let mut lines = Vec::new();
    let mut intervals = Vec::new();
    for k in (low / spacing).ceil() as i64..=(high / spacing).floor() as i64 {
        let origin = normal * (k as f64 * spacing);
        let (a, b) = (origin + direction * along_low, origin + direction * along_high);
        intervals.clear();
        for face in polygons {
            if let Some(interval) = face.clip(a, b, inset) { intervals.push(interval); }
        }
        merge_intervals(&mut intervals);
        for &(lo, hi) in &intervals { if hi > lo { lines.push([a.lerp(b, lo), a.lerp(b, hi)]); } }
    }
    lines
}

impl Scene {
    /// Trama una cara con normal exterior dada por su giro. La dirección se
    /// proyecta sobre el plano de la cara; la separación es siempre en mm del papel.
    /// No añade un oclusor: permite decorar una cara ya añadida por solid.
    pub fn hatch_face(&mut self, points: &[V3], direction: V3, spacing_mm: f64) {
        self.hatch_faces(std::slice::from_ref(&points.to_vec()), direction, spacing_mm);
    }
    /// Como `hatch_face`, para una cara partida en trozos coplanarios: cada línea
    /// cruza los trozos de un solo trazo, como si la cara fuera una.
    pub fn hatch_faces(&mut self, pieces: &[Vec<V3>], direction: V3, spacing_mm: f64) {
        if pieces.is_empty() || !spacing_mm.is_finite() || spacing_mm <= 0.0 || !direction.finite() { return; }
        let n = normal(&pieces[0]);
        if !self.camera.facing(n) { return; }
        let direction = direction - n * direction.dot(n);
        let faces: Vec<Face> = pieces.iter().filter_map(|piece| Face::new(piece, &self.camera)).collect();
        if faces.len() != pieces.len() { return; }
        let origin = self.camera.project(V3::default()).0;
        let projected = self.camera.project(direction).0 - origin;
        if projected.len() < 1e-12 { return; }
        let angle = projected.y.atan2(projected.x).to_degrees();
        // Una cara sola se encoge un poco, como siempre; entre trozos no puede haber
        // holgura, o cada línea se partiría en sus juntas.
        let inset = if faces.len() == 1 { super::FACE_INSET_MM } else { 0.0 };
        for line in hatch_polygons(&faces, self.paper, angle, spacing_mm, inset) {
            self.add_stroke(line.iter().map(|q| Vertex { point: *q, depth: faces[0].depth(*q) }).collect(), false);
        }
    }
}

impl Scene {
    /// Trama una superficie con ángulo de papel y fase global. Conserva la profundidad
    /// de cada faceta en los vértices, pero une sus intervalos contiguos sin levantar pluma.
    pub fn hatch_surface(&mut self, faces: &[Vec<V3>], angle_deg: f64, spacing_mm: f64) {
        if !angle_deg.is_finite() || !spacing_mm.is_finite() || spacing_mm <= 0.0 { return; }
        let faces: Vec<_> = faces.iter().filter(|f| self.camera.facing(normal(f)))
            .filter_map(|f| Face::new(f, &self.camera)).collect();
        for line in hatch_polygons(&faces, self.paper, angle_deg, spacing_mm, 0.0) {
            let mut intervals = Vec::new();
            for face in &faces {
                if let Some((lo, hi)) = face.clip(line[0], line[1], 0.0) {
                    if hi - lo > 1e-12 { intervals.push((lo, hi, face)); }
                }
            }
            intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut vertices = Vec::new();
            let mut end = 0.0;
            for (lo, hi, face) in intervals {
                if hi <= end + 1e-12 && !vertices.is_empty() { continue; }
                let start = line[0].lerp(line[1], lo.max(end));
                let stop = line[0].lerp(line[1], hi);
                if vertices.is_empty() { vertices.push(Vertex { point: start, depth: face.depth(start) }); }
                vertices.push(Vertex { point: stop, depth: face.depth(stop) });
                end = hi;
            }
            self.add_stroke(vertices, false);
        }
    }
}
