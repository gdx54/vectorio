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
use vectorio_core::photo::EncodedImage;
use vectorio_core::trace::TraceKey;
use vectorio_core::{fill, p, ColorAdjust, Coverage, Document, Fill, FillStyle, Geometry, Item, Pen, Photo, Polyline, Traced};

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
        Fill { style: self.fill, pen: 0, spacing_mm: self.spacing, angle_deg: angle, inset_mm: 0.0, ..Default::default() }
    }
}

/// Clave de la vista previa: si cambia, se recalcula.
#[derive(Clone, PartialEq)]
struct PlotKey {
    result: u64,
    look: Look,
    scale: f64,
    widths: Vec<f64>,
}

/// Trazos finales de cada capa en mm (origen en la esquina de la imagen).
struct PlotPreview {
    layers: Vec<Vec<Polyline>>,
}

/// Cámara de la vista previa. El zoom es relativo al encuadre inicial y `center`
/// indica el punto de la imagen que se mantiene en el centro al redimensionar.
#[derive(Clone, Copy)]
struct PreviewView {
    zoom: f32,
    center: Vec2,
}

impl Default for PreviewView {
    fn default() -> Self { Self { zoom: 1.0, center: Vec2::splat(0.5) } }
}

impl PreviewView {
    fn image_rect(&self, viewport: Rect, source: Vec2) -> Rect {
        let fit = ((viewport.width() - 24.0).max(1.0) / source.x).min((viewport.height() - 24.0).max(1.0) / source.y);
        let size = source * fit * self.zoom;
        Rect::from_min_size(viewport.center() - self.center * size, size)
    }

    fn zoom_at(&mut self, factor: f32, anchor: Pos2, viewport: Rect, source: Vec2) {
        let image = self.image_rect(viewport, source);
        let point = (anchor - image.min) / image.size();
        self.zoom = (self.zoom * factor).clamp(0.25, 32.0);
        let size = self.image_rect(viewport, source).size();
        self.center = point - (anchor - viewport.center()) / size;
    }

    fn pan(&mut self, delta: Vec2, viewport: Rect, source: Vec2) {
        self.center -= delta / self.image_rect(viewport, source).size();
    }
}

pub struct VectorizeUi {
    pub open: bool,
    name: String,
    image: Option<Arc<DynamicImage>>,
    texture: Option<egui::TextureHandle>,
    /// Foto reducida sin ajustes, para rehacer la vista previa al mover los ajustes de color.
    source: Option<Arc<image::RgbImage>>,
    /// Ajustes con que está pintada la textura ahora, e histograma de luz de ese resultado.
    shown_adjust: ColorAdjust,
    histogram: [f32; 64],
    params: VecParams,
    computed: Option<VecParams>,
    result: Option<Arc<VecResult>>,
    result_revision: u64,
    job: Option<(VecParams, mpsc::Receiver<VecResult>)>,
    plot: Option<(PlotKey, Arc<PlotPreview>)>,
    plot_job: Option<(PlotKey, mpsc::Receiver<PlotPreview>)>,
    /// Lo que la vista previa debería mostrar ahora (se calcula cuando quede libre).
    pending_key: Option<PlotKey>,
    preview: Preview,
    look: Look,
    size_pct: f64,
    view: PreviewView,
    /// Grupo de foto que se sustituye al aceptar («Volver a separar»).
    replacing: Option<u64>,
}

/// Lo que se añade al documento al aceptar.
pub struct Import {
    pub items: Vec<Item>,
    pub pens: Vec<Pen>,
    /// Foto viva de las tintas de Mezcla (los objetos llevan `group` provisional 0).
    pub photo: Option<Photo>,
    pub replacing: Option<u64>,
}

impl Default for VectorizeUi {
    fn default() -> Self {
        VectorizeUi {
            open: false,
            name: String::new(),
            image: None,
            texture: None,
            source: None,
            shown_adjust: ColorAdjust::default(),
            histogram: [0.0; 64],
            params: VecParams::default(),
            computed: None,
            result: None,
            result_revision: 0,
            job: None,
            plot: None,
            plot_job: None,
            pending_key: None,
            preview: Preview::Result,
            look: Look::default(),
            size_pct: 100.0,
            view: PreviewView::default(),
            replacing: None,
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
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Foto").to_string();
        self.set_image(ctx, img, name);
        self.replacing = None;
        Ok(())
    }

    /// Vuelve a abrir una foto viva del documento para separar sus colores de nuevo;
    /// al aceptar, sus tintas se sustituyen en el mismo sitio.
    pub fn reopen(&mut self, ctx: &egui::Context, photo: &Photo) -> Result<(), String> {
        let img = photo.image.decode()?;
        self.set_image(ctx, img, photo.name.clone());
        self.params = photo.params.clone();
        self.size_pct = photo.size_pct;
        self.replacing = Some(photo.id);
        Ok(())
    }

    fn set_image(&mut self, ctx: &egui::Context, img: DynamicImage, name: String) {
        self.source = Some(Arc::new(vectorize::working_image(&img, 1600)));
        self.texture = None;
        self.refresh_texture(ctx);
        self.image = Some(Arc::new(img));
        self.name = name;
        self.computed = None;
        self.result = None;
        self.result_revision = self.result_revision.wrapping_add(1);
        // Una foto nueva invalida también los trabajos y la trama de la anterior.
        self.job = None;
        self.plot = None;
        self.plot_job = None;
        self.pending_key = None;
        self.view = PreviewView::default();
        self.open = true;
    }

    /// Pinta la vista «Original» con los ajustes de color actuales.
    fn refresh_texture(&mut self, ctx: &egui::Context) {
        let Some(src) = &self.source else { return };
        let adjust = self.params.adjust.clone();
        if self.texture.is_some() && adjust == self.shown_adjust {
            return;
        }
        let shown = adjust.apply(src);
        let mut hist = [0usize; 64];
        for px in shown.pixels() {
            let y = 0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32;
            hist[(y as usize / 4).min(63)] += 1;
        }
        let peak = hist.iter().copied().max().unwrap_or(1).max(1) as f32;
        self.histogram = hist.map(|n| (n as f32 / peak).sqrt());
        let size = [shown.width() as usize, shown.height() as usize];
        let color = egui::ColorImage::from_rgb(size, shown.as_raw());
        match &mut self.texture {
            Some(t) => t.set(color, egui::TextureOptions::LINEAR),
            None => self.texture = Some(ctx.load_texture("vectorize-src", color, egui::TextureOptions::LINEAR)),
        }
        self.shown_adjust = adjust;
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some((prm, rx)) = &self.job {
            match rx.try_recv() {
                Ok(r) => {
                    self.computed = Some(prm.clone());
                    self.result = Some(Arc::new(r));
                    self.result_revision = self.result_revision.wrapping_add(1);
                    self.job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.job = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
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
    pub fn show(&mut self, ctx: &egui::Context, doc: &Document) -> Option<Import> {
        if !self.open {
            return None;
        }
        self.refresh_texture(ctx);
        self.poll(ctx);
        if let Some(res) = self.result.clone() {
            self.pending_key = Some(self.key(doc, &res));
        }
        self.poll_plot(ctx);
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

        let size = Vec2::new((screen.width() - 80.0).clamp(760.0f32.min(screen.width() - 16.0), 1240.0), (screen.height() - theme::TOOLBAR_H - 60.0).clamp(420.0, 680.0));
        let pos = Pos2::new(screen.center().x - size.x / 2.0, (theme::TOOLBAR_H + (screen.height() - theme::TOOLBAR_H - size.y) / 2.0).max(theme::TOOLBAR_H));
        let frame = theme::glass_frame(&pal).fill(pal.bar.to_opaque()).inner_margin(egui::Margin::same(0.0));
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
                        ui.label(RichText::new(if self.replacing.is_some() { "Volver a separar la foto" } else { "Foto → dibujo" }).size(15.0).strong().color(pal.text));
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

                    // Cuerpo: color de la foto, vista previa y ajustes del dibujo.
                    let side = 288.0;
                    let left = 252.0;
                    let body_h = size.y - 52.0 - 56.0;
                    ui.horizontal_top(|ui| {
                        ui.set_height(body_h);
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                        ui.allocate_ui_with_layout(Vec2::new(left, body_h), egui::Layout::top_down(egui::Align::Min), |ui| {
                            egui::ScrollArea::vertical().id_salt("vec-color").max_height(body_h).show(ui, |ui| {
                                egui::Frame::none().inner_margin(egui::Margin { left: 16.0, right: 16.0, top: 10.0, bottom: 12.0 }).show(ui, |ui| {
                                    ui.set_width(left - 32.0);
                                    ui.set_max_width(left - 32.0);
                                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                                    self.color_panel(ui);
                                });
                            });
                        });
                        let x = ui.cursor().left();
                        ui.painter().vline(x, ui.min_rect().top()..=ui.min_rect().top() + body_h, Stroke::new(1.0f32, pal.separator));
                        ui.add_space(1.0);
                        let preview = Vec2::new(size.x - side - left - 2.0, body_h);
                        ui.allocate_ui(preview, |ui| {
                            let (r, _) = ui.allocate_exact_size(preview, Sense::hover());
                            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink(14.0)).layout(egui::Layout::top_down(egui::Align::Min)));
                            child.set_clip_rect(child.clip_rect().intersect(r.shrink(14.0)));
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
                                    Some((k, plot)) if k.result == self.result_revision => plot.layers.iter().map(|l| l.len()).sum(),
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
                            let add = ui.add_enabled_ui(ready, |ui| theme::accent_button(ui, if self.replacing.is_some() { "Sustituir en el papel" } else { "Añadir al papel" })).inner;
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
            let built = self.build(doc).map(|(items, pens)| Import { photo: self.photo(), items, pens, replacing: self.replacing });
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
        // Los controles de navegación quedan fuera del área que se amplía y recorta.
        ui.horizontal(|ui| {
            ui.set_min_height(30.0);
            ui.spacing_mut().item_spacing.x = 6.0;
            if ui.button("Alejar").clicked() { self.view.zoom = (self.view.zoom * 0.8).clamp(0.25, 32.0); }
            ui.label(RichText::new(format!("{:.0} %", self.view.zoom * 100.0)).color(pal.secondary));
            if ui.button("Acercar").clicked() { self.view.zoom = (self.view.zoom * 1.25).clamp(0.25, 32.0); }
            if ui.button("Ajustar").clicked() { self.view = PreviewView::default(); }
        });
        let preview_size = Vec2::new(size.x, (size.y - 38.0).max(1.0));
        let (rect, response) = ui.allocate_exact_size(preview_size, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, egui::Rounding::same(theme::CONTROL_RADIUS), Color32::WHITE);
        let Some(tex) = &self.texture else {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Abre una imagen", egui::FontId::proportional(14.0), pal.secondary);
            return;
        };
        let ts = tex.size_vec2();
        let (zoom, scroll, touch) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.multi_touch()));
        if let Some(pos) = response.hover_pos() {
            if zoom != 1.0 { self.view.zoom_at(zoom, pos, rect, ts); }
            if let Some(touch) = touch {
                self.view.pan(touch.translation_delta, rect, ts);
            } else if zoom == 1.0 && scroll != Vec2::ZERO {
                self.view.pan(scroll, rect, ts);
            }
        }
        if response.dragged() && touch.is_none() {
            self.view.pan(response.drag_delta(), rect, ts);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::Grab); }
        if response.double_clicked() { self.view = PreviewView::default(); }
        let img_rect = self.view.image_rect(rect, ts);
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
                let fresh = self.plot.as_ref().filter(|(k, _)| k.result == self.result_revision);
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
            painter.circle_filled(c, 14.0, pal.bar);
            let mut overlay = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(c, Vec2::splat(18.0))));
            overlay.add(egui::Spinner::new());
        }
        if self.job.is_some() {
            let c = rect.right_top() + Vec2::new(-24.0, 24.0);
            painter.circle_filled(c, 14.0, pal.bar);
            let mut overlay = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(c, Vec2::splat(18.0))));
            overlay.add(egui::Spinner::new());
        }
    }

    /// Rotulador de cada capa. En Colores y Mezcla se crean rotuladores nuevos, salvo
    /// que ya tengas uno casi igual; en los modos de tinta se usa el tuyo más parecido.
    /// Rotulador de cada capa. En Colores y Mezcla, cada color del dibujo pasa a ser una
    /// tinta de la paleta; solo se reutiliza una tinta que ya tenga ese mismo color.
    fn assign(&self, doc: &Document, res: &VecResult) -> Vec<PenChoice> {
        let new_pens = matches!(self.params.mode, VecMode::Colors | VecMode::Mix);
        let width = if self.params.mode == VecMode::Mix { self.look.screen.pen_width_mm } else { 0.8 };
        res.layers
            .iter()
            .map(|l| {
                if !new_pens {
                    return PenChoice::Existing(doc.nearest_pen(l.color));
                }
                match doc.pens.iter().position(|p| color_distance(p.color, l.color) <= SAME_INK) {
                    Some(i) => PenChoice::Existing(i),
                    None => {
                        let name = l.name.map(str::to_string).unwrap_or_else(|| Pen::automatic(l.color, width).name);
                        PenChoice::New(l.color, name, width)
                    }
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
    /// Panel izquierdo: cómo se leen la luz y los colores de la foto antes de separarla.
    fn color_panel(&mut self, ui: &mut egui::Ui) {
        let pal = theme::current(ui.ctx());
        theme::section(ui, "COLOR DE LA FOTO");
        ui.label(RichText::new("Cambia cómo se interpretan la luz y los colores. Mira el efecto en «Original» o en el dibujo.").small().color(pal.secondary));
        // Histograma de luz: de sombras (izquierda) a luces (derecha).
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 54.0), Sense::hover());
        let painter = ui.painter_at(r);
        painter.rect_filled(r, egui::Rounding::same(theme::CONTROL_RADIUS), pal.canvas);
        let bw = r.width() / self.histogram.len() as f32;
        for (i, h) in self.histogram.iter().enumerate() {
            let x = r.left() + i as f32 * bw;
            let top = r.bottom() - 4.0 - h * (r.height() - 8.0);
            let shade = (i * 4) as u8;
            painter.rect_filled(Rect::from_min_max(Pos2::new(x, top), Pos2::new(x + bw + 0.5, r.bottom() - 4.0)), 0.0, pal.secondary.gamma_multiply(0.35 + 0.65 * (1.0 - shade as f32 / 255.0)));
        }
        let a = &mut self.params.adjust;
        ui.checkbox(&mut a.auto_levels, "Niveles automáticos").on_hover_text("Estira la luz para que lo más oscuro sea negro y lo más claro blanco");
        theme::section(ui, "LUZ");
        adjust_slider(ui, "Brillo", &mut a.brightness, -1.0..=1.0, 0.0, |v| format!("{:+.0}", v * 100.0));
        adjust_slider(ui, "Contraste", &mut a.contrast, -1.0..=1.0, 0.0, |v| format!("{:+.0}", v * 100.0));
        adjust_slider(ui, "Tonos medios", &mut a.midtones, -1.0..=1.0, 0.0, |v| format!("{:+.0}", v * 100.0));
        theme::section(ui, "COLOR");
        adjust_slider(ui, "Saturación", &mut a.saturation, 0.0..=2.0, 1.0, |v| format!("{:.0} %", v * 100.0));
        adjust_slider(ui, "Tono", &mut a.hue_deg, -180.0..=180.0, 0.0, |v| format!("{v:+.0}°"));
        adjust_slider(ui, "Temperatura", &mut a.warmth, -1.0..=1.0, 0.0, |v| match v {
            v if v > 0.005 => format!("cálida {:.0}", v * 100.0),
            v if v < -0.005 => format!("fría {:.0}", -v * 100.0),
            _ => "neutra".into(),
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for (name, look) in [
                ("B/N", ColorAdjust { saturation: 0.0, contrast: 0.2, ..Default::default() }),
                ("Vivo", ColorAdjust { saturation: 1.5, contrast: 0.25, auto_levels: true, ..Default::default() }),
                ("Suave", ColorAdjust { contrast: -0.3, midtones: 0.3, saturation: 0.8, ..Default::default() }),
            ] {
                if ui.button(name).clicked() {
                    *a = look;
                }
            }
        });
        if ui.add_enabled(!a.is_identity(), egui::Button::new("Restablecer")).clicked() {
            *a = ColorAdjust::default();
        }
        ui.label(RichText::new("Doble clic en un control lo devuelve a su valor inicial.").small().color(pal.secondary));
    }

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
                            // Los efectos de trazo se eligen luego en cada objeto, con todos sus ajustes.
                            for s in FillStyle::ALL.into_iter().filter(|s| *s != FillStyle::Trace) {
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
                theme::section(ui, "TRAMA INICIAL");
                crate::inspector::style_choice(ui, &mut look.screen.screen);
                field(ui, "Separación", |ui| ui.add(egui::Slider::new(&mut look.screen.spacing_mm, 0.4..=4.0).logarithmic(true).suffix(" mm")));
                field(ui, "Grosor punta", |ui| ui.add(egui::Slider::new(&mut look.screen.pen_width_mm, 0.2..=1.5).suffix(" mm")).on_hover_text("Grosor de los rotuladores nuevos"));
                ui.label(RichText::new("Después podrás retocar la trama de cada tinta en el panel derecho, sin volver a importar.").small().color(pal.secondary));
            }
            _ => {}
        }

        theme::section(ui, "ROTULADORES");
        if let Some(res) = self.result.clone() {
            let choices = self.assign(doc, &res);
            let mut colors: Vec<[u8; 3]> = choices.iter().filter_map(|c| if let PenChoice::New(c, ..) = c { Some(*c) } else { None }).collect();
            colors.sort_unstable();
            colors.dedup();
            let new = colors.len();
            let help = match (self.params.mode, new) {
                (VecMode::Colors | VecMode::Mix, 0) => "Usa rotuladores que ya tienes.".to_string(),
                (VecMode::Colors | VecMode::Mix, 1) => "Se añadirá 1 tinta nueva al panel de tintas.".to_string(),
                (VecMode::Colors | VecMode::Mix, n) => format!("Se añadirán {n} tintas nuevas al panel de tintas."),
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
        PlotKey { result: self.result_revision, look: self.look.clone(), scale: self.scale(doc, res), widths: self.widths(doc, &choices) }
    }

    fn build(&self, doc: &Document) -> Option<(Vec<Item>, Vec<Pen>)> {
        let res = self.result.as_ref()?;
        let s = self.scale(doc, res);
        let center = p(doc.paper.width_mm / 2.0, doc.paper.height_mm / 2.0);
        let half = p(res.width * s / 2.0, res.height * s / 2.0);
        let key = self.key(doc, res);
        // La trama añadida usa exactamente los mismos trazos y grosores que la vista
        // previa. Si ya está calculada, se aprovecha sin volver a generar los puntos.
        let cached = self.plot.as_ref().filter(|(cached_key, _)| *cached_key == key);
        let generated = if cached.is_none() && res.layers.iter().any(|layer| layer.coverage.is_some()) { Some(plot_lines(res, &key)) } else { None };
        let plot = cached.map(|(_, plot)| plot.as_ref()).or(generated.as_ref());
        let mut pens = Vec::new();
        let mut items = Vec::new();
        for (i, (layer, choice)) in res.layers.iter().zip(self.assign(doc, res)).enumerate() {
            let pen = match choice {
                PenChoice::Existing(i) => i,
                // Dos capas del mismo color comparten la tinta nueva.
                PenChoice::New(c, name, w) => match pens.iter().position(|p: &Pen| p.color == c) {
                    Some(j) => doc.pens.len() + j,
                    None => {
                        pens.push(Pen::new(&name, c, w));
                        doc.pens.len() + pens.len() - 1
                    }
                },
            };
            let label = layer.name.map(|n| format!("{} · {}", self.name, n)).unwrap_or_else(|| format!("{} · {}", self.name, i + 1));
            let to_local = |l: &Polyline| -> Polyline { l.iter().map(|q| p(q.x * s - half.x, q.y * s - half.y)).collect() };
            let mut item = match &layer.coverage {
                // Mezcla: cada tinta guarda su mapa y su trama, que se puede retocar luego.
                Some(cov) => {
                    let lines = plot?.layers[i].iter().map(|l| l.iter().map(|q| *q - half).collect()).collect();
                    let trace = mix::trace_params(&key.look.screen, i, res.layers.len());
                    let mut ink = Traced::new(Coverage { map: (**cov).clone(), mm_per_px: s }, trace.clone(), i);
                    ink.set_lines(TraceKey { trace, pen_width_mm: key.widths.get(i).copied().unwrap_or(0.8), relay: None }, lines);
                    Item::new(label, Geometry::Traced(ink), center)
                }
                None => Item::new(label, Geometry::Path { lines: layer.lines.iter().map(to_local).collect(), closed: layer.closed }, center),
            };
            item.fill.pen = pen;
            if item.is_screened() {
                item.stroke = None;
            } else if layer.closed && self.look.fill != FillStyle::None {
                item.fill = Fill { pen, ..self.look.fill_for(i) };
                item.stroke = if self.look.outline { Some(pen) } else { None };
            } else {
                item.stroke = Some(pen);
            }
            // Grupo provisional: el documento le da su número al añadir la foto.
            item.group = Some(0);
            items.push(item);
        }
        Some((items, pens))
    }

    /// Foto viva del grupo: la imagen (reducida) y los ajustes de esta separación.
    fn photo(&self) -> Option<Photo> {
        let image = EncodedImage::from_image(self.image.as_ref()?).ok()?;
        Some(Photo { id: 0, name: self.name.clone(), image: vectorio_core::Shared::new(image), params: self.params.clone(), size_pct: self.size_pct })
    }
}

/// Diferencia máxima (RGB) para considerar que una tinta de la paleta ya es ese color.
const SAME_INK: f64 = 6.0;

fn color_distance(a: [u8; 3], b: [u8; 3]) -> f64 {
    a.iter().zip(b).map(|(&x, y)| (x as f64 - y as f64).powi(2)).sum::<f64>().sqrt()
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

/// Control de un ajuste de color: nombre y valor encima, deslizador a todo el ancho.
/// Doble clic lo devuelve a `reset`.
fn adjust_slider(ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>, reset: f32, show: impl Fn(f32) -> String) {
    let pal = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(pal.secondary));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(show(*v)).color(if *v == reset { pal.secondary } else { pal.text }));
        });
    });
    ui.spacing_mut().slider_width = ui.available_width();
    let r = ui.add(egui::Slider::new(v, range).show_value(false));
    if r.double_clicked() {
        *v = reset;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use vectorio_core::raster::Grid;

    fn mixed_result(color: [u8; 3]) -> Arc<VecResult> {
        let mut coverage = Grid::new(8, 6);
        coverage.data.fill(1.0);
        Arc::new(VecResult {
            width: 8.0, height: 6.0,
            layers: vec![vectorize::VecLayer { color, closed: false, lines: vec![], coverage: Some(Arc::new(coverage)), name: None }],
        })
    }

    #[test]
    fn preview_zoom_keeps_the_point_under_the_pointer_and_clamps() {
        let viewport = Rect::from_min_size(Pos2::new(20.0, 50.0), Vec2::new(500.0, 350.0));
        let source = Vec2::new(800.0, 600.0);
        let anchor = Pos2::new(190.0, 170.0);
        let mut view = PreviewView::default();
        view.pan(Vec2::new(35.0, -20.0), viewport, source);
        let before = view.image_rect(viewport, source);
        let point = (anchor - before.min) / before.size();
        for factor in [2.0, 1000.0, 0.00001] {
            view.zoom_at(factor, anchor, viewport, source);
            let after = view.image_rect(viewport, source);
            assert!((after.min + point * after.size()).distance(anchor) < 0.001);
            assert!((0.25..=32.0).contains(&view.zoom));
        }
    }

    #[test]
    fn preview_pan_follows_the_gesture_at_any_zoom() {
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 350.0));
        let source = Vec2::new(800.0, 600.0);
        for zoom in [0.25, 1.0, 8.0, 32.0] {
            let mut view = PreviewView { zoom, ..Default::default() };
            let before = view.image_rect(viewport, source);
            let delta = Vec2::new(45.0, -32.0);
            view.pan(delta, viewport, source);
            assert!((view.image_rect(viewport, source).min - before.min - delta).length() < 0.001);
        }
    }

    #[test]
    fn pinch_events_only_navigate_inside_the_preview() {
        let ctx = egui::Context::default();
        let mut view = VectorizeUi::default();
        view.texture = Some(ctx.load_texture("test-preview", egui::ColorImage::new([8, 6], Color32::WHITE), Default::default()));
        view.preview = Preview::Original;
        let doc = Document::default();
        let mut frame = |time: f64, events| {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0))), time: Some(time), events, ..Default::default() };
            let _ = ctx.run(input, |ctx| { egui::CentralPanel::default().show(ctx, |ui| view.preview_area(ui, Vec2::new(400.0, 300.0), &doc)); });
            view.view.zoom
        };
        frame(0.0, vec![]);
        frame(0.1, vec![egui::Event::PointerMoved(Pos2::new(180.0, 140.0))]);
        assert_eq!(frame(0.2, vec![egui::Event::Zoom(2.0)]), 2.0);
        frame(0.3, vec![egui::Event::PointerMoved(Pos2::new(600.0, 140.0))]);
        assert_eq!(frame(0.4, vec![egui::Event::Zoom(2.0)]), 2.0);
    }

    #[test]
    fn mix_import_is_fill_and_matches_preview_with_existing_objects() {
        let mut doc = Document::default();
        doc.pens[1].width_mm = 1.4;
        doc.add(Item::new("Dibujo anterior", Geometry::Shape(vectorio_core::Shape::Heart { size: 25.0 }), p(30.0, 40.0)));
        let mut view = VectorizeUi::default();
        view.params.mode = VecMode::Mix;
        view.look.screen.screen = Screen::Dots;
        view.look.screen.spacing_mm = 3.0;
        view.size_pct = 10.0;
        view.result = Some(mixed_result(doc.pens[1].color));
        let res = view.result.as_ref().unwrap();
        let key = view.key(&doc, res);
        let expected = plot_lines(res, &key);
        assert!(!expected.layers[0].is_empty());
        let (items, pens) = view.build(&doc).unwrap();
        assert!(pens.is_empty());
        let item = &items[0];
        assert!(item.is_screened() && item.has_fill());
        assert_eq!(item.stroke, None);
        assert_eq!(item.fill.pen, 1);
        // Tinta viva: sus trazos ya están al día y recalcularlos da lo mismo.
        let ink = item.traced().unwrap();
        assert_eq!(item.group, Some(0));
        assert!(ink.is_current(doc.pens[1].width_mm));
        assert_eq!(vectorio_core::Traced::generate(&ink.coverage, ink.made.as_ref().unwrap(), ink.seed), *ink.lines);
        let paths = item.plot(&doc.pens);
        assert_eq!(paths[0].1.len(), expected.layers[0].len());
        let scale = view.scale(&doc, res);
        let offset = item.place.pos - p(res.width * scale / 2.0, res.height * scale / 2.0);
        for (actual, expected) in paths[0].1.iter().zip(&expected.layers[0]) {
            for (a, b) in actual.iter().zip(expected) { assert!(a.dist(*b + offset) < 1e-9); }
        }
        // Navegar por la vista previa no cambia el tamaño ni el dibujo importado.
        view.view.zoom = 12.0;
        view.view.center = Vec2::new(-0.3, 0.8);
        assert_eq!(view.build(&doc).unwrap().0, items);
    }

    #[test]
    fn color_import_adds_each_new_color_as_an_ink_once() {
        let doc = Document::default();
        let square = vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 0.0)];
        let layer = |color| vectorize::VecLayer { color, closed: true, lines: vec![square.clone()], coverage: None, name: None };
        let near_red = [doc.pens[1].color[0] - 20, doc.pens[1].color[1], doc.pens[1].color[2] + 10];
        let mut view = VectorizeUi::default();
        view.params.mode = VecMode::Colors;
        view.result = Some(Arc::new(VecResult {
            width: 8.0, height: 6.0,
            layers: vec![layer(doc.pens[0].color), layer(near_red), layer([150, 96, 60]), layer([150, 96, 60])],
        }));
        let (items, pens) = view.build(&doc).unwrap();
        // El negro ya estaba; el rojo parecido y el marrón son tintas nuevas, y el
        // marrón repetido no se duplica.
        assert_eq!(pens.iter().map(|p| (p.name.as_str(), p.color)).collect::<Vec<_>>(), vec![("Rojo", near_red), ("Marrón", [150, 96, 60])]);
        let n = doc.pens.len();
        assert_eq!(items.iter().map(|it| it.fill.pen).collect::<Vec<_>>(), vec![0, n, n + 1, n + 1]);
    }

    #[test]
    fn loading_another_photo_discards_previous_results_and_jobs() {
        let ctx = egui::Context::default();
        let mut view = VectorizeUi::default();
        let doc = Document::default();
        let result = mixed_result([20, 20, 20]);
        let key = view.key(&doc, &result);
        let (tx, rx) = mpsc::channel();
        tx.send((*result).clone()).unwrap();
        view.job = Some((view.params.clone(), rx));
        view.result = Some(result);
        view.plot = Some((key.clone(), Arc::new(PlotPreview { layers: vec![] })));
        let (_, plot_rx) = mpsc::channel();
        view.plot_job = Some((key.clone(), plot_rx));
        view.pending_key = Some(key);
        view.view.zoom = 4.0;
        let path = std::env::temp_dir().join(format!("vectorio-photo-reload-{}.png", std::process::id()));
        image::RgbImage::from_pixel(8, 6, image::Rgb([255, 0, 0])).save(&path).unwrap();
        view.load(&ctx, &path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(view.job.is_none() && view.result.is_none() && view.plot.is_none() && view.plot_job.is_none() && view.pending_key.is_none());
        assert_eq!(view.view.zoom, 1.0);
        assert_eq!(view.result_revision, 1);
    }

    #[test]
    fn the_colour_panel_sits_left_and_repaints_the_original_view() {
        let ctx = egui::Context::default();
        let doc = Document::default();
        let mut view = VectorizeUi::default();
        let img = image::RgbImage::from_fn(40, 30, |x, y| image::Rgb([(x * 6) as u8, (y * 8) as u8, 120]));
        view.set_image(&ctx, DynamicImage::ImageRgb8(img), "prueba".into());
        let input = || egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))), ..Default::default() };
        let mut texts = Vec::new();
        for _ in 0..2 {
            let out = ctx.run(input(), |ctx| { let _ = view.show(ctx, &doc); });
            texts.clear();
            fn collect(shape: &egui::Shape, texts: &mut Vec<(String, f32)>) {
                match shape {
                    egui::Shape::Text(t) => texts.push((t.galley.text().to_string(), t.pos.x)),
                    egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, texts)),
                    _ => {}
                }
            }
            out.shapes.iter().for_each(|c| collect(&c.shape, &mut texts));
        }
        let x_of = |w: &str| texts.iter().find(|(t, _)| t == w).map(|(_, x)| *x).unwrap_or_else(|| panic!("falta «{w}»: {texts:?}"));
        for w in ["COLOR DE LA FOTO", "Brillo", "Contraste", "Tonos medios", "Saturación", "Tono", "Temperatura", "Restablecer"] {
            assert!(x_of(w) < x_of("ESTILO"), "«{w}» debería ir en el panel izquierdo");
        }
        assert!(x_of("COLOR DE LA FOTO") < 400.0);
        // Mover un ajuste rehace la vista «Original» y lo pasa a la separación.
        view.params.adjust.saturation = 0.0;
        let _ = ctx.run(input(), |ctx| { let _ = view.show(ctx, &doc); });
        assert_eq!(view.shown_adjust.saturation, 0.0);
    }
}
