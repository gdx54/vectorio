//! Adaptador de VOLUMETRIC. Un trabajador mantiene cachés, descarta pedidos
//! anteriores y consulta cancelación dentro de los bucles geométricos.
use crate::{
    app::{VectorioApp, ViewMode},
    theme,
};
use egui::{Color32, Rect, RichText, Sense, Stroke};
use std::{
    io::Read,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
};
use vectorio_core::{p, volumetric::*, Paper, Pen};

#[derive(Clone, PartialEq)]
struct Request {
    mesh: Arc<Mesh>,
    settings: Settings,
    paper: Paper,
    passes: Vec<(u64, Pass)>,
}
struct Reply {
    version: u64,
    result: Result<Drawing, String>,
}
struct Worker {
    tx: mpsc::Sender<(u64, Request)>,
    rx: mpsc::Receiver<Reply>,
    latest: Arc<AtomicU64>,
}
impl Worker {
    fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel::<(u64, Request)>();
        let (out, input) = mpsc::channel();
        let latest = Arc::new(AtomicU64::new(0));
        let active = latest.clone();
        std::thread::spawn(move || {
            let mut engine = Engine::default();
            while let Ok(mut job) = rx.recv() {
                for next in rx.try_iter() {
                    job = next;
                }
                let (version, request) = job;
                let cancel = || active.load(Ordering::Relaxed) != version;
                if cancel() {
                    continue;
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    engine.draw_passes(
                        request.mesh,
                        &request.settings,
                        &request
                            .passes
                            .iter()
                            .map(|(_, p)| p.clone())
                            .collect::<Vec<_>>(),
                        &request.paper,
                        &cancel,
                    )
                }))
                .unwrap_or_else(|_| {
                    Err("No se pudo calcular el dibujo. Reduce la complejidad del modelo.".into())
                });
                if !cancel() {
                    if out.send(Reply { version, result }).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            }
        });
        Self {
            tx,
            rx: input,
            latest,
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.latest.fetch_add(1, Ordering::Relaxed);
    }
}
struct Loaded {
    mesh: Mesh,
    report: MeshReport,
    recipe: Option<Recipe>,
}
#[derive(Clone, Copy, PartialEq)]
enum Layout {
    Dual,
    Paper,
}
#[derive(Clone, Debug, PartialEq)]
struct UiPass {
    id: u64,
    pass: Pass,
}
#[derive(Clone)]
struct EditSnapshot {
    settings: Settings,
    passes: Vec<UiPass>,
    style_name: String,
}

pub struct VolumetricUi {
    editing: Option<u64>,
    mesh: Arc<Mesh>,
    primitive: Option<Primitive>,
    settings: Settings,
    passes: Vec<UiPass>,
    next_pass_id: u64,
    selected: Option<u64>,
    style_name: String,
    undo: Vec<EditSnapshot>,
    redo: Vec<EditSnapshot>,
    passes_open: bool,
    menu_open: bool,
    scene_open: Option<usize>,
    scene_collapsed: bool,
    saved_styles: Vec<styles::SavedStyle>,
    styles_loaded: bool,
    save_style_open: bool,
    save_style_name: String,
    thumbnails: styles::Thumbnails,
    estimate: stats::Estimate,
    layout: Layout,
    locked: bool,
    spatial_focus: Option<egui::Id>,
    axes: bool,
    worker: Option<Worker>,
    version: u64,
    requested: Option<Request>,
    drawing: Option<Drawing>,
    drawn_ids: Vec<u64>,
    drawn_scene: Option<(Settings, Paper)>,
    history_epoch: u64,
    scene_editing: bool,
    scene_edit_time: f64,
    ready: bool,
    error: Option<String>,
    loading: Option<mpsc::Receiver<Result<Loaded, String>>>,
    report: MeshReport,
    load_error: Option<String>,
    import_epoch: Arc<AtomicU64>,
}
impl Default for VolumetricUi {
    fn default() -> Self {
        Self {
            editing: None,
            mesh: {
                static DEFAULT: std::sync::OnceLock<Arc<Mesh>> = std::sync::OnceLock::new();
                DEFAULT
                    .get_or_init(|| Arc::new(Primitive::Torus.mesh()))
                    .clone()
            },
            primitive: Some(Primitive::Torus),
            settings: Default::default(),
            passes: Settings::default()
                .passes()
                .into_iter()
                .enumerate()
                .map(|(i, mut pass)| {
                    pass.pen = [0, 2, 1][i];
                    UiPass { id: i as u64, pass }
                })
                .collect(),
            next_pass_id: 3,
            selected: None,
            style_name: tm!("Curvas de nivel").into(),
            undo: vec![],
            redo: vec![],
            passes_open: false,
            menu_open: false,
            scene_open: None,
            scene_collapsed: false,
            saved_styles: vec![],
            styles_loaded: false,
            save_style_open: false,
            save_style_name: String::new(),
            thumbnails: Default::default(),
            estimate: Default::default(),
            layout: Layout::Paper,
            locked: false,
            spatial_focus: None,
            axes: true,
            worker: None,
            version: 0,
            requested: None,
            drawing: None,
            drawn_ids: vec![],
            drawn_scene: None,
            history_epoch: 0,
            scene_editing: false,
            scene_edit_time: f64::NEG_INFINITY,
            ready: false,
            error: None,
            loading: None,
            report: Default::default(),
            load_error: None,
            import_epoch: Arc::new(AtomicU64::new(0)),
        }
    }
}
impl VolumetricUi {
    pub(crate) fn editing(&self) -> bool {
        self.editing.is_some()
    }
    pub(crate) fn open_scene(
        &mut self,
        ctx: &egui::Context,
        generator: String,
        params: vectorio_core::generate::Params,
        context: vectorio_core::generate::GenContext,
        paper: Paper,
        pens: Vec<Pen>,
    ) {
        self.editing = None;
        self.worker = None;
        self.ready = false;
        self.error = None;
        self.load_error = None;
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        self.import_epoch.fetch_add(1, Ordering::Relaxed);
        let epoch = self.import_epoch.clone();
        let version = epoch.load(Ordering::Relaxed);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let generator = vectorio_core::generate::find(&generator)
                    .ok_or("No se encuentra el generador")?;
                let (mesh, camera) = generator.volume(&params, &context)?;
                let (mesh, report) = mesh.validated()?;
                let mut settings = Settings::default();
                settings.yaw = camera.yaw_deg;
                settings.elevation = camera.elev_deg;
                settings.seed = context.seed;
                let mut recipe = Recipe::new(mesh.clone(), settings);
                recipe.paper = paper;
                recipe.inks = pens;
                recipe.passes = vec![
                    Pass::new(
                        "Contorno",
                        PassKind::Outline(OutlineParams::Surface { facets: 320 }),
                        0,
                    ),
                    Pass::new("Aristas", PassKind::Edges { angle: 25. }, 0),
                ];
                Ok(Loaded {
                    mesh,
                    report,
                    recipe: Some(recipe),
                })
            }))
            .unwrap_or_else(|_| {
                Err("No se pudo preparar la escena 3D. Reduce su complejidad.".into())
            });
            if epoch.load(Ordering::Relaxed) == version {
                let _ = tx.send(result);
                ctx.request_repaint();
            }
        });
        self.loading = Some(rx);
    }

    pub(crate) fn open_model(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.editing = None;
        self.layout = Layout::Paper;
        self.load(ctx, path);
    }
    fn choose_primitive(&mut self, primitive: Primitive) {
        self.primitive = Some(primitive);
        self.mesh = Arc::new(primitive.mesh());
        self.report = Default::default();
        self.load_error = None;
        self.settings.model_rotation = [0.; 3];
        self.undo.clear();
        self.redo.clear();
        self.scene_editing = false;
        self.history_epoch = self.history_epoch.wrapping_add(1);
    }

    fn request(&mut self, ctx: &egui::Context, paper: &Paper) {
        if self.loading.is_some() {
            return;
        }
        let request = self.geometry_request(paper);
        if self.requested.as_ref() == Some(&request) {
            return;
        }
        self.version = self.version.wrapping_add(1);
        self.ready = false;
        self.error = None;
        let worker = self.worker.get_or_insert_with(|| Worker::new(ctx.clone()));
        worker.latest.store(self.version, Ordering::Relaxed);
        if worker.tx.send((self.version, request.clone())).is_ok() {
            self.requested = Some(request);
        } else {
            self.error = Some("El trabajador no está disponible; vuelve a abrir Vectorio.".into());
        }
    }
    fn poll(&mut self, app: &mut VectorioApp) {
        if let Some(worker) = &self.worker {
            for reply in worker.rx.try_iter() {
                if reply.version == self.version {
                    match reply.result {
                        Ok(d) => {
                            self.drawn_ids = self.requested.as_ref().map_or_else(Vec::new, |r| {
                                r.passes.iter().map(|(id, _)| *id).collect()
                            });
                            self.drawn_scene = self.requested.as_ref().map(|r| {
                                (
                                    r.settings.clone(),
                                    reading_content_paper(
                                        &r.paper,
                                        r.passes.iter().map(|(_, p)| p),
                                    ),
                                )
                            });
                            self.drawing = Some(d);
                            self.ready = true;
                        }
                        Err(e) => {
                            self.error = Some(e);
                            self.ready = false;
                        }
                    }
                }
            }
        }
        if let Some(rx) = &self.loading {
            match rx.try_recv() {
                Ok(result) => {
                    self.loading = None;
                    match result {
                        Ok(loaded) => {
                            if let Some(recipe) = loaded.recipe {
                                self.settings = recipe.settings;
                                app.doc.paper = recipe.paper;
                                let mut passes = recipe.passes;
                                if !recipe.inks.is_empty() {
                                    let map: Vec<_> = recipe
                                        .inks
                                        .into_iter()
                                        .map(|pen| {
                                            if let Some(i) =
                                                app.doc.pens.iter().position(|p| *p == pen)
                                            {
                                                i
                                            } else {
                                                let i = app.doc.pens.len();
                                                app.doc.pens.push(pen);
                                                i
                                            }
                                        })
                                        .collect();
                                    for pass in &mut passes {
                                        pass.pen = map[pass.pen];
                                    }
                                }
                                self.replace_passes(passes, tr!("Receta importada"), false);
                            }
                            self.undo.clear();
                            self.redo.clear();
                            self.scene_editing = false;
                            self.history_epoch = self.history_epoch.wrapping_add(1);
                            self.mesh = Arc::new(loaded.mesh);
                            self.primitive = None;
                            self.report = loaded.report;
                            self.requested = None;
                            self.error = None;
                        }
                        Err(e) => self.load_error = Some(e),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.loading = None;
                    self.load_error = Some("No se pudo abrir el modelo.".into());
                }
                _ => {}
            }
        }
    }
    fn load(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.version = self.version.wrapping_add(1);
        if let Some(w) = &self.worker {
            w.latest.store(self.version, Ordering::Relaxed);
        }
        self.ready = false;
        self.requested = None;
        self.error = None;
        self.load_error = None;
        let (tx, rx) = mpsc::channel();
        let epoch = self
            .import_epoch
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        let active = self.import_epoch.clone();
        self.loading = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let cancel = || active.load(Ordering::Relaxed) != epoch;
            let result = (|| {
                if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("obj") || e.eq_ignore_ascii_case("stl"))
                {
                    let (mesh, report) = load_mesh_cancellable(&path, &cancel)?;
                    return Ok(Loaded {
                        mesh,
                        report,
                        recipe: None,
                    });
                }
                let mut text = String::new();
                std::fs::File::open(path)
                    .map_err(|e| e.to_string())?
                    .take(MAX_RECIPE_BYTES + 1)
                    .read_to_string(&mut text)
                    .map_err(|e| e.to_string())?;
                let recipe = Recipe::from_json(&text)?;
                let (mesh, report) = recipe.mesh.clone().validated()?;
                if cancel() {
                    return Err("Cálculo cancelado.".into());
                }
                Ok(Loaded {
                    mesh,
                    report,
                    recipe: Some(recipe),
                })
            })();
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
    pub fn export(&self, app: &VectorioApp) -> Result<(), String> {
        if !self.current(&app.doc.paper) || app.doc.pens.is_empty() {
            return Err(tr!("Espera al dibujo actualizado antes de exportar."));
        }
        let drawing = self
            .materialized(&app.doc.pens)
            .ok_or_else(|| tr!("No hay dibujo para exportar."))?;
        if drawing.parts.iter().all(|p| p.lines.is_empty()) {
            return Err(tr!(
                "No hay curvas en este rango. Cambia el campo o la orientación."
            ));
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("SVG", &["svg"])
            .set_file_name("volumetric.svg")
            .save_file()
        {
            let doc = drawing.document(app.doc.paper.clone(), app.doc.pens.clone(), []);
            crate::files::atomic_write(
                &path,
                vectorio_core::svg_out::export(&doc, &doc.layers()).as_bytes(),
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn current(&self, paper: &Paper) -> bool {
        self.ready
            && self.loading.is_none()
            && self.requested.as_ref() == Some(&self.geometry_request(paper))
    }
    pub fn export_per_pen(&self, app: &VectorioApp) -> Result<(), String> {
        if !self.current(&app.doc.paper) || app.doc.pens.is_empty() {
            return Err(tr!("Espera al dibujo actualizado antes de exportar."));
        }
        let d = self
            .materialized(&app.doc.pens)
            .ok_or_else(|| tr!("No hay dibujo para exportar."))?;
        let doc = d.document(app.doc.paper.clone(), app.doc.pens.clone(), []);
        if let Some(dir) = rfd::FileDialog::new()
            .set_title(tr!("Carpeta para un SVG por rotulador"))
            .pick_folder()
        {
            for (name, svg) in vectorio_core::svg_out::export_per_pen(&doc, &doc.layers()) {
                crate::files::atomic_write(&dir.join(name), svg.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    fn composition_paper(&self, paper: &Paper) -> Paper {
        reading_content_paper(paper, self.passes.iter().map(|p| &p.pass))
    }
    fn geometry_request(&self, paper: &Paper) -> Request {
        let mut passes: Vec<_> = self
            .passes
            .iter()
            .map(|entry| {
                let mut pass = entry.pass.clone();
                pass.name.clear();
                pass.pen = 0;
                if let Some(map) = &mut pass.reading {
                    map.pens = (0..map.pens.len()).collect();
                }
                // Se conservan también las pasadas apagadas: encenderlas no recalcula.
                pass.enabled = true;
                (entry.id, pass)
            })
            .collect();
        passes.sort_by_key(|(id, _)| *id);
        Request {
            mesh: self.mesh.clone(),
            settings: self.settings.clone(),
            paper: paper.clone(),
            passes,
        }
    }
    fn part(&self, id: u64) -> Option<&DrawingPart> {
        let i = self.drawn_ids.iter().position(|key| *key == id)?;
        self.drawing.as_ref()?.parts.get(i)
    }
    fn materialized(&self, pens: &[Pen]) -> Option<Drawing> {
        let mut drawing = self.drawing.as_ref()?.clone();
        drawing.parts = self
            .passes
            .iter()
            .filter(|p| p.pass.enabled)
            .filter_map(|entry| {
                let mut part = self.part(entry.id)?.clone();
                part.recolor(&entry.pass, pens);
                Some(part)
            })
            .collect();
        Some(drawing)
    }
    fn snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            settings: self.settings.clone(),
            passes: self.passes.clone(),
            style_name: self.style_name.clone(),
        }
    }
    fn checkpoint(&mut self) {
        if self.undo.len() >= 64 {
            self.undo.remove(0);
        }
        self.undo.push(self.snapshot());
        self.redo.clear();
    }
    pub(crate) fn apply_palette(&mut self, id: u64, palette: &crate::palettes::Palette, doc: &mut vectorio_core::Document) -> Result<(), &'static str> {
        palette.validate()?;
        let index = self.passes.iter().position(|p| p.id == id)
            .ok_or(tm!("La pasada ya no existe. Vuelve a abrir Paletas."))?;
        let pass = &self.passes[index].pass;
        let map = pass.reading.as_ref().ok_or(tm!("La pasada ya no tiene un mapa de lectura."))?;
        let (minimum, single) = reading_ui::palette_requirements(&pass.kind, map);
        if !single && !(minimum..=8).contains(&palette.pens.len()) {
            return Err(tm!("La paleta no tiene el número de tintas que necesita esta pasada."));
        }
        self.checkpoint();
        let mut palette = palette.clone();
        if single { palette.pens.truncate(1); }
        let map = self.passes[index].pass.reading.as_mut().unwrap();
        map.pens = palette.apply(doc);
        map.bind_palette(&doc.pens);
        Ok(())
    }
    fn replace_passes(&mut self, passes: Vec<Pass>, name: String, undo: bool) {
        if undo {
            self.checkpoint();
        } else {
            self.undo.clear();
            self.redo.clear();
        }
        self.passes = passes
            .into_iter()
            .map(|pass| {
                let id = self.next_pass_id;
                self.next_pass_id += 1;
                UiPass { id, pass }
            })
            .collect();
        self.selected = None;
        self.style_name = name;
    }
    pub(crate) fn menu_open(&self, ctx: &egui::Context) -> bool {
        self.menu_open || ctx.memory(|m| m.any_popup_open())
    }
    pub(crate) fn scene_control_focused(&self, ctx: &egui::Context) -> bool {
        self.spatial_focus.is_some() && self.spatial_focus == ctx.memory(|m| m.focused())
    }
    pub(crate) fn can_undo_stack(&self, redo: bool) -> bool {
        if redo {
            !self.redo.is_empty()
        } else {
            !self.undo.is_empty()
        }
    }
    pub(crate) fn undo_stack(&mut self, redo: bool) {
        self.history_epoch = self.history_epoch.wrapping_add(1);
        self.scene_editing = false;
        let previous = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        if let Some(previous) = previous {
            let current = self.snapshot();
            if redo {
                self.undo.push(current);
            } else {
                self.redo.push(current);
            }
            self.settings = previous.settings;
            self.passes = previous.passes;
            self.style_name = previous.style_name;
            self.selected = None;
        }
    }
    fn move_pass(&mut self, from: u64, to: usize) {
        if let Some(index) = self.passes.iter().position(|p| p.id == from) {
            if index != to && to < self.passes.len() {
                self.checkpoint();
                let pass = self.passes.remove(index);
                self.passes.insert(to, pass);
            }
        }
    }
    fn set_light(&mut self, azimuth: f64, elevation: f64) {
        if !azimuth.is_finite() || !elevation.is_finite() {
            return;
        }
        self.settings.shadow.azimuth = (azimuth + 180.).rem_euclid(360.) - 180.;
        self.settings.shadow.elevation = elevation.clamp(5., 85.);
        for entry in &mut self.passes {
            let shadow = match &mut entry.pass.kind {
                PassKind::Hatch { settings, .. }
                | PassKind::Ground(settings)
                | PassKind::Outline(OutlineParams::Shadow(settings)) => Some(settings),
                _ => None,
            };
            if let Some(shadow) = shadow {
                shadow.azimuth = self.settings.shadow.azimuth;
                shadow.elevation = self.settings.shadow.elevation;
            }
        }
    }
    fn record_scene_edit(&mut self, before: EditSnapshot, epoch: u64, ctx: &egui::Context) {
        if epoch == self.history_epoch && before.settings != self.settings {
            if !self.scene_editing {
                if self.undo.len() >= 64 {
                    self.undo.remove(0);
                }
                self.undo.push(before);
                self.redo.clear();
                self.scene_editing = true;
            }
            self.scene_edit_time = ctx.input(|i| i.time);
        }
        if ctx.input(|i| i.pointer.any_released()) {
            self.scene_editing = false;
        }
    }
    fn recipe(&self, app: &VectorioApp) -> Recipe {
        let mut recipe = Recipe::new(self.mesh.as_ref().clone(), self.settings.clone());
        recipe.paper = app.doc.paper.clone();
        recipe.inks = app.doc.pens.clone();
        recipe.passes = self
            .passes
            .iter()
            .map(|p| {
                let mut pass = p.pass.clone();
                if let Some(map) = &mut pass.reading {
                    map.bind_palette(&app.doc.pens);
                }
                pass
            })
            .collect();
        recipe
    }
    fn save_recipe(&mut self, app: &VectorioApp) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_file_name("volumen.json")
            .save_file()
        {
            if let Err(error) = self.recipe(app).to_json().and_then(|text| {
                crate::files::atomic_write(&path, text.as_bytes()).map_err(|e| e.to_string())
            }) {
                self.error = Some(error);
            }
        }
    }
}

#[path = "volumetric_finishing.rs"]
mod finishing_ui;
#[path = "volumetric_interaction.rs"]
mod interaction;
#[path = "volumetric_passes_panel.rs"]
mod passes_panel;
#[path = "volumetric_preview.rs"]
mod preview;
#[path = "volumetric_reading.rs"]
mod reading_ui;
#[path = "volumetric_scene_panel.rs"]
mod scene_panel;
#[path = "volumetric_stats.rs"]
mod stats;
#[path = "volumetric_styles.rs"]
mod styles;

fn error_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(255, 138, 128)
    } else {
        Color32::from_rgb(170, 40, 30)
    }
}

pub fn show(app: &mut VectorioApp, ctx: &egui::Context) {
    let mut state = std::mem::take(&mut app.volumetric_ui);
    state.menu_open = false;
    if !state.styles_loaded {
        state.saved_styles = styles::load();
        state.styles_loaded = true;
    }
    state.poll(app);
    for entry in &mut state.passes {
        entry.pass.pen = entry.pass.pen.min(app.doc.pens.len().saturating_sub(1));
        if let Some(map) = &mut entry.pass.reading {
            for pen in &mut map.pens {
                *pen = (*pen).min(app.doc.pens.len().saturating_sub(1));
            }
        }
    }
    for entry in &mut state.passes {
        if let Some(map) = &mut entry.pass.reading {
            map.bind_palette(&app.doc.pens);
        }
    }
    state.spatial_focus = None;
    let before = state.snapshot();
    let epoch = state.history_epoch;
    if ctx.input(|i| {
        i.pointer.any_pressed()
            || (!i.pointer.any_down()
                && (i.time - state.scene_edit_time > 0.3
                    || i.events.iter().any(|event| {
                        matches!(
                            event,
                            egui::Event::Key {
                                pressed: true,
                                repeat: false,
                                ..
                            }
                        )
                    })))
    }) {
        state.scene_editing = false;
    }
    let pal = theme::current(ctx);
    let mut place = false;
    let mut export = false;
    egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show(ctx, |ui| {
            if app.document_modal() {
                ui.disable();
            }
            theme::paint_canvas(ui.painter(), ui.max_rect(), &pal);
            let rects = preview::Workspace::new(ui.max_rect(), state.scene_collapsed);
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rects.styles), |ui| {
                styles::gallery(ui, &mut state, app);
            });
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rects.scene), |ui| {
                if rects.compact || state.scene_collapsed {
                    scene_panel::rail(ui, &mut state);
                } else {
                    theme::glass_panel(ui, &pal, |ui| {
                        ui.set_width(rects.scene.width() - 2. * theme::PANEL_PAD);
                        ui.set_height(rects.scene.height() - 2. * theme::PANEL_PAD);
                        egui::ScrollArea::vertical()
                            .id_salt("vol-scene")
                            .show(ui, |ui| scene_panel::show(ui, &mut state, app, None));
                    });
                }
            });
            if let Some(rect) = rects.passes {
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                    theme::glass_panel(ui, &pal, |ui| {
                        ui.set_width(rect.width() - 2. * theme::PANEL_PAD);
                        ui.set_height(rect.height() - 2. * theme::PANEL_PAD);
                        egui::ScrollArea::vertical()
                            .id_salt("vol-passes")
                            .show(ui, |ui| passes_panel::show(ui, &mut state, app));
                    });
                });
            }
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rects.paper), |ui| {
                preview::previews(ui, &mut state, app)
            });
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rects.footer), |ui| {
                let result = stats::footer(ui, &mut state, app);
                place = result.0;
                export = result.1;
            });
            if rects.compact && state.passes_open {
                let mut open = true;
                egui::Window::new(tr!("Pasadas"))
                    .id(egui::Id::new("vol-pass-sheet"))
                    .open(&mut open)
                    .default_pos(egui::pos2(rects.paper.right() - 244., rects.paper.top()))
                    .default_width(220.)
                    .max_height(rects.paper.height() - 48.)
                    .collapsible(false)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .show(ui, |ui| passes_panel::show(ui, &mut state, app))
                    });
                state.passes_open = open;
            }
            if let Some(section) = state.scene_open {
                let mut open = true;
                egui::Window::new(scene_panel::section_name(section))
                    .id(egui::Id::new("vol-scene-sheet"))
                    .open(&mut open)
                    .default_pos(egui::pos2(rects.scene.right() + 8., rects.paper.top()))
                    .default_width(220.)
                    .max_height(rects.paper.height() - 64.)
                    .collapsible(false)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            scene_panel::show(ui, &mut state, app, Some(section))
                        })
                    });
                if !open {
                    state.scene_open = None;
                }
            }
        });
    state.record_scene_edit(before, epoch, ctx);
    styles::save_window(&mut state, app, ctx);
    for entry in &mut state.passes {
        if let Some(map) = &mut entry.pass.reading {
            map.bind_palette(&app.doc.pens);
        }
    }
    state.request(ctx, &app.doc.paper);
    if export {
        if let Err(error) = state.export(app) {
            state.error = Some(error);
        }
    }
    if place && state.current(&app.doc.paper) {
        if let Some(d) = state.materialized(&app.doc.pens) {
            let volume = VolumeObject {
                id: 0,
                name: state.mesh.name.clone(),
                recipe: state.recipe(app),
                bindings: vec![],
                placement: Default::default(),
            };
            match app.doc.put_volume(volume, &d, state.editing) {
                Ok(ids) => {
                    app.set_selection(ids);
                    app.tool = crate::app::Tool::Select;
                    app.mode = ViewMode::Design;
                    state.editing = None;
                }
                Err(error) => state.error = Some(error),
            }
        }
    }
    app.volumetric_ui = state;
}

#[cfg(test)]
#[path = "volumetric_tests.rs"]
mod tests;

#[path = "volumetric_continuity.rs"]
mod continuity;

#[cfg(test)]
#[path = "volumetric_render_qa.rs"]
pub(crate) mod render_qa;

pub(crate) fn reopen(app: &mut VectorioApp, group: u64) {
    continuity::reopen(app, group)
}
