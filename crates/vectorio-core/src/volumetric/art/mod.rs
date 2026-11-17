//! Lenguajes de superficie: ondas geodésicas, malla cromática y combinación.
mod edges;
pub(super) mod field;
#[cfg(test)]
mod tests;
use super::*;
use crate::BBox;
use std::sync::Arc;
const CANCEL: &str = "Cálculo cancelado.";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WavePattern {
    Waves,
    Interference,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshColor {
    Orientation,
    Height,
    Depth,
    Objects,
}
/// Red abierta y legible a plóter; la calidad de la forma no depende de ella.
pub const DEFAULT_NET: usize = 2500;
/// Valor de los archivos anteriores a la red independiente: dibujar todas las facetas.
pub(crate) fn full_net() -> usize {
    20_000
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArtSettings {
    pub facets: usize,
    /// Triángulos de la red dibujada en Malla e Híbrido. La forma, las ondas y la
    /// ocultación siguen usando `facets`; los archivos antiguos dibujan la malla entera.
    #[serde(default = "full_net")]
    pub mesh_facets: usize,
    pub lines: usize,
    pub pattern: WavePattern,
    pub origin_azimuth: f64,
    pub origin_elevation: f64,
    pub deformation: f64,
    pub frequency: f64,
    pub coloring: MeshColor,
    pub mesh_inks: usize,
    pub outline: bool,
}
impl Default for ArtSettings {
    fn default() -> Self {
        Self {
            facets: 20_000,
            mesh_facets: DEFAULT_NET,
            lines: 84,
            pattern: WavePattern::Waves,
            origin_azimuth: 165.,
            origin_elevation: 30.,
            deformation: 0.14,
            frequency: 5.,
            coloring: MeshColor::Orientation,
            mesh_inks: 3,
            outline: false,
        }
    }
}
impl ArtSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(64..=20_000).contains(&self.facets)
            || !(64..=20_000).contains(&self.mesh_facets)
            || !(4..=160).contains(&self.lines)
            || !(1..=256).contains(&self.mesh_inks)
            || [
                self.origin_azimuth,
                self.origin_elevation,
                self.deformation,
                self.frequency,
            ]
            .iter()
            .any(|x| !x.is_finite())
            || !(-90.0..=90.).contains(&self.origin_elevation)
            || !(0.0..=0.6).contains(&self.deformation)
            || !(0.5..=12.).contains(&self.frequency)
        {
            return Err("Parámetros de ondas o malla fuera de rango.".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct MeshStroke {
    /// Arista de la red; si es más abierta que la malla, sigue la superficie fina.
    pub points: Vec<V3>,
    pub role: usize,
}
#[derive(Clone, Debug)]
pub struct ArtDrawing {
    pub edges: Arc<Vec<MeshStroke>>,
}
type FieldKey = (WavePattern, f64, f64, f64, f64, u64);
type EdgeKey = (MeshColor, usize, usize, bool, f64, f64, Option<f64>);
#[derive(Default)]
pub(super) struct Cache {
    mesh: Option<Arc<Mesh>>,
    graph: Option<field::Graph>,
    field_key: Option<FieldKey>,
    values: Vec<f64>,
    fields: Vec<(FieldKey, Vec<f64>)>,
    levels: Vec<(FieldKey, usize, Arc<Vec<Contour>>)>,
    edge_groups: Vec<(EdgeKey, Arc<Vec<MeshStroke>>)>,
    curve_key: Option<usize>,
    curves: Option<Arc<Vec<Contour>>>,
    edge_key: Option<EdgeKey>,
    edges: Option<Arc<Vec<MeshStroke>>>,
    nets: Vec<(usize, Arc<Mesh>)>,
    lift: Option<f64>,
}
impl Cache {
    pub fn draw(
        &mut self,
        mesh: Arc<Mesh>,
        s: &Settings,
        camera: Camera,
        area: BBox,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(Vec<DrawingPart>, Arc<Vec<Contour>>, ArtDrawing), String> {
        if self.mesh.as_ref().is_none_or(|m| !Arc::ptr_eq(m, &mesh)) {
            *self = Self::default();
            self.mesh = Some(mesh.clone());
        }
        let has_waves = s.recipe != RecipeKind::TriMesh;
        let has_mesh = s.recipe != RecipeKind::SurfaceWaves;
        let hybrid = s.recipe == RecipeKind::Hybrid;
        let art = &s.art;
        let curves = if has_waves {
            if self.graph.is_none() {
                self.graph = Some(field::Graph::new(&mesh, cancel)?);
            }
            let key = field_key(s);
            if self.field_key != Some(key) {
                self.values = if let Some((_, values)) = self.fields.iter().find(|(k, _)| *k == key)
                {
                    values.clone()
                } else {
                    let values = self.graph.as_ref().unwrap().values(art, s.seed, cancel)?;
                    self.fields.push((key, values.clone()));
                    values
                };
                self.field_key = Some(key);
                self.curve_key = None;
            }
            if self.curve_key != Some(art.lines) {
                if let Some((_, _, curves)) = self
                    .levels
                    .iter()
                    .find(|(k, n, _)| *k == key && *n == art.lines)
                {
                    self.curves = Some(curves.clone());
                    self.curve_key = Some(art.lines);
                }
            }
            if self.curve_key != Some(art.lines) {
                let (lo, hi) = self
                    .values
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                        (a.min(*v), b.max(*v))
                    });
                let levels: Vec<_> = if hi - lo > 1e-10 {
                    (1..=art.lines)
                        .map(|i| lo + (hi - lo) * i as f64 / (art.lines as f64 + 1.))
                        .collect()
                } else {
                    vec![]
                };
                self.curves = Some(Arc::new(isolines(&mesh, &self.values, &levels, cancel)?));
                self.curve_key = Some(art.lines);
                self.levels
                    .push((key, art.lines, self.curves.as_ref().unwrap().clone()));
            }
            self.curves.as_ref().unwrap().clone()
        } else {
            Arc::new(vec![])
        };
        let strokes = if has_mesh {
            let key = edge_key(s);
            if self.edge_key != Some(key) {
                self.edges = Some(
                    if let Some((_, edges)) = self.edge_groups.iter().find(|(k, _)| *k == key) {
                        edges.clone()
                    } else {
                        let net = self.net(&mesh, art.mesh_facets, cancel)?;
                        let mut edges = edges::strokes(&net, art, &camera, hybrid, cancel)?;
                        if !Arc::ptr_eq(&net, &mesh) {
                            edges::drape(&mut edges, &mesh, cancel)?;
                        }
                        let edges = Arc::new(edges);
                        self.edge_groups.push((key, edges.clone()));
                        edges
                    },
                );
                self.edge_key = Some(key);
            }
            self.edges.as_ref().unwrap().clone()
        } else {
            Arc::new(vec![])
        };
        let mut render_camera = camera;
        render_camera.scale /= 1000.;
        render_camera.world_center = render_camera.world_center * 1000.;
        render_camera.perspective = render_camera
            .perspective
            .map(|[d, n]| [d * 1000., n * 1000.]);
        let rotate = |q: V3| {
            let q = q - camera.world_center;
            let axis = camera.axes().2;
            let (sin, cos) = s.paper_rotation.to_radians().sin_cos();
            (camera.world_center
                + q * cos
                + axis.cross(q) * sin
                + axis * (axis.dot(q) * (1. - cos)))
                * 1000.
        };
        let mut scene = crate::scene::Scene::new(render_camera, area);
        scene.set_intersections(false);
        scene.set_hidden_lines(s.hidden_lines);
        scene.set_line_halo(s.halo_mm);
        let names = match s.recipe {
            RecipeKind::TriMesh => ["Malla · grupo 1", "Malla · grupo 2", "Malla · grupo 3"],
            RecipeKind::Hybrid => ["Ondas de superficie", "Malla · grupo 1", "Malla · grupo 2"],
            _ => ["Ondas principales", "Ondas secundarias", "Silueta"],
        };
        let mut names: Vec<String> = names.into_iter().map(str::to_owned).collect();
        if s.recipe == RecipeKind::TriMesh {
            names.extend((3..art.mesh_inks).map(|i| format!("Malla · grupo {}", i + 1)));
        } else if hybrid {
            names.extend((2..art.mesh_inks).map(|i| format!("Malla · grupo {}", i + 1)));
        }
        let parts: Vec<_> = names.iter().map(|name| scene.part(name)).collect();
        if s.visible_only {
            let pts = mesh.points();
            for (i, t) in mesh.triangles.iter().enumerate() {
                if i % 128 == 0 && cancel() {
                    return Err(CANCEL.into());
                }
                let mut t = *t;
                t.sort();
                scene.occluder(&t.map(|j| rotate(pts[j])));
            }
        }
        for curve in curves.iter() {
            if cancel() {
                return Err(CANCEL.into());
            }
            scene.set_part(
                parts[if hybrid || curve.level % s.major_every == 0 {
                    0
                } else {
                    1
                }],
            );
            scene.line(&curve.points.iter().map(|q| rotate(*q)).collect::<Vec<_>>());
        }
        // Las aristas apoyadas sobre la malla fina se hunden un poco entre muestras; se
        // acercan a la cámara por su rayo de vista, así su trazo en el papel no cambia y
        // su propia superficie no las oculta.
        let lift = if strokes.iter().any(|s| s.points.len() > 2) {
            *self.lift.get_or_insert_with(|| 0.25 * mean_edge(&mesh))
        } else {
            0.
        };
        let away = camera.axes().2;
        let toward = |q: V3| match camera.perspective {
            Some([d, _]) => (camera.world_center - away * d - q).normalized(),
            None => away * -1.,
        };
        for stroke in strokes.iter() {
            if cancel() {
                return Err(CANCEL.into());
            }
            scene.set_part(parts[stroke.role]);
            let line: Vec<_> = if stroke.points.len() > 2 {
                stroke.points.iter().map(|q| rotate(*q + toward(*q) * lift)).collect()
            } else {
                stroke.points.iter().map(|q| rotate(*q)).collect()
            };
            scene.line(&line);
        }
        if art.outline && has_waves {
            scene.set_part(parts[if hybrid { 0 } else { 2 }]);
            for [a, b] in feature_edges(&mesh, &camera, 180.) {
                scene.line(&[rotate(a), rotate(b)]);
            }
        }
        let result = scene.render_cancellable(cancel).ok_or(CANCEL)?;
        let result = result
            .parts
            .into_iter()
            .filter(|p| p.id.0 > 0)
            .map(|part| DrawingPart {
                families: vec![],
                ramp: None,
                pen: part.id.0 as usize - 1,
                name: part.name,
                lines: part
                    .lines
                    .into_iter()
                    .map(|l| crate::geom::simplify(&l, s.simplify_mm))
                    .filter(|l| crate::geom::length(l) >= 0.05)
                    .collect(),
            })
            .collect();
        Ok((result, curves, ArtDrawing { edges: strokes }))
    }
}

fn mean_edge(mesh: &Mesh) -> f64 {
    let pts = mesh.points();
    let total: f64 = mesh
        .triangles
        .iter()
        .map(|t| {
            (pts[t[1]] - pts[t[0]]).length()
                + (pts[t[2]] - pts[t[1]]).length()
                + (pts[t[0]] - pts[t[2]]).length()
        })
        .sum();
    total / (3 * mesh.triangles.len().max(1)) as f64
}
fn field_key(s: &Settings) -> FieldKey {
    let a = &s.art;
    (
        a.pattern,
        a.origin_azimuth,
        a.origin_elevation,
        a.deformation,
        a.frequency,
        if a.deformation > 0. { s.seed } else { 0 },
    )
}
fn edge_key(s: &Settings) -> EdgeKey {
    let depth = s.art.coloring == MeshColor::Depth;
    (
        s.art.coloring,
        s.art.mesh_inks,
        s.art.mesh_facets,
        s.recipe == RecipeKind::Hybrid,
        if depth { s.yaw } else { 0. },
        if depth { s.elevation } else { 0. },
        (depth && s.projection.perspective).then_some(s.projection.distance),
    )
}
impl Cache {
    /// Red reducida desde la malla de calidad: sus vértices son vértices de la malla fina.
    fn net(
        &mut self,
        mesh: &Arc<Mesh>,
        target: usize,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Arc<Mesh>, String> {
        if target >= mesh.triangles.len() {
            return Ok(mesh.clone());
        }
        if let Some((_, net)) = self.nets.iter().find(|(n, _)| *n == target) {
            return Ok(net.clone());
        }
        let net = Arc::new(super::reduce::reduced(mesh, target, 0.04, cancel)?);
        if self.nets.len() >= 4 {
            self.nets.remove(0);
        }
        self.nets.push((target, net.clone()));
        Ok(net)
    }
    pub(super) fn retain_programs(&mut self, programs: &[Settings]) {
        let limit = programs.len() + 16;
        if self.fields.len() > limit {
            self.fields
                .retain(|(key, _)| programs.iter().any(|s| *key == field_key(s)));
        }
        if self.levels.len() > limit {
            self.levels.retain(|(key, lines, _)| {
                programs
                    .iter()
                    .any(|s| *key == field_key(s) && *lines == s.art.lines)
            });
        }
        if self.edge_groups.len() > limit {
            self.edge_groups
                .retain(|(key, _)| programs.iter().any(|s| *key == edge_key(s)));
        }
    }
}
