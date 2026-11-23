//! Órdenes de la aplicación. Las lanzan igual los atajos de teclado y el menú nativo de
//! macOS, así que solo hay un sitio donde se decide qué hace cada una.

use crate::app::{Tool, VectorioApp, ViewMode, PAPER_TINTS};
use crate::canvas::Interaction;
use egui::{Key, Modifiers};
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    New,
    Open,
    ImportSvg,
    ImportImage,
    ImportModel,
    Save,
    SaveAs,
    ExportSvg,
    ExportPerPen,
    CloseWindow,
    Quit,
    Undo,
    Redo,
    Duplicate,
    SeparateStrokes,
    Delete,
    SelectAll,
    SelectNext,
    SelectPrev,
    Deselect,
    BringForward,
    SendBackward,
    BringToFront,
    SendToBack,
    ToggleLock,
    Reseed,
    Tool(Tool),
    Mode(ViewMode),
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ZoomActual,
    PlayPause,
    Paper(usize),
    Shortcuts,
    FirstDrawing,
    Appearance(crate::theme::Appearance),
    Preferences,
    Palettes,
}

/// Atajos con ⌘ que funcionan aunque se esté escribiendo en un campo de texto.
const COMMAND_KEYS: &[(Modifiers, Key, Command)] = &[
    (Modifiers::COMMAND, Key::N, Command::New),
    (Modifiers::COMMAND, Key::O, Command::Open),
    (Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::I, Command::ImportImage),
    (Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S, Command::SaveAs),
    (Modifiers::COMMAND, Key::S, Command::Save),
    (Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::E, Command::ExportPerPen),
    (Modifiers::COMMAND, Key::E, Command::ExportSvg),
    (Modifiers::COMMAND, Key::W, Command::CloseWindow),
    (Modifiers::COMMAND, Key::Q, Command::Quit),
    (Modifiers::COMMAND, Key::D, Command::Duplicate),
    (Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::G, Command::SeparateStrokes),
    (Modifiers::COMMAND, Key::A, Command::SelectAll),
    (Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::CloseBracket, Command::BringToFront),
    (Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::OpenBracket, Command::SendToBack),
    (Modifiers::COMMAND, Key::CloseBracket, Command::BringForward),
    (Modifiers::COMMAND, Key::OpenBracket, Command::SendBackward),
    (Modifiers::COMMAND, Key::L, Command::ToggleLock),
    (Modifiers::COMMAND, Key::R, Command::Reseed),
    (Modifiers::COMMAND, Key::Comma, Command::Preferences),
    (Modifiers::COMMAND, Key::Num1, Command::Mode(ViewMode::Design)),
    (Modifiers::COMMAND, Key::Num2, Command::Mode(ViewMode::Create)),
    (Modifiers::COMMAND, Key::Num3, Command::Mode(ViewMode::Output)),
    (Modifiers::COMMAND, Key::Num4, Command::Mode(ViewMode::Volumetric)),
    (Modifiers::COMMAND, Key::Num5, Command::Mode(ViewMode::Photo)),
    (Modifiers::COMMAND, Key::Equals, Command::ZoomIn),
    (Modifiers::COMMAND, Key::Plus, Command::ZoomIn),
    (Modifiers::COMMAND, Key::Minus, Command::ZoomOut),
    (Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Num0, Command::ZoomActual),
    (Modifiers::COMMAND, Key::Num0, Command::ZoomFit),
    (Modifiers::COMMAND, Key::Slash, Command::Shortcuts),
];

/// Teclas sueltas: solo cuando no se está escribiendo.
const PLAIN_KEYS: &[(Key, Command)] = &[
    (Key::V, Command::Tool(Tool::Select)),
    (Key::A, Command::Tool(Tool::Strokes)),
    (Key::P, Command::Tool(Tool::Draw)),
    (Key::T, Command::Tool(Tool::Text)),
    (Key::S, Command::Tool(Tool::Shape)),
    (Key::I, Command::ImportImage),
    (Key::Delete, Command::Delete),
    (Key::Backspace, Command::Delete),
];

/// Lista para la hoja de atajos (⌘/).
pub const CHEATSHEET: &[(&str, &[(&str, &str)])] = &[
    (
        tm!("Herramientas"),
        &[("V", tm!("Seleccionar y mover")), ("A", tm!("Seleccionar trazos")), ("P", tm!("Mano alzada")), ("T", tm!("Caja de texto manuscrito")), ("S", tm!("Formas")), ("I · ⇧⌘I", tm!("Convertir foto"))],
    ),
    (
        tm!("Archivo"),
        &[("⌘N", tm!("Nuevo")), ("⌘O", tm!("Abrir proyecto")), ("⌘,", tm!("Ajustes")), ("⌘S · ⇧⌘S", tm!("Guardar · Guardar como")), ("⌘E · ⇧⌘E", tm!("Exportar SVG · uno por rotulador"))],
    ),
    (
        tm!("Edición"),
        &[
            ("⌘Z · ⇧⌘Z", tm!("Deshacer · Rehacer")),
            ("⌘D · ⌫", tm!("Duplicar · Eliminar")),
            ("⌘A", tm!("Seleccionar todo")),
            (tm!("Arrastrar en vacío"), tm!("Seleccionar con recuadro")),
            ("⇧⌘G", tm!("Separar trazos en objeto nuevo")),
            (tm!("⇧ · ⌘ clic"), tm!("Añadir o quitar de la selección")),
            ("⇥ · ⇧⇥", tm!("Objeto siguiente · anterior")),
            (tm!("Esc"), tm!("Cancelar, cerrar, deseleccionar")),
        ],
    ),
    (
        tm!("Objeto"),
        &[("⌘] · ⌘[", tm!("Adelante · Atrás")), ("⇧⌘] · ⇧⌘[", tm!("Al frente · Al fondo")), ("⌘L", tm!("Bloquear")), ("⌘R", tm!("Otra variación de la letra")), (tm!("Flechas (⇧)"), tm!("Mover 1 mm (10 mm)"))],
    ),
    (
        tm!("Vista"),
        &[("⌘1 · ⌘2 · ⌘5", tm!("Diseño · Escenas · Foto")), ("⌘4 · ⌘3", tm!("Modelo 3D · Salida")), ("N", tm!("Otra semilla en Escenas")), ("⌘= · ⌘−", tm!("Acercar · Alejar")), ("⌘0 · ⇧⌘0", tm!("Ajustar · Tamaño real")), (tm!("Espacio"), tm!("Simular (Salida) · arrastrar para desplazar")), (tm!("Pellizco · ⌘ rueda"), tm!("Zoom"))],
    ),
];

impl VectorioApp {
    /// Lee el teclado y lanza las órdenes que correspondan.
    pub fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.awaiting_document_change() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape) || i.consume_key(Modifiers::COMMAND, Key::W)) {
                self.cancel_document_change();
            }
            return;
        }
        if self.palette_ui.open {
            if !ctx.wants_keyboard_input() && !ctx.memory(|m| m.any_popup_open()) && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.palette_ui.close_request();
            }
            return;
        }
        if self.prefs_ui.open {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape) || i.consume_key(Modifiers::COMMAND, Key::W)) {
                self.prefs_ui.open = false;
            }
            return;
        }
        if self.color_edit.is_some() { return; }
        if self.renaming_pen.is_some() || self.renaming_item.is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.finish_rename(true);
            }
            return;
        }
        if self.show_help {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape) || i.consume_key(Modifiers::COMMAND, Key::Slash) || i.consume_key(Modifiers::COMMAND, Key::W)) {
                self.show_help = false;
            }
            return;
        }
        let typing = ctx.wants_keyboard_input() && !(self.mode == ViewMode::Volumetric && self.volumetric_ui.scene_control_focused(ctx));
        let act = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));

        // ⌘Z en un campo de texto deshace el texto (lo gestiona el propio campo).
        if !typing {
            if act(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) {
                self.run(ctx, Command::Redo);
            } else if act(Modifiers::COMMAND, Key::Z) {
                self.run(ctx, Command::Undo);
            }
        }
        for &(m, k, cmd) in COMMAND_KEYS {
            if act(m, k) {
                self.run(ctx, cmd);
            }
        }
        if !(self.mode == ViewMode::Volumetric && self.volumetric_ui.menu_open(ctx))
            && act(Modifiers::NONE, Key::Escape)
        {
            self.escape(ctx);
        }
        let focused = ctx.memory(|m| m.focused());
        let control_focus = focused.is_some() && focused != self.canvas_id;
        if typing || control_focus || matches!(self.mode, ViewMode::Create | ViewMode::Photo | ViewMode::Volumetric) {
            return;
        }
        for &(k, cmd) in PLAIN_KEYS {
            if act(Modifiers::NONE, k) {
                self.run(ctx, cmd);
            }
        }
        // En el lienzo, Tab recorre la selección; fuera, egui recorre los controles.
        if self.selected.is_some() && focused.is_some() && focused == self.canvas_id {
            if act(Modifiers::SHIFT, Key::Tab) {
                self.run(ctx, Command::SelectPrev);
            } else if act(Modifiers::NONE, Key::Tab) {
                self.run(ctx, Command::SelectNext);
            }
        }
        if self.mode == ViewMode::Output && act(Modifiers::NONE, Key::Space) {
            self.run(ctx, Command::PlayPause);
        }
        let step = if ctx.input(|i| i.modifiers.shift) { 10.0 } else { 1.0 };
        let nudges = [(Key::ArrowLeft, (-step, 0.0)), (Key::ArrowRight, (step, 0.0)), (Key::ArrowUp, (0.0, -step)), (Key::ArrowDown, (0.0, step))];
        for (k, (dx, dy)) in nudges {
            if ctx.input(|i| i.key_pressed(k)) {
                if self.tool == Tool::Strokes && self.mode == ViewMode::Design {
                    self.stroke_selection.move_by(&mut self.doc, vectorio_core::p(dx, dy));
                    continue;
                }
                for id in self.selection() {
                    if let Some(it) = self.doc.get_mut(id).filter(|it| !it.locked) {
                        it.place.pos = it.place.pos + vectorio_core::p(dx, dy);
                    }
                }
            }
        }
    }

    /// Esc deshace lo más inmediato: un menú abierto, un gesto a medias, la selección y,
    /// por último, vuelve a la herramienta de selección.
    fn escape(&mut self, ctx: &egui::Context) {
        if ctx.memory(|m| m.any_popup_open()) {
            ctx.memory_mut(|m| m.close_popup());
            return;
        }
        if self.prefs_ui.open {
            self.prefs_ui.open = false;
            return;
        }
        if self.mode == ViewMode::Create { self.create_ui.cancel(); self.mode = ViewMode::Design; return; }
        if matches!(self.mode, ViewMode::Volumetric | ViewMode::Photo) { self.mode = ViewMode::Design; return; }
        if self.tool == Tool::Strokes {
            if self.stroke_selection.drag_start.take().is_some() || self.stroke_selection.marquee.take().is_some() { return; }
            if self.stroke_selection.live.take().is_some() { return; }
            if !self.stroke_selection.ids.is_empty() { self.stroke_selection.ids.clear(); return; }
        }
        match std::mem::take(&mut self.interaction) {
            Interaction::None | Interaction::Pan => {}
            Interaction::Move { orig, .. } | Interaction::Scale { orig, .. } | Interaction::Rotate { orig, .. } => {
                for (id, place) in orig {
                    if let Some(it) = self.doc.get_mut(id) {
                        it.place = place;
                    }
                }
                return;
            }
            Interaction::ResizeText { id, width, height, .. } => {
                if let Some(it) = self.doc.get_mut(id) {
                    if let vectorio_core::Geometry::Text(spec) = &mut it.geometry { spec.wrap_mm = width; spec.frame_height_mm = height; }
                }
                return;
            }
            Interaction::Draw(_) | Interaction::Box(..) | Interaction::TextBox(..) | Interaction::Marquee(..) => return,
        }
        if self.pen_focus {
            self.pen_focus = false;
        } else if self.selected.is_some() {
            self.set_selection(vec![]);
        } else {
            self.tool = Tool::Select;
        }
    }

    pub fn run(&mut self, ctx: &egui::Context, cmd: Command) {
        if self.awaiting_document_change() {
            if cmd == Command::CloseWindow { self.cancel_document_change(); }
            return;
        }
        // Menú y teclado comparten la misma barrera: ninguna hoja permite editar detrás.
        if self.prefs_ui.open && !matches!(cmd, Command::CloseWindow | Command::Quit) { return; }
        if (self.color_edit.is_some() || self.show_help || self.palette_ui.open)
            && !matches!(cmd, Command::CloseWindow | Command::Quit) { return; }
        match cmd {
            Command::New => self.new_document(ctx),
            Command::Open => self.open_dialog(ctx),
            Command::ImportSvg => self.import_svg_dialog(ctx),
            Command::ImportModel => self.import_model_dialog(ctx),
            Command::ImportImage => {
                if self.vec_ui.open_dialog(ctx) { self.mode = ViewMode::Photo; }
            }
            Command::Save => { self.save(ctx, false); }
            Command::SaveAs => { self.save(ctx, true); }
            Command::ExportSvg => self.export_svg(ctx),
            Command::ExportPerPen => self.export_per_pen(ctx),
            Command::CloseWindow => {
                if self.palette_ui.open {
                    self.palette_ui.close_request();
                } else if self.prefs_ui.open {
                    self.prefs_ui.open = false;
                } else if self.color_edit.is_some() {
                    self.cancel_color_edit();
                } else if self.show_help {
                    self.show_help = false;
                } else {
                    // Como en cualquier app de Mac: se cierra la ventana, no el programa.
                    crate::macos::close_window(ctx);
                }
            }
            Command::Quit => self.request_quit(ctx),
            Command::Undo | Command::Redo => {
                if ctx.wants_keyboard_input() && !(self.mode == ViewMode::Volumetric && self.volumetric_ui.scene_control_focused(ctx)) {
                    // Desde el menú, con un campo de texto activo: que lo deshaga el campo.
                    let modifiers = if cmd == Command::Redo { Modifiers::COMMAND | Modifiers::SHIFT } else { Modifiers::COMMAND };
                    ctx.input_mut(|i| i.events.push(egui::Event::Key { key: Key::Z, physical_key: None, pressed: true, repeat: false, modifiers }));
                } else if self.mode == ViewMode::Volumetric {
                    self.volumetric_ui.undo_stack(cmd == Command::Redo);
                } else if cmd == Command::Undo {
                    self.undo();
                } else {
                    self.redo();
                }
            }
            Command::Duplicate => self.duplicate_selected(),
            Command::SeparateStrokes => self.separate_strokes(ctx),
            Command::Delete => {
                if self.mode == ViewMode::Design && self.tool == Tool::Strokes {
                    self.stroke_selection.delete(&mut self.doc);
                } else if self.pen_focus && self.doc.pens.len() > 1 {
                    // La lista de rotuladores tiene el foco: se borra el rotulador activo.
                    self.remove_pen(self.active_pen);
                } else if !self.pen_focus {
                    self.delete_selected();
                }
            }
            Command::SelectAll => {
                if ctx.wants_keyboard_input() {
                    ctx.input_mut(|i| i.events.push(egui::Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND }));
                } else if self.mode == ViewMode::Design && self.tool == Tool::Strokes {
                    self.stroke_selection.select_all(&self.doc);
                } else {
                    self.pen_focus = false;
                    self.set_selection(self.doc.items.iter().filter(|it| it.visible).map(|it| it.id).collect());
                }
            }
            Command::SelectNext | Command::SelectPrev => {
                let n = self.doc.items.len();
                if n > 0 {
                    let cur = self.selected.and_then(|id| self.doc.index_of(id));
                    let next = match (cur, cmd == Command::SelectNext) {
                        (None, true) => n - 1,
                        (None, false) => 0,
                        // La lista de objetos muestra arriba el de encima (el último).
                        (Some(i), true) => (i + n - 1) % n,
                        (Some(i), false) => (i + 1) % n,
                    };
                    self.set_selection(vec![self.doc.items[next].id]);
                }
            }
            Command::Deselect => self.set_selection(vec![]),
            Command::BringForward | Command::SendBackward | Command::BringToFront | Command::SendToBack => {
                let forward = matches!(cmd, Command::BringForward | Command::BringToFront);
                let edge = matches!(cmd, Command::BringToFront | Command::SendToBack);
                self.doc.reorder_items(&self.selection(), forward, edge);
            }
            Command::ToggleLock => {
                if let Some(it) = self.selected.and_then(|id| self.doc.get_mut(id)) {
                    it.locked = !it.locked;
                }
            }
            Command::Reseed => {
                if let Some(it) = self.selected.and_then(|id| self.doc.get_mut(id)) {
                    if let vectorio_core::Geometry::Text(spec) = &mut it.geometry {
                        spec.seed = spec.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 1;
                    }
                }
            }
            Command::Tool(t) => {
                self.choose_tool(t);
                self.mode = ViewMode::Design;
            }
            Command::Mode(m) => {
                self.mode = m;
                if m == ViewMode::Output && self.sim.progress <= 0.0 {
                    self.sim.progress = 1.0;
                }
            }
            Command::ZoomIn => if self.mode == ViewMode::Output { self.output_ui.zoom_by(1.25) } else { self.view.zoom_by(1.25) },
            Command::ZoomOut => if self.mode == ViewMode::Output { self.output_ui.zoom_by(0.8) } else { self.view.zoom_by(0.8) },
            Command::ZoomFit => if self.mode == ViewMode::Output { self.output_ui.fit() } else { self.view.fitted = false },
            Command::ZoomActual => {
                if self.mode == ViewMode::Output { self.output_ui.actual(); }
                else { let k = (96.0 / 25.4) / self.view.zoom; self.view.zoom_by(k); }
            }
            Command::PlayPause => {
                if self.mode != ViewMode::Output {
                    self.mode = ViewMode::Output;
                }
                if self.sim.progress >= 1.0 {
                    self.sim.progress = 0.0;
                }
                self.sim.playing = !self.sim.playing;
            }
            Command::Paper(i) => { let prefs = crate::prefs::Preferences { paper_tint: i.min(PAPER_TINTS.len() - 1), ..self.prefs.clone() }; self.set_prefs(ctx, prefs); }
            Command::Shortcuts => {
                self.show_help = !(self.show_help && self.help_page == crate::app::HelpPage::Shortcuts);
                self.help_page = crate::app::HelpPage::Shortcuts;
            }
            Command::FirstDrawing => {
                self.show_help = true;
                self.help_page = crate::app::HelpPage::FirstDrawing;
            }
            Command::Appearance(appearance) => {
                let prefs = crate::prefs::Preferences { appearance, ..self.prefs.clone() };
                self.set_prefs(ctx, prefs);
            }
            Command::Preferences => self.prefs_ui.show_window(&self.prefs),
            Command::Palettes => {
                let _ = self.palettes.reload();
                self.palette_ui.open_for(crate::palette_ui::Destination::Document, self.doc.pens.clone());
            }
        }
        ctx.request_repaint();
    }
}

// ── Menú nativo de macOS ────────────────────────────────────────────────────────

/// Barra de menús de macOS. Los clics llegan por un canal y despiertan la interfaz.
pub struct NativeMenu {
    _menu: muda::Menu,
    ids: Vec<(muda::MenuId, Command)>,
}

/// Canal de eventos del menú y orden de cerrar vigente; sobreviven a la reconstrucción del menú.
struct MenuEvents {
    tx: mpsc::Sender<muda::MenuEvent>,
    rx: std::sync::Mutex<mpsc::Receiver<muda::MenuEvent>>,
    close: std::sync::Mutex<Option<muda::MenuId>>,
}

fn events() -> &'static MenuEvents {
    static EVENTS: std::sync::OnceLock<MenuEvents> = std::sync::OnceLock::new();
    EVENTS.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        MenuEvents { tx, rx: std::sync::Mutex::new(rx), close: std::sync::Mutex::new(None) }
    })
}

impl NativeMenu {
    pub fn install(ctx: &egui::Context) -> Option<NativeMenu> {
        use muda::{accelerator::Accelerator, AboutMetadata, Menu, MenuItem, PredefinedMenuItem as P, Submenu};
        let mut ids = Vec::new();
        let mut item = |text: &str, accel: Option<&str>, cmd: Command| {
            let a: Option<Accelerator> = accel.and_then(|s| s.parse().ok());
            let it = MenuItem::new(text, true, a);
            ids.push((it.id().clone(), cmd));
            it
        };
        let sep = P::separator;

        let about = P::about(
            Some(&tr!("Acerca de Vectorio")),
            Some(AboutMetadata {
                name: Some("Vectorio".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                comments: Some(tr!("Cosas bonitas con un plotter de rotuladores.")),
                credits: Some(tr!("Tipografías de trazo único: Hershey Text de Evil Mad Scientist (SIL Open Font License), derivadas del trabajo del Dr. A. V. Hershey.")),
                ..Default::default()
            }),
        );
        let app_menu = Submenu::with_items(
            "Vectorio",
            true,
            &[&about, &sep(), &item(&tr!("Ajustes…"), Some("CmdOrCtrl+Comma"), Command::Preferences), &sep(), &P::services(Some(&tr!("Servicios"))), &sep(), &P::hide(Some(&tr!("Ocultar Vectorio"))), &P::hide_others(Some(&tr!("Ocultar otros"))), &P::show_all(Some(&tr!("Mostrar todo"))), &sep(), &item(&tr!("Salir de Vectorio"), Some("CmdOrCtrl+Q"), Command::Quit)],
        )
        .ok()?;

        let file = Submenu::with_items(
            tr!("Archivo"),
            true,
            &[
                &item(&tr!("Nuevo"), Some("CmdOrCtrl+N"), Command::New),
                &item(&tr!("Abrir proyecto…"), Some("CmdOrCtrl+O"), Command::Open),
                &sep(),
                &item(&tr!("Guardar"), Some("CmdOrCtrl+S"), Command::Save),
                &item(&tr!("Guardar como…"), Some("Shift+CmdOrCtrl+S"), Command::SaveAs),
                &sep(),
                &item(&tr!("Importar SVG…"), None, Command::ImportSvg),
                &item(&tr!("Convertir foto…"), Some("Shift+CmdOrCtrl+I"), Command::ImportImage),
                &item(&tr!("Importar modelo 3D…"), None, Command::ImportModel),
                &sep(),
                &item(&tr!("Exportar SVG…"), Some("CmdOrCtrl+E"), Command::ExportSvg),
                &item(&tr!("Exportar un SVG por rotulador…"), Some("Shift+CmdOrCtrl+E"), Command::ExportPerPen),
                &sep(),
                &item(&tr!("Cerrar ventana"), Some("CmdOrCtrl+W"), Command::CloseWindow),
            ],
        )
        .ok()?;

        // Sin Cortar/Copiar/Pegar: así los campos de texto siguen recibiendo esas teclas.
        let edit = Submenu::with_items(
            tr!("Edición"),
            true,
            &[
                &item(&tr!("Deshacer"), Some("CmdOrCtrl+Z"), Command::Undo),
                &item(&tr!("Rehacer"), Some("Shift+CmdOrCtrl+Z"), Command::Redo),
                &sep(),
                &item(&tr!("Duplicar"), Some("CmdOrCtrl+D"), Command::Duplicate),
                &item(&tr!("Eliminar"), None, Command::Delete),
                &item(&tr!("Separar trazos en objeto nuevo"), Some("Shift+CmdOrCtrl+G"), Command::SeparateStrokes),
                &sep(),
                &item(&tr!("Seleccionar todo"), Some("CmdOrCtrl+A"), Command::SelectAll),
                &item(&tr!("Seleccionar siguiente"), None, Command::SelectNext),
                &item(&tr!("Seleccionar anterior"), None, Command::SelectPrev),
                &item(&tr!("Deseleccionar"), None, Command::Deselect),
                &sep(),
                &item(&tr!("Paletas…"), None, Command::Palettes),
            ],
        )
        .ok()?;

        let object = Submenu::with_items(
            tr!("Objeto"),
            true,
            &[
                &item(&tr!("Traer adelante"), Some("CmdOrCtrl+BracketRight"), Command::BringForward),
                &item(&tr!("Enviar atrás"), Some("CmdOrCtrl+BracketLeft"), Command::SendBackward),
                &item(&tr!("Traer al frente"), Some("Shift+CmdOrCtrl+BracketRight"), Command::BringToFront),
                &item(&tr!("Enviar al fondo"), Some("Shift+CmdOrCtrl+BracketLeft"), Command::SendToBack),
                &sep(),
                &item(&tr!("Bloquear o desbloquear"), Some("CmdOrCtrl+L"), Command::ToggleLock),
                &item(&tr!("Otra variación de la letra"), Some("CmdOrCtrl+R"), Command::Reseed),
            ],
        )
        .ok()?;

        // Las herramientas usan teclas sueltas (V, P, T, S): no se ponen como atajo del
        // menú para no robar esas letras a los campos de texto.
        let tools = Submenu::with_items(
            tr!("Herramientas"),
            true,
            &[
                &item(&tr!("Seleccionar y mover   V"), None, Command::Tool(Tool::Select)),
                &item(&tr!("Seleccionar trazos   A"), None, Command::Tool(Tool::Strokes)),
                &item(&tr!("Mano alzada   P"), None, Command::Tool(Tool::Draw)),
                &item(&tr!("Texto manuscrito   T"), None, Command::Tool(Tool::Text)),
                &item(&tr!("Formas   S"), None, Command::Tool(Tool::Shape)),
            ],
        )
        .ok()?;

        let paper = Submenu::new(tr!("Color del papel"), true);
        for (i, (name, _)) in PAPER_TINTS.iter().enumerate() {
            paper.append(&item(&crate::i18n::tr_key(name), None, Command::Paper(i))).ok()?;
        }
        let appearance = Submenu::new(tr!("Apariencia"), true);
        for value in crate::theme::Appearance::ALL {
            appearance.append(&item(&value.label(), None, Command::Appearance(value))).ok()?;
        }
        let view = Submenu::with_items(
            tr!("Visualización"),
            true,
            &[
                // Mismo orden que la barra; los atajos de siempre se conservan.
                &item(&tr!("Diseño"), Some("CmdOrCtrl+1"), Command::Mode(ViewMode::Design)),
                &item(&tr!("Escenas"), Some("CmdOrCtrl+2"), Command::Mode(ViewMode::Create)),
                &item(&tr!("Foto"), Some("CmdOrCtrl+5"), Command::Mode(ViewMode::Photo)),
                &item(&tr!("Modelo 3D"), Some("CmdOrCtrl+4"), Command::Mode(ViewMode::Volumetric)),
                &item(&tr!("Salida"), Some("CmdOrCtrl+3"), Command::Mode(ViewMode::Output)),
                &sep(),
                &item(&tr!("Acercar"), Some("CmdOrCtrl+Equal"), Command::ZoomIn),
                &item(&tr!("Alejar"), Some("CmdOrCtrl+Minus"), Command::ZoomOut),
                &item(&tr!("Ajustar a la ventana"), Some("CmdOrCtrl+Digit0"), Command::ZoomFit),
                &item(&tr!("Tamaño real"), Some("Shift+CmdOrCtrl+Digit0"), Command::ZoomActual),
                &sep(),
                &paper,
                &appearance,
                &sep(),
                &P::fullscreen(Some(&tr!("Pantalla completa"))),
            ],
        )
        .ok()?;

        let window = Submenu::with_items(
            tr!("Ventana"),
            true,
            &[&P::minimize(Some(&tr!("Minimizar"))), &P::maximize(Some(&tr!("Zoom"))), &sep(), &P::bring_all_to_front(Some(&tr!("Traer todo al frente")))],
        )
        .ok()?;
        let help = Submenu::with_items(tr!("Ayuda"), true, &[&item(&tr!("Mi primer dibujo"), None, Command::FirstDrawing), &item(&tr!("Atajos de teclado"), Some("CmdOrCtrl+Slash"), Command::Shortcuts)]).ok()?;

        let menu = Menu::with_items(&[&app_menu, &file, &edit, &object, &tools, &view, &window, &help]).ok()?;
        #[cfg(target_os = "macos")]
        {
            menu.init_for_nsapp();
            window.set_as_windows_menu_for_nsapp();
            help.set_as_help_menu_for_nsapp();
        }

        let close = ids.iter().find(|(_, c)| *c == Command::CloseWindow).map(|(id, _)| id.clone());
        let shared = events();
        *shared.close.lock().unwrap_or_else(|e| e.into_inner()) = close;
        // muda solo admite un manejador por proceso: se registra una vez y sirve para todos los
        // menús que se construyan después (por ejemplo al cambiar de idioma).
        static HANDLER: std::sync::Once = std::sync::Once::new();
        HANDLER.call_once(|| {
            let ctx = ctx.clone();
            muda::MenuEvent::set_event_handler(Some(move |e: muda::MenuEvent| {
                let shared = events();
                // Con la ventana oculta, una orden del menú (Nuevo, Abrir…) la vuelve a mostrar.
                if Some(&e.id) != shared.close.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                    crate::macos::show_window();
                }
                let _ = shared.tx.send(e);
                ctx.request_repaint();
            }));
        });
        Some(NativeMenu { _menu: menu, ids })
    }

    /// Órdenes elegidas en el menú desde el último fotograma.
    pub fn poll(&self) -> Vec<Command> {
        let rx = events().rx.lock().unwrap_or_else(|e| e.into_inner());
        rx.try_iter().filter_map(|e| self.ids.iter().find(|(id, _)| *id == e.id).map(|(_, c)| *c)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorio_core::{Geometry, Item, P};

    #[test]
    fn settings_blocks_document_shortcuts_and_menu_commands_and_escape_closes_it() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let id = app.doc.add(Item::new("Objeto", Geometry::Path { lines: vec![], closed: false }, P::default()));
        app.selected = Some(id);
        app.prefs_ui.show_window(&app.prefs);
        let before = app.doc.clone();
        for cmd in [Command::Delete, Command::Duplicate, Command::New, Command::Undo, Command::Mode(ViewMode::Create)] {
            app.run(&ctx, cmd);
            assert_eq!(app.doc, before);
        }
        let input = |key| egui::RawInput { events: vec![egui::Event::Key {
            key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE,
        }], ..Default::default() };
        let _ = ctx.run(input(Key::Delete), |ctx| app.shortcuts(ctx));
        assert_eq!(app.doc, before);
        let _ = ctx.run(input(Key::Escape), |ctx| app.shortcuts(ctx));
        assert!(!app.prefs_ui.open);
        app.prefs_ui.show_window(&app.prefs);
        app.run(&ctx, Command::CloseWindow);
        assert!(!app.prefs_ui.open);
        assert_eq!(app.doc, before);
    }
}
