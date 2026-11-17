//! Estilos de pasadas y miniaturas vectoriales calculadas fuera del hilo gráfico.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct SavedStyle {
    pub name: String,
    pub passes: Vec<Pass>,
    pub inks: Vec<Pen>,
}
const _TONAL_NAMES: [&str; 4] = [
    tm!("Grabado"),
    tm!("Aguafuerte cruzado"),
    tm!("Plano técnico"),
    tm!("Topografía en sombra"),
];
const KINDS: [RecipeKind; 7] = [
    RecipeKind::Atlas,
    RecipeKind::Strata,
    RecipeKind::XRay,
    RecipeKind::FlatShadow,
    RecipeKind::SurfaceWaves,
    RecipeKind::TriMesh,
    RecipeKind::Hybrid,
];
const NAMES: [&str; 7] = [
    tm!("Curvas de nivel"),
    tm!("Estratos"),
    tm!("Rayos X"),
    tm!("Sombra plana"),
    tm!("Ondas"),
    tm!("Malla de color"),
    tm!("Ondas y malla"),
];
fn builtins(state: &VolumetricUi, app: &VectorioApp) -> Vec<SavedStyle> {
    let mut styles: Vec<_> = KINDS
        .into_iter()
        .zip(NAMES)
        .map(|(recipe, name)| {
            let s = Settings {
                recipe,
                shadow: state.settings.shadow.clone(),
                ..Default::default()
            };
            let mut passes = s.passes();
            for (i, pass) in passes.iter_mut().enumerate() {
                pass.pen = [0, 2, 1]
                    .get(i)
                    .copied()
                    .unwrap_or(0)
                    .min(app.doc.pens.len().saturating_sub(1));
                if let PassKind::Hatch {
                    settings,
                    middle: true,
                } = &pass.kind
                {
                    pass.enabled = settings.middle_tone;
                }
            }
            SavedStyle {
                name: name.into(),
                passes,
                inks: app.doc.pens.clone(),
            }
        })
        .collect();
    styles.extend(TonalStyle::ALL.into_iter().map(|style| {
        let mut passes = style.passes(&state.settings);
        for pass in &mut passes {
            pass.pen = pass.pen.min(app.doc.pens.len().saturating_sub(1));
        }
        SavedStyle {
            name: style.name().into(),
            passes,
            inks: app.doc.pens.clone(),
        }
    }));
    styles.extend(ReadingStyle::ALL.into_iter().map(|style| {
        let mut passes = style.passes(&state.settings);
        let mut inks = app.doc.pens.clone();
        for pass in &mut passes {
            if let Some(map) = &mut pass.reading {
                for pen in &mut map.pens {
                    *pen = (*pen).min(inks.len().saturating_sub(1));
                }
            }
        }
        if style == ReadingStyle::Anaglyph {
            let mut tint = |name: &str, color: [u8; 3]| {
                if let Some(i) = inks.iter().position(|p| p.color == color) {
                    i
                } else {
                    inks.push(Pen::new(name, color, 0.25));
                    inks.len() - 1
                }
            };
            let red = tint("Rojo anaglifo", [210, 45, 45]);
            let cyan = tint("Cian anaglifo", [0, 170, 190]);
            passes[0].reading.as_mut().unwrap().pens = vec![red, cyan];
        }
        SavedStyle {
            name: style.name().into(),
            passes,
            inks,
        }
    }));
    styles.extend(ArtisticStyle::ALL.into_iter().map(|style| {
        let mut passes = style.passes(&state.settings);
        for pass in &mut passes {
            if let Some(map) = &mut pass.reading {
                for pen in &mut map.pens {
                    *pen = (*pen).min(app.doc.pens.len().saturating_sub(1));
                }
            }
        }
        SavedStyle {
            name: style.name().into(),
            passes,
            inks: app.doc.pens.clone(),
        }
    }));
    styles
}
pub(super) fn apply(state: &mut VolumetricUi, app: &mut VectorioApp, style: &SavedStyle) {
    let mut passes = style.passes.clone();
    let map: Vec<_> = style
        .inks
        .iter()
        .map(|pen| {
            if let Some(i) = app.doc.pens.iter().position(|p| p == pen) {
                i
            } else {
                app.doc.pens.push(pen.clone());
                app.doc.pens.len() - 1
            }
        })
        .collect();
    for pass in &mut passes {
        pass.pen = map.get(pass.pen).copied().unwrap_or(0);
        if let Some(reading) = &mut pass.reading {
            for pen in &mut reading.pens {
                *pen = map.get(*pen).copied().unwrap_or(0);
            }
        }
    }
    state.replace_passes(passes, style.name.clone(), true);
}
fn file() -> Option<PathBuf> {
    Some(crate::paths::support_dir()?.join("estilos-volumen.json"))
}
pub(super) fn load() -> Vec<SavedStyle> {
    file()
        .and_then(|p| std::fs::File::open(p).ok())
        .and_then(|f| {
            let mut text = String::new();
            f.take(8 * 1024 * 1024).read_to_string(&mut text).ok()?;
            let styles: Vec<SavedStyle> = serde_json::from_str(&text).ok()?;
            Some(
                styles
                    .into_iter()
                    .filter(|s| {
                        !s.name.trim().is_empty()
                            && s.passes.iter().all(|p| {
                                p.validate().is_ok()
                                    && p.pen < s.inks.len()
                                    && p.reading
                                        .as_ref()
                                        .is_none_or(|m| m.pens.iter().all(|i| *i < s.inks.len()))
                            })
                    })
                    .collect(),
            )
        })
        .unwrap_or_default()
}
pub(super) fn upsert(styles: &mut Vec<SavedStyle>, style: SavedStyle) {
    if let Some(old) = styles
        .iter_mut()
        .find(|s| s.name.eq_ignore_ascii_case(&style.name))
    {
        *old = style;
    } else {
        styles.push(style);
    }
}
/// Grupos de la galería por intención artística, no por el tipo interno del estilo.
const GROUPS: [&str; 6] = [
    tm!("Línea y contorno"),
    tm!("Sombra y grabado"),
    tm!("Ondas y malla"),
    tm!("Mapas de color"),
    tm!("Técnico"),
    tm!("Mis estilos"),
];
fn group(name: &str) -> usize {
    match name {
        "Sombra plana" | "Grabado" | "Aguafuerte cruzado" | "Topografía en sombra"
        | "Hachuras de relieve" | "Luz ambiente" | "Mapa de luz" | "Isolíneas de luz" => 1,
        "Ondas" | "Malla de color" | "Ondas y malla" | "Ondas con halo" | "Ondas con gradiente" => 2,
        "Mapa de curvatura" | "Mapa gaussiano" | "Mapa de pendientes" | "Mapa térmico"
        | "Mapa de agujas" | "Isolíneas de curvatura" | "Mezcla óptica térmica" => 3,
        "Plano técnico" | "Desarrollo" | "Sección técnica" => 4,
        _ => 0,
    }
}
/// Estilos de serie ordenados por grupo, seguidos de los guardados por el usuario.
fn catalog(state: &VolumetricUi, app: &VectorioApp) -> Vec<(usize, SavedStyle)> {
    let mut choices: Vec<_> = builtins(state, app)
        .into_iter()
        .map(|s| (group(&s.name), s))
        .collect();
    choices.sort_by_key(|(g, _)| *g);
    choices.extend(state.saved_styles.iter().cloned().map(|s| (GROUPS.len() - 1, s)));
    choices
}
pub(super) fn gallery(ui: &mut egui::Ui, state: &mut VolumetricUi, app: &mut VectorioApp) {
    let pal = theme::current(ui.ctx());
    let (groups, choices): (Vec<usize>, Vec<SavedStyle>) = catalog(state, app).into_iter().unzip();
    state.thumbnails.poll();
    state.thumbnails.request(
        ui.ctx(),
        state.mesh.clone(),
        &state.settings,
        &app.doc.paper,
        &choices,
    );
    let mut selected = None;
    theme::glass_panel_with(
        ui,
        &pal,
        theme::CONTROL_RADIUS + 6.,
        egui::Margin::symmetric(8., 5.),
        theme::Material::Panel,
        |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let entries = choices
                    .iter()
                    .enumerate()
                    .map(|(i, style)| crate::catalog::Entry {
                        label: crate::i18n::tr_key(&style.name),
                        category: crate::i18n::tr_key(GROUPS[groups[i]]),
                    })
                    .collect::<Vec<_>>();
                let active = choices.iter().position(|s| s.name == state.style_name);
                let reserve = if ui.ctx().screen_rect().width() < 1100. {
                    318.
                } else {
                    270.
                };
                let width = (ui.available_width() - reserve - 44.).clamp(140., 400.);
                ui.label(tr!("Estilo"));
                selected = crate::catalog::selector(
                    ui,
                    egui::Id::new("vol-style-catalog"),
                    &crate::i18n::tr_key(&state.style_name),
                    active,
                    &entries,
                    width,
                    |i, ui| {
                        if let Some(thumb) = state.thumbnails.images.get(i) {
                            thumb.paint(ui);
                        } else {
                            ui.weak(tr!("Preparando miniatura…"));
                        }
                    },
                );
                ui.add_space((ui.available_width() - reserve).max(0.));
                if ui
                    .add_enabled(
                        !state.undo.is_empty(),
                        egui::Button::new(tr!("Deshacer")).small(),
                    )
                    .clicked()
                {
                    state.undo_stack(false);
                }
                if ui.button(tr!("Guardar estilo…")).clicked() {
                    state.save_style_open = true;
                    state.save_style_name = state.style_name.clone();
                }
                let _more_menu = ui.menu_button(tr!("Más"), |ui| {
                    if ui
                        .add_enabled(!state.redo.is_empty(), egui::Button::new(tr!("Rehacer")))
                        .clicked()
                    {
                        state.undo_stack(true);
                        ui.close_menu();
                    }
                    if ui.button(tr!("Guardar receta…")).clicked() {
                        state.save_recipe(app);
                        ui.close_menu();
                    }
                    if ui.button(tr!("Abrir receta…")).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            state.load(ui.ctx(), path);
                        }
                        ui.close_menu();
                    }
                });
                state.menu_open |= egui::menu::BarState::load(ui.ctx(), ui.id()).is_some();
                if ui.ctx().screen_rect().width() < 1100. && ui.button(tr!("Pasadas")).clicked() {
                    state.passes_open = !state.passes_open;
                }
            });
        },
    );
    if let Some(i) = selected {
        apply(state, app, &choices[i]);
    }
}
pub(super) fn save_window(state: &mut VolumetricUi, app: &VectorioApp, ctx: &egui::Context) {
    if !state.save_style_open {
        return;
    }
    let mut open = true;
    let mut save = false;
    egui::Window::new(tr!("Guardar estilo"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(tr!("Nombre del estilo"));
            ui.text_edit_singleline(&mut state.save_style_name);
            save = ui
                .add_enabled(
                    !state.save_style_name.trim().is_empty(),
                    egui::Button::new(tr!("Guardar")),
                )
                .clicked();
        });
    if save {
        let style = SavedStyle {
            name: state.save_style_name.trim().to_owned(),
            passes: state.passes.iter().map(|p| p.pass.clone()).collect(),
            inks: app.doc.pens.clone(),
        };
        let mut saved = state.saved_styles.clone();
        upsert(&mut saved, style.clone());
        let result = (|| {
            let file = file().ok_or_else(|| tr!("No se encuentra la carpeta de usuario."))?;
            std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
            let text = serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?;
            crate::files::atomic_write(&file, &text).map_err(|e| e.to_string())
        })();
        match result {
            Ok(()) => {
                state.saved_styles = saved;
                state.style_name = style.name;
                open = false;
            }
            Err(e) => state.error = Some(e),
        }
    }
    state.save_style_open = open;
}
#[derive(Clone, PartialEq)]
struct ThumbRequest {
    mesh: Arc<Mesh>,
    settings: Settings,
    paper: Paper,
    styles: Vec<SavedStyle>,
}
struct Thumbnail {
    parts: Vec<(Color32, Vec<vectorio_core::Polyline>)>,
    paper: Paper,
}
impl Thumbnail {
    fn paint(&self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(190., 132.), Sense::hover());
        ui.painter().rect_filled(rect, 2., Color32::WHITE);
        let scale = ((rect.width() - 12.) / self.paper.width_mm as f32)
            .min((rect.height() - 12.) / self.paper.height_mm as f32);
        let origin = rect.center()
            - egui::vec2(self.paper.width_mm as f32, self.paper.height_mm as f32) * scale * 0.5;
        for (color, lines) in &self.parts {
            for line in lines {
                ui.painter().add(egui::Shape::line(
                    line.iter()
                        .map(|q| origin + egui::vec2(q.x as f32, q.y as f32) * scale)
                        .collect(),
                    Stroke::new(0.65f32, *color),
                ));
            }
        }
    }
}
#[derive(Default)]
pub(super) struct Thumbnails {
    worker: Option<(
        mpsc::Sender<(u64, ThumbRequest)>,
        mpsc::Receiver<(u64, Vec<Thumbnail>)>,
        Arc<AtomicU64>,
    )>,
    requested: Option<ThumbRequest>,
    version: u64,
    images: Vec<Thumbnail>,
}
impl Thumbnails {
    fn poll(&mut self) {
        if let Some((_, rx, _)) = &self.worker {
            for (v, images) in rx.try_iter() {
                if v == self.version {
                    self.images = images;
                }
            }
        }
    }
    fn request(
        &mut self,
        ctx: &egui::Context,
        mesh: Arc<Mesh>,
        scene: &Settings,
        paper: &Paper,
        styles: &[SavedStyle],
    ) {
        let mut settings = scene.clone();
        settings.recipe = RecipeKind::Atlas;
        settings.mesh_detail = MeshDetail::Interactive;
        let request = ThumbRequest {
            mesh,
            settings,
            paper: paper.clone(),
            styles: styles.to_vec(),
        };
        if self.requested.as_ref() == Some(&request) {
            return;
        }
        if self.worker.is_none() {
            let (tx, rx) = mpsc::channel::<(u64, ThumbRequest)>();
            let (out, input) = mpsc::channel();
            let latest = Arc::new(AtomicU64::new(0));
            let active = latest.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let mut engine = Engine::default();
                while let Ok(mut job) = rx.recv() {
                    for next in rx.try_iter() {
                        job = next;
                    }
                    let (version, request) = job;
                    let cancel = || active.load(Ordering::Relaxed) != version;
                    let mut thumbnails = vec![];
                    for style in &request.styles {
                        if cancel() {
                            break;
                        }
                        let mut passes = style.passes.clone();
                        for p in &mut passes {
                            let mut p_reading = p.reading.take();
                            match &mut p.kind {
                                PassKind::Reading(p) => {
                                    p.facets = p.facets.min(128);
                                    if let Some(map) = &mut p_reading {
                                        map.samples = 8;
                                        map.spacing_mm = map.spacing_mm.max(2.);
                                        map.legend = false;
                                    }
                                }
                                PassKind::ToneHatch(p) => {
                                    p.cuts = p.cuts.min(32);
                                    p.spacing_mm = p.spacing_mm.max(1.2);
                                }
                                PassKind::Field(p) | PassKind::Sections(p) => {
                                    p.cuts = p.cuts.min(16)
                                }
                                PassKind::Waves(p) => {
                                    p.facets = p.facets.min(240);
                                    p.lines = p.lines.min(18);
                                }
                                PassKind::Mesh(p) => p.facets = p.facets.min(240),
                                PassKind::Outline(OutlineParams::Surface { facets }) => {
                                    *facets = (*facets).min(240)
                                }
                                PassKind::Hatch { settings, .. }
                                | PassKind::Ground(settings)
                                | PassKind::Outline(OutlineParams::Shadow(settings)) => {
                                    settings.facets = settings.facets.min(200);
                                    settings.hatch_spacing_mm = settings.hatch_spacing_mm.max(0.8);
                                }
                                _ => {}
                            }
                            if let Some(map) = &mut p_reading {
                                map.bind_palette(&style.inks);
                            }
                            p.reading = p_reading;
                        }
                        let drawing = engine.draw_passes(
                            request.mesh.clone(),
                            &request.settings,
                            &passes,
                            &request.paper,
                            &cancel,
                        );
                        let parts = drawing
                            .map(|mut d| {
                                d.recolor(&passes, &style.inks);
                                d.parts
                                    .iter()
                                    .flat_map(|part| {
                                        part.inks().map(|(pen, lines)| {
                                            (
                                                style.inks.get(pen).map_or(Color32::BLACK, |p| {
                                                    theme::pen_color(p.color)
                                                }),
                                                lines.to_vec(),
                                            )
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        thumbnails.push(Thumbnail {
                            parts,
                            paper: request.paper.clone(),
                        });
                    }
                    if !cancel() {
                        let _ = out.send((version, thumbnails));
                        ctx.request_repaint();
                    }
                }
            });
            self.worker = Some((tx, input, latest));
        }
        self.version = self.version.wrapping_add(1);
        self.images.clear();
        let (tx, _, latest) = self.worker.as_ref().unwrap();
        latest.store(self.version, Ordering::Relaxed);
        if tx.send((self.version, request.clone())).is_ok() {
            self.requested = Some(request);
        }
    }
}
impl Drop for Thumbnails {
    fn drop(&mut self) {
        if let Some((_, _, latest)) = &self.worker {
            latest.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_stack_roundtrips_and_same_name_replaces_it_without_losing_palette() {
        let mut saved = vec![];
        let style = SavedStyle {
            name: "Relieve".into(),
            passes: Settings::default().passes(),
            inks: vectorio_core::doc::default_pens(),
        };
        upsert(&mut saved, style.clone());
        let mut replacement = style.clone();
        replacement.name = "relieve".into();
        replacement.passes.reverse();
        replacement.passes[0].pen = 2;
        upsert(&mut saved, replacement.clone());
        assert_eq!(saved, vec![replacement]);
        let decoded: Vec<SavedStyle> =
            serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(decoded, saved);
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let mut state = VolumetricUi::default();
        apply(&mut state, &mut app, &decoded[0]);
        assert_eq!(
            state
                .passes
                .iter()
                .map(|p| p.pass.clone())
                .collect::<Vec<_>>(),
            decoded[0].passes
        );
    }
}
