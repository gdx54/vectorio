//! Editor de tinta integrado: controles en español y vista previa en el lienzo.

use crate::theme;
use egui::{ecolor::Hsva, Color32, RichText, Sense, Stroke, Vec2};
use vectorio_core::Pen;

pub struct ColorEditor {
    pub title: String,
    pub subtitle: String,
    pub cancelled: bool,
    original: [u8; 3],
    hsv: Hsva,
    hex: String,
}

impl Default for ColorEditor {
    fn default() -> Self { Self::new([0, 0, 0]) }
}

fn hex(color: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}

fn parse_hex(value: &str) -> Option<[u8; 3]> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 || !value.is_ascii() { return None; }
    Some([u8::from_str_radix(&value[0..2], 16).ok()?, u8::from_str_radix(&value[2..4], 16).ok()?, u8::from_str_radix(&value[4..6], 16).ok()?])
}

impl ColorEditor {
    pub fn new(color: [u8; 3]) -> Self {
        Self { title: ts!("Color del rotulador").into(), subtitle: String::new(), cancelled: false, original: color, hsv: Hsva::from(theme::pen_color(color)), hex: hex(color) }
    }

    fn set_color(&mut self, color: [u8; 3]) {
        let hue = self.hsv.h;
        self.hsv = Hsva::from(theme::pen_color(color));
        if self.hsv.s == 0.0 { self.hsv.h = hue; }
        self.hex = hex(color);
    }

    /// Devuelve false al terminar. Cancelar y Esc restauran el color inicial.
    pub fn show(&mut self, ctx: &egui::Context, pen: &mut Pen) -> bool {
        let pal = theme::current(ctx);
        let mut open = true;
        let cancel = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let mut cancelled = cancel;
        let screen = ctx.screen_rect();
        let veil = egui::Area::new(egui::Id::new("ink-editor-veil")).fixed_pos(screen.min).order(egui::Order::Middle).show(ctx, |ui| {
            ui.allocate_rect(screen, Sense::click_and_drag());
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(24));
        });
        ctx.move_to_top(veil.response.layer_id);
        let height = (screen.height() - 40.0).clamp(240.0, 510.0);
        let width = 300.0;
        let previous_color = pen.color;
        egui::Area::new(egui::Id::new("ink-editor"))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                theme::glass_sheet(ui, &pal, |ui| {
                    ui.set_width(width);
                    let content_height = height - 36.0;
                    ui.set_height(content_height);
                    let top = ui.cursor().min.y;
                    ui.horizontal(|ui| {
                        ui.set_min_height(28.0);
                        ui.label(RichText::new(&self.title).size(16.0).strong().color(pal.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if theme::close_button(ui).clicked() { cancelled = true; }
                        });
                    });
                    let subtitle = if self.title == "Color del rotulador" { tr!("%{name} · tinta compartida", name = crate::i18n::pen_name(pen)) } else { self.subtitle.clone() };
                    ui.add(egui::Label::new(RichText::new(subtitle).color(pal.secondary)).truncate());
                    ui.add_space(6.0);
                    let header_height = ui.cursor().min.y - top;
                    let body_height = (content_height - header_height - 46.0).max(20.0);
                    // Solo el contenido puede desplazarse; cierre y acciones permanecen fijos.
                    egui::ScrollArea::vertical().id_salt("ink-editor-body")
                        .max_height(body_height).auto_shrink([false, false]).show(ui, |ui| {
                            ui.set_max_width(ui.available_width());
                            ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                            ui.spacing_mut().slider_width = 130.0;
                            ui.horizontal(|ui| {
                                ui.label(ts!("Antes"));
                                if theme::swatch(ui, self.original, 26.0).on_hover_text(ts!("Recuperar el color inicial")).clicked() {
                                    self.set_color(self.original);
                                    pen.color = self.original;
                                }
                                ui.add_space(12.0);
                                ui.label(ts!("Ahora"));
                                theme::swatch(ui, pen.color, 26.0);
                            });
                            ui.add_space(4.0);
                            let mut changed = color_field(ui, &mut self.hsv);
                            ui.label(ts!("Tono"));
                            changed |= hue_bar(ui, &mut self.hsv.h);
                            if changed {
                                let color = Color32::from(self.hsv);
                                pen.color = [color.r(), color.g(), color.b()];
                                self.hex = hex(pen.color);
                            }
                            ui.add_space(4.0);
                            egui::Grid::new("ink-values").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                                ui.label(ts!("Hexadecimal"));
                                let response = ui.add(egui::TextEdit::singleline(&mut self.hex).desired_width(150.0).char_limit(7));
                                if response.changed() {
                                    if let Some(color) = parse_hex(&self.hex) {
                                        let typed = self.hex.clone();
                                        self.set_color(color);
                                        self.hex = typed;
                                        pen.color = color;
                                    }
                                }
                                ui.end_row();
                                for (index, label) in [ts!("Rojo"), ts!("Verde"), ts!("Azul")].iter().enumerate() {
                                    ui.label(*label);
                                    if ui.add(egui::Slider::new(&mut pen.color[index], 0..=255)).changed() { self.set_color(pen.color); }
                                    ui.end_row();
                                }
                            });
                            if parse_hex(&self.hex).is_none() {
                                ui.label(RichText::new(ts!("Escribe 6 cifras, por ejemplo #327AC8.")).color(ui.visuals().error_fg_color).small());
                            }
                        });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if theme::accent_button_sized(ui, ts!("Hecho"), Some(Vec2::new(132.0, 34.0))).clicked() { open = false; }
                            if ui.add_sized([132.0, 34.0], egui::Button::new(ts!("Cancelar"))).clicked() { cancelled = true; }
                        });
                    });
                });
            });
        let changed_color = pen.color;
        pen.color = previous_color;
        pen.set_color(changed_color);
        if cancelled {
            self.cancelled = true;
            pen.color = self.original;
            open = false;
        }
        open
    }
}

fn color_field(ui: &mut egui::Ui, hsv: &mut Hsva) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 112.0), Sense::click_and_drag());
    let mut mesh = egui::Mesh::default();
    const STEPS: u32 = 24;
    for y in 0..=STEPS {
        for x in 0..=STEPS {
            let s = x as f32 / STEPS as f32;
            let v = 1.0 - y as f32 / STEPS as f32;
            mesh.colored_vertex(egui::pos2(rect.left() + s * rect.width(), rect.top() + (1.0 - v) * rect.height()), Color32::from(Hsva { s, v, ..*hsv }));
        }
    }
    for y in 0..STEPS {
        for x in 0..STEPS {
            let i = y * (STEPS + 1) + x;
            mesh.add_triangle(i, i + 1, i + STEPS + 1);
            mesh.add_triangle(i + 1, i + STEPS + 2, i + STEPS + 1);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    let mut changed = false;
    if let Some(p) = response.interact_pointer_pos() {
        hsv.s = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        hsv.v = (1.0 - (p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        changed = true;
    }
    let pos = egui::pos2(rect.left() + hsv.s * rect.width(), rect.top() + (1.0 - hsv.v) * rect.height());
    ui.painter().circle_stroke(pos, 6.0, Stroke::new(3.0f32, Color32::BLACK));
    ui.painter().circle_stroke(pos, 6.0, Stroke::new(1.5f32, Color32::WHITE));
    response.on_hover_cursor(egui::CursorIcon::Crosshair).on_hover_text(ts!("Arrastra para ajustar la saturación y la luminosidad"));
    changed
}

fn hue_bar(ui: &mut egui::Ui, hue: &mut f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click_and_drag());
    let mut mesh = egui::Mesh::default();
    for i in 0..=36 {
        let h = i as f32 / 36.0;
        let color = Color32::from(Hsva { h, s: 1.0, v: 1.0, a: 1.0 });
        mesh.colored_vertex(egui::pos2(rect.left() + h * rect.width(), rect.top()), color);
        mesh.colored_vertex(egui::pos2(rect.left() + h * rect.width(), rect.bottom()), color);
        if i > 0 {
            let n = i * 2;
            mesh.add_triangle(n - 2, n - 1, n);
            mesh.add_triangle(n - 1, n, n + 1);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    let mut changed = false;
    if let Some(p) = response.interact_pointer_pos() {
        *hue = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        changed = true;
    }
    let x = rect.left() + *hue * rect.width();
    ui.painter().vline(x, rect.y_range(), Stroke::new(4.0f32, Color32::BLACK));
    ui.painter().vline(x, rect.y_range(), Stroke::new(2.0f32, Color32::WHITE));
    response.on_hover_cursor(egui::CursorIcon::Crosshair).on_hover_text(ts!("Arrastra para elegir el tono"));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_accepts_rgb_and_rejects_invalid_or_unicode_input() {
        assert_eq!(parse_hex(" #3a7FC2 "), Some([58, 127, 194]));
        assert_eq!(parse_hex("000000"), Some([0, 0, 0]));
        for value in ["#123", "#GGGGGG", "💙ab", "é1234", "1234567"] {
            assert_eq!(parse_hex(value), None);
        }
    }

    #[test]
    fn color_actions_remain_visible_in_short_and_tall_windows() {
        fn find_text(shape: &egui::Shape, label: &str) -> Option<egui::Rect> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => Some(egui::Rect::from_min_size(text.pos, text.galley.size())),
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find_text(shape, label)),
                _ => None,
            }
        }
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut editor = ColorEditor::new([28, 28, 32]);
        let mut pen = Pen::new("Negro fino", [28, 28, 32], 0.4);
        for height in [620.0, 900.0, 480.0] {
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(980.0, height));
            let mut output = None;
            for _ in 0..3 {
                output = Some(ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| { editor.show(ctx, &mut pen); }));
            }
            let output = output.unwrap();
            for label in ["Cancelar", "Hecho"] {
                let (clip, rect) = output.shapes.iter().find_map(|shape| find_text(&shape.shape, label).map(|r| (shape.clip_rect, r))).expect("Acción visible");
                assert!(screen.contains_rect(rect) && clip.contains_rect(rect), "{label} queda fuera de la ventana de {height} puntos: {rect:?}");
            }
        }
    }
}
