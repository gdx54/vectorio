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
mod intersections;
mod solids;
mod polyhedron;
mod hatch;
mod shadow;
mod clip;
#[cfg(test)]
mod tests;

pub use camera::{Camera, V3};
pub use solids::Lathe;
pub use clip::{Load, Mark, FACE_BUDGET, SEGMENT_BUDGET};
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
/// Un hueco borra lo que está en su plano con esta holgura, en metros: la trama
/// de la pared sí, un objeto delante o un edificio detrás no.
pub const HOLE_TOLERANCE_M: f64 = 0.01;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone { Lit, Mid, Dark }

/// Umbrales del coseno entre la normal exterior y la dirección hacia el sol.
#[derive(Clone, Copy, Debug)]
pub struct ToneThresholds { pub lit: f64, pub dark: f64 }
impl Default for ToneThresholds { fn default() -> Self { Self { lit: 0.65, dark: 0.15 } } }

#[derive(Clone, Debug)]
struct Stroke { vertices: Vec<Vertex>, part: PartId, whole: bool, fill: bool }

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

thread_local! {static CAPTURE_GEOMETRY:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
/// La conversión de Crear solicita las caras completas sin cargar los renders normales.
pub fn capture_geometry<T>(build:impl FnOnce()->T)->T{
    struct Reset(bool);impl Drop for Reset{fn drop(&mut self){CAPTURE_GEOMETRY.with(|v|v.set(self.0));}}
    let _reset=Reset(CAPTURE_GEOMETRY.with(|v|v.replace(true)));build()
}
#[derive(Clone, Debug)]
pub struct Scene {
    geometry:Option<Vec<(u64,Vec<V3>)>>,
    enabled: bool,
    intersections: bool,
    hidden_lines: bool,
    halo_mm: f64,
    next_solid: u64,
    camera: Camera,
    paper: BBox,
    names: Vec<String>,
    active: PartId,
    faces: Vec<Face>,
    holes: Vec<Face>,
    strokes: Vec<Stroke>,
    filling: bool,
    sun: Option<V3>,
    thresholds: ToneThresholds,
    quiet: bool,
    shadow_group: Option<ShadowGroup>,
}

impl Scene {
    /// Crea también la parte inicial «contorno». El rectángulo debe tener área finita.
    pub fn new(camera: impl Into<Camera>, paper: BBox) -> Self {
        let camera=camera.into();
        let enabled = camera.valid() && valid_area(paper);
        let camera = if camera.valid() { camera } else { Camera::default() };
        let paper = if valid_area(paper) { paper } else { BBox { min: crate::p(0.0, 0.0), max: crate::p(1.0, 1.0) } };
        Self { geometry:CAPTURE_GEOMETRY.with(|v|v.get().then(Vec::new)), enabled, halo_mm: 0., hidden_lines: false, intersections: true, next_solid: 0, camera, paper, names: vec!["contorno".into()], active: PartId(0), faces: Vec::new(), holes: Vec::new(), strokes: Vec::new(), filling: false,
            sun: None, thresholds: ToneThresholds::default(), quiet: false, shadow_group: None }
    }
    /// Activa los encuentros analíticos. Los sólidos añadidos con el interruptor
    /// apagado no los generan aunque después se vuelva a activar.
    pub fn set_intersections(&mut self, enabled: bool) { self.intersections = enabled; }
    /// Dibuja únicamente los intervalos ocultos, con el mismo recorte analítico.
    pub fn set_hidden_lines(&mut self, enabled: bool) { self.hidden_lines = enabled; }
    /// Reserva un halo físico junto a los intervalos ocultos; no altera trazos delanteros.
    pub fn set_line_halo(&mut self, mm: f64) { self.halo_mm=if mm.is_finite(){mm.clamp(0.,5.)}else{0.}; }
    pub fn camera(&self) -> &Camera { &self.camera }
    /// Malla real de las superficies de la escena, incluidas las caras ocultas.
    pub fn volume_mesh(&self,name:&str)->Result<crate::volumetric::Mesh,String>{
        let mut mesh=crate::volumetric::Mesh{name:name.into(),vertices:Vec::new(),triangles:Vec::new(),objects:Vec::new(),face_objects:Vec::new()};
        let mut owners=std::collections::BTreeMap::new();
        let geometry=self.geometry.as_ref().ok_or("La escena no conserva geometría 3D. Vuelve a generarla desde Crear.")?;
        for (solid,points) in geometry {
            if points.len()<3{continue;}
            let index=mesh.vertices.len();mesh.vertices.extend(points.iter().map(|q|[q.x,q.y,q.z]));
            let next=owners.len();let owner=*owners.entry(*solid).or_insert(next);
            for i in 1..points.len()-1{mesh.triangles.push([index,index+i,index+i+1]);mesh.face_objects.push(owner);}
        }
        mesh.objects=(0..owners.len()).map(|i|format!("Objeto {}",i+1)).collect();
        mesh.validated().map(|(m,_)|m)
    }

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
        if self.enabled && vertices.iter().all(|v| v.point.x.is_finite() && v.point.y.is_finite() && v.depth.is_finite()) && vertices.len() >= 2 { self.strokes.push(Stroke { vertices, part: self.active, whole, fill: self.filling }); }
    }
    pub fn line(&mut self, points: &[V3]) {
        if !self.enabled || !points.iter().all(|v| v.finite()) { return; }
        if self.camera.perspective.is_some()&&points.iter().any(|q|self.camera.near_value(*q)<0.){for pair in points.windows(2){if let Some(edge)=self.camera.clip_segment(pair[0],pair[1]){self.add_stroke(edge.iter().map(|q|Vertex::from(self.camera.project(*q))).collect(),false);}}return;}
        self.add_stroke(points.iter().map(|q| Vertex::from(self.camera.project(*q))).collect(), false);
    }
    pub fn closed(&mut self, points: &[V3]) {
        if !self.enabled || !points.iter().all(|v| v.finite()) { return; }
        if points.len() < 2 { return; }
        if self.camera.perspective.is_some()&&points.iter().any(|q|self.camera.near_value(*q)<0.){let mut ring=points.to_vec();if ring.first()!=ring.last(){ring.push(ring[0]);}self.line(&ring);return;}
        let mut vertices: Vec<_> = points.iter().map(|q| Vertex::from(self.camera.project(*q))).collect();
        if points.first() != points.last() { vertices.push(vertices[0]); }
        self.add_stroke(vertices, false);
    }
    pub fn whole_line(&mut self, points: &[V3]) {
        if !self.enabled || !points.iter().all(|v| v.finite()) { return; }
        if points.iter().any(|p|self.camera.near_value(*p)<0.){return;}
        self.add_stroke(points.iter().map(|q| Vertex::from(self.camera.project(*q))).collect(), true);
    }
    /// Polígono plano y convexo. El sentido de giro no afecta a la ocultación.
    pub fn occluder(&mut self, points: &[V3]) {
        if !self.enabled || !points.iter().all(|v| v.finite()) { return; }
        if let Some(geometry)=&mut self.geometry{if valid_face(points){geometry.push((self.next_solid,points.to_vec()));self.next_solid+=1;}}
        if let Some(face) = Face::new(points, &self.camera) { self.faces.push(face); }
    }
    /// Hueco plano y convexo sobre una cara (ventana, puerta). Borra de los trazos
    /// no atómicos lo que cae dentro y en su plano: la trama de la pared, las sombras
    /// sobre ella. No oculta lo que está delante ni lo que está detrás, así que un
    /// hueco en una fachada trasera no perfora otros edificios. Los marcos dibujados
    /// con `whole_line` y el relleno de `hatch_hole` se conservan.
    pub fn hole(&mut self, points: &[V3]) {
        if !self.enabled || !points.iter().all(|v| v.finite()) { return; }
        if let Some(face) = Face::new(points, &self.camera) { self.holes.push(face); }
    }
    /// Relleno de un hueco: como `hatch_face`, pero sin que los huecos lo recorten.
    pub fn hatch_hole(&mut self, points: &[V3], direction: V3, spacing_mm: f64) {
        self.filling = true; self.hatch_face(points, direction, spacing_mm); self.filling = false;
    }
    pub fn render(&self) -> SceneOutput { hidden::render(self) }
    /// Variante cancelable para previsualizaciones interactivas.
    pub fn render_cancellable(&self, cancel: &dyn Fn() -> bool) -> Option<SceneOutput> { hidden::render_cancellable(self, cancel) }
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
