//! Estado de la aplicación, bucle principal, barra de herramientas, atajos y archivos.

use crate::canvas::{self, Interaction, Sim, View};
use crate::commands::{self, NativeMenu};
use crate::inspector;
use crate::theme::{self, Palette};
use crate::vectorize_ui::VectorizeUi;
use egui::{Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use vectorio_core::optimize::{self, Stats};
use vectorio_core::trace::{TraceKey, Traced};
use vectorio_core::{svg_in, svg_out, Document, Geometry, Item, PlotLayer, Polyline, Shape, Shared, P};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Strokes,
    Draw,
    Text,
    Shape,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    Design,
    Create,
    Photo,
    Volumetric,
    Output,
}

#[derive(Clone, Copy)]
enum UnsavedChoice { Save, Discard, Cancel }

enum DocumentChange { New, Open(Box<Document>, PathBuf), Quit }

/// Cómo seguir desde un aviso.
#[derive(Clone, Debug, PartialEq)]
pub enum ToastAction { Reveal(PathBuf), SaveAs }

#[derive(Clone)]
struct Toast { msg: String, until: Option<f64>, error: bool, action: Option<ToastAction> }

/// Origen de lo que se exporta desde la barra.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportSource { Canvas, Empty, Model, Route }

/// «hace 5 min», «hace 2 h», «hace 3 días».
fn age_label(elapsed: std::time::Duration) -> String {
    let minutes = elapsed.as_secs() / 60;
    if minutes < 1 { tr!("hace un momento") }
    else if minutes < 60 { tr!("hace %{n} min", n = minutes) }
    else if minutes < 48 * 60 { tr!("hace %{n} h", n = minutes / 60) }
    else { tr!("hace %{n} días", n = minutes / (24 * 60)) }
}

fn file_label(path: &Path) -> String {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_owned()
}

/// Qué falta para poder exportar el lienzo vacío desde cada espacio.
pub(crate) fn empty_canvas_message(mode: ViewMode) -> String {
    match mode {
        ViewMode::Create => tr!("El lienzo está vacío. Pulsa «Añadir al lienzo» para exportar la escena."),
        ViewMode::Photo => tr!("El lienzo está vacío. Pulsa «Añadir al lienzo» para exportar el dibujo de la foto."),
        _ => tr!("El lienzo está vacío. Dibuja, escribe o importa algo antes de exportar."),
    }
}

/// Página de la hoja de ayuda.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HelpPage { Shortcuts, FirstDrawing }

/// Muestra un archivo en Finder (o su carpeta en otros sistemas).
pub fn reveal(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(not(target_os = "macos"))]
    if let Some(dir) = if path.is_dir() { Some(path) } else { path.parent() } {
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}

/// Resultado de convertir el documento en capas de plotter.
#[derive(Default)]
pub struct Rendered {
    pub rev: u64,
    pub layers: Vec<PlotLayer>,
    pub stats: Stats,
    /// Trazos de cada objeto tal como estaba al calcularlos, para dibujarlo en el lienzo
    /// (si el objeto se ha movido desde entonces se transforma sin recalcular).
    pub items: HashMap<u64, ItemPlot>,
}

pub struct ItemPlot {
    pub snapshot: Item,
    pub parts: Vec<(usize, Vec<Polyline>)>,
}

pub const PAPER_TINTS: [(&str, [u8; 3]); 5] = [
    (tm!("Blanco"), [255, 255, 255]),
    (tm!("Marfil"), [250, 245, 232]),
    (tm!("Kraft"), [205, 170, 125]),
    (tm!("Gris"), [120, 122, 128]),
    (tm!("Negro"), [22, 22, 24]),
];

pub struct VectorioApp {
    pub doc: Document,
    recovery:crate::recovery::Recovery,
    committed: Document,
    saved: Document,
    pub(crate) quitting: bool,
    /// Tintas vivas cuyos trazos se están calculando: objeto → (ajustes, resultado).
    trace_jobs: HashMap<u64, (TraceKey, mpsc::Receiver<Vec<Polyline>>)>,
    shown: Document,
    history: Vec<Document>,
    future: Vec<Document>,
    pub rev: u64,
    pub rendered: Rendered,
    job: Option<mpsc::Receiver<Rendered>>,
    /// Objeto principal de la selección (el que muestra el inspector).
    pub selected: Option<u64>,
    /// Resto de objetos seleccionados (recuadro, ⇧ o ⌘ clic). Solo valen mientras el
    /// principal sea `also_anchor`; si otro código cambia `selected`, se descartan.
    also: Vec<u64>,
    also_anchor: Option<u64>,
    /// El último clic fue en la lista de rotuladores: ⌫ borra el rotulador activo.
    pub pen_focus: bool,
    /// Rotulador cuyo color se está eligiendo en el editor integrado.
    pub color_edit: Option<usize>,
    pub color_picker: crate::colorpanel::ColorEditor,
    pub palettes: crate::palettes::Library,
    pub palette_ui: crate::palette_ui::PaletteUi,
    color_original: Option<Document>,
    color_targets: Vec<usize>,
    /// Rotulador que se está renombrando.
    pub renaming_pen: Option<usize>,
    pub renaming_item: Option<u64>,
    pub rename_focus: bool,
    rename_original: String,
    pub tool: Tool,
    pub(crate) stroke_selection: crate::strokes::Selection,
    pending_change: Option<DocumentChange>,
    pub shape_kind: usize,
    pub active_pen: usize,
    pub view: View,
    pub mode: ViewMode,
    pub sim: Sim,
    pub create_ui: crate::create::CreateUi,
    pub volumetric_ui: crate::volumetric_ui::VolumetricUi,
    pub output_ui: crate::output::OutputUi,
    pub machine_ui: crate::machine_ui::MachineUi,
    pub prefs: crate::prefs::Preferences,
    prefs_dirty: bool,
    pub prefs_ui: crate::prefs_ui::PrefsUi,
    pub file: Option<PathBuf>,
    pub vec_ui: VectorizeUi,
    pub interaction: Interaction,
    pub(crate) canvas_id: Option<egui::Id>,
    pub focus_text: bool,
    /// Pedidos del lienzo que necesitan el contexto: abrir el diálogo de importar y avisar
    /// de que Formas ha vuelto a Seleccionar.
    pub pick_import: bool,
    pub notify_tool_return: bool,
    toast: Option<Toast>,
    pub show_help: bool,
    pub help_page: HelpPage,
    menu: Option<NativeMenu>,
    /// Estilos de trazo guardados por el usuario y nombre del que se va a guardar.
    pub styles: Vec<crate::styles::SavedStyle>,
    pub style_name: String,
}

impl VectorioApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::blank(&cc.egui_ctx);
        app.palettes = crate::palettes::Library::load_user();
        app.prefs = crate::prefs::Preferences::load();
        app.apply_prefs(&cc.egui_ctx);
        // Los documentos nuevos salen con el papel y los ajustes elegidos en Ajustes.
        app.doc = app.prefs.new_document.document();
        app.committed = app.doc.clone();
        app.shown = app.doc.clone();
        app.saved = app.doc.clone();
        app.menu = NativeMenu::install(&cc.egui_ctx);
        crate::macos::install_reopen_handler(&cc.egui_ctx);
        // Archivo pasado al abrir (`vectorio dibujo.vectorio`, `vectorio foto.jpg`).
        if let Some(path) = std::env::args_os().nth(1).map(PathBuf::from).filter(|p| p.exists()) {
            app.open_path(&cc.egui_ctx, &path);
        }
        if let Some(dir)=crate::paths::support_dir().map(|p|p.join("Recuperacion")){
            if app.file.is_none(){
                for path in crate::recovery::candidates(&dir){
                    if let Ok(snapshot)=crate::recovery::read(&path){
                        let name=snapshot.file.as_deref().and_then(Path::file_stem).and_then(|s|s.to_str()).map(str::to_owned).unwrap_or_else(||tr!("Sin título"));
                        let age=std::fs::metadata(&path).and_then(|m|m.modified()).ok().and_then(|t|t.elapsed().ok()).map(age_label).unwrap_or_default();
                        let choice=rfd::MessageDialog::new().set_title(ts!("Recuperar proyecto")).set_description(tr!("Hay una copia automática de «%{name}» guardada %{age} en una sesión interrumpida. ¿Quieres recuperarla?",name=name,age=age)).set_buttons(rfd::MessageButtons::YesNoCancelCustom(tr!("Recuperar"),tr!("Descartar copia"),tr!("Más tarde"))).show();
                        if matches!(choice,rfd::MessageDialogResult::Custom(ref s) if s==ts!("Recuperar")){
                            app.replace_document(snapshot.doc,snapshot.file);app.saved=Document::default();let _=std::fs::remove_file(path);
                        }else if matches!(choice,rfd::MessageDialogResult::Custom(ref s) if s==ts!("Descartar copia")){let _=std::fs::remove_file(path);}
                        break;
                    }
                }
            }
            app.recovery=crate::recovery::Recovery::start(dir);
        }
        app
    }

    /// App con un papel en blanco, sin menú nativo ni archivos abiertos.
    pub(crate) fn blank(ctx: &egui::Context) -> Self {
        theme::install(ctx);
        let doc = Document::default();
        VectorioApp {
            recovery:Default::default(),
            committed: doc.clone(),
            saved: doc.clone(),
            quitting: false,
            trace_jobs: HashMap::new(),
            styles: crate::styles::load(),
            style_name: String::new(),
            shown: doc.clone(),
            doc,
            history: vec![],
            future: vec![],
            rev: 1,
            rendered: Rendered::default(),
            job: None,
            selected: None,
            also: vec![],
            also_anchor: None,
            pen_focus: false,
            color_edit: None,
            color_picker: Default::default(),
            palettes: Default::default(),
            palette_ui: Default::default(),
            color_original: None,
            color_targets: vec![],
            renaming_pen: None,
            renaming_item: None,
            rename_focus: false,
            rename_original: String::new(),
            tool: Tool::Select,
            stroke_selection: Default::default(),
            pending_change: None,
            shape_kind: 1,
            active_pen: 0,
            view: View::default(),
            mode: ViewMode::Design,
            sim: Sim::default(),
            output_ui: Default::default(),
            machine_ui: Default::default(),
            create_ui: Default::default(),
            volumetric_ui: Default::default(),
            prefs: Default::default(),
            prefs_dirty: false,
            prefs_ui: Default::default(),
            file: None,
            vec_ui: VectorizeUi::default(),
            interaction: Interaction::default(),
            canvas_id: None,
            focus_text: false,
            pick_import: false,
            notify_tool_return: false,
            toast: None,
            show_help: false,
            help_page: HelpPage::Shortcuts,
            menu: None,
        }
    }

    /// Confirmación breve que desaparece sola.
    pub fn notify(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        self.toast = Some(Toast { msg: msg.into(), until: Some(ctx.input(|i| i.time) + 3.5), error: false, action: None });
    }

    /// Archivo escrito: la confirmación dura algo más y permite mostrarlo en Finder.
    pub fn notify_written(&mut self, ctx: &egui::Context, msg: impl Into<String>, path: PathBuf) {
        self.toast = Some(Toast { msg: msg.into(), until: Some(ctx.input(|i| i.time) + 6.0), error: false, action: Some(ToastAction::Reveal(path)) });
    }

    /// Error: se queda hasta que se cierra y puede ofrecer cómo seguir.
    pub fn notify_error(&mut self, ctx: &egui::Context, msg: impl Into<String>, action: Option<ToastAction>) {
        self.toast = Some(Toast { msg: msg.into(), until: None, error: true, action });
        ctx.request_repaint();
    }

    /// Nombre del proyecto para títulos y nombres de archivo sugeridos.
    pub fn project_name(&self) -> String {
        self.file.as_deref().and_then(Path::file_stem).and_then(|s| s.to_str()).map(str::to_owned).unwrap_or_else(|| tr!("Sin título"))
    }

    /// Diálogo de guardado con el nombre derivado del proyecto y su carpeta.
    pub(crate) fn save_dialog(&self, suffix: &str, extension: &str) -> rfd::FileDialog {
        let stem = crate::files::file_stem(&self.project_name(), "Vectorio");
        let dialog = rfd::FileDialog::new().set_file_name(format!("{stem}{suffix}.{extension}"));
        match self.file.as_deref().and_then(Path::parent) {
            Some(dir) => dialog.set_directory(dir),
            None => dialog,
        }
    }

    /// Pone en marcha lo que las preferencias controlan en la interfaz.
    fn apply_prefs(&self, ctx: &egui::Context) {
        rust_i18n::set_locale(self.prefs.language.resolve());
        self.prefs.appearance.apply(ctx);
        if (ctx.zoom_factor() - self.prefs.ui_scale).abs() > f32::EPSILON { ctx.set_zoom_factor(self.prefs.ui_scale); }
    }

    /// Cambia las preferencias, las aplica y las deja pendientes de guardar.
    pub fn set_prefs(&mut self, ctx: &egui::Context, prefs: crate::prefs::Preferences) {
        let language_changed = prefs.language != self.prefs.language;
        self.prefs = prefs.sanitized();
        self.apply_prefs(ctx);
        // El menú nativo se construye con los textos del idioma: se rehace al cambiarlo.
        if language_changed && self.menu.is_some() { self.menu = NativeMenu::install(ctx); }
        self.prefs_dirty = true;
    }

    /// Marca las preferencias para guardarlas en cuanto termine el gesto en curso.
    pub(crate) fn prefs_changed(&mut self) { self.prefs_dirty = true; }

    /// Guarda las preferencias cuando no hay un gesto en curso (un deslizador, un arrastre).
    fn save_prefs(&mut self, ctx: &egui::Context) {
        if !self.prefs_dirty || ctx.input(|i| i.pointer.any_down()) { return; }
        self.prefs_dirty = false;
        if let Err(error) = self.prefs.save() { self.notify(ctx, tr!("No se pudieron guardar los ajustes: %{error}", error = error)); }
    }

    pub fn selected_item(&self) -> Option<&Item> {
        self.selected.and_then(|id| self.doc.get(id))
    }

    /// Todos los objetos seleccionados, en el orden del documento.
    pub fn selection(&self) -> Vec<u64> {
        let Some(main) = self.selected else { return vec![] };
        let extra: &[u64] = if self.also_anchor == Some(main) { &self.also } else { &[] };
        self.doc.items.iter().map(|it| it.id).filter(|id| *id == main || extra.contains(id)).collect()
    }

    pub fn is_selected(&self, id: u64) -> bool {
        self.selected == Some(id) || (self.also_anchor == self.selected && self.also.contains(&id))
    }

    /// Selecciona varios objetos; el último pasa a ser el principal.
    pub fn set_selection(&mut self, mut ids: Vec<u64>) {
        self.stroke_selection = Default::default();
        if (!ids.is_empty() || self.pen_focus) && self.tool == Tool::Strokes { self.tool = Tool::Select; }
        ids.dedup();
        self.selected = ids.pop();
        self.also = ids;
        self.also_anchor = self.selected;
    }

    pub fn choose_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.stroke_selection = Default::default();
            if tool == Tool::Strokes {
                self.set_selection(vec![]);
                self.pen_focus = false;
            }
            self.interaction = Interaction::None;
            self.tool = tool;
        }
    }

    pub fn rename_pen(&mut self, pen: usize) {
        self.finish_rename(false);
        if let Some(p) = self.doc.pens.get(pen) {
            self.rename_original = p.name.clone();
            self.renaming_pen = Some(pen);
            self.rename_focus = true;
        }
    }

    pub fn rename_item(&mut self, id: u64) {
        self.finish_rename(false);
        if let Some(it) = self.doc.get(id) {
            self.rename_original = it.name.clone();
            self.renaming_item = Some(id);
            self.rename_focus = true;
        }
    }

    pub fn finish_rename(&mut self, cancel: bool) {
        let pen = self.renaming_pen;
        let name = if let Some(i) = self.renaming_pen.take() {
            self.doc.pens.get_mut(i).map(|p| &mut p.name)
        } else {
            self.renaming_item.take().and_then(|id| self.doc.get_mut(id)).map(|it| &mut it.name)
        };
        if let Some(name) = name {
            *name = if cancel || name.trim().is_empty() { self.rename_original.clone() } else { name.trim().to_string() };
        }
        if !cancel {
            if let Some(pen) = pen.and_then(|i| self.doc.pens.get_mut(i)) { pen.custom_name = true; }
        }
        self.rename_focus = false;
    }

    pub fn remove_pen(&mut self, pen: usize) {
        if self.doc.pens.len() <= 1 || pen >= self.doc.pens.len() { return; }
        self.finish_rename(false);
        if self.color_edit == Some(pen) { self.color_edit = None; }
        self.color_edit = self.color_edit.map(|i| if i > pen { i - 1 } else { i });
        self.active_pen = if self.active_pen == pen { 0 } else if self.active_pen > pen { self.active_pen - 1 } else { self.active_pen };
        self.doc.remove_pen(pen);
    }

    /// Añade o quita un objeto de la selección (⇧ o ⌘ clic).
    pub fn toggle_selected(&mut self, id: u64) {
        let mut ids = self.selection();
        if let Some(i) = ids.iter().position(|x| *x == id) {
            ids.remove(i);
        } else {
            ids.push(id);
        }
        self.set_selection(ids);
    }

    pub fn add_item(&mut self, item: Item) -> u64 {
        let id = self.doc.add(item);
        self.selected = Some(id);
        id
    }

    /// Borra los objetos seleccionados que no están bloqueados.
    pub fn delete_selected(&mut self) {
        let ids = self.selection();
        self.doc.items.retain(|it| !(ids.contains(&it.id) && !it.locked));
        let left: Vec<u64> = ids.into_iter().filter(|id| self.doc.get(*id).is_some()).collect();
        self.set_selection(left);
    }

    /// Saca los trazos elegidos a objetos nuevos y los deja seleccionados para cambiarles
    /// la tinta, moverlos u ocultarlos.
    pub fn separate_strokes(&mut self, ctx: &egui::Context) {
        if self.mode != ViewMode::Design || self.tool != Tool::Strokes {
            self.notify(ctx, tr!("Elige líneas con Seleccionar trazos (A) para separarlas."));
            return;
        }
        let created = self.stroke_selection.separate(&mut self.doc, |name| tr!("%{name} · separado", name = name));
        if created.is_empty() {
            self.notify(ctx, tr!("Elige solo una parte de las líneas de un objeto para separarlas."));
            return;
        }
        let message = if created.len() == 1 {
            tr!("Separado en un objeto nuevo: puedes cambiarle la tinta, moverlo u ocultarlo.")
        } else {
            tr!("Separado en %{n} objetos nuevos: puedes cambiarles la tinta, moverlos u ocultarlos.", n = created.len())
        };
        self.set_selection(created);
        self.notify(ctx, message);
    }

    /// Fija la foto viva pulsada con Seleccionar trazos para poder tocar sus líneas.
    pub fn freeze_live_photo(&mut self, ctx: &egui::Context) {
        let Some(id) = self.stroke_selection.live.take() else { return };
        let frozen = crate::strokes::freeze_live(&mut self.doc, id);
        self.stroke_selection.index.invalidate();
        if frozen > 0 {
            self.notify(ctx, tr!("Foto convertida en trazos fijos: ya puedes elegir sus líneas."));
        }
    }

    pub fn duplicate_selected(&mut self) {
        let new = self.doc.duplicate(&self.selection(), vectorio_core::p(8.0, 8.0));
        if !new.is_empty() {
            self.set_selection(new);
        }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.future.push(std::mem::replace(&mut self.doc, prev));
            self.committed = self.doc.clone();
            self.fix_selection();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.future.pop() {
            self.history.push(std::mem::replace(&mut self.doc, next));
            self.committed = self.doc.clone();
            self.fix_selection();
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// La paleta edita una tinta compartida; la fila de un objeto edita solo su color.
    pub fn edit_pen_color(&mut self, i: usize) {
        self.color_targets.clear();
        if let Some(pen) = self.doc.pens.get(i) {
            self.color_original = Some(self.doc.clone());
            self.color_edit = Some(i);
            self.color_picker = crate::colorpanel::ColorEditor::new(pen.color);
            self.color_picker.subtitle = tr!("%{name} · tinta compartida", name = crate::i18n::pen_name(pen));
        }
    }

    pub fn edit_item_color(&mut self, id: u64, fill: bool) {
        self.edit_items_color(&[id], fill);
    }

    pub fn edit_items_color(&mut self, ids: &[u64], fill: bool) {
        let original = self.doc.clone();
        let name = ids.first().and_then(|id| self.doc.get(*id)).map(|it| it.name.clone()).unwrap_or_default();
        let targets: Vec<usize> = ids.iter().filter_map(|id| self.doc.isolate_ink(*id, fill)).collect();
        let Some(index) = targets.first().copied() else { return };
        self.color_original = Some(original);
        self.color_targets = targets;
        self.color_edit = Some(index);
        self.color_picker = crate::colorpanel::ColorEditor::new(self.doc.pens[index].color);
        self.color_picker.title = if fill { tr!("Color del relleno") } else { tr!("Color del contorno") };
        self.color_picker.subtitle = if self.color_targets.len() == 1 { name } else { tr!("%{n} objetos seleccionados", n = self.color_targets.len()) };
    }

    pub fn cancel_color_edit(&mut self) {
        if let Some(original) = self.color_original.take() { self.doc = original; }
        self.color_edit = None;
        self.color_targets.clear();
    }

    fn show_color_panel(&mut self, ctx: &egui::Context) {
        let Some(i) = self.color_edit else { return };
        let Some(pen) = self.doc.pens.get_mut(i) else { self.cancel_color_edit(); return };
        let open = self.color_picker.show(ctx, pen);
        let color = pen.color;
        for index in &self.color_targets {
            if let Some(pen) = self.doc.pens.get_mut(*index) { pen.set_color(color); }
        }
        if !open {
            if self.color_picker.cancelled { self.cancel_color_edit(); }
            else { self.color_edit = None; self.color_original = None; self.color_targets.clear(); }
        }
    }

    fn fix_selection(&mut self) {
        self.stroke_selection = Default::default();
        if self.selected.is_some_and(|id| self.doc.get(id).is_none()) {
            self.selected = None;
        }
        let doc = &self.doc;
        self.also.retain(|id| doc.get(*id).is_some());
        self.active_pen = self.active_pen.min(self.doc.pens.len().saturating_sub(1));
    }

    /// Registra cambios para deshacer cuando termina el gesto en curso y lanza el
    /// recálculo de las capas de plotter en segundo plano.
    /// Recalcula en segundo plano los trazos de las tintas vivas cuyos ajustes o grosor
    /// de rotulador han cambiado. Un solo cálculo por tinta a la vez: si los ajustes
    /// cambian mientras tanto (un deslizador), al terminar se lanza el siguiente.
    fn update_traces(&mut self, ctx: &egui::Context) {
        let mut done = Vec::new();
        self.trace_jobs.retain(|id, (key, rx)| match rx.try_recv() {
            Ok(lines) => {
                done.push((*id, key.clone(), Shared::new(lines)));
                false
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => false,
        });
        if !done.is_empty() {
            // También en la versión confirmada: el resultado no es un paso de deshacer.
            for doc in [&mut self.doc, &mut self.committed, &mut self.saved] {
                let keys = doc.trace_keys();
                for (id, key, lines) in &done {
                    if keys.get(id) != Some(key) {
                        continue;
                    }
                    if let Some(t) = doc.get_mut(*id).and_then(|it| it.traced_mut()) {
                        t.lines = lines.clone();
                        t.made = Some(key.clone());
                    }
                }
            }
        }
        // Las tintas en relevo dependen de sus compañeras: si cambia una, se recalculan todas.
        let keys = self.doc.trace_keys();
        for it in &self.doc.items {
            let (Some(t), Some(key)) = (it.traced(), keys.get(&it.id)) else { continue };
            if t.made.as_ref() == Some(key) || self.trace_jobs.contains_key(&it.id) {
                continue;
            }
            let (tx, rx) = mpsc::channel();
            let (key, coverage, seed, ctx) = (key.clone(), t.coverage.clone(), t.seed, ctx.clone());
            let job = key.clone();
            std::thread::spawn(move || {
                let _ = tx.send(Traced::generate(&coverage, &job, seed));
                ctx.request_repaint();
            });
            self.trace_jobs.insert(it.id, (key, rx));
        }
    }

    /// ¿Hay tintas vivas recalculándose?
    pub fn tracing(&self) -> bool {
        !self.trace_jobs.is_empty()
    }

    pub(crate) fn track_changes(&mut self, ctx: &egui::Context) {
        if self.doc != self.shown {
            self.shown = self.doc.clone();
            self.rev += 1;
        }
        let gesture = ctx.input(|i| i.pointer.any_down()) || self.color_edit.is_some() || self.renaming_pen.is_some() || self.renaming_item.is_some();
        if !gesture && self.doc != self.committed {
            self.history.push(std::mem::replace(&mut self.committed, self.doc.clone()));
            if self.history.len() > 120 {
                self.history.remove(0);
            }
            self.future.clear();
        }
        if let Some(rx) = &self.job {
            match rx.try_recv() {
                Ok(r) => {
                    self.rendered = r;
                    self.job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.job = None,
            }
        }
        if self.job.is_none() && self.rendered.rev != self.rev {
            let (tx, rx) = mpsc::channel();
            let doc = self.doc.clone();
            let rev = self.rev;
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let per_item = doc.plot_items();
                let items = doc
                    .items
                    .iter()
                    .zip(per_item.iter())
                    .map(|(it, parts)| (it.id, ItemPlot { snapshot: it.clone(), parts: parts.clone() }))
                    .collect();
                let layers = doc.layers_from(per_item);
                let stats = optimize::stats(&layers, &doc.plot);
                let _ = tx.send(Rendered { rev, layers, stats, items });
                ctx.request_repaint();
            });
            self.job = Some(rx);
        }
    }

    pub fn is_computing(&self) -> bool {
        self.job.is_some()
    }

    /// El plan de salida corresponde al dibujo y a los ajustes actuales: ni el dibujo ni
    /// las tintas se están recalculando y el plan ya no está desfasado.
    pub fn output_ready(&self) -> bool {
        self.rendered.rev == self.rev && !self.tracing() && !self.is_computing() && self.output_ui.ready(self.rev, &self.doc.output, &self.prefs.machine.dynamics())
    }

    // ── Archivos ────────────────────────────────────────────────────────────────

    pub fn modified(&self) -> bool { self.doc != self.saved }

    pub(crate) fn document_modal(&self) -> bool {
        self.awaiting_document_change() || self.prefs_ui.open || self.color_edit.is_some() || self.show_help || self.palette_ui.open
    }

    pub(crate) fn awaiting_document_change(&self) -> bool { self.pending_change.is_some() }

    pub(crate) fn cancel_document_change(&mut self) { self.pending_change = None; }

    fn request_change(&mut self, ctx: &egui::Context, change: DocumentChange) {
        if self.pending_change.is_some() { return; }
        if self.modified() {
            self.pending_change = Some(change);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.memory_mut(|m| {
                m.close_popup();
                if let Some(id) = m.focused() { m.surrender_focus(id); }
            });
            ctx.request_repaint();
        } else {
            self.perform_change(ctx, change);
        }
    }

    fn perform_change(&mut self, ctx: &egui::Context, change: DocumentChange) {
        match change {
            DocumentChange::New => self.replace_document(self.prefs.new_document.document(), None),
            DocumentChange::Open(doc, path) => self.replace_document(*doc, Some(path)),
            DocumentChange::Quit => {
                self.recovery.finish();
                self.machine_ui.shutdown();
                self.quitting = true;
                crate::macos::quit(ctx);
            }
        }
    }

    fn unsaved_sheet(&mut self, ctx: &egui::Context, pal: &Palette) {
        let Some(change) = &self.pending_change else { return };
        let document = self.file.as_deref().and_then(Path::file_name).and_then(|n| n.to_str())
            .map(str::to_owned).unwrap_or_else(|| tr!("Sin título"));
        let prompt = match change {
            DocumentChange::New => tr!("¿Guardar los cambios de «%{name}» antes de crear otro proyecto?", name = document),
            DocumentChange::Open(..) => tr!("¿Guardar los cambios de «%{name}» antes de abrir otro proyecto?", name = document),
            DocumentChange::Quit => tr!("¿Guardar los cambios de «%{name}» antes de salir?", name = document),
        };
        let screen = ctx.screen_rect();
        let veil = egui::Area::new("unsaved-veil".into()).fixed_pos(screen.min).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.allocate_rect(screen, Sense::click_and_drag());
            ui.painter().rect_filled(screen, 0.0, theme::veil(ui.visuals().dark_mode));
        });
        ctx.move_to_top(veil.response.layer_id);
        let mut choice = None;
        let sheet = egui::Area::new("unsaved-sheet".into()).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .order(egui::Order::Foreground).show(ctx, |ui| {
                theme::glass_sheet(ui, pal, |ui| {
                    ui.set_width(380.0_f32.min(screen.width() - 40.0));
                    ui.heading(tr!("Cambios sin guardar"));
                    ui.label(prompt);
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(tr!("Cancelar")).clicked() { choice = Some(UnsavedChoice::Cancel); }
                        if ui.button(tr!("Descartar cambios")).clicked() { choice = Some(UnsavedChoice::Discard); }
                        if theme::accent_button(ui, &tr!("Guardar")).clicked() { choice = Some(UnsavedChoice::Save); }
                    });
                });
            });
        ctx.move_to_top(sheet.response.layer_id);
        if let Some(choice) = choice {
            let pending = self.pending_change.take().unwrap();
            if self.resolve_unsaved(choice, |app| app.save(ctx, false)) {
                self.perform_change(ctx, pending);
            }
        }
    }

    fn resolve_unsaved(&mut self, choice: UnsavedChoice, save: impl FnOnce(&mut Self) -> bool) -> bool {
        match choice {
            UnsavedChoice::Save => save(self),
            UnsavedChoice::Discard => true,
            UnsavedChoice::Cancel => false,
        }
    }

    pub fn request_quit(&mut self, ctx: &egui::Context) {
        self.request_change(ctx, DocumentChange::Quit);
    }

    /// Una sesión nueva nunca hereda historial, selección ni resultados de otra.
    fn replace_document(&mut self, doc: Document, path: Option<PathBuf>) {
        self.mode = ViewMode::Design;
        self.choose_tool(Tool::Select);
        self.create_ui.cancel();
        self.volumetric_ui=Default::default();
        self.renaming_pen = None;
        self.renaming_item = None;
        self.rename_focus = false;
        self.color_edit = None;
        self.color_original = None;
        self.color_targets.clear();
        self.pen_focus = false;
        self.doc = doc;
        self.committed = self.doc.clone();
        self.saved = self.doc.clone();
        self.shown = self.doc.clone();
        self.history.clear();
        self.future.clear();
        self.trace_jobs.clear();
        self.job = None;
        self.rev = self.rev.wrapping_add(1);
        self.rendered = Rendered::default();
        self.output_ui = Default::default();
        self.vec_ui = Default::default();
        self.interaction = Default::default();
        self.canvas_id = None;
        self.focus_text = false;
        self.sim = Default::default();
        self.set_selection(vec![]);
        self.fix_selection();
        self.file = path;
        self.view.fitted = false;
    }

    pub fn new_document(&mut self, ctx: &egui::Context) {
        self.request_change(ctx, DocumentChange::New);
    }

    /// Abrir solo abre proyectos: sustituye el documento y vuelve a Diseño.
    pub fn open_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().add_filter(tr!("Proyecto Vectorio"), &["vectorio"]).pick_file() {
            self.open_path(ctx, &path);
        }
    }

    /// Importar desde el papel vacío: un SVG se añade al lienzo; una foto abre Foto.
    pub fn import_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().set_title(tr!("Importar"))
            .add_filter(tr!("SVG o foto"), &["svg", "png", "jpg", "jpeg", "tif", "tiff", "webp", "bmp", "gif"])
            .pick_file()
        {
            self.open_path(ctx, &path);
        }
    }

    /// Añade los trazados de un SVG al documento actual.
    pub fn import_svg_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().set_title(tr!("Importar SVG")).add_filter("SVG", &["svg"]).pick_file() {
            self.import_svg(ctx, &path);
            self.mode = ViewMode::Design;
        }
    }

    /// Abre un modelo OBJ o STL en Modelo 3D sin tocar el documento.
    pub fn import_model_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().set_title(tr!("Importar modelo 3D")).add_filter(tr!("Modelo 3D"), &["obj", "stl"]).pick_file() {
            self.open_path(ctx, &path);
        }
    }

    pub fn open_path(&mut self, ctx: &egui::Context, path: &Path) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "obj" | "stl" => {
                self.volumetric_ui.open_model(ctx, path.to_path_buf());
                self.mode = ViewMode::Volumetric;
            },
            "vectorio" | "json" => match crate::files::read_project(path).map_err(|e| e.to_string()).and_then(|s| Document::from_json(&s)) {
                Ok(doc) => {
                    self.request_change(ctx, DocumentChange::Open(Box::new(doc), path.to_path_buf()));
                }
                Err(e) => self.notify_error(ctx, tr!("No se pudo abrir «%{file}»: %{error}", file = file_label(path), error = e), None),
            },
            "svg" => self.import_svg(ctx, path),
            _ => match self.vec_ui.load(ctx, path) {
                Ok(()) => self.mode = ViewMode::Photo,
                Err(e) => self.notify_error(ctx, tr!("No se pudo abrir «%{file}»: %{error}", file = file_label(path), error = e), None),
            },
        }
    }

    fn import_svg(&mut self, ctx: &egui::Context, path: &Path) {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => return self.notify_error(ctx, tr!("No se pudo leer «%{file}»: %{error}", file = file_label(path), error = e), None),
        };
        match svg_in::import(&data) {
            Ok(list) if !list.is_empty() => {
                let n = list.len();
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("SVG").to_string();
                let mut last = None;
                for (i, imp) in list.into_iter().enumerate() {
                    let pen = self.doc.nearest_pen(imp.color);
                    let mut item = Item::new(format!("{name} {}", i + 1), Geometry::Path { lines: imp.lines, closed: imp.closed }, P::default());
                    item.stroke = Some(pen);
                    item.fill.pen = pen;
                    last = Some(self.doc.add(item));
                }
                self.set_selection(last.into_iter().collect());
                self.notify(ctx, tr!("%{n} trazados importados de %{name}", n = n, name = name));
            }
            Ok(_) => self.notify_error(ctx, tr!("«%{file}» no contiene trazados.", file = file_label(path)), None),
            Err(e) => self.notify_error(ctx, tr!("«%{file}» no es un SVG válido: %{error}", file = file_label(path), error = e), None),
        }
    }

    fn save_to(&mut self, path: &Path) -> Result<(), String> {
        crate::files::atomic_write(path, self.doc.to_json().as_bytes()).map_err(|e| e.to_string())?;
        self.saved = self.doc.clone();
        self.file = Some(path.to_path_buf());
        Ok(())
    }

    pub fn save(&mut self, ctx: &egui::Context, save_as: bool) -> bool {
        let path = match (&self.file, save_as) {
            (Some(p), false) => Some(p.clone()),
            _ => self.save_dialog("", "vectorio").add_filter(tr!("Proyecto Vectorio"), &["vectorio"]).save_file(),
        };
        if let Some(path) = path {
            match self.save_to(&path) {
                Ok(()) => {
                    self.notify_written(ctx, tr!("Guardado %{name}", name = file_label(&path)), path);
                    return true;
                }
                Err(e) => self.notify_error(ctx, tr!("No se pudo guardar «%{file}»: %{error}", file = file_label(&path), error = e), Some(ToastAction::SaveAs)),
            }
        }
        false
    }

    /// El lienzo tiene algo que trazar.
    pub fn canvas_has_content(&self) -> bool {
        self.doc.items.iter().any(|it| it.visible)
    }

    /// Qué exporta el botón Exportar en el espacio actual.
    pub(crate) fn export_source(&self) -> ExportSource {
        match self.mode {
            ViewMode::Volumetric => ExportSource::Model,
            ViewMode::Output => ExportSource::Route,
            _ if !self.canvas_has_content() => ExportSource::Empty,
            _ => ExportSource::Canvas,
        }
    }

    pub fn export_svg(&mut self, ctx: &egui::Context) {
        match self.export_source() {
            ExportSource::Model => {
                if let Err(error) = self.volumetric_ui.export(self) { self.notify_error(ctx, error, None); }
                return;
            }
            ExportSource::Route => {
                if self.output_ready() { self.output_ui.export_svg(&self.doc); }
                else { self.notify(ctx, tr!("Espera a que termine el cálculo de la salida.")); }
                return;
            }
            ExportSource::Empty => return self.notify_error(ctx, empty_canvas_message(self.mode), None),
            ExportSource::Canvas => {}
        }
        if !self.check_export_ready(ctx) { return; }
        let layers = self.doc.layers();
        if layers.iter().all(|l| l.paths.is_empty()) {
            return self.notify_error(ctx, tr!("El lienzo no tiene trazos que exportar."), None);
        }
        let Some(path) = self.save_dialog("", "svg").add_filter("SVG", &["svg"]).save_file() else {
            return;
        };
        match crate::files::atomic_write(&path, svg_out::export(&self.doc, &layers).as_bytes()) {
            Ok(()) => self.notify_written(ctx, tr!("SVG exportado con %{n} capas (una por rotulador)", n = layers.len()), path),
            Err(e) => self.notify_error(ctx, tr!("No se pudo exportar «%{file}»: %{error}", file = file_label(&path), error = e), None),
        }
    }

    /// Lotes: siempre en una carpeta nueva con el nombre del proyecto, sin sustituir nada.
    fn write_batch(&mut self, ctx: &egui::Context, title: String, folder: String, files: Vec<(String, String)>) {
        let Some(parent) = ({
            let dialog = rfd::FileDialog::new().set_title(title);
            match self.file.as_deref().and_then(Path::parent) { Some(dir) => dialog.set_directory(dir), None => dialog }
        }).pick_folder() else { return };
        let n = files.len();
        let files: Vec<(String, Vec<u8>)> = files.into_iter().map(|(name, svg)| (name, svg.into_bytes())).collect();
        match crate::files::write_batch(&parent, &folder, &files) {
            Ok(dir) => {
                let name = file_label(&dir);
                self.notify_written(ctx, tr!("%{n} archivos SVG en la carpeta nueva «%{name}»", n = n, name = name), dir);
            }
            Err(e) => self.notify_error(ctx, tr!("No se exportó ningún archivo en «%{file}»: %{error}", file = file_label(&parent), error = e), None),
        }
    }

    pub fn export_text_pages(&mut self, ctx: &egui::Context, id: u64) {
        if !self.check_export_ready(ctx) { return; }
        let Some(item) = self.doc.get(id) else { return };
        let Geometry::Text(spec) = &item.geometry else { return };
        let count = vectorio_core::font::compose(spec).page_count;
        let name: String = item.name.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_').take(50).collect();
        let name = if name.is_empty() { "texto".to_owned() } else { name };
        let mut doc = self.doc.clone();
        let mut files = Vec::new();
        for page in 0..count {
            if let Some(Item { geometry: Geometry::Text(spec), .. }) = doc.get_mut(id) { spec.page = page; }
            files.push((format!("{name}-pagina-{:03}.svg", page + 1), svg_out::export(&doc, &doc.layers())));
        }
        let folder = format!("{} · {}", crate::files::file_stem(&self.project_name(), "Vectorio"), tr!("páginas"));
        self.write_batch(ctx, tr!("Carpeta para las páginas SVG"), folder, files);
    }

    pub fn export_per_pen(&mut self, ctx: &egui::Context) {
        if self.mode == ViewMode::Volumetric {
            if let Err(error) = self.volumetric_ui.export_per_pen(self) { self.notify_error(ctx, error, None); }
            return;
        }
        if self.export_source() == ExportSource::Empty {
            return self.notify_error(ctx, empty_canvas_message(self.mode), None);
        }
        if !self.check_export_ready(ctx) { return; }
        if self.mode == ViewMode::Output && !self.output_ready() {
            self.notify(ctx, tr!("Espera a que termine el cálculo de la salida.")); return;
        }
        let layers = if self.mode == ViewMode::Output { self.output_ui.plan.as_ref().unwrap().layers() } else { self.doc.layers() };
        let files = svg_out::export_per_pen(&self.doc, &layers);
        if files.is_empty() {
            return self.notify_error(ctx, tr!("El lienzo no tiene trazos que exportar."), None);
        }
        let folder = format!("{} · {}", crate::files::file_stem(&self.project_name(), "Vectorio"), tr!("rotuladores"));
        self.write_batch(ctx, tr!("Carpeta para un SVG por rotulador"), folder, files);
    }

    fn check_export_ready(&mut self, ctx: &egui::Context) -> bool {
        if self.doc.traces_current() && !self.tracing() { return true; }
        self.notify(ctx, tr!("Espera a que terminen de actualizarse las tintas antes de exportar."));
        false
    }

    // ── Entrada ─────────────────────────────────────────────────────────────────

    /// Espacio Foto: al añadir, el dibujo pasa al documento y se vuelve a Diseño con él
    /// seleccionado; al cancelar se vuelve sin tocar nada.
    pub(crate) fn show_photo(&mut self, ctx: &egui::Context) {
        let paper = theme::pen_color(PAPER_TINTS[self.prefs.paper_tint].1);
        let enabled = !self.document_modal();
        match self.vec_ui.show(ctx, &self.doc, paper, enabled) {
            Some(crate::vectorize_ui::Outcome::Add(import)) => {
                self.doc.pens.extend(import.pens);
                let replacing = import.replacing.is_some();
                let ids = self.doc.add_photo_items(import.items, import.photo, import.replacing);
                self.set_selection(ids);
                self.tool = Tool::Select;
                self.mode = ViewMode::Design;
                self.notify(ctx, if replacing {
                    tr!("Foto separada de nuevo en el mismo sitio.")
                } else {
                    tr!("Dibujo añadido y seleccionado entero. Retoca cada tinta en el panel derecho.")
                });
            }
            Some(crate::vectorize_ui::Outcome::Cancel) => self.mode = ViewMode::Design,
            None => {}
        }
    }

    pub(crate) fn dropped_files(&mut self, ctx: &egui::Context) {
        if self.document_modal() { return; }
        let files: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        // Un gesto con varios archivos 3D abre el último, sin que otro archivo
        // del mismo gesto vuelva a tapar el espacio del modelo.
        if let Some(path) = files.iter().rev().find(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("obj") || ext.eq_ignore_ascii_case("stl"))) {
            self.open_path(ctx, path);
            return;
        }
        for f in files {
            self.open_path(ctx, &f);
        }
    }

    // ── Interfaz ────────────────────────────────────────────────────────────────

    pub(crate) fn toolbar(&mut self, ctx: &egui::Context, pal: &Palette) {
        // La barra flota sobre el lienzo, que ocupa toda la ventana: no hay franja ni línea.
        let bar = Rect::from_min_size(Pos2::ZERO, Vec2::new(ctx.screen_rect().width(), theme::TOOLBAR_H));
        egui::Area::new(egui::Id::new("toolbar")).fixed_pos(Pos2::ZERO).order(egui::Order::Middle).constrain(false).show(ctx, |ui| {
            if self.document_modal() { ui.disable(); }
            theme::toolbar_scrim(ui.painter(), bar, pal);
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(bar), |ui| {
            // Arrastrar la barra vacía mueve la ventana; doble clic la amplía.
            let bg = ui.interact(ui.max_rect(), egui::Id::new("toolbar-drag"), Sense::click_and_drag());
            if bg.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if bg.double_clicked() {
                let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            ui.horizontal_centered(|ui| {
                // Los semáforos son nativos y no siguen el tamaño de la interfaz.
                ui.add_space(theme::TRAFFIC_LIGHTS_W / ui.ctx().zoom_factor());
                let title = self.project_name();
                let title = if self.modified() { format!("{title} · {}", tr!("Sin guardar")) } else { title };
                ui.vertical(|ui| {
                    ui.set_max_width(if ctx.screen_rect().width()<1180.{90.}else{180.});
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Vectorio").size(13.5).strong().color(pal.text));
                    ui.add(egui::Label::new(egui::RichText::new(title).size(11.0).color(pal.secondary)).truncate());
                });
                // Guardar vive junto al nombre del proyecto; Exportar queda para los archivos de salida.
                let save = ui.add_enabled(self.modified() || self.file.is_none() && self.canvas_has_content(), egui::Button::new(tr!("Guardar")).small())
                    .on_hover_text(tr!("Guardar proyecto (⌘S) · clic secundario: Guardar como…"))
                    .on_disabled_hover_text(tr!("No hay cambios sin guardar"));
                if save.clicked() { self.save(ctx, false); }
                save.context_menu(|ui| {
                    if ui.button(tr!("Guardar como… (⇧⌘S)")).clicked() { ui.close_menu(); self.save(ctx, true); }
                });
                ui.add_space(14.0);

                if self.mode == ViewMode::Design {
                capsule(ui, pal, |ui| {
                    for (tool, icon, tip) in [
                        (Tool::Select, Icon::Select, tm!("Seleccionar y mover (V)")),
                        (Tool::Strokes, Icon::Strokes, tm!("Seleccionar trazos (A)")),
                        (Tool::Draw, Icon::Draw, tm!("Mano alzada (P)")),
                        (Tool::Text, Icon::Text, tm!("Texto manuscrito (T)")),
                        (Tool::Shape, Icon::Shape, tm!("Formas y generativos (S)")),
                    ] {
                        if icon_button(ui, pal, icon, self.tool == tool).on_hover_text(crate::i18n::tr_key(tip)).clicked() {
                            self.choose_tool(tool);
                        }
                    }
                });
                if self.tool == Tool::Shape {
                    let presets = Shape::presets();
                    egui::ComboBox::from_id_salt("shape-kind").icon(crate::theme::combo_icon)
                        .selected_text(crate::i18n::ts(presets[self.shape_kind].label()))
                        .width(if ctx.screen_rect().width()<1180.{90.}else{130.})
                        .show_ui(ui, |ui| {
                            for (i, s) in presets.iter().enumerate() {
                                ui.selectable_value(&mut self.shape_kind, i, crate::i18n::ts(s.label()));
                            }
                        });
                }
                if matches!(self.tool, Tool::Draw | Tool::Text | Tool::Shape) {
                    if let Some(pen) = self.doc.pens.get(self.active_pen) {
                        let name = crate::i18n::pen_name(pen);
                        let color = theme::pen_color(pen.color);
                        let (dot, response) = ui.allocate_exact_size(Vec2::new(18.0, 28.0), Sense::click());
                        ui.painter().circle_filled(dot.center(), 6.5, color);
                        ui.painter().circle_stroke(dot.center(), 7.0, Stroke::new(1.0f32, pal.outline));
                        let label = tr!("Tinta para dibujar: %{name}", name = name);
                        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
                        if response.on_hover_text(&label).clicked() { self.pen_focus = true; self.set_selection(vec![]); }
                        if ctx.screen_rect().width() >= 1180. {
                            ui.add(egui::Label::new(egui::RichText::new(name).size(11.5).color(pal.secondary)).truncate());
                        }
                    }
                }
                ui.add_space(6.0);
                capsule(ui, pal, |ui| {
                    if icon_button(ui, pal, Icon::Image, false).on_hover_text(tr!("Foto → dibujo (I)")).clicked() && self.vec_ui.open_dialog(ctx) {
                        self.mode = ViewMode::Photo;
                    }
                    let open = icon_button(ui, pal, Icon::Open, false).on_hover_text(tr!("Abrir proyecto (⌘O) · clic secundario: importar"));
                    if open.clicked() { self.open_dialog(ctx); }
                    open.context_menu(|ui| {
                        if ui.button(tr!("Importar SVG…")).clicked() { ui.close_menu(); self.import_svg_dialog(ctx); }
                        if ui.button(tr!("Importar modelo 3D…")).clicked() { ui.close_menu(); self.import_model_dialog(ctx); }
                    });
                });
                } else {
                    ui.label(egui::RichText::new(if self.mode == ViewMode::Create { tr!("Vista previa pendiente") } else if self.mode == ViewMode::Photo { tr!("Foto → dibujo") } else if self.mode == ViewMode::Volumetric { String::new() } else { tr!("Preparación · %{plotter}", plotter = self.prefs.plotter()) }).color(pal.secondary));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    let export = theme::accent_button(ui, &tr!("Exportar"));
                    let popup_id = egui::Id::new("export-menu");
                    if export.clicked() {
                        ui.memory_mut(|m| m.toggle_popup(popup_id));
                    }
                    egui::popup_below_widget(ui, popup_id, &export, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                        ui.set_min_width(240.0);
                        ui.set_max_width(300.0);
                        self.export_menu(ui, ctx, pal);
                    });
                    ui.add_space(8.0);
                    let modes=[(ViewMode::Design,tr!("Diseño")),(ViewMode::Create,tr!("Escenas")),(ViewMode::Photo,tr!("Foto")),(ViewMode::Volumetric,tr!("Modelo 3D")),(ViewMode::Output,tr!("Salida"))];
                    if workspace_selector(ui,pal,&mut self.mode,&modes)&&self.mode==ViewMode::Output&&self.sim.progress<=0.{self.sim.progress=1.;}
                    ui.add_space(8.0);
                    capsule(ui, pal, |ui| {
                        let volume = self.mode == ViewMode::Volumetric;
                        let can_redo = if volume { self.volumetric_ui.can_undo_stack(true) } else { self.can_redo() };
                        let can_undo = if volume { self.volumetric_ui.can_undo_stack(false) } else { self.can_undo() };
                        let redo = icon_button_state(ui, pal, Icon::Redo, false, can_redo);
                        if redo.on_hover_text(tr!("Rehacer (⇧⌘Z)")).clicked() && can_redo {
                            if volume { self.volumetric_ui.undo_stack(true); } else { self.redo(); }
                        }
                        let undo = icon_button_state(ui, pal, Icon::Undo, false, can_undo);
                        if undo.on_hover_text(tr!("Deshacer (⌘Z)")).clicked() && can_undo {
                            if volume { self.volumetric_ui.undo_stack(false); } else { self.undo(); }
                        }
                    });
                });
            });
            });
        });
    }

    /// Contenido del menú Exportar: cada entrada dice de dónde sale el archivo.
    fn export_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, pal: &Palette) {
        let note = |ui: &mut egui::Ui, text: String| { ui.add(egui::Label::new(egui::RichText::new(text).small().color(pal.secondary)).wrap()); };
        if matches!(self.mode, ViewMode::Create | ViewMode::Photo) {
            note(ui, tr!("Vista previa pendiente: no se exporta hasta que pulses «Añadir al lienzo»."));
            ui.separator();
        }
        let source = self.export_source();
        match source {
            ExportSource::Route => {
                if ui.button(tr!("Plan con pausas…")).on_hover_text(tr!("JSON con la ruta, los cambios de tinta y las esperas de secado")).clicked() {
                    if self.output_ready() { self.output_ui.export_plan(); }
                    else { self.notify(ctx, tr!("Espera a que termine el cálculo de la salida.")); }
                }
                if ui.button(tr!("SVG de la ruta, sin pausas (⌘E)")).clicked() { self.export_svg(ctx); }
                if ui.button(tr!("Un SVG por rotulador…")).clicked() { self.export_per_pen(ctx); }
            }
            ExportSource::Model => {
                if ui.button(tr!("SVG del modelo 3D (⌘E)")).clicked() { self.export_svg(ctx); }
                if ui.button(tr!("Un SVG por rotulador del modelo 3D…")).clicked() { self.export_per_pen(ctx); }
            }
            ExportSource::Canvas | ExportSource::Empty => {
                let enabled = source == ExportSource::Canvas;
                if ui.add_enabled(enabled, egui::Button::new(tr!("SVG del lienzo (⌘E)"))).clicked() { self.export_svg(ctx); }
                if ui.add_enabled(enabled, egui::Button::new(tr!("Un SVG por rotulador del lienzo…"))).clicked() { self.export_per_pen(ctx); }
                if !enabled { note(ui, empty_canvas_message(self.mode)); }
            }
        }
    }

    fn floating_panels(&mut self, ctx: &egui::Context, pal: &Palette) {
        let screen = ctx.screen_rect();
        let top = theme::TOOLBAR_H + theme::INSET;
        let avail_h = screen.height() - top - theme::INSET - 64.0;
        egui::Area::new(egui::Id::new("left-panel"))
            .fixed_pos(Pos2::new(theme::INSET, top))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                if self.document_modal() { ui.disable(); }
                theme::glass_panel(ui, pal, |ui| {
                    ui.set_width(inspector::LEFT_W);
                    ui.set_max_height(avail_h);
                    egui::ScrollArea::vertical().id_salt("left-scroll").show(ui, |ui| inspector::left_panel(self, ui));
                });
            });
        egui::Area::new(egui::Id::new("right-panel"))
            .fixed_pos(Pos2::new(screen.right() - inspector::RIGHT_W - theme::INSET - 28.0, top))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                if self.document_modal() { ui.disable(); }
                theme::glass_panel(ui, pal, |ui| {
                    ui.set_width(inspector::RIGHT_W);
                    ui.set_max_height(avail_h);
                    egui::ScrollArea::vertical().id_salt("right-scroll").show(ui, |ui| inspector::right_panel(self, ui));
                });
            });
    }

    fn status_bar(&mut self, ctx: &egui::Context, pal: &Palette) {
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("status"))
            .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -theme::INSET))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                theme::glass_panel_with(ui, pal, theme::PANEL_RADIUS, egui::Margin::symmetric(16.0, 8.0), theme::Material::Capsule, |ui| {
                    ui.set_max_width(screen.width() - 2.0 * (inspector::RIGHT_W + 60.0));
                    ui.horizontal(|ui| {
                        let st = &self.rendered.stats;
                        let n = self.rendered.layers.len();
                        let pens = if n == 1 { tr!("1 rotulador") } else { tr!("%{n} rotuladores", n = n) };
                        let info = tr!("Dibujo %{draw} · Viajes %{travel} · %{lifts} subidas · %{pens} · ≈ %{time}",
                            draw = crate::fmt::distance(st.draw_mm), travel = crate::fmt::distance(st.travel_mm), lifts = st.lifts, pens = pens,
                            time = crate::fmt::estimate(st.seconds));
                        ui.label(egui::RichText::new(info).color(pal.secondary));
                        if self.is_computing() {
                            ui.spinner();
                        }
                        ui.separator();
                        ui.label(egui::RichText::new(format!("{:.0} %", self.view.zoom_percent())).color(pal.secondary));
                        if ui.small_button(tr!("Ajustar")).on_hover_text("⌘0").clicked() {
                            self.view.fitted = false;
                        }
                    });
                });
            });
    }

    /// Hoja con todos los atajos (⌘/).
    fn help_sheet(&mut self, ctx: &egui::Context, pal: &Palette) {
        let screen = ctx.screen_rect();
        let veil = egui::Area::new(egui::Id::new("help-veil")).fixed_pos(screen.min).order(egui::Order::Middle).show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(screen, 0.0, theme::veil(ui.visuals().dark_mode));
            resp.clicked()
        });
        ctx.move_to_top(veil.response.layer_id);
        if veil.inner {
            self.show_help = false;
        }
        egui::Area::new(egui::Id::new("help-sheet")).anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, theme::TOOLBAR_H / 2.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            theme::glass_sheet(ui, pal, |ui| {
                if self.help_page == HelpPage::FirstDrawing {
                    ui.set_max_width(460.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(tr!("Mi primer dibujo")).size(15.0).strong().color(pal.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(tr!("Esc para cerrar")).color(pal.secondary));
                        });
                    });
                    ui.add_space(10.0);
                    let steps = [
                        tr!("Elige una tinta en TINTAS, a la izquierda. Es la tinta para dibujar."),
                        tr!("Pulsa Mano alzada (P) y arrastra sobre el papel."),
                        tr!("Con Seleccionar trazos (A) puedes mover o borrar una línea suelta."),
                        tr!("Guarda el proyecto con Guardar, junto al nombre, o ⌘S."),
                        tr!("Exportar escribe el SVG del lienzo. En Salida revisas el recorrido, lo simulas y guardas el plan con pausas."),
                    ];
                    for (i, step) in steps.into_iter().enumerate() {
                        ui.horizontal_top(|ui| {
                            ui.label(egui::RichText::new(format!("{}", i + 1)).strong().color(pal.accent));
                            ui.add(egui::Label::new(step).wrap());
                        });
                        ui.add_space(4.0);
                    }
                    ui.add_space(6.0);
                    if theme::accent_button(ui, &tr!("Empezar a dibujar")).clicked() {
                        self.show_help = false;
                        self.mode = ViewMode::Design;
                        self.choose_tool(Tool::Draw);
                    }
                    return;
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr!("Atajos de teclado")).size(15.0).strong().color(pal.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(tr!("Esc para cerrar")).color(pal.secondary));
                    });
                });
                ui.add_space(10.0);
                egui::Grid::new("help-grid").num_columns(2).spacing([36.0, 4.0]).show(ui, |ui| {
                    ui.vertical(|ui| help_groups(ui, pal, &commands::CHEATSHEET[..3]));
                    ui.vertical(|ui| help_groups(ui, pal, &commands::CHEATSHEET[3..]));
                    ui.end_row();
                });
            });
        });
    }

    fn toast(&mut self, ctx: &egui::Context, pal: &Palette) {
        let now = ctx.input(|i| i.time);
        let Some(toast) = self.toast.clone() else { return };
        if toast.until.is_some_and(|until| now > until) {
            self.toast = None;
            return;
        }
        let mut close = false;
        let mut act = None;
        egui::Area::new(egui::Id::new("toast"))
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, theme::TOOLBAR_H + 16.0))
            .order(egui::Order::Foreground)
            .interactable(toast.error || toast.action.is_some())
            .show(ctx, |ui| {
                theme::glass_panel_with(ui, pal, theme::PANEL_RADIUS, egui::Margin::symmetric(18.0, 9.0), theme::Material::Capsule, |ui| {
                    ui.set_max_width((ctx.screen_rect().width() - 80.0).clamp(200.0, 620.0));
                    ui.horizontal(|ui| {
                        if toast.error {
                            ui.label(egui::RichText::new("⚠").color(Color32::from_rgb(210, 60, 50)));
                        }
                        ui.add(egui::Label::new(&toast.msg).wrap());
                        match &toast.action {
                            Some(ToastAction::Reveal(_)) => if ui.button(tr!("Mostrar en Finder")).clicked() { act = toast.action.clone(); },
                            Some(ToastAction::SaveAs) => if ui.button(tr!("Elegir ubicación…")).clicked() { act = toast.action.clone(); },
                            None => {}
                        }
                        if toast.error && ui.button(tr!("Cerrar")).clicked() { close = true; }
                    });
                });
            });
        match act {
            Some(ToastAction::Reveal(path)) => { reveal(&path); self.toast = None; }
            Some(ToastAction::SaveAs) => { self.toast = None; self.save(ctx, true); }
            None if close => self.toast = None,
            None => {}
        }
        if toast.until.is_some() { ctx.request_repaint_after(std::time::Duration::from_millis(250)); }
    }
}

impl eframe::App for VectorioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let pal = theme::current(ctx);
        self.dropped_files(ctx);
        let picked = self.menu.as_ref().map(|m| m.poll()).unwrap_or_default();
        for cmd in picked {
            // Con una hoja abierta, del menú solo valen cerrar y lo que no toca el documento.
            self.run(ctx, cmd);
        }
        if crate::macos::take_quit_request() { self.request_quit(ctx); }
        if self.quitting { return; }
        // El botón rojo oculta la ventana igual que ⌘W; ⌘Q sale del programa.
        if cfg!(target_os = "macos") && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.awaiting_document_change() { self.cancel_document_change(); } else { crate::macos::close_window(ctx); }
        }
        if !cfg!(target_os = "macos") && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_quit(ctx);
        }
        self.shortcuts(ctx);
        crate::machine_ui::update(self, ctx);
        self.toolbar(ctx, &pal);
        if self.mode == ViewMode::Output {
            crate::output::show(self, ctx);
        } else if self.mode == ViewMode::Create {
            crate::create::show(self, ctx);
        } else if self.mode == ViewMode::Volumetric {
            crate::volumetric_ui::show(self, ctx);
        } else if self.mode == ViewMode::Photo {
            self.show_photo(ctx);
        } else {
            egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
                if self.document_modal() { ui.disable(); }
                theme::paint_canvas(ui.painter(), ui.max_rect(), &pal);
                canvas::show(self, ui)
            });
            self.floating_panels(ctx, &pal);
            self.status_bar(ctx, &pal);
        }
        if let Some(error) = self.vec_ui.take_load_error() { self.notify_error(ctx, error, None); }
        if std::mem::take(&mut self.pick_import) { self.import_dialog(ctx); }
        if std::mem::take(&mut self.notify_tool_return) {
            self.notify(ctx, tr!("Forma añadida. Vuelves a Seleccionar para ajustarla; pulsa S para otra."));
        }
        if self.show_help {
            self.help_sheet(ctx, &pal);
        }
        if let Some(prefs) = self.prefs_ui.show(ctx, &self.prefs, &self.doc, &pal) { self.set_prefs(ctx, prefs); }
        self.save_prefs(ctx);
        self.show_color_panel(ctx);
        crate::palette_ui::show(self, ctx);
        crate::machine_ui::pen_dialog(self, ctx);
        self.unsaved_sheet(ctx, &pal);
        self.toast(ctx, &pal);
        theme::restyle_controls(ctx);
        self.update_traces(ctx);
        self.track_changes(ctx);
        self.recovery.update(ctx,&self.doc,self.rev,&self.file,self.modified());
    }
}

// ── Controles con aspecto macOS ────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub enum Icon {
    Select,
    Strokes,
    Draw,
    Text,
    Shape,
    Image,
    Open,
    Undo,
    Redo,
}

/// Radio del pomo de los segmentos de una sola fila (28 px de alto): una cápsula.
const KNOB_RADIUS: f32 = 14.0;

/// Grupo de botones de la barra: una cápsula de vidrio propia (28 + 2·3 = 34 px de alto).
pub fn capsule<R>(ui: &mut egui::Ui, pal: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    theme::glass_panel_with(ui, pal, KNOB_RADIUS + 3.0, egui::Margin::same(3.0), theme::Material::Capsule, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.horizontal(add).inner
    })
    .inner
}

/// Control segmentado dentro de un panel u hoja: cápsula hundida sin sombra.
pub fn segmented<R>(ui: &mut egui::Ui, pal: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(pal.control)
        .rounding(Rounding::same(KNOB_RADIUS + 2.0))
        .inner_margin(egui::Margin::same(2.0))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(add).inner
        })
        .inner
}

fn mode_key(mode: ViewMode) -> u8 {
    match mode { ViewMode::Design => 0, ViewMode::Create => 1, ViewMode::Photo => 2, ViewMode::Volumetric => 3, ViewMode::Output => 4 }
}

/// El pomo usa una identidad estable aunque cambien las herramientas del espacio.
fn workspace_selector(
    ui: &mut egui::Ui,
    pal: &Palette,
    value: &mut ViewMode,
    options: &[(ViewMode, String)],
) -> bool {
    let before = *value;
    let compact = ui.ctx().screen_rect().width() < 1180.;
    let font = egui::FontId::proportional(if compact { 11.5 } else { 12.5 });
    let padding = if compact { 16. } else { 22. };
    capsule(ui, pal, |ui| {
        // Se reserva el ancho entero y se reparten los segmentos de izquierda a derecha, para
        // que el orden de lectura y de tabulación coincida con el visual aunque la barra se
        // construya de derecha a izquierda.
        let widths: Vec<f32> = options.iter().map(|(_, label)| ui.painter().layout_no_wrap(label.clone(), font.clone(), pal.text).size().x + padding).collect();
        let total = widths.iter().sum::<f32>() + ui.spacing().item_spacing.x * (widths.len().saturating_sub(1)) as f32;
        let (whole, _) = ui.allocate_exact_size(egui::vec2(total, 28.), Sense::hover());
        let mut segments = Vec::new();
        let mut x = whole.left();
        for ((mode, label), width) in options.iter().zip(widths) {
            let rect = Rect::from_min_size(egui::pos2(x, whole.top()), egui::vec2(width, 28.));
            x += width + ui.spacing().item_spacing.x;
            let response = ui.interact(rect, egui::Id::new("workspace-segment").with(mode_key(*mode)), Sense::click());
            if response.clicked() {
                *value = *mode;
            }
            segments.push((*mode, label, rect, response));
        }
        let left = segments
            .iter()
            .map(|(_, _, r, _)| r.left())
            .fold(f32::INFINITY, f32::min);
        if let Some((_, _, rect, _)) = segments.iter().find(|(mode, _, _, _)| *mode == *value) {
            let id = egui::Id::new("workspace-selection");
            let x = left
                + ui.ctx()
                    .animate_value_with_time(id.with("x"), rect.left() - left, 0.16);
            let width = ui
                .ctx()
                .animate_value_with_time(id.with("w"), rect.width(), 0.16);
            let knob =
                Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(width, rect.height()));
            ui.painter().rect_filled(
                knob.translate(egui::vec2(0., 0.5)),
                KNOB_RADIUS,
                Color32::from_black_alpha(18),
            );
            ui.painter().rect_filled(knob, KNOB_RADIUS, pal.knob);
            ui.painter()
                .rect_stroke(knob, KNOB_RADIUS, Stroke::new(0.75f32, pal.rim_hi));
        }
        for (mode, label, rect, response) in segments {
            let selected = *value == mode;
            if !selected && response.hovered() {
                ui.painter()
                    .rect_filled(rect, KNOB_RADIUS, pal.control_hover);
            }
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                font.clone(),
                if selected { pal.text } else { pal.secondary },
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::RadioButton,
                    ui.is_enabled(),
                    selected,
                    label,
                )
            });
            theme::focus_ring(ui, &response);
        }
    });
    *value != before
}

pub fn text_segment(ui: &mut egui::Ui, pal: &Palette, label: &str, selected: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(12.5), pal.text);
    let size = Vec2::new(galley.size().x + 22.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    paint_segment_bg(ui, pal, rect, selected, resp.hovered(), KNOB_RADIUS, true);
    let color = if selected { pal.text } else { pal.secondary };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(12.5), color);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, ui.is_enabled(), selected, label));
    theme::focus_ring(ui, &resp);
    resp
}

/// Control segmentado que ocupa todo el ancho disponible, con segmentos iguales.
pub fn choice<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let pal = theme::current(ui.ctx());
    let mut changed = false;
    let width = ui.available_width();
    egui::Frame::none()
        .fill(pal.control)
        .rounding(Rounding::same(theme::CONTROL_RADIUS + 2.0))
        .inner_margin(egui::Margin::same(2.0))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let n = options.len().max(1) as f32;
            let w = ((width - 4.0 - 2.0 * (n - 1.0)) / n).max(30.0);
            ui.horizontal(|ui| {
                for (v, label) in options {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
                    let selected = *value == *v;
                    paint_segment_bg(ui, &pal, rect, selected, resp.hovered(), theme::CONTROL_RADIUS, false);
                    let color = if selected { pal.text } else { pal.secondary };
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, *label, egui::FontId::proportional(12.0), color);
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, ui.is_enabled(), selected, *label));
                    theme::focus_ring(ui, &resp);
                    if resp.clicked() && !selected {
                        *value = *v;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Pomo del segmento seleccionado. Al cambiar de segmento se desliza hasta el nuevo
/// (el estado vive en el `Ui` del grupo, que es estable entre fotogramas).
fn paint_segment_bg(ui: &egui::Ui, pal: &Palette, rect: Rect, selected: bool, hovered: bool, radius: f32, animate: bool) {
    let r = Rounding::same(radius);
    if selected {
        let knob = if animate {
            // Posición relativa al grupo: si la hoja o la ventana se desplazan, el pomo no «viaja».
            let id = ui.id().with("knob");
            let left = ui.max_rect().left();
            let x = left + ui.ctx().animate_value_with_time(id.with("x"), rect.min.x - left, 0.16);
            let w = ui.ctx().animate_value_with_time(id.with("w"), rect.width(), 0.16);
            Rect::from_min_size(Pos2::new(x, rect.min.y), Vec2::new(w, rect.height()))
        } else {
            rect
        };
        ui.painter().rect_filled(knob.translate(Vec2::new(0.0, 0.5)), r, Color32::from_black_alpha(18));
        ui.painter().rect_filled(knob, r, pal.knob);
        ui.painter().rect_stroke(knob, r, Stroke::new(0.75f32, pal.rim_hi));
    } else if hovered {
        ui.painter().rect_filled(rect, r, pal.control_hover);
    }
}

pub fn icon_button(ui: &mut egui::Ui, pal: &Palette, icon: Icon, selected: bool) -> egui::Response {
    icon_button_state(ui, pal, icon, selected, true)
}

pub fn icon_button_state(ui: &mut egui::Ui, pal: &Palette, icon: Icon, selected: bool, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.add_enabled_ui(enabled, |ui| ui.allocate_exact_size(Vec2::new(34.0, 28.0), Sense::click())).inner;
    paint_segment_bg(ui, pal, rect, selected, resp.hovered() && enabled, KNOB_RADIUS, true);
    let color = if !enabled {
        pal.secondary.gamma_multiply(0.45)
    } else if selected {
        pal.accent
    } else {
        pal.text
    };
    paint_icon(ui.painter(), rect.center(), icon, Stroke::new(1.5f32, color));
    let (label, toggle) = match icon {
        Icon::Strokes => (ts!("Seleccionar trazos (A)"), true),
        Icon::Select => (ts!("Seleccionar y mover (V)"), true),
        Icon::Draw => (ts!("Mano alzada (P)"), true),
        Icon::Text => (ts!("Texto manuscrito (T)"), true),
        Icon::Shape => (ts!("Formas y generativos (S)"), true),
        Icon::Image => (ts!("Foto → dibujo (I)"), false),
        Icon::Open => (ts!("Abrir proyecto, SVG o imagen (⌘O)"), false),
        Icon::Undo => (ts!("Deshacer (⌘Z)"), false),
        Icon::Redo => (ts!("Rehacer (⇧⌘Z)"), false),
    };
    resp.widget_info(|| if toggle {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, resp.enabled(), selected, label)
    } else { egui::WidgetInfo::labeled(egui::WidgetType::Button, resp.enabled(), label) });
    theme::focus_ring(ui, &resp);
    resp
}

fn paint_icon(painter: &egui::Painter, c: Pos2, icon: Icon, s: Stroke) {
    let pt = |x: f32, y: f32| c + Vec2::new(x, y);
    let poly = |pts: Vec<Pos2>, closed: bool| {
        if closed {
            egui::Shape::closed_line(pts, s)
        } else {
            egui::Shape::line(pts, s)
        }
    };
    match icon {
        Icon::Strokes => {
            let points = [pt(-7.0, 4.0), pt(-1.0, -4.0), pt(6.5, 1.5)];
            painter.add(egui::Shape::line(points.to_vec(), s));
            for point in points { painter.circle_filled(point, 2.0, s.color); }
        }
        Icon::Select => {
            painter.add(poly(vec![pt(-4.0, -7.5), pt(-4.0, 6.0), pt(-0.8, 2.8), pt(1.8, 8.0), pt(4.0, 7.0), pt(1.4, 1.9), pt(5.5, 1.9)], true));
        }
        Icon::Draw => {
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let t = i as f32 / 24.0;
                    pt(-8.0 + 16.0 * t, (t * 9.0).sin() * 3.0 - 1.0 + t * 2.0)
                })
                .collect();
            painter.add(egui::Shape::line(pts, s));
        }
        Icon::Text => {
            painter.line_segment([pt(-6.0, -6.5), pt(6.0, -6.5)], s);
            painter.line_segment([pt(0.0, -6.5), pt(0.0, 7.0)], s);
            painter.line_segment([pt(-2.5, 7.0), pt(2.5, 7.0)], s);
        }
        Icon::Shape => {
            painter.circle_stroke(pt(-2.5, 2.0), 5.0, s);
            painter.add(poly(vec![pt(0.5, -7.5), pt(7.5, -7.5), pt(7.5, -0.5), pt(0.5, -0.5)], true));
        }
        Icon::Image => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::new(16.0, 13.0)), Rounding::same(2.5), s);
            painter.add(egui::Shape::line(vec![pt(-8.0, 5.0), pt(-3.0, -0.5), pt(0.5, 3.0), pt(3.5, 0.0), pt(8.0, 4.5)], s));
            painter.circle_stroke(pt(3.5, -3.0), 1.5, s);
        }
        Icon::Open => {
            painter.add(poly(vec![pt(-7.0, -6.0), pt(-2.5, -6.0), pt(-1.0, -4.0), pt(7.0, -4.0), pt(7.0, 6.0), pt(-7.0, 6.0)], true));
            painter.line_segment([pt(-7.0, -1.5), pt(7.0, -1.5)], s);
        }
        Icon::Undo | Icon::Redo => {
            let flip = if matches!(icon, Icon::Undo) { -1.0 } else { 1.0 };
            let arc: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.0 - i as f32 / 16.0) - 0.2;
                    pt(flip * (-a.cos() * 6.0), -a.sin() * 5.0 + 3.0)
                })
                .collect();
            let head = arc[0];
            painter.add(egui::Shape::line(arc, s));
            painter.add(egui::Shape::line(vec![head + Vec2::new(flip * -3.0, -2.5), head, head + Vec2::new(flip * 2.5, -3.0)], s));
        }
    }
}

fn help_groups(ui: &mut egui::Ui, pal: &Palette, groups: &[(&str, &[(&str, &str)])]) {
    for (title, rows) in groups {
        theme::section(ui, &crate::i18n::tr_key(title).to_uppercase());
        egui::Grid::new(("help", *title)).num_columns(2).spacing([14.0, 5.0]).show(ui, |ui| {
            for (keys, what) in rows.iter() {
                ui.label(egui::RichText::new(crate::i18n::tr_key(keys)).strong().color(pal.text));
                ui.label(egui::RichText::new(crate::i18n::tr_key(what)).color(pal.secondary));
                ui.end_row();
            }
        });
        ui.add_space(8.0);
    }
}

#[cfg(test)]
mod generated_history_tests {
    use super::*;
    use vectorio_core::{p, BBox, generate::{self, Detail, GenContext, GeneratedGroup, Params}};
    #[test]
    fn placing_and_updating_a_recipe_each_take_one_undo_step(){
        let ctx=egui::Context::default();let mut app=VectorioApp::blank(&ctx);
        let context=GenContext{area:BBox{min:p(15.0,15.0),max:p(195.0,282.0)},seed:1,pen_mm:0.4,detail:Detail::Final};
        let generator=generate::find("spiro").unwrap();let params=Params::default().normalized(&generator.params());
        let recipe=GeneratedGroup::new("Prueba".into(),"spiro".into(),params.clone(),1,context.area);
        let ids=app.doc.put_generated(recipe.clone(),generator.generate(&params,&context).unwrap(),None);
        app.set_selection(ids);app.track_changes(&ctx);let first=app.doc.clone();assert_eq!(app.history.len(),1);
        let mut context=context;context.seed=2;let mut recipe=recipe;recipe.seed=2;
        app.doc.put_generated(recipe,generator.generate(&params,&context).unwrap(),Some(first.generated[0].id));
        app.track_changes(&ctx);let updated=app.doc.clone();assert_eq!(app.history.len(),2);
        app.undo();assert_eq!(app.doc,first);app.undo();assert!(app.doc.generated.is_empty());
        app.redo();assert_eq!(app.doc,first);app.redo();assert_eq!(app.doc,updated);
    }
}

#[cfg(test)]
mod document_session_tests {
    use super::*;

    #[test]
    fn quit_is_deferred_to_the_viewport_and_an_accepted_close_is_not_hidden() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let output = ctx.run(Default::default(), |ctx| app.request_quit(ctx));
        assert!(app.quitting);
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands.contains(&egui::ViewportCommand::Close));
        assert!(!commands.contains(&egui::ViewportCommand::CancelClose));
        assert!(!commands.contains(&egui::ViewportCommand::Visible(false)));
    }

    #[test]
    fn cancelling_or_failing_a_save_never_authorizes_replacing_the_document() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        app.doc.paper.name = "Sin guardar".into();
        app.file = Some(PathBuf::from("original.vectorio"));
        let before = app.doc.clone();
        assert!(!app.resolve_unsaved(UnsavedChoice::Cancel, |_| panic!("no debe guardar")));
        assert!(!app.resolve_unsaved(UnsavedChoice::Save, |_| false)); // Cancelar Guardar como.
        let dir = tempfile::tempdir().unwrap();
        assert!(!app.resolve_unsaved(UnsavedChoice::Save, |app| app.save_to(dir.path()).is_ok()));
        assert_eq!(app.doc, before);
        assert_eq!(app.file, Some(PathBuf::from("original.vectorio")));
        assert!(app.modified());
        assert!(app.resolve_unsaved(UnsavedChoice::Discard, |_| panic!("no debe guardar")));
    }

    #[test]
    fn custom_buttons_expose_names_roles_and_keyboard_activation() {
        let _locale = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = VectorioApp::blank(&ctx);
        let mut button = None;
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let pal = theme::current(ctx);
                let response = icon_button(ui, &pal, Icon::Select, true);
                button = Some(response.id);
                response.request_focus();
                text_segment(ui, &pal, "Diseño", true);
                icon_button_state(ui, &pal, Icon::Undo, false, false);
            });
        });
        let tree = output.platform_output.accesskit_update.unwrap();
        let find = |label: &str| tree.nodes.iter().find(|(_, node)| node.name() == Some(label)).map(|(_, node)| node).unwrap();
        assert_eq!(format!("{:?}", find("Seleccionar y mover (V)").role()), "RadioButton");
        assert_eq!(format!("{:?}", find("Diseño").role()), "RadioButton");
        assert!(find("Deshacer (⌘Z)").is_disabled());
        for key in [egui::Key::Enter, egui::Key::Space] {
            ctx.memory_mut(|m| m.request_focus(button.unwrap()));
            let input = egui::RawInput { events: vec![egui::Event::Key { key, physical_key: None, pressed: true,
                repeat: false, modifiers: egui::Modifiers::NONE }], ..Default::default() };
            let _ = ctx.run(input, |ctx| {
                app.shortcuts(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    let pal = theme::current(ctx);
                    assert!(icon_button(ui, &pal, Icon::Select, true).clicked());
                });
            });
        }
    }

    #[test]
    fn opening_b_cannot_undo_a_or_save_a_over_b_and_clears_inflight_results() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.vectorio");
        let b = dir.path().join("b.vectorio");
        app.doc.paper.name = "A".into();
        app.track_changes(&ctx);
        app.save_to(&a).unwrap();
        assert!(app.can_undo());
        let doc_b = Document { paper: vectorio_core::Paper { name: "B".into(), ..Default::default() }, ..Default::default() };
        std::fs::write(&b, doc_b.to_json()).unwrap();
        app.open_path(&ctx, &b);
        assert!(!app.can_undo() && !app.can_redo() && !app.modified());
        assert!(app.job.is_none() && app.trace_jobs.is_empty() && app.rendered.items.is_empty());
        app.track_changes(&ctx);
        app.undo();
        app.save_to(&b).unwrap();
        assert_eq!(Document::from_json(&std::fs::read_to_string(&b).unwrap()).unwrap(), doc_b);
        assert_eq!(app.file.as_ref(), Some(&b));
        assert_eq!(Document::from_json(&std::fs::read_to_string(a).unwrap()).unwrap().paper.name, "A");
    }

    #[test]
    fn repeated_new_or_quit_waits_for_one_decision_and_escape_keeps_the_document() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        app.doc.paper.margin_mm += 1.0;
        let before = app.doc.clone();
        let input = egui::RawInput { events: vec![egui::Event::Key {
            key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE,
        }], ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            app.new_document(ctx);
            app.request_quit(ctx);
            assert!(matches!(app.pending_change, Some(DocumentChange::New)));
            app.run(ctx, commands::Command::Delete);
            app.unsaved_sheet(ctx, &theme::current(ctx));
            app.shortcuts(ctx);
        });
        assert!(app.pending_change.is_none());
        assert!(!app.quitting && app.modified());
        assert_eq!(app.doc, before);
    }

    #[test]
    fn deleting_multiple_strokes_preserves_locked_and_hidden_objects_and_undo_redo() {
        use crate::strokes::StrokeId;
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let lines = vec![vec![vectorio_core::p(0.,0.),vectorio_core::p(10.,0.)], vec![vectorio_core::p(0.,5.),vectorio_core::p(10.,5.)]];
        for _ in 0..3 { app.doc.add(Item::new("Capa", Geometry::Path { lines: lines.clone(), closed: false }, P::default())); }
        app.doc.items[1].locked = true;
        app.doc.items[2].visible = false;
        app.track_changes(&ctx);
        let before = app.doc.clone();
        app.choose_tool(Tool::Strokes);
        app.stroke_selection.select_all(&app.doc);
        assert_eq!(app.stroke_selection.ids.len(), 2);
        app.stroke_selection.pick(Some(StrokeId(app.doc.items[0].id, 1)), true);
        app.run(&ctx, commands::Command::Delete);
        app.track_changes(&ctx);
        let after = app.doc.clone();
        assert_eq!(after.items[0].local_lines().0, vec![lines[1].clone()]);
        assert_eq!(&before.items[1..], &after.items[1..]);
        app.undo();
        assert_eq!(app.doc, before);
        app.redo();
        assert_eq!(app.doc, after);
    }

    #[test]
    fn only_a_successful_save_clears_the_modified_state() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        assert!(!app.modified());
        app.doc.paper.margin_mm += 1.0;
        assert!(app.modified());
        app.track_changes(&ctx);
        let dir = tempfile::tempdir().unwrap();
        assert!(app.save_to(dir.path()).is_err());
        assert!(app.modified() && app.file.is_none());
        app.save_to(&dir.path().join("a.vectorio")).unwrap();
        assert!(!app.modified());
        app.doc.paper.margin_mm += 1.0;
        app.track_changes(&ctx);
        assert!(app.modified());
        app.undo();
        assert!(!app.modified());
    }

    #[test]
    fn collecting_a_trace_cache_does_not_mark_an_unchanged_project_as_modified() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let mut doc = Document::default();
        let traced = Traced::new(vectorio_core::Coverage { map: vectorio_core::raster::Grid::new(2, 2), mm_per_px: 1.0 }, Default::default(), 42);
        let id = doc.add(Item::new("Tinta", Geometry::Traced(Box::new(traced)), P::default()));
        app.replace_document(doc, None);
        let key = app.doc.trace_keys()[&id].clone();
        let (tx, rx) = mpsc::channel();
        tx.send(vec![vec![vectorio_core::p(0.0, 0.0), vectorio_core::p(1.0, 1.0)]]).unwrap();
        app.trace_jobs.insert(id, (key, rx));
        app.update_traces(&ctx);
        assert!(!app.modified());
        app.track_changes(&ctx);
        assert!(!app.can_undo());
    }

    #[test]
    fn starting_a_new_document_clears_its_file_history_and_selection() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let doc = Document::default();
        app.history.push(doc.clone());
        app.future.push(doc.clone());
        app.selected = Some(1);
        app.file = Some(PathBuf::from("anterior.vectorio"));
        app.new_document(&ctx);
        assert!(app.file.is_none() && app.selection().is_empty());
        assert!(!app.can_undo() && !app.can_redo() && !app.modified());
    }

    #[test]
    fn export_waits_for_current_trace_keys_even_before_a_worker_has_started() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let mut traced = Traced::new(vectorio_core::Coverage {
            map: vectorio_core::raster::Grid::new(2, 2), mm_per_px: 1.0,
        }, Default::default(), 42);
        traced.made = Some(traced.key(app.doc.pens[0].width_mm));
        let mut item = Item::new("Tinta", Geometry::Traced(Box::new(traced)), P::default());
        item.fill.pen = 0;
        let id = app.doc.add(item);
        assert!(app.check_export_ready(&ctx));
        app.doc.get_mut(id).unwrap().traced_mut().unwrap().trace.angle_deg += 15.0;
        assert!(!app.tracing());
        assert!(!app.check_export_ready(&ctx));
        let key = app.doc.trace_keys()[&id].clone();
        app.doc.get_mut(id).unwrap().traced_mut().unwrap().made = Some(key);
        assert!(app.check_export_ready(&ctx));
    }
}
#[cfg(test)]
mod volume_history_tests{
    use super::*;
    use vectorio_core::volumetric::*;
    #[test]
    fn placing_and_updating_a_3d_object_each_form_one_undo_step(){
        let ctx=egui::Context::default();let mut app=VectorioApp::blank(&ctx);let mut recipe=Recipe::new(Primitive::Cube.mesh(),Settings::default());recipe.inks=app.doc.pens.clone();
        let d=Engine::default().draw_recipe(&recipe,&||false).unwrap();let volume=VolumeObject{id:0,name:"Cubo".into(),recipe:recipe.clone(),placement:Default::default(),bindings:vec![]};
        let ids=app.doc.put_volume(volume.clone(),&d,None).unwrap();let g=app.doc.get(ids[0]).unwrap().group.unwrap();app.set_selection(ids);app.track_changes(&ctx);let first=app.doc.clone();
        recipe.settings.yaw=120.;let d=Engine::default().draw_recipe(&recipe,&||false).unwrap();app.doc.put_volume(VolumeObject{recipe,..volume},&d,Some(g)).unwrap();app.track_changes(&ctx);let updated=app.doc.clone();
        assert_eq!(app.history.len(),2);app.undo();assert_eq!(app.doc,first);app.undo();assert!(app.doc.volumes.is_empty());app.redo();assert_eq!(app.doc,first);app.redo();assert_eq!(app.doc,updated);
    }
}
#[cfg(test)]
mod workspace_coherence_tests{
    use super::*;
    fn frame(app:&mut VectorioApp,ctx:&egui::Context,time:f64)->egui::FullOutput{
        ctx.run(egui::RawInput{time:Some(time),screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(1440.,900.))),..Default::default()},|ctx|app.toolbar(ctx,&theme::current(ctx)))
    }
    #[test]
    fn workspace_knob_slides_between_modes_and_export_keeps_its_tinted_style(){
        let _lock=crate::i18n::locale_lock();rust_i18n::set_locale("es");let ctx=egui::Context::default();let mut app=VectorioApp::blank(&ctx);let pal=theme::current(&ctx);
        let knob=|out:&egui::FullOutput|out.shapes.iter().find_map(|s|if let egui::Shape::Rect(r)=&s.shape{(r.fill==pal.knob&&r.rect.center().x>900.).then_some(r.rect)}else{None}).expect("pomo de espacios");
        let _=frame(&mut app,&ctx,0.);let out=frame(&mut app,&ctx,0.3);let before=knob(&out);
        app.mode=ViewMode::Volumetric;let transition=knob(&frame(&mut app,&ctx,0.316));let settled=knob(&frame(&mut app,&ctx,0.6));
        assert!(before.center().distance(settled.center())>50.);assert!(transition.center().distance(settled.center())>5.,"el selector salta al destino sin animación");
        for(index,mode)in [ViewMode::Design,ViewMode::Create,ViewMode::Photo,ViewMode::Volumetric,ViewMode::Output].into_iter().enumerate(){
            app.mode=mode;let out=frame(&mut app,&ctx,1.+index as f64);
            let export=out.shapes.iter().find_map(|s|if let egui::Shape::Text(t)=&s.shape{(t.galley.text()=="Exportar").then_some(t)}else{None}).unwrap();
            assert!(export.fallback_color==Color32::WHITE,"Exportar cambia de estilo en {mode:?}");
        }
    }
}
#[cfg(test)]
mod compact_workspace_tests{
    use super::*;
    #[test]
    fn the_five_workspace_segments_remain_visible_in_small_windows_and_both_languages(){
        let _lock=crate::i18n::locale_lock();
        for lang in ["es","en"]{rust_i18n::set_locale(lang);
            for mode in [ViewMode::Design,ViewMode::Create,ViewMode::Photo,ViewMode::Volumetric,ViewMode::Output]{
                let ctx=egui::Context::default();let mut app=VectorioApp::blank(&ctx);app.mode=mode;app.tool=Tool::Shape;let mut output=None;
                for frame in 0..3{output=Some(ctx.run(egui::RawInput{time:Some(frame as f64),screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(980.,620.))),..Default::default()},|ctx|app.toolbar(ctx,&theme::current(ctx))));}
                for label in [tr!("Diseño"),tr!("Escenas"),tr!("Foto"),tr!("Modelo 3D"),tr!("Salida"),tr!("Exportar")]{
                    let rect=output.as_ref().unwrap().shapes.iter().find_map(|s|if let egui::Shape::Text(t)=&s.shape{(t.galley.text()==label).then_some(Rect::from_min_size(t.pos,t.galley.size()))}else{None}).unwrap_or_else(||panic!("Falta {label} ({lang}, {mode:?})"));
                    assert!(rect.left()>=0.&&rect.right()<=980.,"{label}: {rect:?} ({lang},{mode:?})");
                }
            }
        }rust_i18n::set_locale("es");
    }
}
