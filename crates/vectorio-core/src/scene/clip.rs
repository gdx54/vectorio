//! Carga de la escena y recorte esférico de lo añadido desde una marca.
//!
//! El recorte sirve a las cúpulas: el mundo interior se dibuja con las piezas de
//! siempre y después se descarta lo que asoma fuera del vidrio.
use super::{Scene, V3, Vertex};

/// Presupuesto de una escena densa: el motor responde con holgura hasta aquí.
pub const FACE_BUDGET: usize = 8_000;
pub const SEGMENT_BUDGET: usize = 40_000;

/// Punto de la escena a partir del cual se puede recortar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark { faces: usize, holes: usize, strokes: usize, shadows: usize }

/// Caras y segmentos añadidos hasta ahora (sin contar la trama de sombras pendiente).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Load { pub faces: usize, pub segments: usize }

impl Load {
    pub fn within_budget(self) -> bool { self.faces <= FACE_BUDGET && self.segments <= SEGMENT_BUDGET }
}

impl Scene {
    pub fn load(&self) -> Load {
        let segments = self.strokes.iter().map(|s| s.vertices.len().saturating_sub(1)).sum();
        Load { faces: self.faces.len(), segments }
    }
    pub fn mark(&self) -> Mark {
        let shadows = self.shadow_group.as_ref().map_or(0, |g| g.polygons.len());
        Mark { faces: self.faces.len(), holes: self.holes.len(), strokes: self.strokes.len(), shadows }
    }
    /// Conserva de lo añadido desde `mark` solo lo que queda dentro de la esfera.
    /// Los trazos se cortan en el borde; las caras, huecos y sombras se quedan si su
    /// centro está dentro. Lo anterior a la marca no se toca.
    pub fn clip_to_sphere(&mut self, mark: Mark, center: V3, radius: f64) {
        if !center.finite() || !radius.is_finite() || radius <= 0.0 { return; }
        let inside = |q: V3| (q - center).length() <= radius;
        let camera = self.camera;
        let face_inside = |face: &super::Face| {
            let n = face.points.len().max(1) as f64;
            let mean = face.points.iter().fold(crate::geom::p(0.0, 0.0), |s, q| s + *q) * (1.0 / n);
            inside(camera.unproject(mean, face.depth(mean)))
        };
        let kept: Vec<_> = self.faces.split_off(mark.faces.min(self.faces.len())).into_iter().filter(|f| face_inside(f)).collect();
        self.faces.extend(kept);
        let kept: Vec<_> = self.holes.split_off(mark.holes.min(self.holes.len())).into_iter().filter(|f| face_inside(f)).collect();
        self.holes.extend(kept);
        let tail = self.strokes.split_off(mark.strokes.min(self.strokes.len()));
        for stroke in tail {
            for piece in clip_stroke(&stroke.vertices, &|v: Vertex| inside(camera.unproject(v.point, v.depth))) {
                self.strokes.push(super::Stroke { vertices: piece, ..stroke });
            }
        }
        if let Some(group) = &mut self.shadow_group {
            let tail = group.polygons.split_off(mark.shadows.min(group.polygons.len()));
            group.polygons.extend(tail.into_iter().filter(|poly| {
                let n = poly.len().max(1) as f64;
                inside(poly.iter().fold(V3::default(), |s, q| s + *q) * (1.0 / n))
            }));
        }
    }
}

/// Parte un trazo en los tramos que cumplen `inside`; el corte se busca por
/// bisección, con una precisión muy por debajo del grosor de la línea.
fn clip_stroke(vertices: &[Vertex], inside: &dyn Fn(Vertex) -> bool) -> Vec<Vec<Vertex>> {
    let mut pieces = Vec::new();
    let mut current: Vec<Vertex> = Vec::new();
    for (i, &v) in vertices.iter().enumerate() {
        let now = inside(v);
        if i > 0 {
            let before = inside(vertices[i - 1]);
            if before != now {
                let cut = boundary(vertices[i - 1], v, before, inside);
                current.push(cut);
                if before {
                    pieces.push(std::mem::take(&mut current));
                }
            }
        }
        if now {
            current.push(v);
        }
    }
    pieces.push(current);
    pieces.retain(|p| p.len() >= 2);
    pieces
}

fn boundary(a: Vertex, b: Vertex, a_inside: bool, inside: &dyn Fn(Vertex) -> bool) -> Vertex {
    let lerp = |t: f64| Vertex { point: a.point + (b.point - a.point) * t, depth: a.depth + (b.depth - a.depth) * t };
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..30 {
        let mid = (low + high) * 0.5;
        if inside(lerp(mid)) == a_inside { low = mid; } else { high = mid; }
    }
    lerp((low + high) * 0.5)
}
