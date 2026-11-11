//! Paneles flotantes: rotuladores y objetos a la izquierda, inspector a la derecha.

use crate::app::{VectorioApp, PAPER_TINTS};
use crate::theme;
use egui::{RichText, Ui};
use vectorio_core::geom::{chaikin, simplify};
use vectorio_core::{Align, Document, Fill, FillStyle, FontStyle, Geometry, Paper, Pen, Shape, TraceParams, TraceStyle, Vibe};
use crate::styles;
use vectorio_core::font;

pub const LEFT_W: f32 = 252.0;
pub const RIGHT_W: f32 = 264.0;

fn pen_label(pens: &[Pen], i: usize) -> RichText {
    match pens.get(i) {
        Some(p) => RichText::new(format!("{} · #{:02X}{:02X}{:02X}", p.name, p.color[0], p.color[1], p.color[2])),
        None => RichText::new("—"),
    }
}

/// El inspector muestra primero la tinta real; la asignación compartida es secundaria.
fn ink_property(ui: &mut Ui, id: &str, pens: &[Pen], pen: &mut usize, fill: bool, edit: &mut Option<bool>) {
    ui.vertical(|ui| {
    if let Some(ink) = pens.get(*pen) {
        ui.horizontal(|ui| {
            let swatch = theme::swatch(ui, ink.color, 24.0);
            let button = ui.button(format!("#{:02X}{:02X}{:02X} · {:.2} mm", ink.color[0], ink.color[1], ink.color[2], ink.width_mm));
            if swatch.clicked() || button.on_hover_text("Editar el color de este objeto").clicked() { *edit = Some(fill); }
        });
    }
    egui::CollapsingHeader::new("Elegir de la paleta…").id_salt(id).show(ui, |ui| {
        pen_combo(ui, id, pens, pen);
    });
    });
}

/// Desplegable de rotuladores con su color.
fn pen_combo(ui: &mut Ui, id: &str, pens: &[Pen], value: &mut usize) -> egui::Response {
    egui::ComboBox::from_id_salt(id)
        .width(150.0)
        .selected_text(pen_label(pens, *value))
        .show_ui(ui, |ui| {
            for i in 0..pens.len() {
                ui.selectable_value(value, i, pen_label(pens, i));
            }
        })
        .response
}

fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let pal = theme::current(ui.ctx());
        ui.add_sized([72.0, 20.0], egui::Label::new(RichText::new(label).color(pal.secondary)).truncate());
        add(ui);
    });
}

fn drag(ui: &mut Ui, v: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, suffix: &str) -> egui::Response {
    ui.add(egui::DragValue::new(v).range(range).speed(speed).suffix(suffix).max_decimals(2))
}

fn drag_u(ui: &mut Ui, v: &mut u32, range: std::ops::RangeInclusive<u32>) -> egui::Response {
    ui.add(egui::DragValue::new(v).range(range).speed(0.1))
}

/// Interruptor de visibilidad dibujado (punto lleno = visible, aro = oculto).
fn visibility_toggle(ui: &mut Ui, visible: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
    let pal = theme::current(ui.ctx());
    let col = if resp.hovered() { pal.accent } else { pal.secondary };
    if visible {
        ui.painter().circle_filled(rect.center(), 4.0, col);
    }
    ui.painter().circle_stroke(rect.center(), 6.0, egui::Stroke::new(1.2f32, col));
    resp
}

// ── Izquierda ───────────────────────────────────────────────────────────────────

/// Enter confirma; salir confirma; Esc cancela desde los atajos de la app.
fn rename_field(ui: &mut Ui, name: &mut String, focus: &mut bool, width: f32) -> bool {
    let response = ui.add(egui::TextEdit::singleline(name).desired_width(width));
    let first = std::mem::take(focus);
    if first {
        response.request_focus();
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
            state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(name.chars().count()))));
            state.store(ui.ctx(), response.id);
        }
    }
    !first && (response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)))
}

fn layer_name(ui: &mut Ui, width: f32, text: RichText) -> egui::Response {
    ui.allocate_ui_with_layout(egui::vec2(width, 28.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_width(width);
        ui.set_max_width(width);
        ui.add(egui::Label::new(text).truncate().selectable(false).sense(egui::Sense::click_and_drag()))
    }).inner
}

/// Una muestra hueca representa el contorno; una llena, el relleno.
fn ink_swatch(ui: &mut Ui, color: Option<[u8; 3]>, fill: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 26.0), egui::Sense::click());
    let pal = theme::current(ui.ctx());
    let center = rect.center();
    if let Some(color) = color {
        let color = theme::pen_color(color);
        if fill { ui.painter().circle_filled(center, 7.0, color); }
        else { ui.painter().circle_stroke(center, 7.0, egui::Stroke::new(3.0f32, color)); }
    } else {
        ui.painter().circle_stroke(center, 7.0, egui::Stroke::new(1.0f32, pal.secondary));
        ui.painter().line_segment([center + egui::vec2(-5.0, 5.0), center + egui::vec2(5.0, -5.0)], egui::Stroke::new(1.0f32, pal.secondary));
    }
    response.on_hover_text(match (fill, color.is_some()) {
        (true, true) => "Color del relleno de este objeto",
        (true, false) => "Sin relleno · clic para añadir color",
        (false, true) => "Color del contorno de este objeto",
        (false, false) => "Sin contorno · clic para añadir color",
    })
}

pub fn left_panel(app: &mut VectorioApp, ui: &mut Ui) {
    let pal = theme::current(ui.ctx());
    theme::section(ui, "CAPAS");
    ui.label(RichText::new("Un objeto por capa · arriba queda delante").small().color(pal.secondary));
    ui.add_space(5.0);
    if app.doc.items.is_empty() {
        ui.label(RichText::new("Dibuja, escribe o importa una imagen para empezar.").color(pal.secondary));
    }
    let ids: Vec<u64> = app.doc.items.iter().rev().map(|it| it.id).collect();
    let mut drop = None;
    // Las tintas de una foto viva cuelgan de una fila de grupo que se puede plegar.
    let mut groups: Vec<(u64, bool)> = Vec::new();
    for id in ids {
        let group = app.doc.get(id).and_then(|it| it.group).filter(|g| app.doc.photo(*g).is_some());
        let Some(g) = group else {
            object_row(app, ui, id, &mut drop);
            continue;
        };
        let open = match groups.iter().find(|(k, _)| *k == g) {
            Some((_, open)) => *open,
            None => {
                let open = group_row(app, ui, g);
                groups.push((g, open));
                open
            }
        };
        if open {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.vertical(|ui| object_row(app, ui, id, &mut drop));
            });
        }
    }
    if let Some((ids, anchor)) = drop {
        app.doc.move_items(&ids, Some(anchor));
    }
    // Las acciones quedan después de la lista: seleccionar nunca desplaza sus filas.
    let selection = app.selection();
    if !selection.is_empty() && !app.pen_focus {
        ui.add_space(6.0);
        ui.label(RichText::new(format!("{} seleccionado{}", selection.len(), if selection.len() == 1 { "" } else { "s" })).small().color(pal.secondary));
        ui.horizontal(|ui| {
            if ui.button("Traer adelante").on_hover_text("Toda la selección · ⌘]").clicked() { app.doc.reorder_items(&selection, true, false); }
            if ui.button("Enviar atrás").on_hover_text("Toda la selección · ⌘[").clicked() { app.doc.reorder_items(&selection, false, false); }
        });
    }
    ui.add_space(6.0);
    ui.label(RichText::new("Doble clic para renombrar · arrastra para ordenar").small().color(pal.secondary));
    ui.add_space(6.0);
    ui.separator();
    theme::section(ui, "TINTAS");
    ui.label(RichText::new("Arrastra para ordenar o suéltala en una capa").small().color(pal.secondary));
    ui.add_space(4.0);
    pen_cards(app, ui);
    ui.add_space(6.0);
    theme::section(ui, "PAPEL (VISTA PREVIA)");
    ui.horizontal(|ui| {
        for (i, (name, color)) in PAPER_TINTS.iter().enumerate() {
            let response = theme::swatch(ui, *color, 22.0).on_hover_text(*name);
            if app.paper_tint == i { ui.painter().circle_stroke(response.rect.center(), 13.5, egui::Stroke::new(2.0f32, pal.accent)); }
            if response.clicked() { app.paper_tint = i; }
        }
    });
}

// ── Tarjetas de tinta ───────────────────────────────────────────────────────────

/// Tinta que se está arrastrando desde la paleta y punto por el que se agarró.
#[derive(Clone, Copy)]
pub struct PenDrag {
    pub pen: usize,
    pub grab: egui::Vec2,
}

const CARD_H: f32 = 74.0;
const CARD_GAP: f32 = 8.0;
const CARD_RADIUS: f32 = 12.0;
const CARD_BAND: f32 = 40.0;

/// Texto que acompaña a la tarjeta arrastrada (qué pasará al soltar). Lo escriben las
/// capas y el lienzo durante el fotograma; la paleta lo muestra y lo borra.
pub fn set_drop_hint(ctx: &egui::Context, hint: String) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("pen-drop-hint"), hint));
}

/// Qué hará la tinta al soltarla sobre un objeto, con la pista de ⌥ si admite relleno.
pub fn drop_hint(app: &VectorioApp, id: u64, fill: bool) -> String {
    let Some(it) = app.doc.get(id) else { return String::new() };
    let alt = if !fill && it.is_fillable() { " · ⌥ relleno" } else { "" };
    format!("{} de «{}»{alt}", if fill { "Relleno" } else { "Contorno" }, it.name)
}

/// Destino de una tinta soltada sobre un objeto: con ⌥ va al relleno (si lo admite);
/// si el objeto no tiene contorno pero sí relleno, también. `None` si está bloqueado.
pub fn pen_drop_fill(app: &VectorioApp, id: u64, alt: bool) -> Option<bool> {
    let it = app.doc.get(id).filter(|it| !it.locked)?;
    Some(it.is_screened() || (it.is_fillable() && (alt || (it.stroke.is_none() && it.has_fill()))))
}

fn truncated(painter: &egui::Painter, text: &str, size: f32, color: egui::Color32, width: f32, strong: bool) -> std::sync::Arc<egui::Galley> {
    let font = egui::FontId::proportional(size);
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(10.0));
    if strong { job.sections.iter_mut().for_each(|s| s.format.extra_letter_spacing = 0.1); }
    painter.layout_job(job)
}

#[derive(Clone, Copy, PartialEq)]
enum CardLook { Normal, Hover, Ghost }

/// Tarjeta: banda superior con el color y una muestra del trazo con su grosor real,
/// y debajo el nombre, el hexadecimal y el grosor.
fn paint_pen_card(painter: &egui::Painter, rect: egui::Rect, pen: &Pen, pal: &theme::Palette, active: bool, look: CardLook) {
    use egui::{epaint::Shadow, Color32, Rounding, Stroke, Vec2};
    let color = theme::pen_color(pen.color);
    if look != CardLook::Normal {
        let lift = if look == CardLook::Ghost { 1.0 } else { 0.5 };
        let shadow = Shadow { offset: Vec2::new(0.0, 6.0 + 8.0 * lift), blur: 14.0 + 18.0 * lift, spread: 0.0, color: pal.shadow.gamma_multiply(0.6 + 0.6 * lift) };
        painter.add(shadow.as_shape(rect, Rounding::same(CARD_RADIUS)));
    }
    painter.rect_filled(rect, CARD_RADIUS, if look == CardLook::Ghost { pal.card.to_opaque() } else { pal.card });
    let band = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), CARD_BAND));
    painter.rect_filled(band, Rounding { nw: CARD_RADIUS, ne: CARD_RADIUS, sw: 0.0, se: 0.0 }, color);
    // Reflejo superior de la banda, como una tinta vista a través del vidrio.
    let sheen = egui::Rect::from_min_size(band.min, Vec2::new(band.width(), band.height() * 0.45));
    painter.rect_filled(sheen, Rounding { nw: CARD_RADIUS, ne: CARD_RADIUS, sw: 0.0, se: 0.0 }, Color32::from_white_alpha(26));
    // Muestra del trazo: una onda con el grosor físico del rotulador.
    let luma = 0.299 * pen.color[0] as f32 + 0.587 * pen.color[1] as f32 + 0.114 * pen.color[2] as f32;
    let ink = if luma > 150.0 { Color32::from_black_alpha(120) } else { Color32::from_white_alpha(170) };
    let width = (pen.width_mm as f32 * 5.0).clamp(1.2, 7.0);
    let (x0, x1) = (band.left() + 12.0, band.right() - 12.0);
    let wave: Vec<egui::Pos2> = (0..=24).map(|i| {
        let t = i as f32 / 24.0;
        egui::pos2(x0 + (x1 - x0) * t, band.center().y + 3.0 + (t * std::f32::consts::TAU * 1.25).sin() * 6.0)
    }).collect();
    painter.add(egui::Shape::line(wave, Stroke::new(width, ink)));
    let text_w = rect.width() - 20.0;
    let name = truncated(painter, &pen.name, 12.5, pal.text, text_w, true);
    painter.galley(egui::pos2(rect.left() + 10.0, band.bottom() + 5.0), name, pal.text);
    let meta = format!("#{:02X}{:02X}{:02X} · {} mm", pen.color[0], pen.color[1], pen.color[2], format!("{:.2}", pen.width_mm).replace('.', ","));
    let meta = truncated(painter, &meta, 10.5, pal.secondary, text_w, false);
    painter.galley(egui::pos2(rect.left() + 10.0, band.bottom() + 20.0), meta, pal.secondary);
    painter.rect_stroke(rect, CARD_RADIUS, Stroke::new(1.0f32, pal.glass_stroke));
    if active { painter.rect_stroke(rect.expand(2.0), CARD_RADIUS + 2.0, Stroke::new(2.0f32, pal.accent)); }
}

fn pen_cards(app: &mut VectorioApp, ui: &mut Ui) {
    use egui::{vec2, Rect, Sense, Stroke};
    let pal = theme::current(ui.ctx());
    let ctx = ui.ctx().clone();
    let n = app.doc.pens.len();
    let width = ui.available_width();
    let card_w = ((width - CARD_GAP) / 2.0).floor();
    let rows = (n + 1).div_ceil(2);
    let (grid, _) = ui.allocate_exact_size(vec2(width, rows as f32 * (CARD_H + CARD_GAP) - CARD_GAP), Sense::hover());
    let slot = |k: usize| vec2((k % 2) as f32 * (card_w + CARD_GAP), (k / 2) as f32 * (CARD_H + CARD_GAP));
    let dragging = egui::DragAndDrop::payload::<PenDrag>(&ctx).map(|d| *d).filter(|d| d.pen < n);
    let pointer = ctx.pointer_latest_pos();
    let over_grid = pointer.filter(|p| grid.expand(CARD_GAP).contains(*p) && ui.clip_rect().contains(*p));
    // Mientras se arrastra, las demás tarjetas se apartan para mostrar dónde caerá.
    let target = dragging.map(|d| match over_grid {
        Some(p) => {
            let local = p - grid.min;
            let row = (local.y / (CARD_H + CARD_GAP)).floor().max(0.0) as usize;
            (row * 2 + usize::from(local.x > width / 2.0)).min(n - 1)
        }
        None => d.pen,
    });
    let mut order: Vec<usize> = (0..n).collect();
    if let (Some(d), Some(t)) = (dragging, target) {
        order.remove(d.pen);
        order.insert(t, d.pen);
    }
    let mut remove = None;
    let mut ghost = None;
    for (k, &pen) in order.iter().enumerate() {
        let id = ui.id().with(("pen-card", pen));
        let home = Rect::from_min_size(grid.min + slot(k), vec2(card_w, CARD_H));
        // Posición animada relativa a la rejilla: el desplazamiento del panel no la retrasa.
        let offset = vec2(
            ctx.animate_value_with_time(id.with("x"), slot(k).x, 0.16),
            ctx.animate_value_with_time(id.with("y"), slot(k).y, 0.16),
        );
        let rect = Rect::from_min_size(grid.min + offset, home.size());
        let renaming = app.renaming_pen == Some(pen);
        let response = ui.interact(home, id, if renaming { Sense::hover() } else { Sense::click_and_drag() });
        let active = app.active_pen == pen && app.pen_focus;
        if dragging.is_some_and(|d| d.pen == pen) {
            // Hueco que ocupará la tarjeta al soltarla.
            let color = theme::pen_color(app.doc.pens[pen].color);
            ui.painter().rect_filled(rect, CARD_RADIUS, color.gamma_multiply(0.12));
            ui.painter().rect_stroke(rect, CARD_RADIUS, Stroke::new(1.5f32, color.gamma_multiply(0.7)));
            ghost = Some(pen);
        } else {
            let look = if response.hovered() && dragging.is_none() { CardLook::Hover } else { CardLook::Normal };
            let rect = if look == CardLook::Hover { rect.translate(vec2(0.0, -1.0)) } else { rect };
            paint_pen_card(ui.painter(), rect, &app.doc.pens[pen], &pal, active, look);
        }
        if renaming {
            let field = Rect::from_min_size(egui::pos2(rect.left() + 6.0, rect.top() + CARD_BAND + 4.0), vec2(rect.width() - 12.0, 24.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(field));
            if rename_field(&mut child, &mut app.doc.pens[pen].name, &mut app.rename_focus, field.width()) { app.finish_rename(false); }
        }
        if response.drag_started() {
            let grab = pointer.map(|p| p - home.min).unwrap_or(vec2(card_w / 2.0, CARD_H / 2.0));
            egui::DragAndDrop::set_payload(&ctx, PenDrag { pen, grab });
        }
        if response.clicked() { app.active_pen = pen; app.pen_focus = true; app.set_selection(vec![]); }
        if response.double_clicked() {
            // Doble clic en la banda: color; en el texto: nombre.
            let on_band = response.interact_pointer_pos().is_some_and(|p| p.y < home.top() + CARD_BAND);
            if on_band { app.edit_pen_color(pen); } else { app.rename_pen(pen); }
        }
        if response.hovered() && dragging.is_none() { ctx.set_cursor_icon(egui::CursorIcon::Grab); }
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Tinta {}", app.doc.pens[pen].name)));
        response.on_hover_text("Clic: dibujar con esta tinta · doble clic: color (arriba) o nombre (abajo) · arrastra para ordenar o suéltala sobre una capa").context_menu(|ui| {
            if ui.button("Renombrar").clicked() { app.rename_pen(pen); ui.close_menu(); }
            if ui.button("Cambiar color compartido…").clicked() { app.edit_pen_color(pen); ui.close_menu(); }
            if ui.add_enabled(n > 1, egui::Button::new("Eliminar rotulador")).on_hover_text("Los objetos conservan su posición y pasan a la primera tinta.").clicked() { remove = Some(pen); ui.close_menu(); }
        });
    }

    // Tarjeta para añadir una tinta, siempre al final.
    let add_rect = Rect::from_min_size(grid.min + slot(n), vec2(card_w, CARD_H));
    let add = ui.interact(add_rect, ui.id().with("pen-card-add"), Sense::click());
    let tint = if add.hovered() { pal.accent } else { pal.secondary };
    ui.painter().rect_filled(add_rect, CARD_RADIUS, if add.hovered() { pal.accent_soft.gamma_multiply(0.8) } else { egui::Color32::TRANSPARENT });
    ui.painter().rect_stroke(add_rect.shrink(0.5), CARD_RADIUS, Stroke::new(1.0f32, tint.gamma_multiply(0.45)));
    let c = add_rect.center() - vec2(0.0, 8.0);
    ui.painter().line_segment([c - vec2(7.0, 0.0), c + vec2(7.0, 0.0)], Stroke::new(1.6f32, tint));
    ui.painter().line_segment([c - vec2(0.0, 7.0), c + vec2(0.0, 7.0)], Stroke::new(1.6f32, tint));
    ui.painter().text(add_rect.center() + vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, "Añadir tinta", egui::FontId::proportional(11.5), tint);
    if add.clicked() {
        app.doc.pens.push(Pen::new(&format!("Rotulador {}", app.doc.pens.len() + 1), [60, 60, 70], 0.8));
        app.active_pen = app.doc.pens.len() - 1;
        app.pen_focus = true;
        app.edit_pen_color(app.active_pen);
    }
    add.on_hover_text("Añadir un rotulador a la paleta");

    // La tarjeta arrastrada sigue al puntero por encima de los paneles y del lienzo.
    let hint = ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::new("pen-drop-hint")));
    if let (Some(pen), Some(d), Some(p)) = (ghost, dragging, pointer) {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("pen-card-ghost")));
        let rect = Rect::from_min_size(p - d.grab, vec2(card_w, CARD_H)).expand(3.0);
        paint_pen_card(&painter, rect, &app.doc.pens[pen], &pal, false, CardLook::Ghost);
        let label = hint.or_else(|| over_grid.map(|_| "Soltar para ordenar".to_owned()));
        if let Some(label) = label {
            let galley = painter.layout_no_wrap(label, egui::FontId::proportional(11.5), egui::Color32::WHITE);
            let pill = Rect::from_center_size(egui::pos2(rect.center().x, rect.bottom() + 16.0), galley.size() + vec2(16.0, 8.0));
            painter.rect_filled(pill, pill.height() / 2.0, pal.accent);
            painter.galley(pill.center() - galley.size() / 2.0, galley, egui::Color32::WHITE);
        }
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        // Soltar sobre la paleta reordena; en otro sitio sin destino, se cancela.
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(t) = target.filter(|t| over_grid.is_some() && *t != pen) {
                app.finish_rename(false);
                let new_of = app.doc.move_pen(pen, t);
                app.active_pen = new_of.get(app.active_pen).copied().unwrap_or(0);
                // Los índices cambian: las tarjetas parten ya de su sitio nuevo.
                ctx.clear_animations();
            }
            egui::DragAndDrop::clear_payload(&ctx);
        }
    }
    if let Some(pen) = remove { app.remove_pen(pen); }
}

/// Fila de una foto viva en Capas. Clic: selecciona todas sus tintas; triángulo: pliega.
/// Devuelve si el grupo está desplegado.
fn group_row(app: &mut VectorioApp, ui: &mut Ui, group: u64) -> bool {
    let pal = theme::current(ui.ctx());
    let members = app.doc.group_items(group);
    let closed_id = egui::Id::new(("photo-group-closed", group));
    let mut closed = ui.data_mut(|d| *d.get_persisted_mut_or_default::<bool>(closed_id));
    let name = app.doc.photo(group).map(|ph| ph.name.clone()).unwrap_or_default();
    let all = !members.is_empty() && members.iter().all(|id| app.is_selected(*id));
    let full = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), 30.0));
    let response = ui.interact(full, egui::Id::new(("photo-group-row", group)), egui::Sense::click());
    if all {
        ui.painter().rect_filled(full, theme::CONTROL_RADIUS, pal.accent_soft);
    } else if response.hovered() {
        ui.painter().rect_filled(full, theme::CONTROL_RADIUS, pal.control);
    }
    let mut toggle = false;
    ui.horizontal(|ui| {
        ui.set_min_height(30.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        let (rect, arrow) = ui.allocate_exact_size(egui::vec2(16.0, 30.0), egui::Sense::click());
        let c = rect.center();
        let points = if closed {
            vec![c + egui::vec2(-2.5, -4.5), c + egui::vec2(3.5, 0.0), c + egui::vec2(-2.5, 4.5)]
        } else {
            vec![c + egui::vec2(-4.5, -2.5), c + egui::vec2(4.5, -2.5), c + egui::vec2(0.0, 3.5)]
        };
        ui.painter().add(egui::Shape::convex_polygon(points, if arrow.hovered() { pal.text } else { pal.secondary }, egui::Stroke::NONE));
        toggle = arrow.on_hover_text(if closed { "Mostrar sus tintas" } else { "Ocultar sus tintas" }).clicked();
        // Muestras solapadas de las tintas de la foto.
        let colors: Vec<[u8; 3]> = members.iter().rev().filter_map(|id| app.doc.get(*id)).filter_map(|it| app.doc.pens.get(it.fill.pen)).map(|p| p.color).take(6).collect();
        let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0 + 8.0 * colors.len().saturating_sub(1) as f32, 30.0), egui::Sense::hover());
        for (i, color) in colors.iter().enumerate() {
            let center = egui::pos2(rect.left() + 5.0 + 8.0 * i as f32, rect.center().y);
            ui.painter().circle(center, 5.5, theme::pen_color(*color), egui::Stroke::new(1.5f32, pal.bar.to_opaque()));
        }
        let count = format!("{} tinta{}", members.len(), if members.len() == 1 { "" } else { "s" });
        let width = (ui.available_width() - 60.0).max(30.0);
        ui.allocate_ui_with_layout(egui::vec2(width, 30.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(width);
            ui.add(egui::Label::new(RichText::new(&name).strong().color(pal.text)).truncate().selectable(false));
        });
        ui.label(RichText::new(count).small().color(pal.secondary));
    });
    if toggle {
        closed = !closed;
        ui.data_mut(|d| d.insert_persisted(closed_id, closed));
    } else if response.clicked() {
        app.pen_focus = false;
        if ui.input(|i| i.modifiers.shift || i.modifiers.command) {
            for id in &members { if !app.is_selected(*id) { app.toggle_selected(*id); } }
        } else {
            app.set_selection(members.clone());
        }
    }
    response.on_hover_text("Foto viva: clic para seleccionar todas sus tintas").context_menu(|ui| {
        if ui.button("Seleccionar sus tintas").clicked() { app.set_selection(members.clone()); app.pen_focus = false; ui.close_menu(); }
        if ui.button("Volver a separar colores…").clicked() { reopen_photo(app, ui.ctx(), group); ui.close_menu(); }
    });
    !closed
}

fn reopen_photo(app: &mut VectorioApp, ctx: &egui::Context, group: u64) {
    let Some(photo) = app.doc.photo(group).cloned() else { return };
    if app.vec_ui.reopen(ctx, &photo).is_err() {
        app.notify(ctx, "No se ha podido abrir la foto guardada.");
    }
}

/// Apartado FOTO de una foto viva: su nombre, volver a separar y congelar.
fn photo_section(app: &mut VectorioApp, ui: &mut Ui, group: u64) {
    let pal = theme::current(ui.ctx());
    let members = app.doc.group_items(group);
    let live = members.iter().filter(|id| app.doc.get(**id).is_some_and(|it| it.traced().is_some())).count();
    let Some(photo) = app.doc.photos.iter_mut().find(|ph| ph.id == group) else { return };
    theme::section(ui, "FOTO");
    row(ui, "Nombre", |ui| { ui.add(egui::TextEdit::singleline(&mut photo.name).desired_width(f32::INFINITY)); });
    ui.label(RichText::new(format!("{} · {} tinta{}", photo.params.mode.label(), members.len(), if members.len() == 1 { "" } else { "s" })).small().color(pal.secondary));
    if ui.button("Volver a separar colores…").on_hover_text("Abre la foto con sus ajustes; las tintas nuevas sustituyen a estas en el mismo sitio").clicked() {
        reopen_photo(app, ui.ctx(), group);
    }
    if live > 0 && ui.button("Convertir en trazos fijos").on_hover_text("Las tramas dejan de recalcularse y ya no se pueden retocar").clicked() {
        for id in &members {
            if let Some(it) = app.doc.get_mut(*id).filter(|it| !it.locked) {
                if let Geometry::Traced(t) = &it.geometry {
                    it.geometry = Geometry::Screened { lines: t.lines.to_vec(), screen: t.trace.style };
                }
            }
        }
    }
    if live >= 2 {
        mix_section(app, ui, &members);
    }
}

/// Apartado MEZCLA de una foto viva: cómo se combinan sus tintas y en qué orden pasan.
fn mix_section(app: &mut VectorioApp, ui: &mut Ui, members: &[u64]) {
    let pal = theme::current(ui.ctx());
    theme::section(ui, "MEZCLA");
    let mut mode = None;
    ui.horizontal(|ui| {
        if ui.button("Cruzar").on_hover_text("Cada tinta con su ángulo, repartidos en 180°: las líneas se cruzan como en una cuatricromía.").clicked() { mode = Some(Mix::Cross); }
        if ui.button("Entrelazar").on_hover_text("Mismo estilo y ángulo para todas, cada tinta en su hueco: las líneas se alternan sin pisarse.").clicked() { mode = Some(Mix::Interleave); }
        if ui.button("Relevo").on_hover_text("Todas las tintas comparten las mismas líneas y se pasan el trazo: cada tramo lo pinta el color que hay debajo.").clicked() { mode = Some(Mix::Relay); }
    });
    if let Some(mode) = mode {
        mix_inks(&mut app.doc, members, mode);
    }
    let relay = members.iter().filter(|id| app.doc.get(**id).and_then(|it| it.traced()).is_some_and(|t| t.trace.relay)).count();
    if relay >= 2 {
        ui.label(RichText::new(format!("En relevo: {relay} tintas comparten las líneas. «Largo» marca cada cuánto puede cambiar de rotulador.")).small().color(pal.secondary));
    }
    let mut order = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Pasadas").color(pal.secondary));
        if ui.button("Claras primero").on_hover_text("El plotter pasa antes las tintas claras y las oscuras quedan encima, como en la acuarela.").clicked() { order = Some(true); }
        if ui.button("Oscuras primero").on_hover_text("Las tintas oscuras van primero y las claras las matizan por encima.").clicked() { order = Some(false); }
    });
    if let Some(light_first) = order {
        app.doc.order_passes(members, light_first);
    }
    let mut pens: Vec<usize> = members.iter().filter_map(|id| app.doc.get(*id)).map(Document::item_pen).collect();
    pens.sort_unstable();
    pens.dedup();
    let names: Vec<&str> = pens.iter().filter_map(|p| app.doc.pens.get(*p)).map(|p| p.name.as_str()).collect();
    ui.label(RichText::new(format!("Orden: {}", names.join(" → "))).small().color(pal.secondary));
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mix {
    Cross,
    Interleave,
    Relay,
}

/// Cruza, entrelaza o pone en relevo las tintas vivas desbloqueadas, en el orden de las capas.
fn mix_inks(doc: &mut Document, ids: &[u64], mode: Mix) {
    let live: Vec<u64> = ids.iter().copied().filter(|id| doc.get(*id).is_some_and(|it| !it.locked && it.traced().is_some())).collect();
    let mut params: Vec<TraceParams> = live.iter().filter_map(|id| doc.get(*id)?.traced()).map(|t| t.trace.clone()).collect();
    match mode {
        Mix::Cross => vectorio_core::mix_cross(&mut params),
        Mix::Interleave => vectorio_core::mix_interleave(&mut params),
        Mix::Relay => vectorio_core::mix_relay(&mut params),
    }
    for (id, trace) in live.iter().zip(params) {
        if let Some(t) = doc.get_mut(*id).and_then(|it| it.traced_mut()) { t.trace = trace; }
    }
}

/// ¿Tienen dos ajustes el mismo aspecto, sin contar ángulo ni desfase?
fn same_look(a: &TraceParams, b: &TraceParams) -> bool {
    let plain = |t: &TraceParams| TraceParams { angle_deg: 0.0, phase: 0.0, ..t.clone() };
    plain(a) == plain(b)
}

/// Apartado ESTILOS: aplicar un estilo predefinido o guardado a las tintas, o guardar el
/// trazo actual con un nombre.
fn styles_section(app: &mut VectorioApp, ui: &mut Ui, ids: &[u64]) {
    let live: Vec<u64> = ids.iter().copied().filter(|id| app.doc.get(*id).is_some_and(|it| !it.locked && it.traced().is_some())).collect();
    let Some(current) = live.first().and_then(|id| app.doc.get(*id)).and_then(|it| it.traced()).map(|t| t.trace.clone()) else { return };
    let pal = theme::current(ui.ctx());
    theme::section(ui, "ESTILOS");
    let mut apply: Option<TraceParams> = None;
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for s in styles::presets() {
            if ui.selectable_label(same_look(&current, &s.trace), &s.name).on_hover_text(s.trace.style.help()).clicked() { apply = Some(s.trace); }
        }
        for (i, s) in app.styles.iter().enumerate() {
            let r = ui.selectable_label(same_look(&current, &s.trace), RichText::new(&s.name).italics())
                .on_hover_text("Estilo guardado. Clic derecho para borrarlo.");
            r.context_menu(|ui| { if ui.button("Borrar estilo").clicked() { remove = Some(i); ui.close_menu(); } });
            if r.clicked() { apply = Some(s.trace.clone()); }
        }
    });
    let mut save = false;
    let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(row, egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // De derecha a izquierda: el botón siempre cabe y el campo ocupa lo que queda.
        let ok = !app.style_name.trim().is_empty();
        let clicked = ui.add_enabled(ok, egui::Button::new("Guardar")).on_hover_text("Guarda este trazo para aplicarlo a otras tintas y fotos").clicked();
        let field = ui.add(egui::TextEdit::singleline(&mut app.style_name).hint_text("Nombre para guardar").desired_width(ui.available_width()));
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        save = ok && (enter || clicked);
    });
    if live.len() > 1 {
        ui.label(RichText::new("Cada tinta conserva su ángulo y su hueco en la mezcla.").small().color(pal.secondary));
    }
    if let Some(look) = apply {
        for id in &app.doc.with_relay_partners(&live) {
            if let Some(t) = app.doc.get_mut(*id).and_then(|it| it.traced_mut()) { t.trace.adopt(&look); }
        }
    }
    let mut changed = false;
    if save {
        let name = std::mem::take(&mut app.style_name);
        styles::upsert(&mut app.styles, &name, &current);
        changed = true;
    }
    if let Some(i) = remove {
        app.styles.remove(i);
        changed = true;
    }
    if changed {
        if let Err(e) = styles::save(&app.styles) {
            app.notify(ui.ctx(), format!("No se pudo guardar el estilo: {e}"));
        }
    }
}

/// Apartado TRAZO de las tintas vivas. Devuelve los ajustes editados.
fn trace_controls(ui: &mut Ui, base: &TraceParams, many: bool, busy: bool) -> TraceParams {
    trace_controls_in(ui, "TRAZO", base, many, busy)
}

/// Efectos de trazo de un relleno: el motor de las fotos sobre la forma, con su tono.
/// La separación y el ángulo son los del relleno.
fn fill_trace_controls(ui: &mut Ui, fill: &mut Fill) {
    let base = fill.trace_params();
    let new = trace_controls_in(ui, "EFECTOS DEL RELLENO", &base, false, false);
    if new != base {
        fill.spacing_mm = new.spacing_mm;
        fill.angle_deg = new.angle_deg;
        fill.trace = new;
    }
    row(ui, "Tono", |ui| {
        ui.add(egui::Slider::new(&mut fill.tone, 0.05..=1.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)))
            .on_hover_text("Cuánto color pone el relleno: en los tonos bajos las líneas se cortan en trazos sueltos.");
    });
}

fn trace_controls_in(ui: &mut Ui, title: &str, base: &TraceParams, many: bool, busy: bool) -> TraceParams {
    let pal = theme::current(ui.ctx());
    let mut t = base.clone();
    let percent = |v: f64, _| format!("{:.0} %", v * 100.0);
    theme::section(ui, title);
    style_choice(ui, &mut t.style);
    ui.label(RichText::new(t.style.help()).small().color(pal.secondary));
    row(ui, "Separación", |ui| { ui.add(egui::Slider::new(&mut t.spacing_mm, 0.3..=6.0).logarithmic(true).suffix(" mm")); });
    if t.style.uses_angle() {
        let label = if many { "Girar" } else { "Ángulo" };
        let hover = if t.style == TraceStyle::Flow { "Dirección de las líneas en las zonas lisas, lejos de los bordes del color." } else { "Dirección de las líneas." };
        row(ui, label, |ui| { ui.add(egui::Slider::new(&mut t.angle_deg, 0.0..=180.0).suffix("°")).on_hover_text(hover); });
    }
    if t.relay && !t.style.uses_dashes() && t.style != TraceStyle::Dots {
        row(ui, "Largo", |ui| {
            ui.add(egui::Slider::new(&mut t.dash_mm, 0.5..=60.0).logarithmic(true).suffix(" mm"))
                .on_hover_text("Cada cuánto puede pasar la línea de un rotulador a otro.");
        });
    }
    if t.style.uses_dashes() {
        row(ui, "Largo", |ui| {
            ui.add(egui::Slider::new(&mut t.dash_mm, 0.5..=60.0).logarithmic(true).suffix(" mm"))
                .on_hover_text("Largo de cada trazo en los tonos claros. En los oscuros los trazos se juntan en líneas.");
        });
        row(ui, "Variación", |ui| {
            ui.add(egui::Slider::new(&mut t.dash_vary, 0.0..=1.0).custom_formatter(percent))
                .on_hover_text("Que no todos los trazos midan lo mismo.");
        });
    }
    row(ui, "Grosor", |ui| {
        let hover = if t.style != TraceStyle::Dots { "Repasa los trazos donde hay más color para que se vean más gruesos." } else { "Agranda los puntos donde hay más color." };
        ui.add(egui::Slider::new(&mut t.weight, 0.0..=1.0).custom_formatter(percent)).on_hover_text(hover);
    });
    if many {
        ui.label(RichText::new("Se aplica a todas las tintas seleccionadas. Girar mantiene la diferencia de ángulo entre ellas.").small().color(pal.secondary));
    }
    if t.relay {
        ui.label(RichText::new("En relevo: los cambios se aplican a todas las tintas que comparten las líneas.").small().color(pal.secondary));
    }
    if t.style.uses_vibe() {
        theme::section(ui, "VIBRACIÓN");
        crate::app::choice(ui, &mut t.vibe, &Vibe::ALL.map(|v| (v, v.label())));
        if t.vibe != Vibe::None {
            row(ui, "Amplitud", |ui| {
                ui.add(egui::Slider::new(&mut t.vibe_mm, 0.05..=4.0).logarithmic(true).suffix(" mm"))
                    .on_hover_text("Cuánto se aparta la línea a cada lado. Nunca se sale de su color.");
            });
            row(ui, "Onda", |ui| {
                ui.add(egui::Slider::new(&mut t.wave_mm, 0.3..=20.0).logarithmic(true).suffix(" mm"))
                    .on_hover_text("Distancia entre dos crestas de la vibración.");
            });
            ui.checkbox(&mut t.vibe_by_tone, "Más fuerte donde hay más color")
                .on_hover_text("La vibración crece en los oscuros y se calma en los claros.");
        }
    }
    if busy {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Recalculando trazos…").small().color(pal.secondary));
        });
    }
    t
}

/// Selector de estilo de trazo en dos filas de tres.
pub fn style_choice(ui: &mut Ui, style: &mut TraceStyle) -> bool {
    let all = TraceStyle::ALL.map(|s| (s, s.label()));
    let first = crate::app::choice(ui, style, &all[..3]);
    ui.add_space(2.0);
    crate::app::choice(ui, style, &all[3..]) || first
}

/// Aplica a cada tinta viva solo lo que ha cambiado; el ángulo se suma como giro.
fn apply_trace(doc: &mut Document, ids: &[u64], base: &TraceParams, new: &TraceParams) {
    if new == base {
        return;
    }
    for id in ids {
        let Some(t) = doc.get_mut(*id).filter(|it| !it.locked).and_then(|it| it.traced_mut()) else { continue };
        apply_trace_params(&mut t.trace, base, new);
    }
}

/// Copia a `t` lo que cambió de `base` a `new`; el ángulo se suma como giro.
fn apply_trace_params(t: &mut TraceParams, base: &TraceParams, new: &TraceParams) {
    macro_rules! copy_changed {
        ($($f:ident),*) => { $( if new.$f != base.$f { t.$f = new.$f; } )* };
    }
    copy_changed!(style, spacing_mm, dash_mm, dash_vary, weight, vibe, vibe_mm, wave_mm, vibe_by_tone, phase);
    if new.angle_deg != base.angle_deg { t.angle_deg = (t.angle_deg + new.angle_deg - base.angle_deg).rem_euclid(180.0); }
}

fn object_row(app: &mut VectorioApp, ui: &mut Ui, id: u64, drop: &mut Option<(Vec<u64>, (u64, bool))>) {
    ui.push_id(("object", id), |ui| {
        let pal = theme::current(ui.ctx());
        let selected = app.is_selected(id);
        let full = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), 32.0));
        let response = ui.interact(full, ui.id().with("row"), egui::Sense::click_and_drag());
        if selected { ui.painter().rect_filled(full, theme::CONTROL_RADIUS, pal.accent_soft); }
        let mut finish = false;
        let row = ui.horizontal(|ui| {
            ui.set_min_height(32.0);
            ui.spacing_mut().item_spacing.x = 6.0;
            let it = app.doc.get(id).unwrap();
            let (visible, locked, fillable) = (it.visible, it.locked, it.is_fillable() || it.is_screened());
            let screened = it.is_screened();
            let stroke = it.stroke.and_then(|p| app.doc.pens.get(p)).map(|p| p.color);
            let fill = if it.has_fill() { app.doc.pens.get(it.fill.pen).map(|p| p.color) } else { None };
            if visibility_toggle(ui, visible).on_hover_text("Mostrar u ocultar objeto").clicked() { app.doc.get_mut(id).unwrap().visible = !visible; }
            ui.add_enabled_ui(!locked && !screened, |ui| {
                if ink_swatch(ui, stroke, false).clicked() { app.edit_item_color(id, false); }
            });
            ui.add_enabled_ui(!locked && fillable, |ui| {
                if ink_swatch(ui, fill, true).clicked() { app.edit_item_color(id, true); }
            });
            let width = (ui.available_width() - if locked { 18.0 } else { 0.0 }).max(30.0);
            let index = app.doc.index_of(id).unwrap();
            let it = &mut app.doc.items[index];
            if app.renaming_item == Some(id) {
                finish = rename_field(ui, &mut it.name, &mut app.rename_focus, width);
                None
            } else {
                let response = layer_name(ui, width, RichText::new(&it.name).color(if visible { pal.text } else { pal.secondary }));
                if locked { ui.label(RichText::new("L").small().color(pal.secondary)).on_hover_text("Objeto bloqueado"); }
                Some(response)
            }
        });
        if finish { app.finish_rename(false); }
        let response = if let Some(label) = row.inner { response.union(label) } else { response };
        if response.clicked() {
            app.pen_focus = false;
            if ui.input(|i| i.modifiers.shift || i.modifiers.command) { app.toggle_selected(id); }
            else { app.set_selection(vec![id]); }
        }
        if response.double_clicked() { app.rename_item(id); }
        if response.drag_started() && !app.doc.get(id).unwrap().locked {
            if !app.is_selected(id) { app.set_selection(vec![id]); }
            app.pen_focus = false;
            egui::DragAndDrop::set_payload(ui.ctx(), app.selection());
        }
        if let Some(ids) = egui::DragAndDrop::payload::<Vec<u64>>(ui.ctx()) {
            if let Some(pointer) = ui.ctx().pointer_latest_pos().filter(|p| full.intersect(ui.clip_rect()).contains(*p)) {
                if !ids.contains(&id) {
                    let above = pointer.y < full.center().y;
                    let y = if above { full.top() } else { full.bottom() };
                    ui.painter().hline(full.x_range(), y, egui::Stroke::new(2.0f32, pal.accent));
                    if ui.input(|i| i.pointer.any_released()) {
                        *drop = Some((ids.as_ref().clone(), (id, above)));
                        egui::DragAndDrop::clear_payload(ui.ctx());
                    }
                }
            }
        }
        if let Some(drag) = egui::DragAndDrop::payload::<PenDrag>(ui.ctx()).map(|d| *d) {
            let alt = ui.input(|i| i.modifiers.alt);
            let over = ui.ctx().pointer_latest_pos().is_some_and(|p| full.intersect(ui.clip_rect()).contains(p));
            if let (true, Some(fill), Some(pen)) = (over, pen_drop_fill(app, id, alt), app.doc.pens.get(drag.pen)) {
                ui.painter().rect_stroke(full, theme::CONTROL_RADIUS, egui::Stroke::new(2.0f32, theme::pen_color(pen.color)));
                set_drop_hint(ui.ctx(), drop_hint(app, id, fill));
                if ui.input(|i| i.pointer.any_released()) {
                    app.doc.assign_pen(id, drag.pen, fill);
                    egui::DragAndDrop::clear_payload(ui.ctx());
                }
            }
        }
        response.on_hover_text("Doble clic: nombre · arrastra: orden de capas, sin cambiar tintas").context_menu(|ui| {
            if !app.is_selected(id) { app.set_selection(vec![id]); }
            app.pen_focus = false;
            if ui.button("Renombrar").clicked() { app.rename_item(id); ui.close_menu(); }
            let locked = app.doc.get(id).unwrap().locked;
            if ui.button(if locked { "Desbloquear" } else { "Bloquear" }).clicked() { app.doc.get_mut(id).unwrap().locked = !locked; ui.close_menu(); }
            for (label, forward, edge) in [("Traer adelante", true, false), ("Enviar atrás", false, false), ("Traer al frente", true, true), ("Enviar al fondo", false, true)] {
                if ui.button(label).clicked() { app.doc.reorder_items(&app.selection(), forward, edge); ui.close_menu(); }
            }
            ui.separator();
            if ui.button("Duplicar selección").clicked() { app.duplicate_selected(); ui.close_menu(); }
            if ui.button("Eliminar selección").clicked() { app.delete_selected(); ui.close_menu(); }
        });
    });
}

// ── Derecha ─────────────────────────────────────────────────────────────────────

pub fn right_panel(app: &mut VectorioApp, ui: &mut Ui) {
    if app.pen_focus {
        pen_settings(app, ui);
        return;
    }
    if app.selection().len() > 1 {
        multi_panel(app, ui);
        return;
    }
    let Some(id) = app.selected.filter(|id| app.doc.get(*id).is_some()) else {
        document_settings(app, ui);
        return;
    };
    let pens = app.doc.pens.clone();
    let paper = app.doc.paper.clone();
    let mut export_pages = false;
    let mut a4 = false;
    let focus = std::mem::take(&mut app.focus_text);
    let mut duplicate = false;
    let mut delete = false;
    let mut edit_color = None;
    let busy = app.tracing();
    let mut relay_edit: Option<(TraceParams, TraceParams)> = None;
    let group = {
        let it = app.doc.get_mut(id).unwrap();
        ui.add(egui::TextEdit::singleline(&mut it.name).font(egui::TextStyle::Heading).frame(false).desired_width(f32::INFINITY));
        ui.add_space(2.0);
        it.group.filter(|g| app.doc.photo(*g).is_some())
    };
    if let Some(group) = group {
        photo_section(app, ui, group);
    }
    styles_section(app, ui, &[id]);
    let it = app.doc.get_mut(id).unwrap();
    let fillable = it.is_fillable();
    let screened = it.is_screened();

    // Contenido específico.
    match &mut it.geometry {
        Geometry::Text(spec) => {
            theme::section(ui, "TEXTO Y PÁRRAFO");
            let editor_id = egui::Id::new(("text-content", id));
            let shortcut = ui.memory(|memory| memory.has_focus(editor_id))
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
            let editor = egui::ScrollArea::vertical().id_salt(("text-editor-scroll", id))
                .max_height(150.0).show(ui, |ui| {
                    egui::TextEdit::multiline(&mut spec.text).desired_rows(6).desired_width(f32::INFINITY)
                        .id(editor_id).hint_text("Escribe o pega tu texto completo…").show(ui)
                }).inner;
            if focus { editor.response.request_focus(); editor.response.scroll_to_me(Some(egui::Align::Min)); }
            let mut page_break = shortcut;
            ui.horizontal(|ui| {
                page_break |= ui.button("Salto de página").on_hover_text("Insertar en el cursor · ⌘Enter").clicked();
                if ui.button("Exportar páginas…").on_hover_text("Un SVG por página; los demás objetos se repiten en cada hoja").clicked() { export_pages = true; }
            });
            if page_break {
                let range = editor.state.cursor.char_range().map(|range| range.sorted());
                let (cursor, end) = range.map(|range| (range[0].index, range[1].index)).unwrap_or_else(|| { let end = spec.text.chars().count(); (end, end) });
                let byte = spec.text.char_indices().nth(cursor).map(|(i, _)| i).unwrap_or(spec.text.len());
                let end = spec.text.char_indices().nth(end).map(|(i, _)| i).unwrap_or(spec.text.len());
                spec.text.replace_range(byte..end, "\u{c}");
                let mut state = editor.state;
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(cursor + 1))));
                state.store(ui.ctx(), editor.response.id);
                editor.response.request_focus();
            }

            theme::section(ui, "CAJA DE TEXTO");
            if !spec.is_frame() && ui.button("Convertir en caja de texto").clicked() {
                spec.wrap_mm = spec.wrap_mm.max(120.0); spec.frame_height_mm = 100.0; spec.padding_mm = 2.0;
            }
            row(ui, "Ancho", |ui| { drag(ui, &mut spec.wrap_mm, 0.0..=2000.0, 0.5, " mm").on_hover_text("Ancho de la caja; 0 conserva el texto sin ajuste de línea"); });
            row(ui, "Alto", |ui| { drag(ui, &mut spec.frame_height_mm, 0.0..=2000.0, 0.5, " mm").on_hover_text("Alto de la caja; 0 permite un texto sin límite vertical"); });
            row(ui, "Margen", |ui| { drag(ui, &mut spec.padding_mm, 0.0..=25.0, 0.2, " mm"); });
            ui.horizontal(|ui| {
                let fit = ui.button("Ocupar hoja").clicked();
                let folio = ui.button("Folio A4").on_hover_text("Papel A4 vertical y caja en su área útil").clicked();
                if fit || folio {
                    let target = if folio { Paper::presets().into_iter().find(|p| p.name == "A4 vertical").unwrap() } else { paper.clone() };
                    spec.wrap_mm = target.width_mm - target.margin_mm * 2.0;
                    spec.frame_height_mm = target.height_mm - target.margin_mm * 2.0;
                    spec.padding_mm = 2.0;
                    spec.page = 0;
                    it.place = vectorio_core::Placement { pos: vectorio_core::p(target.margin_mm, target.margin_mm), ..Default::default() };
                    a4 = folio;
                }
            });

            theme::section(ui, "PÁRRAFO");
            let label = |a: Align| match a { Align::Left => "Izquierda", Align::Center => "Centrado", Align::Right => "Derecha", Align::Justify => "Justificado" };
            egui::ComboBox::from_id_salt("paragraph-align").width(ui.available_width()).selected_text(label(spec.align)).show_ui(ui, |ui| {
                for a in [Align::Left, Align::Center, Align::Right, Align::Justify] { ui.selectable_value(&mut spec.align, a, label(a)); }
            });
            row(ui, "Interlínea", |ui| { drag(ui, &mut spec.line_height, 1.0..=3.5, 0.02, " ×"); });
            row(ui, "Párrafos", |ui| { drag(ui, &mut spec.paragraph_spacing_mm, 0.0..=40.0, 0.2, " mm").on_hover_text("Espacio después de cada párrafo"); });
            row(ui, "Sangría", |ui| { drag(ui, &mut spec.first_indent_mm, 0.0..=50.0, 0.2, " mm").on_hover_text("Primera línea del párrafo"); });
            ui.checkbox(&mut spec.hyphenate, "Guionado automático en español");
            let flow = font::compose(spec);
            spec.page = spec.page.min(flow.page_count - 1);
            ui.horizontal(|ui| {
                if ui.add_enabled(spec.page > 0, egui::Button::new("Anterior")).clicked() { spec.page -= 1; }
                ui.label(format!("{} / {}", spec.page + 1, flow.page_count));
                if ui.add_enabled(spec.page + 1 < flow.page_count, egui::Button::new("Siguiente")).clicked() { spec.page += 1; }
            });
            if flow.page_count > 1 { ui.label(egui::RichText::new("El texto continúa en las siguientes páginas.").small().weak()); }

            theme::section(ui, "LETRA MANUSCRITA");
            row(ui, "Letra", |ui| {
                let before = spec.style;
                egui::ComboBox::from_id_salt("font-style").width(ui.available_width()).selected_text(spec.style.label()).show_ui(ui, |ui| {
                    for (i, style) in FontStyle::ALL.into_iter().enumerate() {
                        if i == 8 { ui.separator(); }
                        ui.selectable_value(&mut spec.style, style, style.label());
                    }
                });
                if spec.style != before && spec.style.is_builtin() != before.is_builtin() { spec.slant_deg = if spec.style.is_builtin() { 8.0 } else { 0.0 }; }
            });
            row(ui, "Tamaño", |ui| { drag(ui, &mut spec.size_mm, 1.0..=200.0, 0.1, " mm").on_hover_text("Altura de las mayúsculas"); });
            if spec.style.joins() { ui.checkbox(&mut spec.connect, "Unir letras"); }
            egui::CollapsingHeader::new("Carácter y variación").id_salt("handwriting-details").show(ui, |ui| {
                row(ui, "Inclinación", |ui| { drag(ui, &mut spec.slant_deg, -20.0..=30.0, 0.2, "°"); });
                if spec.style.has_jitter() { row(ui, "Pulso", |ui| { ui.add(egui::Slider::new(&mut spec.jitter, 0.0..=2.0)); }); }
                row(ui, "Espaciado", |ui| { drag(ui, &mut spec.letter_spacing, -0.3..=1.0, 0.01, " ×"); });
                if ui.button("Otra variación de la letra").clicked() { spec.seed = spec.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 1; }
            });
        }
        Geometry::Shape(shape) => {
            theme::section(ui, &shape.label().to_uppercase());
            shape_params(ui, shape);
        }
        Geometry::Screened { lines, screen } => {
            theme::section(ui, "TRAMA FIJA");
            ui.label(format!("{} · {} trazos", screen.label(), lines.len()));
            ui.label(RichText::new("Recorridos de tinta ya calculados: no se recalculan.").small().color(theme::current(ui.ctx()).secondary));
        }
        Geometry::Traced(t) => {
            let new = trace_controls(ui, &t.trace, false, busy);
            if new != t.trace && !it.locked {
                if t.trace.relay { relay_edit = Some((t.trace.clone(), new)); } else { t.trace = new; }
            }
            ui.label(RichText::new(format!("{} trazos", t.lines.len())).small().color(theme::current(ui.ctx()).secondary));
        }
        Geometry::Path { lines, closed } => {
            theme::section(ui, "TRAZADO");
            let pts: usize = lines.iter().map(|l| l.len()).sum();
            ui.label(RichText::new(format!("{} trazos · {} puntos", lines.len(), pts)).color(theme::current(ui.ctx()).secondary));
            ui.checkbox(closed, "Figuras cerradas (se pueden rellenar)");
            ui.horizontal(|ui| {
                if ui.button("Suavizar").clicked() {
                    let c = *closed;
                    for l in lines.iter_mut() {
                        *l = chaikin(l, 1, c);
                    }
                }
                if ui.button("Simplificar").clicked() {
                    for l in lines.iter_mut() {
                        let s = simplify(l, 0.3);
                        if s.len() >= 2 {
                            *l = s;
                        }
                    }
                }
            });
        }
    }

    ui.add_space(4.0);
    ui.separator();
    if screened {
        theme::section(ui, "COLOR DEL RELLENO");
        ui.add_enabled_ui(!it.locked, |ui| ink_property(ui, "screened-pen", &pens, &mut it.fill.pen, true, &mut edit_color));
    } else {
    theme::section(ui, "CONTORNO");
    let mut has_stroke = it.stroke.is_some();
    ui.horizontal(|ui| {
        if ui.checkbox(&mut has_stroke, "").changed() {
            it.stroke = if has_stroke { Some(it.fill.pen) } else { None };
        }
        if let Some(pen) = it.stroke.as_mut() {
            ui.add_enabled_ui(!it.locked, |ui| ink_property(ui, "stroke-pen", &pens, pen, false, &mut edit_color));
        } else {
            ui.label(RichText::new("Sin contorno").weak());
        }
    });

    theme::section(ui, "RELLENO");
    ui.add_enabled_ui(fillable, |ui| {
        let fill: &mut Fill = &mut it.fill;
        row(ui, "Estilo", |ui| {
            egui::ComboBox::from_id_salt("fill-style").selected_text(fill.style.label()).show_ui(ui, |ui| {
                for s in FillStyle::ALL {
                    ui.selectable_value(&mut fill.style, s, s.label());
                }
            });
        });
        if fill.style != FillStyle::None {
            ui.add_enabled_ui(!it.locked, |ui| ink_property(ui, "fill-pen", &pens, &mut fill.pen, true, &mut edit_color));
            if !matches!(fill.style, FillStyle::Solid | FillStyle::Trace) {
                row(ui, "Separación", |ui| {
                    ui.add(egui::Slider::new(&mut fill.spacing_mm, 0.3..=10.0).suffix(" mm").logarithmic(true));
                });
            }
            if matches!(fill.style, FillStyle::Hatch | FillStyle::Cross | FillStyle::Zigzag | FillStyle::Solid | FillStyle::Wave) {
                row(ui, "Ángulo", |ui| {
                    ui.add(egui::Slider::new(&mut fill.angle_deg, 0.0..=180.0).suffix("°"));
                });
            }
            row(ui, "Margen", |ui| {
                ui.add(egui::Slider::new(&mut fill.inset_mm, 0.0..=5.0).suffix(" mm"));
            });
            if fill.style == FillStyle::Trace {
                fill_trace_controls(ui, fill);
            }
        }
    });
    if !fillable {
        ui.label(RichText::new("Solo las figuras cerradas admiten relleno.").small().color(theme::current(ui.ctx()).secondary));
    }
    }

    ui.add_space(4.0);
    ui.separator();
    theme::section(ui, "POSICIÓN");
    let place = &mut it.place;
    ui.horizontal(|ui| {
        ui.label("X");
        drag(ui, &mut place.pos.x, -2000.0..=2000.0, 0.5, " mm");
        ui.label("Y");
        drag(ui, &mut place.pos.y, -2000.0..=2000.0, 0.5, " mm");
    });
    ui.horizontal(|ui| {
        let mut pct = place.scale * 100.0;
        ui.label("Escala");
        if drag(ui, &mut pct, 1.0..=5000.0, 0.5, " %").changed() {
            place.scale = pct / 100.0;
        }
        ui.label("Giro");
        drag(ui, &mut place.rotation_deg, -360.0..=360.0, 0.5, "°");
    });
    ui.checkbox(&mut it.locked, "Bloquear");

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button("Duplicar").on_hover_text("⌘D").clicked() {
            duplicate = true;
        }
        if ui.button("Eliminar").on_hover_text("⌫").clicked() {
            delete = true;
        }
    });
    if a4 {
        app.doc.paper = Paper::presets().into_iter().find(|p| p.name == "A4 vertical").unwrap();
        app.view.fitted = false;
    }
    if export_pages { app.export_text_pages(ui.ctx(), id); }
    if duplicate {
        app.duplicate_selected();
    }
    if delete {
        app.delete_selected();
    }
    if let Some(fill) = edit_color { app.edit_item_color(id, fill); }
    if let Some((base, new)) = relay_edit {
        let targets = app.doc.with_relay_partners(&[id]);
        apply_trace(&mut app.doc, &targets, &base, &new);
    }
}

fn pen_settings(app: &mut VectorioApp, ui: &mut Ui) {
    let index = app.active_pen.min(app.doc.pens.len().saturating_sub(1));
    let count = app.doc.items.iter().filter(|it| it.stroke == Some(index) || (it.has_fill() && it.fill.pen == index)).count();
    let pal = theme::current(ui.ctx());
    ui.heading("Rotulador");
    ui.label(RichText::new(format!("Tinta compartida por {count} objetos")).color(pal.secondary));
    ui.add_space(8.0);
    ui.label("Nombre");
    if ui.add(egui::TextEdit::singleline(&mut app.doc.pens[index].name).desired_width(f32::INFINITY)).changed() {
        app.doc.pens[index].custom_name = true;
    }
    ui.add_space(4.0);
    row(ui, "Color", |ui| {
        let swatch = theme::swatch(ui, app.doc.pens[index].color, 28.0);
        if ui.button("Cambiar…").clicked() || swatch.clicked() { app.edit_pen_color(index); }
    });
    row(ui, "Grosor", |ui| {
        drag(ui, &mut app.doc.pens[index].width_mm, 0.1..=5.0, 0.01, " mm");
    });
    ui.add_space(8.0);
    ui.label(RichText::new("El nombre y el color describen una tinta. El orden de los objetos se cambia en la lista de capas.").small().color(pal.secondary));
}

fn shape_params(ui: &mut Ui, shape: &mut Shape) {
    match shape {
        Shape::Rect { w, h, radius } => {
            row(ui, "Ancho", |ui| drop(drag(ui, w, 1.0..=1000.0, 0.5, " mm")));
            row(ui, "Alto", |ui| drop(drag(ui, h, 1.0..=1000.0, 0.5, " mm")));
            row(ui, "Esquinas", |ui| drop(drag(ui, radius, 0.0..=500.0, 0.2, " mm")));
        }
        Shape::Ellipse { rx, ry } => {
            row(ui, "Radio X", |ui| drop(drag(ui, rx, 0.5..=500.0, 0.5, " mm")));
            row(ui, "Radio Y", |ui| drop(drag(ui, ry, 0.5..=500.0, 0.5, " mm")));
        }
        Shape::Polygon { sides, r } => {
            row(ui, "Lados", |ui| drop(drag_u(ui, sides, 3..=40)));
            row(ui, "Radio", |ui| drop(drag(ui, r, 0.5..=500.0, 0.5, " mm")));
        }
        Shape::Star { points, outer, inner } => {
            row(ui, "Puntas", |ui| drop(drag_u(ui, points, 3..=40)));
            row(ui, "Exterior", |ui| drop(drag(ui, outer, 0.5..=500.0, 0.5, " mm")));
            row(ui, "Interior", |ui| drop(drag(ui, inner, 0.5..=500.0, 0.5, " mm")));
        }
        Shape::Heart { size } => row(ui, "Tamaño", |ui| drop(drag(ui, size, 1.0..=1000.0, 0.5, " mm"))),
        Shape::Spiral { turns, r } => {
            row(ui, "Vueltas", |ui| drop(drag(ui, turns, 0.5..=100.0, 0.1, "")));
            row(ui, "Radio", |ui| drop(drag(ui, r, 0.5..=500.0, 0.5, " mm")));
        }
        Shape::Spirograph { big_r, small_r, offset, size } => {
            row(ui, "Aro", |ui| drop(drag_u(ui, big_r, 10..=200)));
            row(ui, "Rueda", |ui| drop(drag_u(ui, small_r, 1..=199)));
            row(ui, "Lápiz", |ui| drop(drag(ui, offset, 0.0..=200.0, 0.2, "")));
            row(ui, "Tamaño", |ui| drop(drag(ui, size, 1.0..=1000.0, 0.5, " mm")));
        }
        Shape::Lissajous { a, b, phase_deg, w, h } => {
            row(ui, "Frec. X", |ui| drop(drag_u(ui, a, 1..=30)));
            row(ui, "Frec. Y", |ui| drop(drag_u(ui, b, 1..=30)));
            row(ui, "Fase", |ui| drop(ui.add(egui::Slider::new(phase_deg, 0.0..=180.0).suffix("°"))));
            row(ui, "Ancho", |ui| drop(drag(ui, w, 1.0..=1000.0, 0.5, " mm")));
            row(ui, "Alto", |ui| drop(drag(ui, h, 1.0..=1000.0, 0.5, " mm")));
        }
        Shape::Rose { n, d, r } => {
            row(ui, "Pétalos n", |ui| drop(drag_u(ui, n, 1..=30)));
            row(ui, "d", |ui| drop(drag_u(ui, d, 1..=30)));
            row(ui, "Radio", |ui| drop(drag(ui, r, 0.5..=500.0, 0.5, " mm")));
        }
    }
}

fn document_settings(app: &mut VectorioApp, ui: &mut Ui) {
    let pal = theme::current(ui.ctx());
    ui.label(RichText::new("Documento").heading());
    theme::section(ui, "PAPEL");
    let paper = &mut app.doc.paper;
    let mut refit = false;
    row(ui, "Tamaño", |ui| {
        egui::ComboBox::from_id_salt("paper").selected_text(paper.name.clone()).show_ui(ui, |ui| {
            for pr in Paper::presets() {
                let sel = pr.name == paper.name;
                if ui.selectable_label(sel, &pr.name).clicked() && !sel {
                    let margin = paper.margin_mm;
                    *paper = Paper { margin_mm: margin, ..pr };
                    refit = true;
                }
            }
        });
    });
    ui.label(RichText::new(format!("{:.0} × {:.0} mm", paper.width_mm, paper.height_mm)).small().color(pal.secondary));
    row(ui, "Margen", |ui| drop(drag(ui, &mut paper.margin_mm, 0.0..=60.0, 0.2, " mm")));
    if refit {
        app.view.fitted = false;
    }

    theme::section(ui, "PLOTTER");
    let plot = &mut app.doc.plot;
    row(ui, "Dibujo", |ui| drop(drag(ui, &mut plot.draw_speed, 1.0..=300.0, 0.5, " mm/s")));
    row(ui, "Viaje", |ui| drop(drag(ui, &mut plot.travel_speed, 1.0..=500.0, 0.5, " mm/s")));
    row(ui, "Subir/bajar", |ui| drop(drag(ui, &mut plot.lift_time, 0.0..=3.0, 0.01, " s")));
    ui.checkbox(&mut plot.optimize, "Optimizar recorrido").on_hover_text("Ordena los trazos de cada rotulador para viajar menos");
    row(ui, "Unir", |ui| drop(drag(ui, &mut plot.join_tolerance, 0.0..=2.0, 0.01, " mm").on_hover_text("Une trazos cuyos extremos están a menos de esta distancia")));

    ui.add_space(8.0);
    ui.separator();
    theme::section(ui, "IDEAS PARA EMPEZAR");
    for tip in [
        "T · arrastra una caja y escribe a mano",
        "P · dibuja a mano alzada; cierra la figura para rellenarla",
        "S · arrastra para crear formas y generativos",
        "I · convierte una foto en dibujo por colores",
        "Plotter · simula el orden real de dibujo",
        "Pellizca o ⌘ + rueda para hacer zoom; espacio para desplazar",
    ] {
        ui.label(RichText::new(tip).small().color(pal.secondary));
    }
}

/// Inspector con varios objetos: los cambios se aplican a todos a la vez.
fn multi_panel(app: &mut VectorioApp, ui: &mut Ui) {
    let ids = app.selection();
    let pens = app.doc.pens.clone();
    let pal = theme::current(ui.ctx());
    // Si la selección es una foto viva entera, se titula con su nombre.
    let group = ids.first().and_then(|id| app.doc.get(*id)).and_then(|it| it.group)
        .filter(|g| app.doc.photo(*g).is_some() && ids.iter().all(|id| app.doc.get(*id).is_some_and(|it| it.group == Some(*g))));
    let title = group.and_then(|g| app.doc.photo(g)).map(|ph| ph.name.clone()).unwrap_or_else(|| format!("{} objetos", ids.len()));
    ui.label(RichText::new(title).heading());
    ui.label(RichText::new("Lo que cambies aquí se aplica a todos. Arrástralos para moverlos juntos.").small().color(pal.secondary));
    let first = ids.iter().filter_map(|id| app.doc.get(*id)).find(|it| !it.is_screened()).cloned();
    let fillable: Vec<u64> = ids.iter().copied().filter(|id| app.doc.get(*id).is_some_and(|it| it.is_fillable())).collect();
    let mut edit_ink = None;
    let mut edit_all = None;

    ui.add_space(4.0);
    ui.separator();
    if let Some(group) = group {
        photo_section(app, ui, group);
    }
    let traced: Vec<u64> = ids.iter().copied().filter(|id| app.doc.get(*id).is_some_and(|it| it.traced().is_some())).collect();
    styles_section(app, ui, &traced);
    if let Some(base) = traced.first().and_then(|id| app.doc.get(*id)).and_then(|it| it.traced()).map(|t| t.trace.clone()) {
        let new = trace_controls(ui, &base, traced.len() > 1, app.tracing());
        let targets = app.doc.with_relay_partners(&traced);
        apply_trace(&mut app.doc, &targets, &base, &new);
    }
    if let Some(first) = first {
    theme::section(ui, "CONTORNO");
    let mut stroke = first.stroke;
    let mut has = stroke.is_some();
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui.checkbox(&mut has, "").changed();
        if has {
            let mut pen = stroke.unwrap_or(first.fill.pen);
            let old = pen;
            ink_property(ui, "multi-stroke", &pens, &mut pen, false, &mut edit_all);
            changed |= pen != old;
            stroke = Some(pen);
        } else {
            ui.label(RichText::new("Sin contorno").weak());
        }
    });
    if changed {
        for id in &ids {
            if let Some(it) = app.doc.get_mut(*id).filter(|it| !it.is_screened()) {
                // Sin contorno solo si el objeto tiene relleno; si no, desaparecería.
                it.stroke = if has { stroke.or(Some(it.fill.pen)) } else if it.is_fillable() { None } else { it.stroke };
            }
        }
    }

    }
    let screened: Vec<u64> = ids.iter().copied().filter(|id| app.doc.get(*id).is_some_and(|it| it.is_screened())).collect();
    if !screened.is_empty() {
        theme::section(ui, "RELLENOS DE MEZCLA");
        ui.label(RichText::new("Cada trama conserva su color.").small().color(pal.secondary));
        for id in screened {
            if let Some(it) = app.doc.get_mut(id) {
                let name = it.name.clone();
                ui.add(egui::Label::new(name).truncate());
                let mut edit = None;
                ui.add_enabled_ui(!it.locked, |ui| ink_property(ui, &format!("screened-{id}"), &pens, &mut it.fill.pen, true, &mut edit));
                if edit.is_some() { edit_ink = Some(id); }
            }
        }
    }

    if !fillable.is_empty() {
        theme::section(ui, "RELLENO");
        if ui.button("Cambiar color de los rellenos…").clicked() { edit_all = Some(true); }
        let base = fillable.iter().find_map(|id| app.doc.get(*id)).map(|it| it.fill.clone()).unwrap_or_default();
        let mut f = base.clone();
        row(ui, "Estilo", |ui| {
            egui::ComboBox::from_id_salt("multi-fill-style").selected_text(f.style.label()).show_ui(ui, |ui| {
                for s in FillStyle::ALL {
                    ui.selectable_value(&mut f.style, s, s.label());
                }
            });
        });
        if f.style != FillStyle::None {
            if !matches!(f.style, FillStyle::Solid | FillStyle::Trace) {
                row(ui, "Separación", |ui| {
                    ui.add(egui::Slider::new(&mut f.spacing_mm, 0.3..=10.0).suffix(" mm").logarithmic(true));
                });
            }
            if matches!(f.style, FillStyle::Hatch | FillStyle::Cross | FillStyle::Zigzag | FillStyle::Solid | FillStyle::Wave) {
                row(ui, "Ángulo", |ui| {
                    ui.add(egui::Slider::new(&mut f.angle_deg, 0.0..=180.0).suffix("°"));
                });
            }
            row(ui, "Margen", |ui| {
                ui.add(egui::Slider::new(&mut f.inset_mm, 0.0..=5.0).suffix(" mm"));
            });
            if f.style == FillStyle::Trace {
                fill_trace_controls(ui, &mut f);
            }
        }
        ui.label(RichText::new("Cada objeto conserva su rotulador de relleno.").small().color(pal.secondary));
        if f != base {
            for id in &fillable {
                if let Some(it) = app.doc.get_mut(*id) {
                    let pen = it.fill.pen;
                    if f.style != base.style {
                        it.fill.style = f.style;
                    }
                    if f.spacing_mm != base.spacing_mm {
                        it.fill.spacing_mm = f.spacing_mm;
                    }
                    if f.angle_deg != base.angle_deg {
                        it.fill.angle_deg = f.angle_deg;
                    }
                    if f.inset_mm != base.inset_mm {
                        it.fill.inset_mm = f.inset_mm;
                    }
                    if f.tone != base.tone {
                        it.fill.tone = f.tone;
                    }
                    if f.trace != base.trace {
                        // Como con las tintas: solo cambia lo que se ha tocado.
                        apply_trace_params(&mut it.fill.trace, &base.trace, &f.trace);
                    }
                    it.fill.pen = pen;
                }
            }
        }
    }

    ui.add_space(6.0);
    ui.separator();
    let mut lock = ids.iter().all(|id| app.doc.get(*id).is_some_and(|it| it.locked));
    if ui.checkbox(&mut lock, "Bloquear todos").changed() {
        for id in &ids {
            if let Some(it) = app.doc.get_mut(*id) {
                it.locked = lock;
            }
        }
    }
    ui.horizontal(|ui| {
        if ui.button("Duplicar").on_hover_text("⌘D").clicked() {
            app.duplicate_selected();
        }
        if ui.button("Eliminar").on_hover_text("⌫").clicked() {
            app.delete_selected();
        }
        if ui.button("Deseleccionar").on_hover_text("Esc").clicked() {
            app.set_selection(vec![]);
        }
    });
    if let Some(fill) = edit_all { app.edit_items_color(&ids, fill); }
    else if let Some(id) = edit_ink { app.edit_item_color(id, true); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorio_core::raster::Grid;
    use vectorio_core::{p, Coverage, Item, Traced};

    fn ink(doc: &mut Document, angle: f64) -> u64 {
        let cov = Coverage { map: Grid::new(4, 4), mm_per_px: 1.0 };
        doc.add(Item::new("tinta", Geometry::Traced(Traced::new(cov, TraceParams { angle_deg: angle, ..Default::default() }, 0)), p(0.0, 0.0)))
    }

    #[test]
    fn editing_several_inks_changes_only_what_moved_and_rotates_angles_together() {
        let mut doc = Document::default();
        let ids = [ink(&mut doc, 15.0), ink(&mut doc, 105.0), ink(&mut doc, 60.0)];
        doc.get_mut(ids[2]).unwrap().locked = true;
        let base = doc.get(ids[0]).unwrap().traced().unwrap().trace.clone();
        let rotated = TraceParams { angle_deg: 95.0, ..base.clone() };
        apply_trace(&mut doc, &ids, &base, &rotated);
        let angle = |id: u64, doc: &Document| doc.get(id).unwrap().traced().unwrap().trace.angle_deg;
        assert_eq!((angle(ids[0], &doc), angle(ids[1], &doc), angle(ids[2], &doc)), (95.0, 5.0, 60.0));
        let dots = TraceParams { style: TraceStyle::Dots, ..rotated.clone() };
        apply_trace(&mut doc, &ids, &rotated, &dots);
        let t = doc.get(ids[1]).unwrap().traced().unwrap();
        assert_eq!((t.trace.style, t.trace.angle_deg, t.trace.spacing_mm), (TraceStyle::Dots, 5.0, base.spacing_mm));
        assert!(!t.is_current(0.6), "un ajuste nuevo obliga a recalcular");
        let wavy = TraceParams { vibe: Vibe::Tremor, weight: 0.5, ..dots.clone() };
        apply_trace(&mut doc, &ids, &dots, &wavy);
        let t = doc.get(ids[0]).unwrap().traced().unwrap();
        assert_eq!((t.trace.vibe, t.trace.weight, t.trace.angle_deg), (Vibe::Tremor, 0.5, 95.0));
        assert_eq!(doc.get(ids[2]).unwrap().traced().unwrap().trace.vibe, Vibe::None, "la bloqueada no cambia");
    }

    /// Dibuja el panel derecho sin pantalla y devuelve todos los textos que aparecen.
    fn panel_texts(app: &mut VectorioApp, ctx: &egui::Context) -> String {
        let mut texts = String::new();
        for _ in 0..2 {
            let out = ctx.run(egui::RawInput::default(), |ctx| {
                egui::SidePanel::right("inspector").min_width(320.0).show(ctx, |ui| right_panel(app, ui));
            });
            fn collect(shape: &egui::Shape, texts: &mut String) {
                match shape {
                    egui::Shape::Text(t) => { texts.push_str(t.galley.text()); texts.push('\n'); }
                    egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, texts)),
                    _ => {}
                }
            }
            texts.clear();
            out.shapes.iter().for_each(|c| collect(&c.shape, &mut texts));
        }
        texts
    }

    #[test]
    fn a_live_photo_shows_its_mix_and_styles_sections() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let photo = vectorio_core::Photo { id: 0, name: "Foto".into(), image: vectorio_core::Shared::new(vectorio_core::photo::EncodedImage { bytes: vec![] }), params: Default::default(), size_pct: 80.0 };
        let cov = || Coverage { map: Grid::new(4, 4), mm_per_px: 1.0 };
        let items = (0..2).map(|i| {
            let mut it = Item::new(format!("tinta {i}"), Geometry::Traced(Traced::new(cov(), TraceParams::default(), i)), p(0.0, 0.0));
            it.group = Some(0);
            it
        }).collect();
        let ids = app.doc.add_photo_items(items, Some(photo), None);
        app.selected = Some(ids[0]);
        let one = panel_texts(&mut app, &ctx);
        for word in ["FOTO", "MEZCLA", "Cruzar", "Entrelazar", "Relevo", "Claras primero", "ESTILOS", "Grabado", "Guardar"] {
            assert!(one.contains(word), "falta «{word}» con una tinta elegida:\n{one}");
        }
        app.toggle_selected(ids[1]);
        assert_eq!(app.selection().len(), 2);
        let both = panel_texts(&mut app, &ctx);
        for word in ["ESTILOS", "Curvas de nivel", "Contornos", "Espiral", "Garabato"] {
            assert!(both.contains(word), "falta «{word}» con dos tintas elegidas:\n{both}");
        }
    }

    #[test]
    fn a_trace_filled_shape_shows_its_effect_controls() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let mut it = Item::new("Corazón", Geometry::Shape(vectorio_core::Shape::Heart { size: 40.0 }), p(0.0, 0.0));
        it.fill.style = vectorio_core::FillStyle::Trace;
        let id = app.doc.add(it);
        app.selected = Some(id);
        let t = panel_texts(&mut app, &ctx);
        for word in ["RELLENO", "EFECTOS DEL RELLENO", "Tono"] {
            assert!(t.contains(word), "falta «{word}» con relleno de trazo:\n{t}");
        }
        assert_eq!(t.matches("Separación").count(), 1, "Separación debe salir una sola vez, dentro de los efectos:\n{t}");
    }
}
