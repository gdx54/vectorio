use super::{cross, Scene, V3};
use super::hatch::hatch_polygons;
use super::hidden::Face;
use crate::geom::{p, P};

#[derive(Clone, Debug)]
pub(super) struct ShadowGroup {
    z: f64,
    clip: Option<[f64; 4]>,
    angle: f64,
    sun: V3,
    polygons: Vec<Vec<V3>>,
}

fn hull(mut points: Vec<P>) -> Vec<P> {
    points.sort_unstable_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| a.dist(*b) < 1e-12);
    if points.len() < 3 { return Vec::new(); }
    let mut ring = Vec::new();
    // Dos pasadas separadas evitan depender de identidades de vértice.
    for q in &points {
        while ring.len() >= 2 && cross(ring[ring.len() - 1] - ring[ring.len() - 2], *q - ring[ring.len() - 1]) <= 1e-12 {
            ring.pop();
        }
        ring.push(*q);
    }
    let lower = ring.len();
    for q in points.iter().rev().skip(1) {
        while ring.len() > lower && cross(ring[ring.len() - 1] - ring[ring.len() - 2], *q - ring[ring.len() - 1]) <= 1e-12 {
            ring.pop();
        }
        ring.push(*q);
    }
    ring.pop();
    ring
}

fn clip_world(mut polygon: Vec<P>, bounds: [f64; 4]) -> Vec<P> {
    for (axis, bound, sign) in [(0, bounds[0], 1.0), (1, bounds[1], 1.0), (0, bounds[2], -1.0), (1, bounds[3], -1.0)] {
        let mut next = Vec::new();
        if polygon.is_empty() { return polygon; }
        let value = |q: P| (if axis == 0 { q.x } else { q.y } - bound) * sign;
        for (&a, &b) in polygon.iter().zip(polygon.iter().cycle().skip(1)).take(polygon.len()) {
            let (va, vb) = (value(a), value(b));
            if va >= 0.0 { next.push(a); }
            if (va >= 0.0) != (vb >= 0.0) { next.push(a.lerp(b, va / (va - vb))); }
        }
        polygon = next;
    }
    polygon
}

impl Scene {
    /// Abre un grupo de sombras sobre z. Descarta el anterior sin dibujarlo.
    /// El ángulo está en grados de papel, positivo hacia abajo; todos los grupos
    /// con ángulo y separación iguales comparten fase respecto al origen de hoja.
    pub fn begin_shadows(&mut self, ground_z: f64, world_clip: Option<[f64; 4]>, hatch_angle_deg: f64) {
        // Un grupo nuevo descarta el anterior sin tramar; evita pasadas inesperadas.
        self.shadow_group = None;
        if !ground_z.is_finite() || !hatch_angle_deg.is_finite() { return; }
        if let Some(b) = world_clip { if !b.iter().all(|v| v.is_finite()) || b[0] >= b[2] || b[1] >= b[3] { return; } }
        let Some(sun) = self.sun else { return; };
        self.shadow_group = Some(ShadowGroup { z: ground_z, clip: world_clip, angle: hatch_angle_deg, sun, polygons: Vec::new() });
    }
    pub(super) fn collect_shadow(&mut self, vertices: &[V3]) {
        let Some(group) = &mut self.shadow_group else { return; };
        // Solo la porción sobre el suelo arroja sombra. Los cortes con el plano
        // son combinaciones de parejas de vértices y mantienen la envolvente convexa.
        let mut above: Vec<_> = vertices.iter().copied().filter(|v| v.z >= group.z).collect();
        for a in vertices.iter().filter(|v| v.z < group.z) {
            for b in vertices.iter().filter(|v| v.z > group.z) {
                above.push(*a + (*b - *a) * ((group.z - a.z) / (b.z - a.z)));
            }
        }
        let mut polygon = hull(above.iter().map(|v| {
            let q = *v - group.sun * ((v.z - group.z) / group.sun.z);
            p(q.x, q.y)
        }).collect());
        if let Some(bounds) = group.clip { polygon = clip_world(polygon, bounds); }
        if polygon.len() >= 3 { group.polygons.push(polygon.iter().map(|q| V3::new(q.x, q.y, group.z)).collect()); }
    }
    /// Consume el grupo y añade su unión de intervalos a la parte activa.
    /// Las sombras se ocultan después contra todos los sólidos de la escena.
    pub fn hatch_shadows(&mut self, spacing_mm: f64) {
        if !spacing_mm.is_finite() || spacing_mm <= 0.0 { return; }
        let Some(group) = self.shadow_group.take() else { return; };
        let faces: Vec<_> = group.polygons.iter().filter_map(|poly| Face::new(poly, &self.camera)).collect();
        for line in hatch_polygons(&faces, self.paper, group.angle, spacing_mm, 0.0) {
            self.line(&line.map(|q| self.camera.ground(q, group.z)));
        }
    }
}
