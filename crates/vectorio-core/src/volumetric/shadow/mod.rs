//! Sombra plana: máscaras vectoriales de sombra propia y arrojada, con una tinta.
mod mask;
pub(super) mod project;
#[cfg(test)]
mod tests;
pub(super) mod tone;
use super::*;
use crate::{p, BBox, Polyline, P};
use std::{collections::BTreeMap, sync::Arc};
const CANCEL: &str = "Cálculo cancelado.";
const PIECE_LIMIT: usize = 250_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShadowSettings {
    pub azimuth: f64,
    pub elevation: f64,
    pub threshold: f64,
    pub facets: usize,
    pub ground: bool,
    pub ground_gap: f64,
    pub middle_tone: bool,
    pub outline: bool,
    pub hatch_spacing_mm: f64,
    pub hatch_angle: f64,
}
impl Default for ShadowSettings {
    fn default() -> Self {
        Self {
            azimuth: 135.,
            elevation: 45.,
            threshold: 0.14,
            facets: 1200,
            ground: true,
            ground_gap: 0.,
            middle_tone: false,
            outline: false,
            hatch_spacing_mm: 0.22,
            hatch_angle: 35.,
        }
    }
}
impl ShadowSettings {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.azimuth,
            self.elevation,
            self.threshold,
            self.ground_gap,
            self.hatch_spacing_mm,
            self.hatch_angle,
        ]
        .iter()
        .any(|x| !x.is_finite())
            || !(5.0..=85.).contains(&self.elevation)
            || !(0.0..=0.9).contains(&self.threshold)
            || !(64..=8000).contains(&self.facets)
            || !(0.0..=2.).contains(&self.ground_gap)
            || !(0.08..=2.).contains(&self.hatch_spacing_mm)
        {
            return Err("Parámetros de sombra plana fuera de rango.".into());
        }
        Ok(())
    }
    pub fn light(&self) -> V3 {
        let (s, c) = self.azimuth.to_radians().sin_cos();
        let (se, ce) = self.elevation.to_radians().sin_cos();
        V3::new(c * ce, s * ce, se)
    }
}
#[derive(Clone, Debug)]
pub struct ShadedFace {
    pub cosine: f64,
    pub dark: bool,
    pub middle: bool,
    pub cast: Vec<Vec<V3>>,
}
#[derive(Clone, Debug)]
pub struct ShadowDrawing {
    /// Máscaras 2D en mm; el fondo es blanco/papel y elimina el suelo tras el objeto.
    pub floor: Vec<Polyline>,
    pub object: Vec<Polyline>,
    pub dark: Vec<Polyline>,
    pub middle: Vec<Polyline>,
    pub ground_world: Vec<Vec<V3>>,
    pub faces: Arc<Vec<ShadedFace>>,
    pub world_bounds: [V3; 2],
}
type LightKey = (f64, f64, f64, bool);
#[derive(Default)]
pub(super) struct Cache {
    mesh: Option<Arc<Mesh>>,
    normals: Vec<V3>,
    key: Option<LightKey>,
    fields: Vec<(LightKey, Arc<Vec<ShadedFace>>)>,
    field: Option<Arc<Vec<ShadedFace>>>,
}
pub(super) fn normals(mesh: &Mesh, cancel: &dyn Fn() -> bool) -> Result<Vec<V3>, String> {
    let points = mesh.points();
    let mut edges = BTreeMap::<(usize, usize), Vec<(usize, bool)>>::new();
    for (i, t) in mesh.triangles.iter().enumerate() {
        if i % 256 == 0 && cancel() {
            return Err(CANCEL.into());
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            edges
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push((i, a < b));
        }
    }
    let mut open = vec![false; mesh.triangles.len()];
    let mut adjacency = vec![Vec::new(); mesh.triangles.len()];
    for e in edges.values() {
        if e.len() != 2 {
            for &(i, _) in e {
                open[i] = true;
            }
        }
        if e.len() == 2 {
            let (a, sa) = e[0];
            let (b, sb) = e[1];
            adjacency[a].push((b, sa == sb));
            adjacency[b].push((a, sa == sb));
        }
    }
    let center = (mesh.bounds()[0] + mesh.bounds()[1]) * 0.5;
    let mut flipped = vec![None; mesh.triangles.len()];
    for initial in 0..flipped.len() {
        if flipped[initial].is_some() {
            continue;
        }
        let mut component = vec![initial];
        flipped[initial] = Some(false);
        let mut at = 0;
        while at < component.len() {
            let a = component[at];
            at += 1;
            for &(b, different) in &adjacency[a] {
                if flipped[b].is_none() {
                    flipped[b] = Some(flipped[a].unwrap() ^ different);
                    component.push(b);
                }
            }
        }
        let volume: f64 = component
            .iter()
            .map(|i| {
                let t = mesh.triangles[*i];
                let a = points[t[0]] - center;
                let b = points[t[1]] - center;
                let c = points[t[2]] - center;
                let value = a.dot(b.cross(c));
                if flipped[*i].unwrap() {
                    -value
                } else {
                    value
                }
            })
            .sum();
        if component.iter().all(|i| !open[*i]) && volume < -1e-12 {
            for i in component {
                flipped[i] = Some(!flipped[i].unwrap());
            }
        }
    }
    Ok(mesh
        .triangles
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let n = (points[t[1]] - points[t[0]])
                .cross(points[t[2]] - points[t[0]])
                .normalized();
            if flipped[i].unwrap() {
                n * (-1.)
            } else {
                n
            }
        })
        .collect())
}
impl Cache {
    pub(super) fn field(
        &mut self,
        mesh: Arc<Mesh>,
        settings: &ShadowSettings,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Arc<Vec<ShadedFace>>, String> {
        if self.mesh.as_ref().is_none_or(|m| !Arc::ptr_eq(m, &mesh)) {
            self.normals = normals(&mesh, cancel)?;
            self.mesh = Some(mesh.clone());
            self.key = None;
            self.fields.clear();
        }
        let key = light_key(settings);
        if self.key == Some(key) {
            return Ok(self.field.as_ref().unwrap().clone());
        }
        if let Some((_, field)) = self.fields.iter().find(|(k, _)| *k == key) {
            self.field = Some(field.clone());
            self.key = Some(key);
            return Ok(field.clone());
        }
        let camera = Camera {
            yaw_deg: settings.azimuth,
            elev_deg: settings.elevation,
            scale: 1.,
            world_center: V3::default(),
            paper_center: p(0., 0.),
            perspective: None,
        };
        let projection = project::Projection::new(&mesh, &|q| camera.project(q));
        let light = settings.light();
        let mut faces = Vec::new();
        let mut count = 0;
        for (i, n) in self.normals.iter().enumerate() {
            if cancel() {
                return Err(CANCEL.into());
            }
            let cosine = n.dot(light);
            let dark = cosine <= settings.threshold;
            let middle =
                settings.middle_tone && !dark && cosine < (settings.threshold + 0.28).min(0.95);
            let cast = if dark {
                vec![]
            } else {
                projection.shadowed(i, cancel)?
            };
            count += cast.len();
            if count > PIECE_LIMIT {
                return Err("Demasiadas regiones de sombra; reduce el faceteado.".into());
            }
            faces.push(ShadedFace {
                cosine,
                dark,
                middle,
                cast,
            });
        }
        let faces = Arc::new(faces);
        self.key = Some(key);
        self.field = Some(faces.clone());
        self.fields.push((key, faces.clone()));
        Ok(faces)
    }
}
fn project_polygon(poly: &[V3], project: &dyn Fn(V3) -> (P, f64)) -> project::Polygon {
    poly.iter()
        .map(|world| {
            let (point, depth) = project(*world);
            project::Vertex {
                world: *world,
                point,
                depth,
            }
        })
        .collect()
}
pub(super) fn draw(
    mesh: Arc<Mesh>,
    mut camera: Camera,
    settings: &Settings,
    paper: &crate::Paper,
    cache: &mut Cache,
    frame: Option<Camera>,
    cancel: &dyn Fn() -> bool,
) -> Result<(Vec<super::DrawingPart>, Camera, ShadowDrawing), String> {
    let config = &settings.shadow;
    let faces = cache.field(mesh.clone(), config, cancel)?;
    let points = mesh.points();
    let ground_z = mesh.bounds()[0].z - config.ground_gap;
    let light = config.light();
    let mut ground_world = Vec::new();
    let mut lo = mesh.bounds()[0];
    let mut hi = mesh.bounds()[1];
    if config.ground {
        for t in &mesh.triangles {
            let poly: Vec<_> = t
                .iter()
                .map(|i| {
                    let q = points[*i];
                    q - light * ((q.z - ground_z) / light.z)
                })
                .collect();
            for q in &poly {
                lo = V3::new(lo.x.min(q.x), lo.y.min(q.y), lo.z.min(q.z));
                hi = V3::new(hi.x.max(q.x), hi.y.max(q.y), hi.z.max(q.z));
            }
            ground_world.push(poly);
        }
    }
    let area = BBox {
        min: p(paper.margin_mm, paper.margin_mm),
        max: p(
            paper.width_mm - paper.margin_mm,
            paper.height_mm - paper.margin_mm,
        ),
    };
    camera.world_center = (lo + hi) * 0.5;
    let radius = (hi - lo).length() * 0.5;
    camera.scale = area.width().min(area.height()) / (2. * radius.max(1e-9)) * settings.zoom;
    super::continuity::configure_camera(&mut camera, settings, radius);
    if let Some(frame) = frame {
        camera = frame;
    }
    let project = |q: V3| {
        let (point, depth) = camera.project(q);
        (
            camera.paper_center
                + (point - camera.paper_center).rotate(settings.paper_rotation.to_radians()),
            depth,
        )
    };
    let projection = project::Projection::camera(&mesh, &camera, settings.paper_rotation);
    let mut dark = Vec::new();
    let mut middle = Vec::new();
    let mut object = Vec::new();
    for (i, face) in faces.iter().enumerate() {
        if cancel() {
            return Err(CANCEL.into());
        }
        if let Some(tri) = &projection.faces[i] {
            object.push(tri.polygon.iter().map(|v| v.point).collect());
        }
        let visible = projection.visible(i, cancel)?;
        if face.dark {
            dark.extend(
                visible
                    .iter()
                    .map(|poly| poly.iter().map(|v| v.point).collect::<Polyline>()),
            );
        } else {
            if face.middle {
                middle.extend(
                    visible
                        .iter()
                        .map(|poly| poly.iter().map(|v| v.point).collect::<Polyline>()),
                );
            }
            for patch in &face.cast {
                let region = project_polygon(patch, &project);
                let planes = project::boundary(&region);
                for poly in &visible {
                    let result = project::intersection(poly, &planes);
                    if !result.is_empty() {
                        dark.push(result.iter().map(|v| v.point).collect());
                    }
                }
            }
        }
        if dark.len() + middle.len() > PIECE_LIMIT {
            return Err("Demasiadas regiones de sombra; reduce el faceteado.".into());
        }
    }
    let floor: Vec<Polyline> = ground_world
        .iter()
        .map(|poly| poly.iter().map(|q| project(*q).0).collect())
        .collect();
    let [shadows, mid] = mask::hatch(
        &dark,
        &middle,
        &floor,
        &object,
        area,
        config.hatch_angle,
        config.hatch_spacing_mm,
        cancel,
    )?;
    let mut parts = vec![
        super::DrawingPart {
            families: vec![],
            ramp: None,
            pen: 0,
            name: "Sombra única · objeto y suelo".into(),
            lines: shadows,
        },
        super::DrawingPart {
            families: vec![],
            ramp: None,
            pen: 1,
            name: "Tono intermedio".into(),
            lines: mid,
        },
        super::DrawingPart {
            families: vec![],
            ramp: None,
            pen: 2,
            name: "Contorno opcional".into(),
            lines: vec![],
        },
    ];
    if config.outline {
        let mut render_camera = camera;
        render_camera.scale /= 1000.;
        render_camera.world_center = render_camera.world_center * 1000.;
        render_camera.perspective = render_camera
            .perspective
            .map(|[d, n]| [d * 1000., n * 1000.]);
        let rotate = |q: V3| {
            let center = camera.world_center;
            let q = q - center;
            let axis = camera.axes().2;
            let (s, c) = settings.paper_rotation.to_radians().sin_cos();
            (center + q * c + axis.cross(q) * s + axis * (axis.dot(q) * (1. - c))) * 1000.
        };
        let mut scene = crate::scene::Scene::new(render_camera, area);
        scene.set_intersections(false);
        scene.set_hidden_lines(settings.hidden_lines);
        scene.set_line_halo(settings.halo_mm);
        for t in &mesh.triangles {
            scene.occluder(&t.map(|i| rotate(points[i])));
        }
        for [a, b] in super::feature_edges(&mesh, &camera, 180.) {
            scene.line(&[rotate(a), rotate(b)]);
        }
        parts[2].lines = scene
            .render_cancellable(cancel)
            .ok_or(CANCEL)?
            .parts
            .remove(0)
            .lines;
    }
    Ok((
        parts,
        camera,
        ShadowDrawing {
            floor,
            object,
            dark,
            middle,
            ground_world,
            faces,
            world_bounds: [lo, hi],
        },
    ))
}

/// Trama independiente del suelo, con la fase y el recorte físicos originales.
pub(super) fn ground_lines(
    d: &ShadowDrawing,
    s: &Settings,
    paper: &crate::Paper,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Polyline>, String> {
    let area = BBox {
        min: p(paper.margin_mm, paper.margin_mm),
        max: p(
            paper.width_mm - paper.margin_mm,
            paper.height_mm - paper.margin_mm,
        ),
    };
    let [ground, _] = mask::hatch(
        &[],
        &[],
        &d.floor,
        &d.object,
        area,
        s.shadow.hatch_angle,
        s.shadow.hatch_spacing_mm,
        cancel,
    )?;
    Ok(ground)
}

fn light_key(s: &ShadowSettings) -> LightKey {
    (s.azimuth, s.elevation, s.threshold, s.middle_tone)
}
impl Cache {
    pub(super) fn retain_programs(&mut self, programs: &[Settings]) {
        if self.fields.len() > programs.len() + 16 {
            self.fields
                .retain(|(key, _)| programs.iter().any(|s| *key == light_key(&s.shadow)));
        }
    }
}
