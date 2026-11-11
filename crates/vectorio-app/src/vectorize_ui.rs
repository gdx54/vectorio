//! Ventana «Foto → dibujo»: vectoriza en segundo plano (todos los núcleos) y enseña en
//! vivo cómo quedará en el papel: rellenos, tramas de mezcla y grosor de cada rotulador.

use crate::theme;
use egui::{Color32, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use image::DynamicImage;
use rayon::prelude::*;
use std::path::Path;
use std::sync::{mpsc, Arc};
use vectorio_core::mix::{self, MixPalette, Screen, ScreenParams};
use vectorio_core::vectorize::{self, VecMode, VecParams, VecResult};
use vectorio_core::{fill, p, Document, Fill, FillStyle, Geometry, Item, Pen, Polyline};

const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "tif", "tiff", "webp", "bmp", "gif"];

#[derive(Clone, Copy, PartialEq)]
enum Preview {
    Original,
    Result,
    Overlay,
}

/// Cómo se convierte cada capa en trazos (todo lo que no requiere volver a vectorizar).
#[derive(Clone, PartialEq)]
struct Look {
    fill: FillStyle,
    spacing: f64,
    angle: f64,
    vary_angle: bool,
    outline: bool,
    screen: ScreenParams,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            fill: FillStyle::Hatch,
            spacing: 1.2,
            angle: 45.0,
            vary_angle: false,
            outline: false,
            screen: ScreenParams { screen: Screen::Hatch, spacing_mm: 0.9, angle_deg: 15.0, pen_width_mm: 0.6 },
        }
    }
}

impl Look {
    fn fill_for(&self, i: usize) -> Fill {
        let angle = if self.vary_angle { (self.angle + 37.0 * i as f64).rem_euclid(180.0) } else { self.angle };
        Fill { style: self.fill, pen: 0, spacing_mm: self.spacing, angle_deg: angle, inset_mm: 0.0 }
    }
}

/// Clave de la vista previa: si cambia, se recalcula.
#[derive(Clone, PartialEq)]
struct PlotKey {
    result: usize,
    look: Look,
    scale: f64,
    widths: Vec<f64>,
}

/// Trazos finales de cada capa en mm (origen en la esquina de la imagen).
struct PlotPreview {
    layers: Vec<Vec<Polyline>>,
}

pub struct VectorizeUi {
    pub open: bool,
    name: String,
    image: Option<Arc<DynamicImage>>,
    texture: Option<egui::TextureHandle>,
    params: VecParams,
    computed: Option<VecParams>,
    result: Option<Arc<VecResult>>,
    job: Option<(VecParams, mpsc::Receiver<VecResult>)>,
    plot: Option<(PlotKey, Arc<PlotPreview>)>,
    plot_job: Option<(PlotKey, mpsc::Receiver<PlotPreview>)>,
    /// Lo que la vista previa debería mostrar ahora (se calcula cuando quede libre).
    pending_key: Option<PlotKey>,
    preview: Preview,
    look: Look,
    size_pct: f64,
}

impl Default for VectorizeUi {
    fn default() -> Self {
        VectorizeUi {
            open: false,
            name: String::new(),
            image: None,
            texture: None,
            params: VecParams::default(),
            computed: None,
            result: None,
            job: None,
            plot: None,
            plot_job: None,
            pending_key: None,
            preview: Preview::Result,
            look: Look::default(),
            size_pct: 100.0,
        }
    }
}

/// Rotulador de cada capa: uno del documento o uno nuevo (color, nombre, grosor).
#[derive(Clone)]
enum PenChoice {
    Existing(usize),
    New([u8; 3], String, f64),
}

impl VectorizeUi {
    pub fn open_dialog(&mut self, ctx: &egui::Context) {
        if self.image.is_some() && !self.open {
            self.open = true;
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Imagen", IMAGE_EXT)
            .pick_file()
        {
            let _ = self.load(ctx, &path);
        }
    }

    pub fn load(&mut self, ctx: &egui::Context, path: &Path) -> Result<(), String> {
        let img = image::open(path).map_err(|e| e.to_string())?;
        let shown = vectorize::working_image(&img, 1600);
        let size = [shown.width() as usize, shown.height() as usize];
        let color = egui::ColorImage::from_rgb(size, shown.as_raw());
        self.texture = Some(ctx.load_texture("vectorize-src", color, egui::TextureOptions::LINEAR));
        self.image = Some(Arc::new(img));
        self.name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Foto").to_string();
        self.computed = None;
        self.result = None;
        self.open = true;
        Ok(())
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some((prm, rx)) = &self.job {
            match rx.try_recv() {
                Ok(r) => {
                    self.computed = Some(prm.clone());
                    self.result = Some(Arc::new(r));
                    self.job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.job = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        self.poll_plot(ctx);
        let stale = self.computed.as_ref() != Some(&self.params);
        let running_same = self.job.as_ref().is_some_and(|(p, _)| *p == self.params);
        if stale && self.job.is_none() && !running_same {
            if let Some(img) = self.image.clone() {
                let prm = self.params.clone();
                let (tx, rx) = mpsc::channel();
                let ctx = ctx.clone();
                let job_prm = prm.clone();
                std::thread::spawn(move || {
                    let r = vectorize::vectorize(&img, &job_prm);
                    let _ = tx.send(r);
                    ctx.request_repaint();
                });
                self.job = Some((prm, rx));
            }
        }
    }

    /// Muestra la hoja modal. Devuelve los objetos (y rotuladores nuevos) al aceptar.
    pub fn show(&mut self, ctx: &egui::Context, doc: &Document) -> Option<(Vec<Item>, Vec<Pen>)> {
        if !self.open {
            return None;
        }
        if let Some(res) = self.result.clone() {
            self.pending_key = Some(self.key(doc, &res));
        }
        self.poll(ctx);
        let pal = theme::current(ctx);
        let ready = self.result.is_some() && self.computed.as_ref() == Some(&self.params);

        // Teclado: Esc o ⌘W cierran, ⏎ añade (si no se está escribiendo en un campo).
        let mut close = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape) || i.consume_key(Modifiers::COMMAND, Key::W));
        let mut accept = !ctx.wants_keyboard_input() && ready && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));

        // Velo sobre el resto de la ventana: bloquea el lienzo y cierra al pulsar fuera.
        let screen = ctx.screen_rect();
        let veil = egui::Area::new(egui::Id::new("vectorize-veil"))
            .fixed_pos(screen.min)
            .order(egui::Order::Middle)
            .interactable(true)
            .show(ctx, |ui| {
                let resp = ui.allocate_rect(screen, Sense::click());
                let a = if ui.visuals().dark_mode { 110 } else { 60 };
                ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(a));
                resp.clicked()
            });
        ctx.move_to_top(veil.response.layer_id);
        close |= veil.inner;

        let size = Vec2::new((screen.width() - 80.0).clamp(560.0, 980.0), (screen.height() - theme::TOOLBAR_H - 60.0).clamp(420.0, 680.0));
        let pos = Pos2::new(screen.center().x - size.x / 2.0, (theme::TOOLBAR_H + (screen.height() - theme::TOOLBAR_H - size.y) / 2.0).max(theme::TOOLBAR_H));
        let frame = theme::glass_frame(&pal).fill(pal.glass.to_opaque()).inner_margin(egui::Margin::same(0.0));
        egui::Area::new(egui::Id::new("vectorize-sheet"))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.set_width(size.x);
                    ui.set_max_width(size.x);
                    ui.set_height(size.y);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // Cabecera.
                    let head = ui.horizontal(|ui| {
                        ui.set_height(52.0);
                        ui.add_space(18.0);
                        ui.label(RichText::new("Foto → dibujo").size(15.0).strong().color(pal.text));
                        ui.label(RichText::new(&self.name).color(pal.secondary));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(14.0);
                            if close_button(ui, &pal).on_hover_text("Cerrar (Esc)").clicked() {
                                close = true;
                            }
                            ui.add_space(8.0);
                            ui.allocate_ui(Vec2::new(250.0, 28.0), |ui| {
                                crate::app::choice(ui, &mut self.preview, &[(Preview::Original, "Original"), (Preview::Result, "Dibujo"), (Preview::Overlay, "Superpuesto")]);
                            });
                        });
                    });
                    separator(ui, &pal, pos.x..=pos.x + size.x, head.response.rect.bottom());

                    // Cuerpo: vista previa y ajustes.
                    let side = 288.0;
                    let body_h = size.y - 52.0 - 56.0;
                    ui.horizontal_top(|ui| {
                        ui.set_height(body_h);
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                        let preview = Vec2::new(size.x - side - 1.0, body_h);
                        ui.allocate_ui(preview, |ui| {
                            let (r, _) = ui.allocate_exact_size(preview, Sense::hover());
                            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink(14.0)));
                            self.preview_area(&mut child, r.shrink(14.0).size(), doc);
                        });
                        let x = ui.cursor().left();
                        ui.painter().vline(x, ui.min_rect().top()..=ui.min_rect().top() + body_h, Stroke::new(1.0f32, pal.separator));
                        ui.add_space(1.0);
                        ui.allocate_ui_with_layout(Vec2::new(side, body_h), egui::Layout::top_down(egui::Align::Min), |ui| {
                            egui::ScrollArea::vertical().id_salt("vec-controls").max_height(body_h).show(ui, |ui| {
                                egui::Frame::none().inner_margin(egui::Margin { left: 16.0, right: 16.0, top: 10.0, bottom: 12.0 }).show(ui, |ui| {
                                    ui.set_width(side - 32.0);
                                    ui.set_max_width(side - 32.0);
                                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 7.0);
                                    self.controls(ui, doc);
                                });
                            });
                        });
                    });

                    // Pie: resumen y acciones.
                    separator(ui, &pal, pos.x..=pos.x + size.x, pos.y + 52.0 + body_h);
                    ui.horizontal(|ui| {
                        ui.set_height(56.0);
                        ui.add_space(18.0);
                        let summary = match &self.result {
                            Some(res) => {
                                let (w, h) = self.target_size(doc, res);
                                let n: usize = match &self.plot {
                                    Some((k, plot)) if k.result == Arc::as_ptr(res) as usize => plot.layers.iter().map(|l| l.len()).sum(),
                                    _ => res.layers.iter().map(|l| l.lines.len()).sum(),
                                };
                                format!("{w:.0} × {h:.0} mm · {n} trazos · {} capas", res.layers.len())
                            }
                            None => "Calculando…".into(),
                        };
                        ui.label(RichText::new(summary).color(pal.secondary));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(16.0);
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let add = ui.add_enabled_ui(ready, |ui| theme::accent_button(ui, "Añadir al papel")).inner;
                            if add.on_hover_text("⏎").clicked() {
                                accept = true;
                            }
                            if ui.button("Cancelar").on_hover_text("Esc").clicked() {
                                close = true;
                            }
                            if ui.button("Otra foto…").clicked() {
                                if let Some(path) = rfd::FileDialog::new().add_filter("Imagen", IMAGE_EXT).pick_file() {
                                    let _ = self.load(ui.ctx(), &path);
                                }
                            }
                        });
                    });
                });
            });

        if accept && ready {
            let built = self.build(doc);
            self.open = false;
            return built;
        }
        if close {
            self.open = false;
        }
        None
    }

    fn preview_area(&mut self, ui: &mut egui::Ui, size: Vec2, doc: &Document) {
        let pal = theme::current(ui.ctx());
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, egui::Rounding::same(theme::CONTROL_RADIUS), Color32::WHITE);
        let Some(tex) = &self.texture else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Abre una imagen", egui::FontId::proportional(14.0), pal.secondary);
            return;
        };
        let ts = tex.size_vec2();
        let k = ((rect.width() - 24.0) / ts.x).min((rect.height() - 24.0) / ts.y);
        let img_rect = Rect::from_center_size(rect.center(), ts * k);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        match self.preview {
            Preview::Original => {
                painter.image(tex.id(), img_rect, uv, Color32::WHITE);
            }
            Preview::Overlay => {
                painter.image(tex.id(), img_rect, uv, Color32::from_white_alpha(70));
            }
            Preview::Result => {}
        }
        if self.preview != Preview::Original {
            if let Some(res) = self.result.clone() {
                let choices = self.assign(doc, &res);
                let colors: Vec<[u8; 3]> = choices.iter().map(|c| choice_color(doc, c)).collect();
                let widths = self.widths(doc, &choices);
                let fresh = self.plot.as_ref().filter(|(k, _)| k.result == Arc::as_ptr(&res) as usize);
                let mut shapes = Vec::new();
                match fresh {
                    // Simulación: trazos finales con el grosor real de cada rotulador.
                    Some((key, plot)) => {
                        let mm = img_rect.width() / (res.width * key.scale) as f32;
                        for ((lines, c), w) in plot.layers.iter().zip(&colors).zip(&widths) {
                            let st = Stroke::new((*w as f32 * mm).max(0.6), Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 225));
                            for l in lines {
                                let pts: Vec<Pos2> = l.iter().map(|q| img_rect.min + Vec2::new(q.x as f32, q.y as f32) * mm).collect();
                                if pts.len() >= 2 {
                                    shapes.push(egui::Shape::line(pts, st));
                                }
                            }
                        }
                    }
                    // Mientras se calcula la primera vez: contornos finos.
                    None => {
                        let s = img_rect.width() / res.width as f32;
                        for (layer, c) in res.layers.iter().zip(&colors) {
                            let st = Stroke::new(1.0f32, Color32::from_rgb(c[0], c[1], c[2]));
                            for l in &layer.lines {
                                let pts: Vec<Pos2> = l.iter().map(|q| img_rect.min + Vec2::new(q.x as f32, q.y as f32) * s).collect();
                                if pts.len() >= 2 {
                                    shapes.push(egui::Shape::line(pts, st));
                                }
                            }
                        }
                    }
                }
                painter.extend(shapes);
            }
        }
        if self.plot_job.is_some() && self.job.is_none() {
            let c = rect.right_top() + Vec2::new(-24.0, 24.0);
            painter.circle_filled(c, 14.0, pal.glass);
            ui.put(Rect::from_center_size(c, Vec2::splat(18.0)), egui::Spinner::new());
        }
        if self.job.is_some() {
            let c = rect.right_top() + Vec2::new(-24.0, 24.0);
            painter.circle_filled(c, 14.0, pal.glass);
            ui.put(Rect::from_center_size(c, Vec2::splat(18.0)), egui::Spinner::new());
        }
    }

    /// Rotulador de cada capa. En Colores y Mezcla se crean rotuladores nuevos, salvo
    /// que ya tengas uno casi igual; en los modos de tinta se usa el tuyo más parecido.
    fn assign(&self, doc: &Document, res: &VecResult) -> Vec<PenChoice> {
        let new_pens = matches!(self.params.mode, VecMode::Colors | VecMode::Mix);
        let width = if self.params.mode == VecMode::Mix { self.look.screen.pen_width_mm } else { 0.8 };
        res.layers
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let near = doc.nearest_pen(l.color);
                if !new_pens {
                    return PenChoice::Existing(near);
                }
                let d = doc.pens.get(near).map(|p| {
                    let c = p.color;
                    ((c[0] as f64 - l.color[0] as f64).powi(2) + (c[1] as f64 - l.color[1] as f64).powi(2) + (c[2] as f64 - l.color[2] as f64).powi(2)).sqrt()
                });
                if d.is_some_and(|d| d < 28.0) {
                    PenChoice::Existing(near)
                } else {
                    let name = l.name.map(|n| n.to_string()).unwrap_or_else(|| format!("{} {}", self.name, i + 1));
                    PenChoice::New(l.color, name, width)
                }
            })
            .collect()
    }

    fn widths(&self, doc: &Document, choices: &[PenChoice]) -> Vec<f64> {
        choices
            .iter()
            .map(|c| match c {
                PenChoice::Existing(i) => doc.pens.get(*i).map(|p| p.width_mm).unwrap_or(0.8),
                PenChoice::New(_, _, w) => *w,
            })
            .collect()
    }

    /// Recalcula en segundo plano los trazos finales cuando cambia algo que les afecta.
    fn poll_plot(&mut self, ctx: &egui::Context) {
        if let Some((key, rx)) = &self.plot_job {
            match rx.try_recv() {
                Ok(r) => {
                    self.plot = Some((key.clone(), Arc::new(r)));
                    self.plot_job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.plot_job = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.plot_job.is_some() {
            return;
        }
        let Some(res) = self.result.clone() else { return };
        let Some(key) = self.pending_key.take() else { return };
        if self.plot.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let k = key.clone();
        std::thread::spawn(move || {
            let _ = tx.send(plot_lines(&res, &k));
            ctx.request_repaint();
        });
        self.plot_job = Some((key, rx));
    }

    /// Ajustes de la columna derecha.
    fn controls(&mut self, ui: &mut egui::Ui, doc: &Document) {
        let pal = theme::current(ui.ctx());
        theme::section(ui, "ESTILO");
        egui::ComboBox::from_id_salt("vec-mode")
            .width(ui.available_width())
            .selected_text(self.params.mode.label())
            .show_ui(ui, |ui| {
                for m in VecMode::ALL {
                    ui.selectable_value(&mut self.params.mode, m, m.label());
                }
            });
        ui.label(RichText::new(self.params.mode.help()).small().color(pal.secondary));
        let prm = &mut self.params;
        match prm.mode {
            VecMode::Colors => {
                field(ui, "Colores", |ui| ui.add(egui::Slider::new(&mut prm.colors, 2..=12)));
                field(ui, "Manchas mín.", |ui| ui.add(egui::Slider::new(&mut prm.min_size, 0.0..=200.0).logarithmic(true)));
                ui.checkbox(&mut prm.skip_paper, "Omitir el color del papel");
            }
            VecMode::Mix => {
                field(ui, "Rotuladores", |ui| {
                    egui::ComboBox::from_id_salt("vec-palette")
                        .width(ui.available_width())
                        .selected_text(prm.mix_palette.label())
                        .show_ui(ui, |ui| {
                            for m in MixPalette::ALL {
                                ui.selectable_value(&mut prm.mix_palette, m, m.label());
                            }
                        })
                        .response
                });
                if prm.mix_palette == MixPalette::Photo {
                    field(ui, "Colores", |ui| ui.add(egui::Slider::new(&mut prm.colors, 2..=8)));
                }
            }
            VecMode::Lines | VecMode::Silhouette => {
                let mut auto = prm.threshold < 0;
                field(ui, "Umbral", |ui| {
                    let r = ui.checkbox(&mut auto, "Auto");
                    if !auto {
                        if prm.threshold < 0 {
                            prm.threshold = 128;
                        }
                        ui.spacing_mut().slider_width = (ui.available_width() - 58.0).max(40.0);
                        ui.add(egui::Slider::new(&mut prm.threshold, 0..=255));
                    }
                    r
                });
                if auto {
                    prm.threshold = -1;
                }
                field(ui, "Manchas mín.", |ui| ui.add(egui::Slider::new(&mut prm.min_size, 0.0..=200.0).logarithmic(true)));
                ui.checkbox(&mut prm.invert, "Invertir (trazo claro sobre oscuro)");
            }
            VecMode::Engrave => {
                field(ui, "Líneas", |ui| ui.add(egui::Slider::new(&mut prm.engrave_rows, 20..=300)));
                field(ui, "Intensidad", |ui| ui.add(egui::Slider::new(&mut prm.engrave_amp, 0.1..=2.0)));
                ui.checkbox(&mut prm.invert, "Invertir");
            }
        }

        let look = &mut self.look;
        match self.params.mode {
            VecMode::Colors | VecMode::Silhouette => {
                theme::section(ui, "RELLENO DE LAS FORMAS");
                field(ui, "Relleno", |ui| {
                    egui::ComboBox::from_id_salt("vec-fill")
                        .width(ui.available_width())
                        .selected_text(look.fill.label())
                        .show_ui(ui, |ui| {
                            for s in FillStyle::ALL {
                                ui.selectable_value(&mut look.fill, s, s.label());
                            }
                        })
                        .response
                });
                if !matches!(look.fill, FillStyle::None | FillStyle::Solid) {
                    field(ui, "Separación", |ui| ui.add(egui::Slider::new(&mut look.spacing, 0.4..=6.0).logarithmic(true).suffix(" mm")));
                }
                if matches!(look.fill, FillStyle::Hatch | FillStyle::Cross | FillStyle::Zigzag | FillStyle::Solid | FillStyle::Wave) {
                    field(ui, "Ángulo", |ui| ui.add(egui::Slider::new(&mut look.angle, 0.0..=180.0).suffix("°")));
                    ui.checkbox(&mut look.vary_angle, "Un ángulo distinto para cada color");
                }
                if look.fill != FillStyle::None {
                    ui.checkbox(&mut look.outline, "Dibujar también el contorno");
                }
            }
            VecMode::Mix => {
                theme::section(ui, "TRAMA");
                crate::app::choice(ui, &mut look.screen.screen, &[(Screen::Hatch, "Rayado"), (Screen::Dots, "Puntos")]);
                field(ui, "Separación", |ui| ui.add(egui::Slider::new(&mut look.screen.spacing_mm, 0.4..=4.0).logarithmic(true).suffix(" mm")));
                if look.screen.screen == Screen::Hatch {
                    field(ui, "Ángulo", |ui| ui.add(egui::Slider::new(&mut look.screen.angle_deg, 0.0..=180.0).suffix("°")));
                }
                field(ui, "Grosor punta", |ui| ui.add(egui::Slider::new(&mut look.screen.pen_width_mm, 0.2..=1.5).suffix(" mm")));
                let help = if look.screen.screen == Screen::Hatch {
                    "Cada rotulador raya en su propio ángulo; donde se cruzan, los colores se mezclan."
                } else {
                    "Los puntos de cada rotulador se intercalan sin taparse."
                };
                ui.label(RichText::new(help).small().color(pal.secondary));
            }
            _ => {}
        }

        theme::section(ui, "ROTULADORES");
        if let Some(res) = self.result.clone() {
            let choices = self.assign(doc, &res);
            let new = choices.iter().filter(|c| matches!(c, PenChoice::New(..))).count();
            let help = match (self.params.mode, new) {
                (VecMode::Colors | VecMode::Mix, 0) => "Usa rotuladores que ya tienes.".to_string(),
                (VecMode::Colors | VecMode::Mix, n) => format!("Se añadirán {n} rotuladores nuevos; luego puedes cambiarles el color."),
                _ => "Se usará tu rotulador más parecido.".to_string(),
            };
            ui.label(RichText::new(help).small().color(pal.secondary));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for c in &choices {
                    let name = match c {
                        PenChoice::Existing(i) => doc.pens.get(*i).map(|p| p.name.clone()).unwrap_or_default(),
                        PenChoice::New(_, n, _) => format!("{n} (nuevo)"),
                    };
                    theme::swatch(ui, choice_color(doc, c), 16.0).on_hover_text(name);
                }
            });
        }

        theme::section(ui, "DETALLE");
        let prm = &mut self.params;
        field(ui, "Suavizar foto", |ui| ui.add(egui::Slider::new(&mut prm.blur, 0.0..=4.0)));
        if prm.mode != VecMode::Mix {
            field(ui, "Simplificar", |ui| ui.add(egui::Slider::new(&mut prm.detail, 0.2..=3.0)));
            field(ui, "Curvas", |ui| ui.add(egui::Slider::new(&mut prm.smooth, 0..=4)));
        }
        field(ui, "Resolución", |ui| {
            egui::ComboBox::from_id_salt("vec-res")
                .width(ui.available_width())
                .selected_text(format!("{} px", prm.max_px))
                .show_ui(ui, |ui| {
                    for px in [500, 700, 900, 1200, 1600, 2000] {
                        ui.selectable_value(&mut prm.max_px, px, format!("{px} px"));
                    }
                })
                .response
        });

        theme::section(ui, "TAMAÑO EN EL PAPEL");
        field(ui, "Área útil", |ui| ui.add(egui::Slider::new(&mut self.size_pct, 10.0..=100.0).suffix(" %")));
    }

    fn scale(&self, doc: &Document, res: &VecResult) -> f64 {
        let paper = &doc.paper;
        let (pw, ph) = (paper.width_mm - 2.0 * paper.margin_mm, paper.height_mm - 2.0 * paper.margin_mm);
        (pw / res.width).min(ph / res.height) * self.size_pct / 100.0
    }

    fn target_size(&self, doc: &Document, res: &VecResult) -> (f64, f64) {
        let s = self.scale(doc, res);
        (res.width * s, res.height * s)
    }

    fn key(&self, doc: &Document, res: &Arc<VecResult>) -> PlotKey {
        let choices = self.assign(doc, res);
        PlotKey { result: Arc::as_ptr(res) as usize, look: self.look.clone(), scale: self.scale(doc, res), widths: self.widths(doc, &choices) }
    }

    fn build(&self, doc: &Document) -> Option<(Vec<Item>, Vec<Pen>)> {
        let res = self.result.as_ref()?;
        let s = self.scale(doc, res);
        let center = p(doc.paper.width_mm / 2.0, doc.paper.height_mm / 2.0);
        let half = p(res.width * s / 2.0, res.height * s / 2.0);
        let key = self.key(doc, res);
        let mut pens = Vec::new();
        let mut items = Vec::new();
        for (i, (layer, choice)) in res.layers.iter().zip(self.assign(doc, res)).enumerate() {
            let pen = match choice {
                PenChoice::Existing(i) => i,
                PenChoice::New(c, name, w) => {
                    pens.push(Pen::new(&name, c, w));
                    doc.pens.len() + pens.len() - 1
                }
            };
            let label = layer.name.map(|n| format!("{} · {}", self.name, n)).unwrap_or_else(|| format!("{} · {}", self.name, i + 1));
            let to_local = |l: &Polyline| -> Polyline { l.iter().map(|q| p(q.x * s - half.x, q.y * s - half.y)).collect() };
            let mut item = match &layer.coverage {
                // Mezcla: la trama ya es el dibujo.
                Some(cov) => {
                    let lines = mix::render(cov, s, &key.look.screen, i, res.layers.len());
                    let lines = lines.iter().map(|l| l.iter().map(|q| *q - half).collect()).collect();
                    Item::new(label, Geometry::Path { lines, closed: false }, center)
                }
                None => Item::new(label, Geometry::Path { lines: layer.lines.iter().map(to_local).collect(), closed: layer.closed }, center),
            };
            item.fill.pen = pen;
            if layer.closed && self.look.fill != FillStyle::None {
                item.fill = Fill { pen, ..self.look.fill_for(i) };
                item.stroke = if self.look.outline { Some(pen) } else { None };
            } else {
                item.stroke = Some(pen);
            }
            items.push(item);
        }
        Some((items, pens))
    }
}

fn choice_color(doc: &Document, c: &PenChoice) -> [u8; 3] {
    match c {
        PenChoice::Existing(i) => doc.pens.get(*i).map(|p| p.color).unwrap_or([0, 0, 0]),
        PenChoice::New(c, _, _) => *c,
    }
}

/// Trazos finales de cada capa (en mm), en paralelo: una capa por núcleo.
fn plot_lines(res: &VecResult, key: &PlotKey) -> PlotPreview {
    let s = key.scale;
    let n = res.layers.len();
    let layers = res
        .layers
        .par_iter()
        .enumerate()
        .map(|(i, layer)| {
            let w = key.widths.get(i).copied().unwrap_or(0.8);
            if let Some(cov) = &layer.coverage {
                let screen = ScreenParams { pen_width_mm: w, ..key.look.screen.clone() };
                return mix::render(cov, s, &screen, i, n);
            }
            let mm: Vec<Polyline> = layer.lines.iter().map(|l| l.iter().map(|q| p(q.x * s, q.y * s)).collect()).collect();
            if !layer.closed || key.look.fill == FillStyle::None {
                return mm;
            }
            let mut out = fill::generate(&mm, &key.look.fill_for(i), w, i as u64 + 1);
            if key.look.outline {
                out.extend(mm);
            }
            out
        })
        .collect();
    PlotPreview { layers }
}

/// Fila con etiqueta a la izquierda y control que ocupa el resto.
fn field(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> egui::Response) {
    let pal = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(Vec2::new(86.0, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(86.0);
            ui.add(egui::Label::new(RichText::new(label).color(pal.secondary)).truncate());
        });
        ui.spacing_mut().slider_width = (ui.available_width() - 76.0).max(60.0);
        add(ui);
    });
}

fn separator(ui: &egui::Ui, pal: &theme::Palette, x: std::ops::RangeInclusive<f32>, y: f32) {
    ui.painter().with_clip_rect(ui.ctx().screen_rect()).hline(x, y, Stroke::new(1.0f32, pal.separator));
}

/// Botón redondo de cerrar, como el de las hojas de macOS.
fn close_button(ui: &mut egui::Ui, pal: &theme::Palette) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
    let c = rect.center();
    let bg = if resp.hovered() { pal.control_hover } else { pal.control };
    ui.painter().circle_filled(c, 12.0, bg);
    let s = Stroke::new(1.6f32, pal.secondary);
    let d = 4.0;
    ui.painter().line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], s);
    ui.painter().line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], s);
    resp
}
