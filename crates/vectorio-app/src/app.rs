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
    Draw,
    Text,
    Shape,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    Design,
    Plotter,
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
    ("Blanco", [255, 255, 255]),
    ("Marfil", [250, 245, 232]),
    ("Kraft", [205, 170, 125]),
    ("Gris", [120, 122, 128]),
    ("Negro", [22, 22, 24]),
];

pub struct VectorioApp {
    pub doc: Document,
    committed: Document,
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
    color_original: Option<Document>,
    color_targets: Vec<usize>,
    /// Rotulador que se está renombrando.
    pub renaming_pen: Option<usize>,
    pub renaming_item: Option<u64>,
    pub rename_focus: bool,
    rename_original: String,
    pub tool: Tool,
    pub shape_kind: usize,
    pub active_pen: usize,
    pub view: View,
    pub mode: ViewMode,
    pub sim: Sim,
    pub paper_tint: usize,
    pub file: Option<PathBuf>,
    pub vec_ui: VectorizeUi,
    pub interaction: Interaction,
    pub focus_text: bool,
    toast: Option<(String, f64)>,
    pub show_help: bool,
    menu: Option<NativeMenu>,
    /// Estilos de trazo guardados por el usuario y nombre del que se va a guardar.
    pub styles: Vec<crate::styles::SavedStyle>,
    pub style_name: String,
}

impl VectorioApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::blank(&cc.egui_ctx);
        app.menu = NativeMenu::install(&cc.egui_ctx);
        crate::macos::install_reopen_handler();
        // Archivo pasado al abrir (`vectorio dibujo.vectorio`, `vectorio foto.jpg`).
        if let Some(path) = std::env::args_os().nth(1).map(PathBuf::from).filter(|p| p.exists()) {
            app.open_path(&cc.egui_ctx, &path);
        }
        app
    }

    /// App con el documento de bienvenida, sin menú nativo ni archivos abiertos.
    pub(crate) fn blank(ctx: &egui::Context) -> Self {
        theme::install(ctx);
        let mut doc = Document::default();
        welcome(&mut doc);
        VectorioApp {
            committed: doc.clone(),
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
            color_original: None,
            color_targets: vec![],
            renaming_pen: None,
            renaming_item: None,
            rename_focus: false,
            rename_original: String::new(),
            tool: Tool::Select,
            shape_kind: 1,
            active_pen: 0,
            view: View::default(),
            mode: ViewMode::Design,
            sim: Sim::default(),
            paper_tint: 0,
            file: None,
            vec_ui: VectorizeUi::default(),
            interaction: Interaction::default(),
            focus_text: false,
            toast: None,
            show_help: false,
            menu: None,
        }
    }

    pub fn notify(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        self.toast = Some((msg.into(), ctx.input(|i| i.time) + 3.5));
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
        ids.dedup();
        self.selected = ids.pop();
        self.also = ids;
        self.also_anchor = self.selected;
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
            self.color_picker.subtitle = format!("{} · tinta compartida", pen.name);
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
        self.color_picker.title = if fill { "Color del relleno" } else { "Color del contorno" }.into();
        self.color_picker.subtitle = if self.color_targets.len() == 1 { name } else { format!("{} objetos seleccionados", self.color_targets.len()) };
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
            for doc in [&mut self.doc, &mut self.committed] {
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

    fn track_changes(&mut self, ctx: &egui::Context) {
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

    // ── Archivos ────────────────────────────────────────────────────────────────

    pub fn new_document(&mut self) {
        self.finish_rename(false);
        self.color_edit = None;
        self.color_original = None;
        self.pen_focus = false;
        self.doc = Document::default();
        self.selected = None;
        self.fix_selection();
        self.file = None;
        self.view.fitted = false;
    }

    pub fn open_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Todo lo compatible", &["vectorio", "svg", "png", "jpg", "jpeg", "tif", "tiff", "webp", "bmp", "gif"])
            .add_filter("Proyecto Vectorio", &["vectorio"])
            .add_filter("SVG", &["svg"])
            .add_filter("Imagen", &["png", "jpg", "jpeg", "tif", "tiff", "webp", "bmp", "gif"])
            .pick_file()
        {
            self.open_path(ctx, &path);
        }
    }

    pub fn open_path(&mut self, ctx: &egui::Context, path: &Path) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "vectorio" | "json" => match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| Document::from_json(&s)) {
                Ok(doc) => {
                    self.finish_rename(false);
                    self.color_edit = None;
                    self.color_original = None;
                    self.pen_focus = false;
                    self.doc = doc;
                    self.selected = None;
                    self.fix_selection();
                    self.file = Some(path.to_path_buf());
                    self.view.fitted = false;
                }
                Err(e) => self.notify(ctx, format!("No se pudo abrir el proyecto: {e}")),
            },
            "svg" => self.import_svg(ctx, path),
            _ => match self.vec_ui.load(ctx, path) {
                Ok(()) => {}
                Err(e) => self.notify(ctx, format!("No se pudo abrir la imagen: {e}")),
            },
        }
    }

    fn import_svg(&mut self, ctx: &egui::Context, path: &Path) {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => return self.notify(ctx, format!("No se pudo leer el SVG: {e}")),
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
                self.selected = last;
                self.notify(ctx, format!("{n} trazados importados de {name}"));
            }
            Ok(_) => self.notify(ctx, "El SVG no contiene trazados"),
            Err(e) => self.notify(ctx, format!("SVG no válido: {e}")),
        }
    }

    pub fn save(&mut self, ctx: &egui::Context, save_as: bool) {
        let path = match (&self.file, save_as) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new().add_filter("Proyecto Vectorio", &["vectorio"]).set_file_name("dibujo.vectorio").save_file(),
        };
        if let Some(path) = path {
            match std::fs::write(&path, self.doc.to_json()) {
                Ok(()) => {
                    self.notify(ctx, format!("Guardado {}", path.file_name().and_then(|n| n.to_str()).unwrap_or("")));
                    self.file = Some(path);
                }
                Err(e) => self.notify(ctx, format!("No se pudo guardar: {e}")),
            }
        }
    }

    pub fn export_svg(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().add_filter("SVG", &["svg"]).set_file_name("vectorio-plotter.svg").save_file() else {
            return;
        };
        let layers = self.doc.layers();
        match std::fs::write(&path, svg_out::export(&self.doc, &layers)) {
            Ok(()) => self.notify(ctx, format!("SVG exportado con {} capas (una por rotulador)", layers.len())),
            Err(e) => self.notify(ctx, format!("No se pudo exportar: {e}")),
        }
    }

    pub fn export_text_pages(&mut self, ctx: &egui::Context, id: u64) {
        let Some(item) = self.doc.get(id) else { return };
        let Geometry::Text(spec) = &item.geometry else { return };
        let count = vectorio_core::font::compose(spec).page_count;
        let name: String = item.name.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_').take(50).collect();
        let name = if name.is_empty() { "texto" } else { &name };
        let Some(folder) = rfd::FileDialog::new().set_title("Carpeta para las páginas SVG").pick_folder() else { return };
        let mut doc = self.doc.clone();
        for page in 0..count {
            if let Some(Item { geometry: Geometry::Text(spec), .. }) = doc.get_mut(id) { spec.page = page; }
            let svg = svg_out::export(&doc, &doc.layers());
            if let Err(error) = std::fs::write(folder.join(format!("{name}-pagina-{:03}.svg", page + 1)), svg) {
                self.notify(ctx, format!("No se pudieron exportar las páginas: {error}"));
                return;
            }
        }
        self.notify(ctx, format!("{count} páginas SVG exportadas"));
    }

    pub fn export_per_pen(&mut self, ctx: &egui::Context) {
        let Some(dir) = rfd::FileDialog::new().set_title("Carpeta para un SVG por rotulador").pick_folder() else {
            return;
        };
        let layers = self.doc.layers();
        let files = svg_out::export_per_pen(&self.doc, &layers);
        let n = files.len();
        let mut err = None;
        for (name, content) in files {
            if let Err(e) = std::fs::write(dir.join(name), content) {
                err = Some(e);
            }
        }
        match err {
            None => self.notify(ctx, format!("{n} archivos SVG exportados")),
            Some(e) => self.notify(ctx, format!("Error al exportar: {e}")),
        }
    }

    // ── Entrada ─────────────────────────────────────────────────────────────────

    fn dropped_files(&mut self, ctx: &egui::Context) {
        let files: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for f in files {
            self.open_path(ctx, &f);
        }
    }

    // ── Interfaz ────────────────────────────────────────────────────────────────

    fn toolbar(&mut self, ctx: &egui::Context, pal: &Palette) {
        let frame = egui::Frame::none().fill(pal.bar).stroke(Stroke::new(1.0f32, pal.separator));
        egui::TopBottomPanel::top("toolbar").exact_height(theme::TOOLBAR_H).frame(frame).show(ctx, |ui| {
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
                ui.add_space(theme::TRAFFIC_LIGHTS_W);
                let title = self
                    .file
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Sin título".into());
                ui.vertical(|ui| {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Vectorio").size(13.5).strong().color(pal.text));
                    ui.label(egui::RichText::new(title).size(11.0).color(pal.secondary));
                });
                ui.add_space(18.0);

                segmented(ui, pal, |ui| {
                    for (tool, icon, tip) in [
                        (Tool::Select, Icon::Select, "Seleccionar y mover (V)"),
                        (Tool::Draw, Icon::Draw, "Mano alzada (P)"),
                        (Tool::Text, Icon::Text, "Texto manuscrito (T)"),
                        (Tool::Shape, Icon::Shape, "Formas y generativos (S)"),
                    ] {
                        if icon_button(ui, pal, icon, self.tool == tool).on_hover_text(tip).clicked() {
                            self.tool = tool;
                        }
                    }
                });
                if self.tool == Tool::Shape {
                    let presets = Shape::presets();
                    egui::ComboBox::from_id_salt("shape-kind")
                        .selected_text(presets[self.shape_kind].label())
                        .width(130.0)
                        .show_ui(ui, |ui| {
                            for (i, s) in presets.iter().enumerate() {
                                ui.selectable_value(&mut self.shape_kind, i, s.label());
                            }
                        });
                }
                ui.add_space(6.0);
                segmented(ui, pal, |ui| {
                    if icon_button(ui, pal, Icon::Image, false).on_hover_text("Foto → dibujo (I)").clicked() {
                        self.vec_ui.open_dialog(ctx);
                    }
                    if icon_button(ui, pal, Icon::Open, false).on_hover_text("Abrir proyecto, SVG o imagen (⌘O)").clicked() {
                        self.open_dialog(ctx);
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    let export = theme::accent_button(ui, "Exportar");
                    let popup_id = egui::Id::new("export-menu");
                    if export.clicked() {
                        ui.memory_mut(|m| m.toggle_popup(popup_id));
                    }
                    egui::popup_below_widget(ui, popup_id, &export, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                        ui.set_min_width(220.0);
                        if ui.button("SVG para plotter (⌘E)").clicked() {
                            self.export_svg(ctx);
                        }
                        if ui.button("Un SVG por rotulador…").clicked() {
                            self.export_per_pen(ctx);
                        }
                        ui.separator();
                        if ui.button("Guardar proyecto (⌘S)").clicked() {
                            self.save(ctx, false);
                        }
                        if ui.button("Guardar como… (⇧⌘S)").clicked() {
                            self.save(ctx, true);
                        }
                        if ui.button("Nuevo documento (⌘N)").clicked() {
                            self.new_document();
                        }
                    });
                    ui.add_space(8.0);
                    segmented(ui, pal, |ui| {
                        for (mode, label) in [(ViewMode::Plotter, "Plotter"), (ViewMode::Design, "Diseño")] {
                            let sel = self.mode == mode;
                            if text_segment(ui, pal, label, sel).clicked() {
                                self.mode = mode;
                                if mode == ViewMode::Plotter && self.sim.progress <= 0.0 {
                                    self.sim.progress = 1.0;
                                }
                            }
                        }
                    });
                    ui.add_space(8.0);
                    segmented(ui, pal, |ui| {
                        let redo = icon_button_state(ui, pal, Icon::Redo, false, self.can_redo());
                        if redo.on_hover_text("Rehacer (⇧⌘Z)").clicked() && self.can_redo() {
                            self.redo();
                        }
                        let undo = icon_button_state(ui, pal, Icon::Undo, false, self.can_undo());
                        if undo.on_hover_text("Deshacer (⌘Z)").clicked() && self.can_undo() {
                            self.undo();
                        }
                    });
                });
            });
        });
    }

    fn floating_panels(&mut self, ctx: &egui::Context, pal: &Palette) {
        let screen = ctx.screen_rect();
        let top = theme::TOOLBAR_H + theme::INSET;
        let avail_h = screen.height() - top - theme::INSET - 64.0;
        egui::Area::new(egui::Id::new("left-panel"))
            .fixed_pos(Pos2::new(theme::INSET, top))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
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
                theme::glass_panel_with(ui, pal, 22.0, egui::Margin::symmetric(16.0, 8.0), |ui| {
                    ui.set_max_width(screen.width() - 2.0 * (inspector::RIGHT_W + 60.0));
                    ui.horizontal(|ui| {
                        let st = &self.rendered.stats;
                        let fmt_m = |mm: f64| if mm >= 1000.0 { format!("{:.1} m", mm / 1000.0).replace('.', ",") } else { format!("{mm:.0} mm") };
                        let mins = st.seconds / 60.0;
                        let time = if mins >= 60.0 { format!("{:.0} h {:02.0} min", (mins / 60.0).floor(), mins % 60.0) } else { format!("{mins:.0} min") };
                        if self.mode == ViewMode::Plotter {
                            let label = if self.sim.playing { "Pausa" } else { "Reproducir" };
                            if ui.button(label).clicked() {
                                if !self.sim.playing && self.sim.progress >= 1.0 {
                                    self.sim.progress = 0.0;
                                }
                                self.sim.playing = !self.sim.playing;
                            }
                            ui.add(egui::Slider::new(&mut self.sim.progress, 0.0..=1.0).show_value(false));
                            egui::ComboBox::from_id_salt("sim-speed").width(64.0).selected_text(format!("×{:.0}", self.sim.speed)).show_ui(ui, |ui| {
                                for s in [5.0, 20.0, 60.0, 200.0] {
                                    ui.selectable_value(&mut self.sim.speed, s, format!("×{s:.0}"));
                                }
                            });
                            ui.checkbox(&mut self.sim.show_travel, "Desplazamientos");
                            ui.separator();
                        }
                        let info = format!(
                            "Dibujo {} · Viajes {} · {} subidas · {} rotulador{} · ≈ {}",
                            fmt_m(st.draw_mm),
                            fmt_m(st.travel_mm),
                            st.lifts,
                            self.rendered.layers.len(),
                            if self.rendered.layers.len() == 1 { "" } else { "es" },
                            time
                        );
                        ui.label(egui::RichText::new(info).color(pal.secondary));
                        if self.is_computing() {
                            ui.spinner();
                        }
                        ui.separator();
                        ui.label(egui::RichText::new(format!("{:.0} %", self.view.zoom_percent())).color(pal.secondary));
                        if ui.small_button("Ajustar").on_hover_text("⌘0").clicked() {
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
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(if ui.visuals().dark_mode { 110 } else { 60 }));
            resp.clicked()
        });
        ctx.move_to_top(veil.response.layer_id);
        if veil.inner {
            self.show_help = false;
        }
        egui::Area::new(egui::Id::new("help-sheet")).anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, theme::TOOLBAR_H / 2.0)).order(egui::Order::Foreground).show(ctx, |ui| {
            theme::glass_frame(pal).fill(pal.bar.to_opaque()).inner_margin(egui::Margin::same(22.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Atajos de teclado").size(15.0).strong().color(pal.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("Esc para cerrar").color(pal.secondary));
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
        if let Some((msg, until)) = self.toast.clone() {
            if now > until {
                self.toast = None;
                return;
            }
            egui::Area::new(egui::Id::new("toast"))
                .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, theme::TOOLBAR_H + 16.0))
                .order(egui::Order::Foreground)
                .interactable(false)
                .show(ctx, |ui| {
                    theme::glass_frame(pal).rounding(Rounding::same(20.0)).inner_margin(egui::Margin::symmetric(18.0, 9.0)).show(ui, |ui| {
                        ui.label(msg);
                    });
                });
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}

impl eframe::App for VectorioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let pal = theme::current(ctx);
        self.dropped_files(ctx);
        let picked = self.menu.as_ref().map(|m| m.poll()).unwrap_or_default();
        for cmd in picked {
            // Con una hoja abierta, del menú solo valen cerrar y lo que no toca el documento.
            if (self.vec_ui.open || self.color_edit.is_some()) && !matches!(cmd, commands::Command::CloseWindow) {
                continue;
            }
            self.run(ctx, cmd);
        }
        // El botón rojo oculta la ventana igual que ⌘W; ⌘Q sale del programa.
        if cfg!(target_os = "macos") && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            crate::macos::close_window(ctx);
        }
        self.shortcuts(ctx);
        self.toolbar(ctx, &pal);
        egui::CentralPanel::default().frame(egui::Frame::none().fill(pal.canvas)).show(ctx, |ui| canvas::show(self, ui));
        self.floating_panels(ctx, &pal);
        self.status_bar(ctx, &pal);
        if let Some(import) = self.vec_ui.show(ctx, &self.doc) {
            self.doc.pens.extend(import.pens);
            let replacing = import.replacing.is_some();
            let ids = self.doc.add_photo_items(import.items, import.photo, import.replacing);
            self.set_selection(ids);
            self.tool = Tool::Select;
            self.notify(ctx, if replacing {
                "Foto separada de nuevo en el mismo sitio."
            } else {
                "Dibujo añadido y seleccionado entero. Retoca cada tinta en el panel derecho."
            });
        }
        if self.show_help {
            self.help_sheet(ctx, &pal);
        }
        self.show_color_panel(ctx);
        self.toast(ctx, &pal);
        self.update_traces(ctx);
        self.track_changes(ctx);
    }
}

/// Documento inicial con una pequeña muestra de lo que hace Vectorio.
fn welcome(doc: &mut Document) {
    use vectorio_core::{p, FillStyle, TextSpec};
    let mut title = Item::new(
        "Título",
        Geometry::Text(TextSpec { text: "Vectorio".into(), size_mm: 26.0, jitter: 0.6, seed: 3, ..Default::default() }),
        p(40.0, 40.0),
    );
    title.stroke = Some(0);
    doc.add(title);
    let mut sub = Item::new(
        "Subtítulo",
        Geometry::Text(TextSpec {
            text: "cosas bonitas con rotuladores, rellenos y fotos".into(),
            size_mm: 7.0,
            jitter: 0.5,
            seed: 9,
            ..Default::default()
        }),
        p(42.0, 92.0),
    );
    sub.stroke = Some(2);
    doc.add(sub);
    let mut flower = Item::new("Rosa polar", Geometry::Shape(Shape::Rose { n: 5, d: 3, r: 55.0 }), p(300.0, 150.0));
    flower.stroke = Some(6);
    flower.fill = vectorio_core::Fill { style: FillStyle::Hatch, pen: 1, spacing_mm: 1.6, angle_deg: 30.0, inset_mm: 0.0 };
    doc.add(flower);
    let mut heart = Item::new("Corazón", Geometry::Shape(Shape::Heart { size: 70.0 }), p(130.0, 200.0));
    heart.stroke = Some(1);
    heart.fill = vectorio_core::Fill { style: FillStyle::Concentric, pen: 1, spacing_mm: 2.0, angle_deg: 0.0, inset_mm: 0.0 };
    doc.add(heart);
}

// ── Controles con aspecto macOS ────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub enum Icon {
    Select,
    Draw,
    Text,
    Shape,
    Image,
    Open,
    Undo,
    Redo,
}

/// Grupo de botones sobre una cápsula, como los controles segmentados de macOS.
pub fn segmented<R>(ui: &mut egui::Ui, pal: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(pal.control)
        .rounding(Rounding::same(theme::CONTROL_RADIUS + 2.0))
        .inner_margin(egui::Margin::same(2.0))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(add).inner
        })
        .inner
}

pub fn text_segment(ui: &mut egui::Ui, pal: &Palette, label: &str, selected: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(12.5), pal.text);
    let size = Vec2::new(galley.size().x + 22.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    paint_segment_bg(ui, pal, rect, selected, resp.hovered());
    let color = if selected { pal.text } else { pal.secondary };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(12.5), color);
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
                    paint_segment_bg(ui, &pal, rect, selected, resp.hovered());
                    let color = if selected { pal.text } else { pal.secondary };
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, *label, egui::FontId::proportional(12.0), color);
                    if resp.clicked() && !selected {
                        *value = *v;
                        changed = true;
                    }
                }
            });
        });
    changed
}

fn paint_segment_bg(ui: &egui::Ui, pal: &Palette, rect: Rect, selected: bool, hovered: bool) {
    let r = Rounding::same(theme::CONTROL_RADIUS);
    if selected {
        let fill = if ui.visuals().dark_mode { Color32::from_rgba_unmultiplied(255, 255, 255, 36) } else { Color32::WHITE };
        ui.painter().rect_filled(rect.translate(Vec2::new(0.0, 0.5)), r, Color32::from_black_alpha(18));
        ui.painter().rect_filled(rect, r, fill);
    } else if hovered {
        ui.painter().rect_filled(rect, r, pal.control_hover);
    }
}

pub fn icon_button(ui: &mut egui::Ui, pal: &Palette, icon: Icon, selected: bool) -> egui::Response {
    icon_button_state(ui, pal, icon, selected, true)
}

pub fn icon_button_state(ui: &mut egui::Ui, pal: &Palette, icon: Icon, selected: bool, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(34.0, 28.0), Sense::click());
    paint_segment_bg(ui, pal, rect, selected, resp.hovered() && enabled);
    let color = if !enabled {
        pal.secondary.gamma_multiply(0.45)
    } else if selected {
        pal.accent
    } else {
        pal.text
    };
    paint_icon(ui.painter(), rect.center(), icon, Stroke::new(1.5f32, color));
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
        theme::section(ui, &title.to_uppercase());
        egui::Grid::new(("help", *title)).num_columns(2).spacing([14.0, 5.0]).show(ui, |ui| {
            for (keys, what) in rows.iter() {
                ui.label(egui::RichText::new(*keys).strong().color(pal.text));
                ui.label(egui::RichText::new(*what).color(pal.secondary));
                ui.end_row();
            }
        });
        ui.add_space(8.0);
    }
}
