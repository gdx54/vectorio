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
    ImportImage,
    Save,
    SaveAs,
    ExportSvg,
    ExportPerPen,
    CloseWindow,
    Undo,
    Redo,
    Duplicate,
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
    Appearance(crate::theme::Appearance),
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
    (Modifiers::COMMAND, Key::D, Command::Duplicate),
    (Modifiers::COMMAND, Key::A, Command::SelectAll),
    (Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::CloseBracket, Command::BringToFront),
    (Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::OpenBracket, Command::SendToBack),
    (Modifiers::COMMAND, Key::CloseBracket, Command::BringForward),
    (Modifiers::COMMAND, Key::OpenBracket, Command::SendBackward),
    (Modifiers::COMMAND, Key::L, Command::ToggleLock),
    (Modifiers::COMMAND, Key::R, Command::Reseed),
    (Modifiers::COMMAND, Key::Num1, Command::Mode(ViewMode::Design)),
    (Modifiers::COMMAND, Key::Num2, Command::Mode(ViewMode::Plotter)),
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
        "Herramientas",
        &[("V", "Seleccionar y mover"), ("P", "Mano alzada"), ("T", "Caja de texto manuscrito"), ("S", "Formas"), ("I · ⇧⌘I", "Foto → dibujo")],
    ),
    (
        "Archivo",
        &[("⌘N", "Nuevo"), ("⌘O", "Abrir"), ("⌘S · ⇧⌘S", "Guardar · Guardar como"), ("⌘E · ⇧⌘E", "Exportar SVG · uno por rotulador")],
    ),
    (
        "Edición",
        &[
            ("⌘Z · ⇧⌘Z", "Deshacer · Rehacer"),
            ("⌘D · ⌫", "Duplicar · Eliminar"),
            ("⌘A", "Seleccionar todo"),
            ("Arrastrar en vacío", "Seleccionar con recuadro"),
            ("⇧ · ⌘ clic", "Añadir o quitar de la selección"),
            ("⇥ · ⇧⇥", "Objeto siguiente · anterior"),
            ("Esc", "Cancelar, cerrar, deseleccionar"),
        ],
    ),
    (
        "Objeto",
        &[("⌘] · ⌘[", "Adelante · Atrás"), ("⇧⌘] · ⇧⌘[", "Al frente · Al fondo"), ("⌘L", "Bloquear"), ("⌘R", "Otra variación de la letra"), ("Flechas (⇧)", "Mover 1 mm (10 mm)")],
    ),
    (
        "Vista",
        &[("⌘1 · ⌘2", "Diseño · Salida"), ("⌘= · ⌘−", "Acercar · Alejar"), ("⌘0 · ⇧⌘0", "Ajustar · Tamaño real"), ("Espacio", "Simular (Salida) · arrastrar para desplazar"), ("Pellizco · ⌘ rueda", "Zoom")],
    ),
];

impl VectorioApp {
    /// Lee el teclado y lanza las órdenes que correspondan.
    pub fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.color_edit.is_some() { return; }
        if self.renaming_pen.is_some() || self.renaming_item.is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.finish_rename(true);
            }
            return;
        }
        // Una hoja modal abierta gestiona su propio teclado.
        if self.vec_ui.open {
            return;
        }
        if self.show_help {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape) || i.consume_key(Modifiers::COMMAND, Key::Slash) || i.consume_key(Modifiers::COMMAND, Key::W)) {
                self.show_help = false;
            }
            return;
        }
        let typing = ctx.wants_keyboard_input();
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
        if act(Modifiers::NONE, Key::Escape) {
            self.escape(ctx);
        }
        if typing {
            return;
        }
        for &(k, cmd) in PLAIN_KEYS {
            if act(Modifiers::NONE, k) {
                self.run(ctx, cmd);
            }
        }
        if act(Modifiers::SHIFT, Key::Tab) {
            self.run(ctx, Command::SelectPrev);
        } else if act(Modifiers::NONE, Key::Tab) {
            self.run(ctx, Command::SelectNext);
        }
        if self.mode == ViewMode::Plotter && act(Modifiers::NONE, Key::Space) {
            self.run(ctx, Command::PlayPause);
        }
        let step = if ctx.input(|i| i.modifiers.shift) { 10.0 } else { 1.0 };
        let nudges = [(Key::ArrowLeft, (-step, 0.0)), (Key::ArrowRight, (step, 0.0)), (Key::ArrowUp, (0.0, -step)), (Key::ArrowDown, (0.0, step))];
        for (k, (dx, dy)) in nudges {
            if ctx.input(|i| i.key_pressed(k)) {
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
        match cmd {
            Command::New => self.new_document(),
            Command::Open => self.open_dialog(ctx),
            Command::ImportImage => self.vec_ui.open_dialog(ctx),
            Command::Save => self.save(ctx, false),
            Command::SaveAs => self.save(ctx, true),
            Command::ExportSvg => self.export_svg(ctx),
            Command::ExportPerPen => self.export_per_pen(ctx),
            Command::CloseWindow => {
                if self.color_edit.is_some() {
                    self.cancel_color_edit();
                } else if self.vec_ui.open {
                    self.vec_ui.open = false;
                } else if self.show_help {
                    self.show_help = false;
                } else {
                    // Como en cualquier app de Mac: se cierra la ventana, no el programa.
                    crate::macos::close_window(ctx);
                }
            }
            Command::Undo | Command::Redo => {
                if ctx.wants_keyboard_input() {
                    // Desde el menú, con un campo de texto activo: que lo deshaga el campo.
                    let modifiers = if cmd == Command::Redo { Modifiers::COMMAND | Modifiers::SHIFT } else { Modifiers::COMMAND };
                    ctx.input_mut(|i| i.events.push(egui::Event::Key { key: Key::Z, physical_key: None, pressed: true, repeat: false, modifiers }));
                } else if cmd == Command::Undo {
                    self.undo();
                } else {
                    self.redo();
                }
            }
            Command::Duplicate => self.duplicate_selected(),
            Command::Delete => {
                if self.pen_focus && self.doc.pens.len() > 1 {
                    // La lista de rotuladores tiene el foco: se borra el rotulador activo.
                    self.remove_pen(self.active_pen);
                } else if !self.pen_focus {
                    self.delete_selected();
                }
            }
            Command::SelectAll => {
                if ctx.wants_keyboard_input() {
                    ctx.input_mut(|i| i.events.push(egui::Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND }));
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
                    self.selected = Some(self.doc.items[next].id);
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
                self.tool = t;
                self.mode = ViewMode::Design;
            }
            Command::Mode(m) => {
                self.mode = m;
                if m == ViewMode::Plotter && self.sim.progress <= 0.0 {
                    self.sim.progress = 1.0;
                }
            }
            Command::ZoomIn => if self.mode == ViewMode::Plotter { self.output_ui.zoom_by(1.25) } else { self.view.zoom_by(1.25) },
            Command::ZoomOut => if self.mode == ViewMode::Plotter { self.output_ui.zoom_by(0.8) } else { self.view.zoom_by(0.8) },
            Command::ZoomFit => if self.mode == ViewMode::Plotter { self.output_ui.fit() } else { self.view.fitted = false },
            Command::ZoomActual => {
                if self.mode == ViewMode::Plotter { self.output_ui.actual(); }
                else { let k = (96.0 / 25.4) / self.view.zoom; self.view.zoom_by(k); }
            }
            Command::PlayPause => {
                if self.mode != ViewMode::Plotter {
                    self.mode = ViewMode::Plotter;
                }
                if self.sim.progress >= 1.0 {
                    self.sim.progress = 0.0;
                }
                self.sim.playing = !self.sim.playing;
            }
            Command::Paper(i) => self.paper_tint = i.min(PAPER_TINTS.len() - 1),
            Command::Shortcuts => self.show_help = !self.show_help,
            Command::Appearance(appearance) => self.set_appearance(ctx, appearance),
        }
        ctx.request_repaint();
    }
}

// ── Menú nativo de macOS ────────────────────────────────────────────────────────

/// Barra de menús de macOS. Los clics llegan por un canal y despiertan la interfaz.
pub struct NativeMenu {
    _menu: muda::Menu,
    ids: Vec<(muda::MenuId, Command)>,
    rx: mpsc::Receiver<muda::MenuEvent>,
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
            Some("Acerca de Vectorio"),
            Some(AboutMetadata {
                name: Some("Vectorio".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                comments: Some("Cosas bonitas con un plotter de rotuladores.".into()),
                credits: Some("Tipografías de trazo único: Hershey Text de Evil Mad Scientist (SIL Open Font License), derivadas del trabajo del Dr. A. V. Hershey.".into()),
                ..Default::default()
            }),
        );
        let app_menu = Submenu::with_items(
            "Vectorio",
            true,
            &[&about, &sep(), &P::services(Some("Servicios")), &sep(), &P::hide(Some("Ocultar Vectorio")), &P::hide_others(Some("Ocultar otros")), &P::show_all(Some("Mostrar todo")), &sep(), &P::quit(Some("Salir de Vectorio"))],
        )
        .ok()?;

        let file = Submenu::with_items(
            "Archivo",
            true,
            &[
                &item("Nuevo", Some("CmdOrCtrl+N"), Command::New),
                &item("Abrir…", Some("CmdOrCtrl+O"), Command::Open),
                &item("Foto → dibujo…", Some("Shift+CmdOrCtrl+I"), Command::ImportImage),
                &sep(),
                &item("Cerrar", Some("CmdOrCtrl+W"), Command::CloseWindow),
                &item("Guardar", Some("CmdOrCtrl+S"), Command::Save),
                &item("Guardar como…", Some("Shift+CmdOrCtrl+S"), Command::SaveAs),
                &sep(),
                &item("Exportar SVG…", Some("CmdOrCtrl+E"), Command::ExportSvg),
                &item("Exportar un SVG por rotulador…", Some("Shift+CmdOrCtrl+E"), Command::ExportPerPen),
            ],
        )
        .ok()?;

        // Sin Cortar/Copiar/Pegar: así los campos de texto siguen recibiendo esas teclas.
        let edit = Submenu::with_items(
            "Edición",
            true,
            &[
                &item("Deshacer", Some("CmdOrCtrl+Z"), Command::Undo),
                &item("Rehacer", Some("Shift+CmdOrCtrl+Z"), Command::Redo),
                &sep(),
                &item("Duplicar", Some("CmdOrCtrl+D"), Command::Duplicate),
                &item("Eliminar", None, Command::Delete),
                &sep(),
                &item("Seleccionar todo", Some("CmdOrCtrl+A"), Command::SelectAll),
                &item("Seleccionar siguiente", None, Command::SelectNext),
                &item("Seleccionar anterior", None, Command::SelectPrev),
                &item("Deseleccionar", None, Command::Deselect),
            ],
        )
        .ok()?;

        let object = Submenu::with_items(
            "Objeto",
            true,
            &[
                &item("Traer adelante", Some("CmdOrCtrl+BracketRight"), Command::BringForward),
                &item("Enviar atrás", Some("CmdOrCtrl+BracketLeft"), Command::SendBackward),
                &item("Traer al frente", Some("Shift+CmdOrCtrl+BracketRight"), Command::BringToFront),
                &item("Enviar al fondo", Some("Shift+CmdOrCtrl+BracketLeft"), Command::SendToBack),
                &sep(),
                &item("Bloquear o desbloquear", Some("CmdOrCtrl+L"), Command::ToggleLock),
                &item("Otra variación de la letra", Some("CmdOrCtrl+R"), Command::Reseed),
            ],
        )
        .ok()?;

        // Las herramientas usan teclas sueltas (V, P, T, S): no se ponen como atajo del
        // menú para no robar esas letras a los campos de texto.
        let tools = Submenu::with_items(
            "Herramientas",
            true,
            &[
                &item("Seleccionar y mover   V", None, Command::Tool(Tool::Select)),
                &item("Mano alzada   P", None, Command::Tool(Tool::Draw)),
                &item("Texto manuscrito   T", None, Command::Tool(Tool::Text)),
                &item("Formas   S", None, Command::Tool(Tool::Shape)),
            ],
        )
        .ok()?;

        let paper = Submenu::new("Color del papel", true);
        for (i, (name, _)) in PAPER_TINTS.iter().enumerate() {
            paper.append(&item(name, None, Command::Paper(i))).ok()?;
        }
        let appearance = Submenu::new("Apariencia", true);
        for value in crate::theme::Appearance::ALL {
            appearance.append(&item(value.label(), None, Command::Appearance(value))).ok()?;
        }
        let view = Submenu::with_items(
            "Visualización",
            true,
            &[
                &item("Diseño", Some("CmdOrCtrl+1"), Command::Mode(ViewMode::Design)),
                &item("Salida", Some("CmdOrCtrl+2"), Command::Mode(ViewMode::Plotter)),
                &appearance,
                &item("Simular o pausar la salida", None, Command::PlayPause),
                &sep(),
                &item("Acercar", Some("CmdOrCtrl+Equal"), Command::ZoomIn),
                &item("Alejar", Some("CmdOrCtrl+Minus"), Command::ZoomOut),
                &item("Ajustar a la ventana", Some("CmdOrCtrl+Digit0"), Command::ZoomFit),
                &item("Tamaño real", Some("Shift+CmdOrCtrl+Digit0"), Command::ZoomActual),
                &sep(),
                &paper,
                &sep(),
                &P::fullscreen(Some("Pantalla completa")),
            ],
        )
        .ok()?;

        let window = Submenu::with_items(
            "Ventana",
            true,
            &[&P::minimize(Some("Minimizar")), &P::maximize(Some("Zoom")), &sep(), &P::bring_all_to_front(Some("Traer todo al frente"))],
        )
        .ok()?;
        let help = Submenu::with_items("Ayuda", true, &[&item("Atajos de teclado", Some("CmdOrCtrl+Slash"), Command::Shortcuts)]).ok()?;

        let menu = Menu::with_items(&[&app_menu, &file, &edit, &object, &tools, &view, &window, &help]).ok()?;
        #[cfg(target_os = "macos")]
        {
            menu.init_for_nsapp();
            window.set_as_windows_menu_for_nsapp();
            help.set_as_help_menu_for_nsapp();
        }

        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let close = ids.iter().find(|(_, c)| *c == Command::CloseWindow).map(|(id, _)| id.clone());
        muda::MenuEvent::set_event_handler(Some(move |e: muda::MenuEvent| {
            // Con la ventana oculta, una orden del menú (Nuevo, Abrir…) la vuelve a mostrar.
            if Some(&e.id) != close.as_ref() {
                crate::macos::show_window();
            }
            let _ = tx.send(e);
            ctx.request_repaint();
        }));
        Some(NativeMenu { _menu: menu, ids, rx })
    }

    /// Órdenes elegidas en el menú desde el último fotograma.
    pub fn poll(&self) -> Vec<Command> {
        self.rx.try_iter().filter_map(|e| self.ids.iter().find(|(id, _)| *id == e.id).map(|(_, c)| *c)).collect()
    }
}
