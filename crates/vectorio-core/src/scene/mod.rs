//! Escenas ortográficas para plotter. Mundo en metros (z arriba); salida en mm (y abajo).
//!
//! Las caras deben ser planas y convexas; solid espera volúmenes convexos y
//! solid_oriented admite volúmenes cerrados no convexos. Una escena
//! fija su cámara al crearla: cambiarla después invalidaría caras, detalles y tramas.
//! Se eliminan pasadas colineales repetidas dentro de cada parte, conservando la
//! primera. No se optimiza el recorrido ni se mezclan futuras tintas independientes.
#![doc = include_str!("README.md")]
mod camera;
mod hidden;
mod solids;
mod hatch;
mod shadow;
#[cfg(test)]
mod tests;

pub use camera::{Camera, V3};
use crate::geom::{length, BBox, Polyline, P};
use hidden::{Face, Vertex};
use shadow::ShadowGroup;

/// Distancia mínima entre cara y línea para ocultar: 2 mm del mundo, en metros.
/// Protege detalles coplanarios sin introducir desplazamientos en la hoja.
pub const DEPTH_TOLERANCE_M: f64 = 0.002;
/// Encogimiento en mm: evita auto-ocultación en las aristas y queda muy por debajo
/// de 1e-6 mm, incluso en los cortes analíticos de las pruebas.
pub const FACE_INSET_MM: f64 = 1e-9;
pub const MIN_STROKE_MM: f64 = 0.05;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone { Lit, Mid, Dark }

/// Umbrales del coseno entre la normal exterior y la dirección hacia el sol.
#[derive(Clone, Copy, Debug)]
pub struct ToneThresholds { pub lit: f64, pub dark: f64 }
impl Default for ToneThresholds { fn default() -> Self { Self { lit: 0.65, dark: 0.15 } } }

#[derive(Clone, Debug)]
struct Stroke { vertices: Vec<Vertex>, part: PartId, whole: bool }

/// El orden de partes es el de su registro; el de trazos, el de su adición.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenePart { pub id: PartId, pub name: String, pub lines: Vec<Polyline> }

#[derive(Clone, Debug, PartialEq)]
pub struct SceneOutput { pub parts: Vec<ScenePart> }
impl SceneOutput {
    pub fn length_m(&self, id: PartId) -> f64 {
        self.parts.get(id.0 as usize).map_or(0.0, |part| part.lines.iter().map(|l| length(l)).sum::<f64>() / 1000.0)
    }
}

#[derive(Clone, Debug)]
pub struct Scene {
    enabled: bool,
    camera: Camera,
    paper: BBox,
    names: Vec<String>,
    active: PartId,
    faces: Vec<Face>,
    strokes: Vec<Stroke>,
    sun: Option<V3>,
    thresholds: ToneThresholds,
    quiet: bool,
    shadow_group: Option<ShadowGroup>,
}

impl Scene {
    /// Crea también la parte inicial «contorno». El rectángulo debe tener área finita.
    pub fn new(camera: Camera, paper: BBox) -> Self {
        let enabled = camera.valid() && valid_area(paper);
        let camera = if camera.valid() { camera } else { Camera::default() };
        let paper = if valid_area(paper) { paper } else { BBox { min: crate::p(0.0, 0.0), max: crate::p(1.0, 1.0) } };
        Self { enabled, camera, paper, names: vec!["contorno".into()], active: PartId(0), faces: Vec::new(), strokes: Vec::new(),
            sun: None, thresholds: ToneThresholds::default(), quiet: false, shadow_group: None }
    }
    pub fn camera(&self) -> &Camera { &self.camera }
    pub fn paper(&self) -> BBox { self.paper }
    pub fn part(&mut self, name: &str) -> PartId {
        if let Some(i) = self.names.iter().position(|n| n == name) { return PartId(i as u16); }
        if self.names.len() > u16::MAX as usize { return self.active; }
        let id = PartId(self.names.len() as u16);
        self.names.push(name.to_owned());
        id
    }
    pub fn set_part(&mut self, id: PartId) { if (id.0 as usize) < self.names.len() { self.active = id; } }
    pub fn part_names(&self) -> &[String] { &self.names }
    /// Dirección desde la escena hacia el sol. Solo admite soles sobre el horizonte.
    /// Cambiarlo no modifica sombras ya recogidas en un grupo.
    pub fn set_sun(&mut self, sun: Option<V3>) {
        self.sun = sun.filter(|s| s.finite() && s.length().is_finite() && s.z > 0.0).map(V3::normalized);
    }
    pub fn set_tone_thresholds(&mut self, thresholds: ToneThresholds) {
        if !thresholds.dark.is_finite() || !thresholds.lit.is_finite() || thresholds.dark > thresholds.lit { return; }
        self.thresholds = thresholds;
    }
    /// Sin sol, todas las caras tienen tono medio.
    pub fn tone(&self, normal: V3) -> Tone {
        let Some(sun) = self.sun else { return Tone::Mid; };
        let cosine = normal.normalized().dot(sun);
        if cosine >= self.thresholds.lit { Tone::Lit } else if cosine <= self.thresholds.dark { Tone::Dark } else { Tone::Mid }
    }
    /// Afecta a los atajos box3, rotated_box, extrude y lathe añadidos a continuación.
    pub fn set_quiet(&mut self, quiet: bool) { self.quiet = quiet; }
    pub fn paper_mm(&self, meters: f64) -> f64 { meters.abs() * self.camera.scale }
    pub fn visible(&self, meters: f64, minimum_mm: f64) -> bool { self.paper_mm(meters) >= minimum_mm }
    /// Error de cuerda de 0,04 mm en tamaños de hoja habituales; entre 8 y 8.192 lados.
    pub fn circle_segments(&self, radius_m: f64) -> usize {
        let radius = self.paper_mm(radius_m);
        if !radius.is_finite() { return 8; }
        if radius <= 0.04 { return 8; }
        // asin evita la cancelación de acos(1-e/r) con radios grandes.
        (std::f64::consts::PI / (2.0 * (0.02 / radius).sqrt().asin())).ceil().clamp(8.0, 8192.0) as usize
    }
    fn add_stroke(&mut self, vertices: Vec<Vertex>, whole: bool) {
        if self.enabled && vertices.iter().all(|v| v.point.x.is_finite() && v.point.y.is_finite() && v.depth.is_finite()) && vertices.len() >= 2 { self.strokes.push(Stroke { vertices, part: self.active, whole }); }
    }
    pub fn line(&mut self, points: &[V3]) {
        if !self.enabled || !points.iter().all(|v| v.finite()) { return; }
        self.add_stroke(points.iter().map(|q| Vertex::from(self.camera.project(*q))).collect(), false);
    }
    pub fn closed(&mut self, points: &[V3]) {
        if !self.enabled || !points.iter().all(|v| v.finite()) { return; }
        if points.len() < 2 { return; }
        let mut vertices: Vec<_> = points.iter().map(|q| Vertex::from(self.camera.project(*q))).collect();
        if points.first() != points.last() { vertices.push(vertices[0]); }
        self.add_stroke(vertices, false);
    }
    pub fn whole_line(&mut self, points: &[V3]) {
        if !self.enabled || !points.iter().all(|v| v.finite()) { return; }
        self.add_stroke(points.iter().map(|q| Vertex::from(self.camera.project(*q))).collect(), true);
    }
    /// Polígono plano y convexo. El sentido de giro no afecta a la ocultación.
    pub fn occluder(&mut self, points: &[V3]) {
        if !self.enabled || !points.iter().all(|v| v.finite()) { return; }
        if let Some(face) = Face::new(points, &self.camera) { self.faces.push(face); }
    }
    pub fn render(&self) -> SceneOutput { hidden::render(self) }
}

pub(super) fn cross(a: P, b: P) -> f64 { a.x * b.y - a.y * b.x }

// Busca tres puntos no alineados; el primer tramo puede contener vértices redundantes.
pub(super) fn normal(points: &[V3]) -> V3 {
    if points.len() >= 3 {
        for i in 1..points.len() - 1 {
            let n = (points[i] - points[0]).cross(points[i + 1] - points[0]);
            if n.length() > 1e-12 { return n.normalized(); }
        }
    }
    V3::default()
}

pub(super) fn valid_area(b: BBox) -> bool {
    [b.min.x, b.min.y, b.max.x, b.max.y].iter().all(|v| v.is_finite()) && b.width() > 0.0 && b.height() > 0.0
}

pub(super) fn valid_face(points: &[V3]) -> bool {
    if points.len() < 3 || !points.iter().all(|v| v.finite()) { return false; }
    let n = normal(points);
    n.length() > 0.0 && points.iter().all(|p| (*p - points[0]).dot(n).abs() <= 1e-8) &&
        (0..points.len()).all(|i| {
            let a = points[(i + 1) % points.len()] - points[i];
            let b = points[(i + 2) % points.len()] - points[(i + 1) % points.len()];
            a.cross(b).dot(n) >= -1e-10
        })
}
