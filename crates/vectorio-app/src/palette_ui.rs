//! Biblioteca compartida: elegir una paleta o editar una copia antes de guardarla.
use crate::{
    app::VectorioApp,
    palettes::{self, Library, Palette},
    theme,
};
use vectorio_core::Pen;

#[derive(Clone, Copy, Default)]
pub enum Destination {
    #[default]
    Document,
    Reading {
        pass: u64,
        minimum: usize,
        single: bool,
    },
}

struct Editor {
    index: Option<usize>,
    palette: Palette,
    active: usize,
}

#[derive(Default)]
pub struct PaletteUi {
    pub open: bool,
    destination: Destination,
    source: Vec<Pen>,
    selected: usize,
    editor: Option<Editor>,
    delete_confirm: bool,
    discard_confirm: bool,
    error: Option<&'static str>,
}

impl PaletteUi {
    pub fn open_for(&mut self, destination: Destination, source: Vec<Pen>) {
        *self = Self {
            open: true,
            destination,
            source,
            ..Self::default()
        };
    }

    pub fn close_request(&mut self) {
        if self.editor.is_some() {
            self.discard_confirm = true;
        } else {
            self.open = false;
        }
    }

    fn begin(&mut self, index: Option<usize>, palette: Palette) {
        self.editor = Some(Editor {
            index,
            palette,
            active: 0,
        });
        self.delete_confirm = false;
        self.error = None;
    }

    fn show(&mut self, ctx: &egui::Context, library: &mut Library) -> Option<Palette> {
        if !self.open {
            return None;
        }
        let mut open = true;
        let mut applied = None;
        let pal = theme::current(ctx);
        egui::Window::new(tr!("Paletas"))
            .id(egui::Id::new("palette-library"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(660.)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(tr!("Tus juegos de tintas, disponibles en cualquier proyecto."));
                ui.add_space(12.);
                let presets = palettes::presets();
                self.selected = self.selected.min((presets.len() + library.saved.len()).saturating_sub(1));
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(190.);
                        ui.add_enabled_ui(self.editor.is_none(), |ui| {
                            egui::ScrollArea::vertical().id_salt("palette-list")
                                .max_height(286.).show(ui, |ui| {
                                ui.add_space(12.);
                                ui.weak(tr!("Mis paletas"));
                                if library.saved.is_empty() {
                                    ui.label(egui::RichText::new(tr!("Aún no has guardado ninguna.")).small().color(pal.secondary));
                                }
                                for (i, p) in library.saved.iter().enumerate() {
                                    let id = i + presets.len();
                                    if ui.push_id(("saved", i), |ui| palette_row(ui, &p.name, p, self.selected == id)).inner {
                                        self.selected = id;
                                        self.delete_confirm = false;
                                    }
                                }
                                ui.add_space(12.);
                                ui.weak(tr!("Predefinidas"));
                                for (i, p) in presets.iter().enumerate() {
                                    if ui.push_id(("preset", i), |ui| palette_row(ui, &crate::i18n::tr_key(&p.name), p, self.selected == i)).inner {
                                        self.selected = i;
                                        self.delete_confirm = false;
                                    }
                                }
                            });
                            ui.add_space(10.);
                            if ui.button(tr!("Nueva paleta")).clicked() {
                                self.begin(None, Palette { name: tr!("Mi paleta"), pens: vec![Pen::automatic([28,28,32], 0.4)] });
                            }
                            if ui.add_enabled(!self.source.is_empty() && self.source.len() <= palettes::MAX_INKS,
                                egui::Button::new(tr!("Guardar tintas actuales…"))).clicked() {
                                self.begin(None, Palette { name: tr!("Mi paleta"), pens: self.source.clone() });
                            }
                            if ui.small_button(tr!("Recargar")).clicked() {
                                self.error = library.reload().err();
                            }
                        });
                    });
                    ui.add_space(16.);
                    ui.vertical(|ui| {
                        ui.set_width(432.);
                        let selected = presets.get(self.selected)
                            .or_else(|| library.saved.get(self.selected.saturating_sub(presets.len()))).cloned();
                        if self.editor.is_some() {
                            self.edit(ui, library, presets.len());
                        } else if let Some(palette) = selected {
                            let builtin = self.selected < presets.len();
                            let title = if builtin { crate::i18n::tr_key(&palette.name) } else { palette.name.clone() };
                            ui.add(egui::Label::new(egui::RichText::new(&title).heading()).truncate()).on_hover_text(&title);
                            ui.add_space(8.);
                            swatches(ui, &palette);
                            ui.add_space(8.);
                            egui::ScrollArea::vertical().id_salt("palette-preview")
                                .max_height(208.).show(ui, |ui| {
                                for pen in &palette.pens {
                                    ui.horizontal(|ui| {
                                        theme::swatch(ui, pen.color, 16.);
                                        ui.label(crate::i18n::pen_name(pen));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.weak(format!("{} mm", crate::fmt::decimal(pen.width_mm, 2)));
                                        });
                                    });
                                }
                            });
                            ui.add_space(12.);
                            ui.horizontal(|ui| {
                                if !builtin && ui.button(tr!("Editar")).clicked() {
                                    self.begin(Some(self.selected - presets.len()), palette.clone());
                                }
                                if ui.button(tr!("Duplicar")).clicked() {
                                    self.begin(None, Palette { name: tr!("Copia de %{name}", name = title), ..palette.clone() });
                                }
                                if !builtin && ui.button(tr!("Eliminar paleta…")).clicked() {
                                    self.delete_confirm = true;
                                }
                            });
                            if self.delete_confirm {
                                ui.add_space(8.);
                                ui.label(tr!("Se elimina de la biblioteca. Los dibujos conservan sus tintas."));
                                ui.horizontal(|ui| {
                                    if ui.button(tr!("Eliminar paleta")).clicked() {
                                        match library.remove(self.selected - presets.len()) {
                                            Ok(()) => { self.selected = 0; self.delete_confirm = false; self.error = None; }
                                            Err(e) => self.error = Some(e),
                                        }
                                    }
                                    if ui.button(tr!("Cancelar")).clicked() { self.delete_confirm = false; }
                                });
                            } else {
                                ui.add_space(12.);
                                let valid = match self.destination {
                                    Destination::Document => true,
                                    Destination::Reading { minimum, single, .. } =>
                                        single || (palette.pens.len() >= minimum && palette.pens.len() <= 8),
                                };
                                if !valid {
                                    ui.label(tr!("Esta pasada necesita entre %{min} y 8 tintas.", min = match self.destination { Destination::Reading { minimum, .. } => minimum, _ => 1 }));
                                }
                                let action = match self.destination {
                                    Destination::Document => tr!("Añadir al documento"),
                                    Destination::Reading { .. } => tr!("Aplicar a la pasada"),
                                };
                                if ui.add_enabled(valid, egui::Button::new(action)).clicked() {
                                    applied = Some(palette);
                                }
                                if matches!(self.destination, Destination::Reading { single: true, .. }) {
                                    ui.weak(tr!("Esta pasada usa solo la primera tinta de la paleta."));
                                }
                            }
                        }
                    });
                });
                if let Some(error) = self.error.or(library.error) {
                    ui.add_space(10.);
                    ui.colored_label(ui.visuals().error_fg_color, crate::i18n::tr_key(error));
                }
                if self.discard_confirm {
                    ui.separator();
                    ui.label(tr!("Hay una edición de paleta sin guardar."));
                    ui.horizontal(|ui| {
                        if ui.button(tr!("Descartar edición y cerrar")).clicked() {
                            self.editor = None;
                            self.open = false;
                        }
                        if ui.button(tr!("Seguir editando")).clicked() { self.discard_confirm = false; }
                    });
                }
            });
        if !open {
            self.close_request();
        }
        if applied.is_some() {
            self.open = false;
        }
        applied
    }

    fn edit(&mut self, ui: &mut egui::Ui, library: &mut Library, offset: usize) {
        let editor = self.editor.as_mut().unwrap();
        ui.label(tr!("Nombre de la paleta"));
        ui.add(
            egui::TextEdit::singleline(&mut editor.palette.name)
                .desired_width(f32::INFINITY)
                .char_limit(256),
        );
        ui.add_space(8.);
        swatches(ui, &editor.palette);
        egui::ScrollArea::vertical()
            .id_salt("palette-editor")
            .max_height(214.)
            .show(ui, |ui| {
                for (i, pen) in editor.palette.pens.iter_mut().enumerate() {
                    ui.push_id(i, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(editor.active == i, format!("{}", i + 1))
                                .clicked()
                            {
                                editor.active = i;
                            }
                            let mut rgb = pen.color;
                            if ui.color_edit_button_srgb(&mut rgb).changed() {
                                pen.set_color(rgb);
                                editor.active = i;
                            }
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut pen.name)
                                        .desired_width(190.)
                                        .char_limit(256),
                                )
                                .changed()
                            {
                                pen.custom_name = true;
                            }
                            ui.add(
                                egui::DragValue::new(&mut pen.width_mm)
                                    .speed(0.01)
                                    .range(0.01..=100.)
                                    .suffix(" mm"),
                            );
                        });
                    });
                }
            });
        ui.add_space(8.);
        ui.horizontal(|ui| {
            let n = editor.palette.pens.len();
            if ui
                .add_enabled(
                    n < palettes::MAX_INKS,
                    egui::Button::new(tr!("Añadir tinta")),
                )
                .clicked()
            {
                editor.palette.pens.push(Pen::automatic([80, 80, 90], 0.4));
                editor.active = n;
            }
            if ui
                .add_enabled(editor.active > 0, egui::Button::new(tr!("Subir")))
                .clicked()
            {
                editor.palette.pens.swap(editor.active, editor.active - 1);
                editor.active -= 1;
            }
            if ui
                .add_enabled(editor.active + 1 < n, egui::Button::new(tr!("Bajar")))
                .clicked()
            {
                editor.palette.pens.swap(editor.active, editor.active + 1);
                editor.active += 1;
            }
            if ui
                .add_enabled(n > 1, egui::Button::new(tr!("Quitar tinta")))
                .clicked()
            {
                editor.palette.pens.remove(editor.active);
                editor.active = editor.active.min(n - 2);
            }
        });
        ui.add_space(12.);
        let mut save = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            save = ui.button(tr!("Guardar paleta")).clicked();
            cancel = ui.button(tr!("Cancelar")).clicked();
        });
        if save {
            match library.store(editor.index, editor.palette.clone()) {
                Ok(index) => {
                    self.editor = None;
                    self.selected = index + offset;
                    self.error = None;
                    self.discard_confirm = false;
                }
                Err(e) => self.error = Some(e),
            }
        } else if cancel {
            self.editor = None;
            self.error = None;
            self.discard_confirm = false;
        }
    }
}

fn swatches(ui: &mut egui::Ui, palette: &Palette) {
    let width = ui.available_width() / palette.pens.len().max(1) as f32;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.), egui::Sense::hover());
    for (i, pen) in palette.pens.iter().enumerate() {
        let cell = egui::Rect::from_min_size(
            rect.min + egui::vec2(i as f32 * width, 0.),
            egui::vec2(width, rect.height()),
        );
        ui.painter()
            .rect_filled(cell, 0., theme::pen_color(pen.color));
        ui.interact(
            cell,
            ui.id().with(("palette-swatch", i)),
            egui::Sense::hover(),
        )
        .on_hover_text(format!(
            "{} · #{:02X}{:02X}{:02X}",
            crate::i18n::pen_name(pen),
            pen.color[0],
            pen.color[1],
            pen.color[2]
        ));
    }
}

fn palette_row(ui: &mut egui::Ui, name: &str, palette: &Palette, selected: bool) -> bool {
    let mut clicked = false;
    ui.push_id(name, |ui| {
        ui.spacing_mut().item_spacing.y = 3.;
        clicked = ui
            .add_sized(
                [ui.available_width(), 24.],
                egui::Button::new(name).selected(selected),
            )
            .clicked();
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.), egui::Sense::hover());
        let width = rect.width() / palette.pens.len().max(1) as f32;
        for (i, pen) in palette.pens.iter().enumerate() {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    rect.min + egui::vec2(i as f32 * width, 0.),
                    egui::vec2(width, 4.),
                ),
                0.,
                theme::pen_color(pen.color),
            );
        }
        ui.add_space(4.);
    });
    clicked
}

pub fn show(app: &mut VectorioApp, ctx: &egui::Context) {
    let mut state = std::mem::take(&mut app.palette_ui);
    if let Some(palette) = state.show(ctx, &mut app.palettes) {
        match state.destination {
            Destination::Document => {
                let pens = palette.apply(&mut app.doc);
                if let Some(pen) = pens.first() {
                    app.active_pen = *pen;
                }
                app.notify(
                    ctx,
                    tr!("Paleta añadida. Los objetos conservan sus tintas."),
                );
            }
            Destination::Reading { pass, .. } => {
                if let Err(error) = app
                    .volumetric_ui
                    .apply_palette(pass, &palette, &mut app.doc)
                {
                    app.notify(ctx, crate::i18n::tr_key(error));
                }
            }
        }
    }
    app.palette_ui = state;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        app: &mut VectorioApp,
        time: &mut f64,
        events: Vec<egui::Event>,
        size: egui::Vec2,
    ) -> egui::FullOutput {
        *time += 0.1;
        ctx.run(
            egui::RawInput {
                time: Some(*time),
                events,
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    theme::paint_canvas(ui.painter(), ui.max_rect(), &theme::current(ctx));
                });
                show(app, ctx);
            },
        )
    }
    fn settle(ctx: &egui::Context, app: &mut VectorioApp, time: &mut f64) -> egui::FullOutput {
        let mut out = None;
        for _ in 0..4 {
            out = Some(frame(ctx, app, time, vec![], egui::vec2(980., 620.)));
        }
        out.unwrap()
    }
    fn point(out: &egui::FullOutput, label: &str) -> egui::Pos2 {
        out.shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    let r = egui::Rect::from_min_size(t.pos, t.galley.size());
                    (t.galley.text() == label && s.clip_rect.contains(r.center()))
                        .then_some(r.center())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("No aparece {label}"))
    }
    fn click(
        ctx: &egui::Context,
        app: &mut VectorioApp,
        time: &mut f64,
        out: &egui::FullOutput,
        label: &str,
    ) -> egui::FullOutput {
        let pos = point(out, label);
        for pressed in [true, false] {
            let _ = frame(
                ctx,
                app,
                time,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                egui::vec2(980., 620.),
            );
        }
        settle(ctx, app, time)
    }

    #[test]
    fn current_inks_can_be_saved_edited_and_applied_without_modifying_the_drawing() {
        let _lock = crate::i18n::locale_lock();
        for lang in ["es", "en"] {
            rust_i18n::set_locale(lang);
            let ctx = egui::Context::default();
            let mut app = VectorioApp::blank(&ctx);
            let original = app.doc.clone();
            app.palette_ui
                .open_for(Destination::Document, app.doc.pens.clone());
            let mut time = 0.;
            let out = settle(&ctx, &mut app, &mut time);
            let _ = click(
                &ctx,
                &mut app,
                &mut time,
                &out,
                &tr!("Guardar tintas actuales…"),
            );
            let editor = app.palette_ui.editor.as_mut().unwrap();
            editor.palette.name = "Mi colección".into();
            editor.palette.pens[0].set_color([200, 100, 45]);
            editor.palette.pens[0].width_mm = 0.63;
            let out = settle(&ctx, &mut app, &mut time);
            let out = click(&ctx, &mut app, &mut time, &out, &tr!("Guardar paleta"));
            assert_eq!(app.palettes.saved.len(), 1);
            assert_eq!(
                app.doc, original,
                "guardar no cambia las tintas del documento"
            );
            let _ = click(&ctx, &mut app, &mut time, &out, &tr!("Añadir al documento"));
            assert!(!app.palette_ui.open);
            assert_eq!(&app.doc.pens[..original.pens.len()], original.pens);
            assert_eq!(app.doc.items, original.items);
            assert_eq!(app.doc.pens.last().unwrap().color, [200, 100, 45]);
        }
        rust_i18n::set_locale("es");
    }

    #[test]
    fn palette_edits_block_document_commands_and_closing_requires_discarding_the_draft() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let original = app.doc.clone();
        app.palette_ui
            .open_for(Destination::Document, app.doc.pens.clone());
        app.palette_ui.begin(None, palettes::presets().remove(1));
        app.run(&ctx, crate::commands::Command::New);
        assert_eq!(app.doc, original);
        app.run(
            &ctx,
            crate::commands::Command::Mode(crate::app::ViewMode::Create),
        );
        assert_eq!(app.mode, crate::app::ViewMode::Design);
        app.run(&ctx, crate::commands::Command::CloseWindow);
        assert!(app.palette_ui.open && app.palette_ui.discard_confirm);
        let mut time = 0.;
        let out = settle(&ctx, &mut app, &mut time);
        let _ = click(
            &ctx,
            &mut app,
            &mut time,
            &out,
            &tr!("Descartar edición y cerrar"),
        );
        assert!(!app.palette_ui.open);
        assert!(app.palettes.saved.is_empty());
        assert_eq!(app.doc, original);
    }

    #[test]
    #[ignore = "Capturas de la biblioteca sin abrir ni reiniciar ventanas"]
    fn render_palette_library() {
        use image::{Rgba, RgbaImage};
        use std::collections::HashMap;
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/palettes");
        std::fs::create_dir_all(&dir).unwrap();
        for (width, height, dark, editing) in [
            (980, 620, true, false),
            (1440, 900, false, false),
            (980, 620, true, true),
            (1440, 900, false, true),
        ] {
            let ctx = egui::Context::default();
            let mut app = VectorioApp::blank(&ctx);
            ctx.options_mut(|o| {
                o.theme_preference = if dark {
                    egui::ThemePreference::Dark
                } else {
                    egui::ThemePreference::Light
                }
            });
            let mut palette = palettes::presets().remove(1);
            palette.name = "Cuaderno de viaje".into();
            palette.pens[0].set_color([56, 94, 128]);
            palette.pens[1].set_color([185, 84, 65]);
            app.palettes.store(None, palette.clone()).unwrap();
            app.palette_ui
                .open_for(Destination::Document, palette.pens.clone());
            app.palette_ui.selected = palettes::presets().len();
            if editing {
                app.palette_ui.begin(Some(0), palette);
            }
            let mut time = 0.;
            let mut textures = HashMap::new();
            let mut output = None;
            for _ in 0..8 {
                let result = frame(
                    &ctx,
                    &mut app,
                    &mut time,
                    vec![],
                    egui::vec2(width as f32, height as f32),
                );
                for (id, delta) in &result.textures_delta.set {
                    let size = delta.image.size();
                    let pixels: Vec<_> = match &delta.image {
                        egui::ImageData::Color(i) => i.pixels.clone(),
                        egui::ImageData::Font(i) => i.srgba_pixels(None).collect(),
                    };
                    if delta.pos.is_none() {
                        textures.insert(*id, RgbaImage::new(size[0] as u32, size[1] as u32));
                    }
                    let texture = textures.get_mut(id).unwrap();
                    let offset = delta.pos.unwrap_or([0, 0]);
                    for (i, color) in pixels.iter().enumerate() {
                        texture.put_pixel(
                            (offset[0] + i % size[0]) as u32,
                            (offset[1] + i / size[0]) as u32,
                            Rgba(color.to_array()),
                        );
                    }
                }
                output = Some(result);
            }
            let output = output.unwrap();
            for label in [
                "Paletas",
                if editing {
                    "Guardar paleta"
                } else {
                    "Añadir al documento"
                },
            ] {
                let pos = point(&output, label);
                assert!(
                    pos.x >= 0. && pos.x <= width as f32 && pos.y >= 0. && pos.y <= height as f32
                );
            }
            let mut canvas = RgbaImage::from_pixel(width, height, Rgba([245, 245, 245, 255]));
            for primitive in ctx.tessellate(output.shapes, output.pixels_per_point) {
                if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                    for tri in mesh.indices.as_chunks::<3>().0 {
                        crate::volumetric_ui::render_qa::raster(
                            &mut canvas,
                            &textures[&mesh.texture_id],
                            [
                                mesh.vertices[tri[0] as usize],
                                mesh.vertices[tri[1] as usize],
                                mesh.vertices[tri[2] as usize],
                            ],
                            primitive.clip_rect,
                        );
                    }
                }
            }
            canvas
                .save(dir.join(format!(
                    "{}-{width}.png",
                    if editing { "editor" } else { "library" }
                )))
                .unwrap();
        }
    }
}
