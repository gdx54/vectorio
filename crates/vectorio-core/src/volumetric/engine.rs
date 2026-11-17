//! Orquestación de pasadas y caché de salidas sin nombres ni asignación de tintas.
//! Los adaptadores de `passes/` conservan los algoritmos anteriores de proyección.
use super::*;
use crate::{p, Document, Geometry, Item, Polyline};
use std::sync::Arc;
#[derive(Clone, Debug)]
pub struct InkFamily {
    pub slot: usize,
    pub pen: usize,
    pub lines: Vec<Polyline>,
}
#[derive(Clone, Debug)]
pub struct DrawingPart {
    pub pen: usize,
    pub name: String,
    pub lines: Vec<Polyline>,
    pub families: Vec<InkFamily>,
    pub ramp: Option<Ramp>,
}
impl DrawingPart {
    pub fn inks(&self) -> impl Iterator<Item = (usize, &[Polyline])> {
        self.families
            .is_empty()
            .then_some((self.pen, self.lines.as_slice()))
            .into_iter()
            .chain(self.families.iter().map(|f| (f.pen, f.lines.as_slice())))
    }
    pub fn colors<'a>(
        &'a self,
        pass: &Pass,
        palette: &[crate::Pen],
    ) -> Vec<(usize, &'a [Polyline])> {
        if let Some(map) = &pass.reading {
            let pens = map.ordered_pens(palette);
            self.families
                .iter()
                .map(|f| {
                    (
                        pens.get(f.slot).copied().unwrap_or(pass.pen),
                        f.lines.as_slice(),
                    )
                })
                .collect()
        } else {
            vec![(pass.pen, self.lines.as_slice())]
        }
    }
    pub fn recolor(&mut self, pass: &Pass, palette: &[crate::Pen]) {
        self.ramp = pass
            .reading
            .as_ref()
            .filter(|m| m.encoding != Encoding::Optical)
            .map(|map| map.ramp);
        self.pen = pass.pen;
        self.name = pass.name.clone();
        if let Some(map) = &pass.reading {
            let pens = map.ordered_pens(palette);
            for family in &mut self.families {
                family.pen = pens.get(family.slot).copied().unwrap_or(pass.pen);
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct Drawing {
    pub parts: Vec<DrawingPart>,
    pub curves: Arc<Vec<Contour>>,
    pub mesh: Arc<Mesh>,
    pub display_mesh: Arc<Mesh>,
    pub camera: Camera,
    pub seconds: f64,
    pub shadow: Option<Arc<ShadowDrawing>>,
    pub art: Option<Arc<ArtDrawing>>,
}
impl Drawing {
    pub fn items(&self, pens: impl AsRef<[usize]>) -> Vec<Item> {
        self.parts
            .iter()
            .flat_map(|part| {
                part.inks()
                    .filter(|(_, lines)| !lines.is_empty())
                    .map(|(pen, lines)| {
                        let mut item = Item::new(
                            format!("VOLUMETRIC · {}", part.name),
                            Geometry::Path {
                                lines: lines.to_vec(),
                                closed: false,
                            },
                            p(0., 0.),
                        );
                        item.stroke = Some(pens.as_ref().get(pen).copied().unwrap_or(pen));
                        item
                    })
            })
            .collect()
    }
    pub fn recolor(&mut self, passes: &[Pass], palette: &[crate::Pen]) {
        for (part, pass) in self.parts.iter_mut().zip(passes) {
            part.recolor(pass, palette);
        }
    }
    pub fn document(
        &self,
        paper: crate::Paper,
        pens: Vec<crate::Pen>,
        assign: impl AsRef<[usize]>,
    ) -> Document {
        let mut doc = Document {
            paper,
            pens,
            ..Default::default()
        };
        let mut colored = std::borrow::Cow::Borrowed(self);
        if self.parts.iter().any(|p| p.ramp == Some(Ramp::Sequential)) {
            for part in &mut colored.to_mut().parts {
                if part.ramp == Some(Ramp::Sequential) {
                    let mut pens = part
                        .families
                        .iter()
                        .filter(|f| f.slot != usize::MAX)
                        .map(|f| f.pen)
                        .collect::<Vec<_>>();
                    pens.sort_by(|a, b| {
                        doc.pens
                            .get(*a)
                            .map_or(0., |p| lightness(p.color))
                            .total_cmp(&doc.pens.get(*b).map_or(0., |p| lightness(p.color)))
                            .then(a.cmp(b))
                    });
                    for family in &mut part.families {
                        family.pen = pens.get(family.slot).copied().unwrap_or(part.pen);
                    }
                }
            }
        }
        for item in colored.items(assign) {
            doc.add(item);
        }
        doc
    }
}

struct Cached {
    settings: Settings,
    paper: crate::Paper,
    drawing: Arc<Drawing>,
    ground_only: bool,
    frame: Option<Camera>,
    context_only: bool,
}
#[derive(Clone, Default)]
struct Prepared {
    validated: Option<Arc<Mesh>>,
    details: Vec<(MeshDetail, Arc<Mesh>)>,
    facets: Vec<(MeshDetail, usize, Arc<Mesh>)>,
    #[cfg(test)]
    facet_builds: usize,
    #[cfg(test)]
    validations: usize,
    #[cfg(test)]
    reductions: usize,
}

#[cfg(test)]
mod preparation_tests {
    use super::*;
    #[test]
    fn four_views_share_facet_reductions_and_rotation_does_not_repeat_them() {
        let mesh = Arc::new(Primitive::Torus.mesh());
        let mut scene = Settings {
            sheet: SheetViews::Orthographic,
            mesh_detail: MeshDetail::Original,
            ..Default::default()
        };
        let mut passes = ReadingStyle::Thermal.passes(&scene);
        if let PassKind::Reading(p) = &mut passes[0].kind {
            p.facets = 128;
        }
        let mut other = passes[0].clone();
        if let PassKind::Reading(p) = &mut other.kind {
            p.facets = 256;
        }
        passes.push(other);
        let mut engine = Engine::default();
        let paper = crate::Paper::default();
        let first = engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        assert!(!first.parts[0].lines.is_empty());
        assert_eq!(engine.prepared.validations, 1);
        assert_eq!(engine.prepared.facet_builds, 2);
        assert_eq!(engine.views.len(), 4);
        for view in &engine.views {
            for (root, child) in engine.prepared.facets.iter().zip(&view.prepared.facets) {
                assert!(Arc::ptr_eq(&root.2, &child.2));
            }
        }
        scene.model_rotation[2] = 17.;
        engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_eq!(engine.prepared.facet_builds, 2);
        scene.sheet = SheetViews::Single;
        engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_eq!(engine.prepared.facet_builds, 2);
        scene.mesh_detail = MeshDetail::Detailed;
        engine
            .draw_passes(mesh, &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_eq!(engine.prepared.facet_builds, 4);
        scene.sheet = SheetViews::Orthographic;
        engine
            .draw_passes(
                Arc::new(Primitive::Cube.mesh()),
                &scene,
                &passes,
                &paper,
                &|| false,
            )
            .unwrap();
        assert_eq!(engine.prepared.validations, 1);
        assert_eq!(engine.prepared.facet_builds, 2);
        for view in &engine.views {
            assert!(Arc::ptr_eq(
                view.prepared.validated.as_ref().unwrap(),
                engine.prepared.validated.as_ref().unwrap()
            ));
        }
    }
    #[test]
    fn cancelled_facet_preparation_is_retried_and_the_cache_is_bounded() {
        let mesh = Arc::new(Primitive::Torus.mesh());
        let mut prepared = Prepared::default();
        prepared
            .mesh(&mesh, MeshDetail::Original, &|| false)
            .unwrap();
        assert!(prepared
            .mesh_for(&mesh, MeshDetail::Original, Some(128), &|| true)
            .is_err());
        assert!(prepared.facets.is_empty());
        let a = prepared
            .mesh_for(&mesh, MeshDetail::Original, Some(128), &|| false)
            .unwrap();
        let b = prepared
            .mesh_for(&mesh, MeshDetail::Original, Some(128), &|| false)
            .unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        for target in 129..150 {
            prepared
                .mesh_for(&mesh, MeshDetail::Original, Some(target), &|| false)
                .unwrap();
        }
        assert_eq!(prepared.facets.len(), 16);
        assert_eq!(prepared.validations, 1);
    }
    fn large_plane() -> Arc<Mesh> {
        let n = 151;
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for y in 0..n {
            for x in 0..n {
                vertices.push([x as f64, y as f64, (x as f64 * 0.02).sin()]);
            }
        }
        for y in 0..n - 1 {
            for x in 0..n - 1 {
                let i = y * n + x;
                triangles.extend([[i, i + 1, i + n], [i + 1, i + n + 1, i + n]]);
            }
        }
        Arc::new(Mesh {
            name: "Large grid".into(),
            vertices,
            triangles,
            objects: vec![],
            face_objects: vec![],
        })
    }
    #[test]
    fn working_source_is_prepared_once_across_facets_orbit_and_ink_edits() {
        let mesh = large_plane();
        let mut engine = Engine::default();
        let mut scene = Settings::default();
        let mut passes = ReadingStyle::Thermal.passes(&scene);
        if let PassKind::Reading(p) = &mut passes[0].kind {
            p.facets = 64;
        }
        let mut second = passes[0].clone();
        if let PassKind::Reading(p) = &mut second.kind {
            p.facets = 128;
        }
        passes.push(second);
        let paper = crate::Paper::default();
        engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_eq!(engine.prepared.validations, 1);
        assert_eq!(engine.prepared.reductions, 1);
        scene.yaw += 15.;
        engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        passes[0].pen = 1;
        passes[0].reading.as_mut().unwrap().pens.reverse();
        engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_eq!(engine.prepared.validations, 1);
        assert_eq!(engine.prepared.reductions, 1);
        scene.mesh_detail = MeshDetail::Original;
        engine
            .draw_passes(mesh.clone(), &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_eq!(engine.prepared.validations, 1);
        assert_eq!(engine.prepared.reductions, 1);
        scene.mesh_detail = MeshDetail::Interactive;
        engine
            .draw_passes(mesh, &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_eq!(engine.prepared.reductions, 1);
        let replacement = Arc::new(Primitive::Cube.mesh());
        engine
            .draw_passes(replacement, &scene, &passes, &paper, &|| false)
            .unwrap();
        assert_eq!(engine.prepared.validations, 1);
        assert_eq!(engine.prepared.reductions, 0);
    }
    #[test]
    fn cancellation_does_not_store_an_invalid_source_or_reduction() {
        let mesh = large_plane();
        let mut prepared = Prepared::default();
        assert!(prepared
            .mesh(&mesh, MeshDetail::Interactive, &|| true)
            .is_err());
        assert!(prepared.validated.is_none());
        assert!(prepared.details.is_empty());
        let a = prepared
            .mesh(&mesh, MeshDetail::Interactive, &|| false)
            .unwrap();
        let b = prepared
            .mesh(&mesh, MeshDetail::Interactive, &|| false)
            .unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(prepared.validations, 1);
        assert_eq!(prepared.reductions, 1);
    }
}
impl Prepared {
    fn mesh_for(
        &mut self,
        source: &Arc<Mesh>,
        detail: MeshDetail,
        facets: Option<usize>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Arc<Mesh>, String> {
        if let Some(target) = facets {
            if let Some((_, _, mesh)) = self
                .facets
                .iter()
                .find(|(d, n, _)| *d == detail && *n == target)
            {
                return Ok(mesh.clone());
            }
            let base = self.mesh(source, detail, cancel)?;
            let mesh = Arc::new(super::reduce::reduced(&base, target, 0.04, cancel)?);
            if self.facets.len() >= 16 {
                self.facets.remove(0);
            }
            self.facets.push((detail, target, mesh.clone()));
            #[cfg(test)]
            {
                self.facet_builds += 1;
            }
            Ok(mesh)
        } else {
            self.mesh(source, detail, cancel)
        }
    }
    fn mesh(
        &mut self,
        source: &Arc<Mesh>,
        detail: MeshDetail,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Arc<Mesh>, String> {
        if self.validated.is_none() {
            self.validated = Some(Arc::new(
                source.as_ref().clone().validated_cancellable(cancel)?.0,
            ));
            #[cfg(test)]
            {
                self.validations += 1;
            }
        }
        if let Some((_, mesh)) = self.details.iter().find(|(d, _)| *d == detail) {
            return Ok(mesh.clone());
        }
        let original = self.validated.as_ref().unwrap();
        let mesh = if original.triangles.len() <= detail.target() {
            original.clone()
        } else {
            let mesh = Arc::new(super::reduce::reduced(
                original,
                detail.target(),
                detail.error(),
                cancel,
            )?);
            #[cfg(test)]
            {
                self.reductions += 1;
            }
            mesh
        };
        // Solo tres calidades, compartidas por todas las pasadas de este modelo.
        self.details.push((detail, mesh.clone()));
        Ok(mesh)
    }
}
struct EffectEntry {
    pass: Pass,
    settings: Settings,
    light: (f64, f64, f64),
    paper: crate::Paper,
    camera: Camera,
    lines: Arc<Vec<Polyline>>,
}
#[derive(Default)]
pub struct Engine {
    pub(super) views: Vec<Engine>,
    pipelines: Vec<(Option<usize>, super::pipeline::Pipeline)>,
    source: Option<Arc<Mesh>>,
    prepared: Prepared,
    cache: Vec<Cached>,
    effects: Vec<EffectEntry>,
    tones: super::shadow::tone::ToneCache,
    pub(super) readings: super::reading::Cache,
    pub(super) decorations: super::finishing::Cache,
    #[cfg(test)]
    pub(super) finishes: usize,
    #[cfg(test)]
    pub(super) renders: usize,
}
impl Engine {
    fn reset_source(&mut self, source: &Arc<Mesh>) {
        if self.source.as_ref().is_none_or(|m| !Arc::ptr_eq(m, source)) {
            self.cache.clear();
            self.effects.clear();
            self.tones = Default::default();
            self.readings.clear();
            self.decorations.clear();
            self.pipelines.clear();
            self.prepared = Prepared::default();
            self.views.clear();
            self.source = Some(source.clone());
        }
    }
    pub(super) fn prepare_views(
        &mut self,
        source: &Arc<Mesh>,
        scene: &Settings,
        passes: &[Pass],
        cancel: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        // Source and facet reductions are camera-independent. Arc snapshots let
        // child engines reuse them without locks or retaining failed work.
        self.prepared.mesh(source, scene.mesh_detail, cancel)?;
        for pass in passes.iter().filter(|p| p.enabled) {
            let settings = super::passes::program(scene, pass).0;
            self.prepared
                .mesh_for(source, settings.mesh_detail, facet_key(&settings), cancel)?;
        }
        for view in &mut self.views {
            view.reset_source(source);
            view.prepared = self.prepared.clone();
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn tones_builds(&self) -> usize {
        self.tones.builds
    }
    /// Adaptador para la interfaz anterior: sus siete modos son pilas de pasadas.
    pub fn draw(
        &mut self,
        source: Arc<Mesh>,
        settings: &Settings,
        paper: &crate::Paper,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Drawing, String> {
        let mut drawing = self.draw_passes(source, settings, &settings.passes(), paper, cancel)?;
        // La vista 3D histórica usa índices de familia, con las ondas en el papel 0.
        if settings.recipe == RecipeKind::Hybrid {
            if let Some(art) = &mut drawing.art {
                let art = Arc::make_mut(art);
                for edge in Arc::make_mut(&mut art.edges) {
                    edge.role += 1;
                }
            }
        }
        Ok(drawing)
    }
    pub fn draw_recipe(
        &mut self,
        recipe: &Recipe,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Drawing, String> {
        let source = self
            .source
            .as_ref()
            .filter(|m| ***m == recipe.mesh)
            .cloned()
            .unwrap_or_else(|| Arc::new(recipe.mesh.clone()));
        let mut passes = recipe.passes.clone();
        if !recipe.inks.is_empty() {
            for pass in &mut passes {
                if let Some(map) = &mut pass.reading {
                    map.bind_palette(&recipe.inks);
                }
            }
        }
        let mut drawing =
            self.draw_passes(source, &recipe.settings, &passes, &recipe.paper, cancel)?;
        drawing.recolor(&recipe.passes, &recipe.inks);
        Ok(drawing)
    }
    /// Cada pasada puede usar cualquier tinta y repetirse, desactivarse o reordenarse.
    pub fn draw_passes(
        &mut self,
        source: Arc<Mesh>,
        scene: &Settings,
        passes: &[Pass],
        paper: &crate::Paper,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Drawing, String> {
        scene.validate()?;
        for pass in passes {
            pass.validate()?;
        }
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        self.reset_source(&source);
        if scene.sheet != SheetViews::Single {
            return self.draw_sheet(source, scene, passes, paper, cancel);
        }
        let start = std::time::Instant::now();
        let full_paper = paper;
        let content_paper = reading_content_paper(paper, passes.iter());
        let paper = &content_paper;
        let mut legend_row = 0;

        if self.effects.len() > passes.len() + 32 {
            self.effects.clear();
        }
        let active: Vec<_> = passes
            .iter()
            .filter(|p| p.enabled)
            .map(|p| {
                (
                    super::passes::program(scene, p).0,
                    matches!(p.kind, PassKind::Ground(_)),
                    matches!(p.kind, PassKind::Reading(_)),
                )
            })
            .collect();
        if self.cache.len() > active.len() + 32 {
            self.cache.retain(|c| {
                c.paper == *paper
                    && active.iter().any(|(s, ground, context)| {
                        *s == c.settings && *ground == c.ground_only && *context == c.context_only
                    })
            });
        }
        let programs: Vec<_> = active.iter().map(|(s, _, _)| s.clone()).collect();
        let facets: Vec<_> = programs.iter().map(facet_key).collect();
        if self.pipelines.len() > facets.len() + 2 {
            self.pipelines.retain(|(key, _)| facets.contains(key));
        }
        for (_, pipeline) in &mut self.pipelines {
            pipeline.retain_programs(&programs);
        }
        // El encuadre compartido se elige por parámetros, nunca por el orden de
        // las pasadas. La sombra incluye el suelo; el resto conserva el marco del objeto.
        let frame_program = active.iter().min_by_key(|(s, ground, _)| {
            (
                usize::from(s.recipe != RecipeKind::FlatShadow),
                facet_key(s).unwrap_or(usize::MAX),
                serde_json::to_string(&s.shadow).unwrap_or_default(),
                *ground,
            )
        });
        let frame = if let Some((s, ground, context)) = frame_program {
            Some(
                self.render(source.clone(), s, paper, *ground, None, *context, cancel)?
                    .camera,
            )
        } else {
            None
        };
        self.cache.retain(|c| c.frame.is_none() || c.frame == frame);
        let mut result: Option<Drawing> = None;
        let mut parts = Vec::new();
        for pass in passes {
            if !pass.enabled {
                if pass.reading.as_ref().is_some_and(|m| m.legend) {
                    legend_row += 1;
                }
                parts.push(DrawingPart {
                    pen: pass.pen,
                    name: pass.name.clone(),
                    lines: vec![],
                    families: vec![],
                    ramp: None,
                });
                continue;
            }
            if cancel() {
                return Err("Cálculo cancelado.".into());
            }
            let (settings, role) = super::passes::program(scene, pass);
            let ground = matches!(pass.kind, PassKind::Ground(_));
            let context = matches!(pass.kind, PassKind::Reading(_));
            let mut drawing = self.render(
                source.clone(),
                &settings,
                paper,
                ground,
                None,
                context,
                cancel,
            )?;
            if frame.is_some_and(|frame| frame != drawing.camera) {
                drawing = self.render(
                    source.clone(),
                    &settings,
                    paper,
                    ground,
                    frame,
                    context,
                    cancel,
                )?;
            }
            let mut part = drawing
                .parts
                .get(role)
                .ok_or("Familia de pasada no válida.")?
                .clone();
            if !context
                && (pass.mask != TonalMask::All
                    || pass.visibility == Visibility::Hidden
                    || matches!(pass.kind, PassKind::ToneHatch(_)))
            {
                part.lines =
                    self.finish(&drawing, &part.lines, pass, scene, &settings, paper, cancel)?;
            }
            if let Some(map) = &pass.reading {
                let families = self.readings.process(
                    &drawing,
                    &part.lines,
                    pass,
                    scene,
                    paper,
                    full_paper,
                    legend_row,
                    &mut self.tones,
                    cancel,
                )?;
                part.families = families
                    .into_iter()
                    .enumerate()
                    .map(|(slot, lines)| InkFamily {
                        slot,
                        pen: map.pens[slot],
                        lines,
                    })
                    .collect();
                part.lines = part.families.iter().flat_map(|f| f.lines.clone()).collect();
                if map.legend {
                    legend_row += 1;
                }
                part.recolor(pass, &crate::doc::default_pens());
            }
            self.decorations
                .apply(&mut part, pass, scene.seed, paper, cancel)?;
            part.recolor(pass, &crate::doc::default_pens());
            part.name = pass.name.clone();
            part.pen = pass.pen;
            parts.push(part);
            if let Some(output) = &mut result {
                if output.curves.is_empty() && !drawing.curves.is_empty() {
                    output.curves = drawing.curves.clone();
                }
                if drawing.shadow.is_some() {
                    output.shadow = drawing.shadow.clone();
                }
                if drawing.art.as_ref().is_some_and(|a| !a.edges.is_empty()) || output.art.is_none()
                {
                    output.art = drawing.art.clone();
                }
            } else {
                result = Some(drawing.as_ref().clone());
            }
        }
        // También valida papel y prepara la vista 3D con una pila vacía.
        let mut result = match result {
            Some(drawing) => drawing,
            None => {
                let mut settings = scene.clone();
                settings.recipe = RecipeKind::XRay;
                self.render(source, &settings, paper, false, None, false, cancel)?
                    .as_ref()
                    .clone()
            }
        };
        result.parts = parts;
        result.seconds = start.elapsed().as_secs_f64();
        Ok(result)
    }
    fn finish(
        &mut self,
        drawing: &Drawing,
        lines: &[Polyline],
        pass: &Pass,
        scene: &Settings,
        settings: &Settings,
        paper: &crate::Paper,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Vec<Polyline>, String> {
        let mut key = pass.clone();
        key.name.clear();
        key.pen = 0;
        key.enabled = true;
        key.finish = FinishSettings {
            halo_mm: key.finish.halo_mm,
            ..Default::default()
        };
        if let Some(map) = &mut key.reading {
            map.pens = (0..map.pens.len()).collect();
        }
        let light = (
            scene.shadow.azimuth,
            scene.shadow.elevation,
            scene.shadow.threshold,
        );
        if let Some(e) = self.effects.iter().find(|e| {
            e.pass == key
                && e.settings == *settings
                && e.paper == *paper
                && e.camera == drawing.camera
                && e.light == light
        }) {
            return Ok(e.lines.as_ref().clone());
        }
        let mut tonal_scene = scene.clone();
        tonal_scene.paper_rotation = settings.paper_rotation;
        let mut lines = if pass.visibility == Visibility::Hidden
            && matches!(
                pass.kind,
                PassKind::Hatch { .. } | PassKind::Ground(_) | PassKind::ToneHatch(_)
            ) {
            Vec::new() // Las tramas sobre regiones visibles no tienen geometría trasera.
        } else if let PassKind::ToneHatch(params) = &pass.kind {
            self.tones
                .hatch(drawing, params, &tonal_scene, paper, cancel)?
        } else {
            lines.to_vec()
        };
        lines = self
            .tones
            .apply(drawing, &lines, pass.mask, &tonal_scene, cancel)?;
        if pass.visibility == Visibility::Hidden {
            lines = super::shadow::tone::dashed(&lines, pass.dash, cancel)?;
        }
        #[cfg(test)]
        {
            self.finishes += 1;
        }
        self.effects.push(EffectEntry {
            pass: key,
            settings: settings.clone(),
            light,
            paper: paper.clone(),
            camera: drawing.camera,
            lines: Arc::new(lines.clone()),
        });
        Ok(lines)
    }
    fn render(
        &mut self,
        source: Arc<Mesh>,
        settings: &Settings,
        paper: &crate::Paper,
        ground_only: bool,
        frame: Option<Camera>,
        context_only: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Arc<Drawing>, String> {
        if let Some(i) = self.cache.iter().position(|c| {
            c.settings == *settings
                && c.paper == *paper
                && c.ground_only == ground_only
                && c.frame == frame
                && c.context_only == context_only
        }) {
            let cached = self.cache.remove(i);
            let drawing = cached.drawing.clone();
            self.cache.push(cached);
            return Ok(drawing);
        }
        let facets = facet_key(settings);
        let prepared = self
            .prepared
            .mesh_for(&source, settings.mesh_detail, facets, cancel)?;
        let index = self
            .pipelines
            .iter()
            .position(|(key, _)| *key == facets)
            .unwrap_or_else(|| {
                self.pipelines.push((facets, Default::default()));
                self.pipelines.len() - 1
            });
        let mut drawing =
            self.pipelines[index]
                .1
                .draw(prepared, settings, paper, frame, context_only, cancel)?;
        if ground_only {
            let lines = super::shadow::ground_lines(
                drawing.shadow.as_ref().unwrap(),
                settings,
                paper,
                cancel,
            )?;
            drawing.parts.push(DrawingPart {
                name: "Sombra arrojada".into(),
                pen: 0,
                lines,
                families: vec![],
                ramp: None,
            });
        }
        let drawing = Arc::new(drawing);
        #[cfg(test)]
        {
            self.renders += 1;
        }
        // Caché acotada; no acumula indefinidamente modelos ni ediciones anteriores.
        // La poda se hace al iniciar el dibujo: ninguna pila activa se limita a 32 pasadas.
        self.cache.push(Cached {
            settings: settings.clone(),
            paper: paper.clone(),
            drawing: drawing.clone(),
            ground_only,
            frame,
            context_only,
        });
        Ok(drawing)
    }
}

fn facet_key(s: &Settings) -> Option<usize> {
    if s.recipe == RecipeKind::FlatShadow {
        Some(s.shadow.facets)
    } else if s.recipe.is_art() {
        Some(s.art.facets)
    } else {
        None
    }
}
