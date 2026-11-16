//! Caché geométrica sin color y adaptación al documento y al motor de visibilidad.
use super::*;
use crate::{p, BBox};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Default)]
pub(super) struct Pipeline {
    source: Option<Arc<Mesh>>,
    mesh: Option<Arc<Mesh>>,
    validated: Option<Arc<Mesh>>,
    reduced: Option<Arc<Mesh>>,
    detail: Option<MeshDetail>,
    display_mesh: Option<Arc<Mesh>>,
    model_key: Option<([f64; 3], [f64; 3], Option<usize>)>,
    shadows: super::shadow::Cache,
    art: super::art::Cache,
    section_key: Option<Settings>,
    sections: Vec<(Settings, Arc<Vec<Contour>>)>,
    curves: Option<Arc<Vec<Contour>>>,
}
impl Pipeline {
    pub fn draw(
        &mut self,
        source: Arc<Mesh>,
        s: &Settings,
        paper: &crate::Paper,
        frame: Option<Camera>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Drawing, String> {
        let start = std::time::Instant::now();
        s.validate()?;
        if ![paper.width_mm, paper.height_mm, paper.margin_mm]
            .iter()
            .all(|x| x.is_finite())
            || paper.width_mm < 10.
            || paper.height_mm < 10.
            || paper.width_mm > 2000.
            || paper.height_mm > 2000.
            || paper.margin_mm < 0.
            || paper.margin_mm * 2. >= paper.width_mm.min(paper.height_mm)
        {
            return Err("Papel o márgenes no válidos.".into());
        }
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let changed = self
            .source
            .as_ref()
            .is_none_or(|m| !Arc::ptr_eq(m, &source));
        if changed {
            self.validated = Some(Arc::new(
                source.as_ref().clone().validated_cancellable(cancel)?.0,
            ));
            self.source = Some(source);
            self.detail = None;
        }
        let detail_changed = self.detail != Some(s.mesh_detail);
        if detail_changed {
            let original = self.validated.as_ref().unwrap();
            let reduced = if original.triangles.len() <= s.mesh_detail.target() {
                original.clone()
            } else {
                Arc::new(super::reduce::reduced(
                    original,
                    s.mesh_detail.target(),
                    s.mesh_detail.error(),
                    cancel,
                )?)
            };
            self.reduced = Some(reduced);
            self.detail = Some(s.mesh_detail);
            self.model_key = None;
        }
        let key = (
            s.model_scale,
            s.model_rotation,
            if s.recipe == RecipeKind::FlatShadow {
                Some(s.shadow.facets)
            } else if s.recipe.is_art() {
                Some(s.art.facets)
            } else {
                None
            },
        );
        if self.model_key != Some(key) {
            let source = self.reduced.as_ref().unwrap();
            let faceted = if let Some(target) = key.2 {
                Some(super::reduce::reduced(source, target, 0.04, cancel)?)
            } else {
                None
            };
            let source = faceted.as_ref().unwrap_or(source.as_ref());
            let transformed = Arc::new(source.transformed(s.model_scale, s.model_rotation));
            let display =
                if s.recipe == RecipeKind::FlatShadow || transformed.triangles.len() <= 20_000 {
                    transformed.clone()
                } else {
                    Arc::new(super::reduce::reduced(&transformed, 20_000, 0.003, cancel)?)
                };
            self.mesh = Some(transformed);
            self.display_mesh = Some(display);
            self.model_key = Some(key);
            self.section_key = None;
            self.sections.clear();
        }
        let mesh = self.mesh.as_ref().unwrap().clone();
        let mut camera = Camera {
            yaw_deg: s.yaw,
            elev_deg: s.elevation,
            ..Default::default()
        };
        let sheet = BBox {
            min: p(paper.margin_mm, paper.margin_mm),
            max: p(
                paper.width_mm - paper.margin_mm,
                paper.height_mm - paper.margin_mm,
            ),
        };
        // Ajustar en un círculo inscrito también deja sitio para girar el dibujo.
        let [lo, hi] = mesh.bounds();
        let radius = (hi - lo).length() * 0.5;
        camera.world_center = (lo + hi) * 0.5;
        camera.paper_center = p(
            paper.width_mm * 0.5 + s.position_mm[0],
            paper.height_mm * 0.5 + s.position_mm[1],
        );
        camera.scale = sheet.width().min(sheet.height()) / (2. * radius.max(1e-9)) * s.zoom;
        super::continuity::configure_camera(&mut camera, s, radius);
        if let Some(frame) = frame {
            camera = frame;
        }
        if s.recipe == RecipeKind::FlatShadow {
            let (parts, camera, shadow) = super::shadow::draw(
                mesh.clone(),
                camera,
                s,
                paper,
                &mut self.shadows,
                frame,
                cancel,
            )?;
            return Ok(Drawing {
                parts,
                curves: Arc::new(vec![]),
                mesh,
                display_mesh: self.display_mesh.as_ref().unwrap().clone(),
                camera,
                seconds: start.elapsed().as_secs_f64(),
                shadow: Some(Arc::new(shadow)),
                art: None,
            });
        }
        if s.recipe.is_art() {
            let (parts, curves, art) = self.art.draw(mesh.clone(), s, camera, sheet, cancel)?;
            return Ok(Drawing {
                parts,
                curves,
                mesh,
                display_mesh: self.display_mesh.as_ref().unwrap().clone(),
                camera,
                seconds: start.elapsed().as_secs_f64(),
                shadow: None,
                art: Some(Arc::new(art)),
            });
        }
        let same_key = |k: &Settings| section_key_eq(k, s);
        let same = self.section_key.as_ref().is_some_and(same_key);
        if !same {
            if let Some((_, curves)) = self.sections.iter().find(|(k, _)| same_key(k)) {
                self.curves = Some(curves.clone());
                self.section_key = Some(s.clone());
            }
        }
        let same = self.section_key.as_ref().is_some_and(same_key);
        if !same {
            let values = field_values(&mesh, s.field, v3(s.direction), &camera);
            let min = values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let (a, b) = (
                min + (max - min) * s.range[0],
                min + (max - min) * s.range[1],
            );
            let step = if s.spacing > 0. {
                s.spacing
            } else {
                (b - a) / (s.cuts as f64 + 1.)
            };
            let levels = if step > 1e-10 {
                let count = if s.spacing > 0. {
                    ((b - a) / step).ceil() as usize
                } else {
                    s.cuts
                };
                if count > 160 {
                    return Err(
                        "El espaciado produciría más de 160 cortes; aumenta la separación.".into(),
                    );
                }
                (0..count)
                    .map(|i| a + step * (i as f64 + 1. + s.offset))
                    .filter(|v| *v > a && *v < b)
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };
            self.curves = Some(Arc::new(isolines(&mesh, &values, &levels, cancel)?));
            self.section_key = Some(s.clone());
            self.sections
                .push((s.clone(), self.curves.as_ref().unwrap().clone()));
        }
        let curves = self.curves.as_ref().unwrap().clone();
        // Elevar la escala del mundo conserva la tolerancia heredada del motor
        // arquitectónico por debajo de una micra del modelo normalizado.
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
            let (sin, cos) = s.paper_rotation.to_radians().sin_cos();
            center + q * cos + axis.cross(q) * sin + axis * (axis.dot(q) * (1. - cos))
        };
        let mut scene = crate::scene::Scene::new(render_camera, sheet);
        scene.set_intersections(false);
        scene.set_hidden_lines(s.hidden_lines);
        scene.set_line_halo(s.halo_mm);
        let main = scene.part("Curvas principales");
        let secondary = scene.part("Curvas secundarias");
        let structure = scene.part("Estructura");
        if s.visible_only && (s.recipe != RecipeKind::Strata || s.hidden_lines) {
            let pts = mesh.points();
            for (i, t) in mesh.triangles.iter().enumerate() {
                if i % 128 == 0 && cancel() {
                    return Err("Cálculo cancelado.".into());
                }
                let mut t = *t;
                t.sort();
                scene.occluder(&t.map(|i| rotate(pts[i]) * 1000.));
            }
        }
        if s.recipe != RecipeKind::XRay {
            for curve in curves.iter() {
                if cancel() {
                    return Err("Cálculo cancelado.".into());
                }
                scene.set_part(if curve.level % s.major_every == 0 {
                    main
                } else {
                    secondary
                });
                let shift = if s.recipe == RecipeKind::Strata {
                    (curve.level as f64 - s.cuts as f64 * 0.5) * s.strata_shift_mm / camera.scale
                } else {
                    0.
                };
                let right = camera.axes().0;
                let pts: Vec<_> = curve
                    .points
                    .iter()
                    .map(|q| (rotate(*q) + right * shift) * 1000.)
                    .collect();
                scene.line(&pts);
            }
        } else {
            scene.set_part(structure);
            for [a, b] in feature_edges(&mesh, &camera, s.edge_angle) {
                scene.line(&[rotate(a) * 1000., rotate(b) * 1000.]);
            }
        }
        let result = scene
            .render_cancellable(cancel)
            .ok_or("Cálculo cancelado.")?;
        let parts = result
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
        Ok(Drawing {
            parts,
            curves,
            mesh,
            display_mesh: self.display_mesh.as_ref().unwrap().clone(),
            camera,
            seconds: start.elapsed().as_secs_f64(),
            shadow: None,
            art: None,
        })
    }
}
/// Aristas de frontera, de silueta o pliegue; omite diagonales coplanarias.
pub fn feature_edges(mesh: &Mesh, camera: &Camera, angle: f64) -> Vec<[V3; 2]> {
    let p = mesh.points();
    let mut weld = super::contours::Welder::new(1e-7);
    let ids: Vec<_> = p.iter().map(|q| weld.insert(*q)).collect();
    let mut edges = BTreeMap::<(usize, usize), Vec<V3>>::new();
    for t in &mesh.triangles {
        let n = (p[t[1]] - p[t[0]]).cross(p[t[2]] - p[t[0]]).normalized();
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (a, b) = (ids[a], ids[b]);
            edges.entry((a.min(b), a.max(b))).or_default().push(n);
        }
    }
    let threshold = angle.to_radians().cos();
    edges
        .into_iter()
        .filter(|((a, _), n)| {
            n.len() != 2
                || camera.facing_at(n[0], weld.points[*a])
                    != camera.facing_at(n[1], weld.points[*a])
                || n[0].dot(n[1]).abs() < threshold
        })
        .map(|((a, b), _)| [weld.points[a], weld.points[b]])
        .collect()
}

fn section_key_eq(a: &Settings, b: &Settings) -> bool {
    a.field == b.field
        && a.direction == b.direction
        && a.cuts == b.cuts
        && a.spacing == b.spacing
        && a.offset == b.offset
        && a.range == b.range
        && (b.field != Field::CameraDepth || (a.yaw == b.yaw && a.elevation == b.elevation))
}
impl Pipeline {
    pub(super) fn retain_programs(&mut self, programs: &[Settings]) {
        if self.sections.len() > programs.len() + 16 {
            self.sections
                .retain(|(key, _)| programs.iter().any(|s| section_key_eq(key, s)));
        }
        self.art.retain_programs(programs);
        self.shadows.retain_programs(programs);
    }
}
