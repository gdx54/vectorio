//! Paneles flotantes: rotuladores y objetos a la izquierda, inspector a la derecha.

use crate::app::{VectorioApp, PAPER_TINTS};
use crate::theme;
use egui::{RichText, Ui};
use vectorio_core::geom::{chaikin, simplify};
use vectorio_core::{Align, Fill, FillStyle, FontStyle, Geometry, Paper, Pen, Shape};

pub const LEFT_W: f32 = 236.0;
pub const RIGHT_W: f32 = 264.0;

fn pen_label(pens: &[Pen], i: usize) -> RichText {
    match pens.get(i) {
        Some(p) => RichText::new(&p.name).color(theme::pen_color(p.color)),
        None => RichText::new("—"),
    }
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

pub fn left_panel(app: &mut VectorioApp, ui: &mut Ui) {
    theme::section(ui, "ROTULADORES");
    let pal = theme::current(ui.ctx());
    // Acción del menú: (rotulador, destino) para mover, o destino usize::MAX para borrar.
    let mut action: Option<(usize, usize)> = None;
    let n = app.doc.pens.len();
    // Arriba el que se dibuja el último (queda encima), como en una lista de capas.
    let mut rows: Vec<(usize, egui::Rect)> = Vec::with_capacity(n);
    let mut dragging: Option<usize> = None;
    let mut dropped: Option<usize> = None;
    for i in (0..n).rev() {
        let active = app.active_pen == i;
        let renaming = app.renaming_pen == Some(i);
        let editing_color = app.color_edit == Some(i);
        // La fila se registra antes que sus controles para que éstos queden encima:
        // el grosor y la muestra siguen respondiendo, y el resto de la fila se arrastra.
        let full = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), 22.0)).expand2(egui::vec2(4.0, 1.0));
        let resp = ui.interact(full, egui::Id::new(("pen-row", i)), egui::Sense::click_and_drag());
        // Fondo de la fila, que se rellena después (debajo de los controles).
        let bg = ui.painter().add(egui::Shape::Noop);
        let row = ui.horizontal(|ui| {
            ui.set_min_height(22.0);
            let pen = &mut app.doc.pens[i];
            // Muestra de color: doble clic abre el selector del sistema.
            let (sw, sw_resp) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
            ui.painter().circle_filled(sw.center(), 8.0, theme::pen_color(pen.color));
            ui.painter().circle_stroke(sw.center(), 8.0, egui::Stroke::new(1.0f32, pal.glass_stroke));
            if editing_color {
                ui.painter().circle_stroke(sw.center(), 10.5, egui::Stroke::new(1.5f32, pal.accent));
            }
            if renaming {
                let r = ui.add(egui::TextEdit::singleline(&mut pen.name).desired_width(104.0));
                if !r.has_focus() && !r.gained_focus() {
                    r.request_focus();
                }
                if r.lost_focus() {
                    app.renaming_pen = None;
                }
            } else {
                ui.allocate_ui_with_layout(egui::vec2(104.0, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(104.0);
                    ui.add(egui::Label::new(RichText::new(&pen.name).color(pal.text)).truncate().selectable(false));
                });
            }
            ui.add(egui::DragValue::new(&mut pen.width_mm).range(0.1..=5.0).speed(0.01).suffix(" mm").max_decimals(2))
                .on_hover_text("Grosor del trazo");
            sw_resp
        });
        let full = full.union(row.response.rect.expand2(egui::vec2(4.0, 1.0)));
        rows.push((i, full));
        let sw_resp = row.inner;
        if resp.clicked() || sw_resp.clicked() {
            app.active_pen = i;
            app.pen_focus = true;
        }
        if resp.double_clicked() || sw_resp.double_clicked() {
            app.edit_pen_color(i);
        }
        if resp.dragged() {
            dragging = Some(i);
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if resp.drag_stopped() {
            dropped = Some(i);
        }
        if active || dragging == Some(i) {
            let lift = dragging == Some(i);
            let fill = if lift {
                pal.accent.gamma_multiply(0.22)
            } else if app.pen_focus {
                pal.accent.gamma_multiply(0.16)
            } else {
                egui::Color32::TRANSPARENT
            };
            let stroke = egui::Stroke::new(1.0f32, pal.accent.gamma_multiply(if app.pen_focus || lift { 0.8 } else { 0.45 }));
            ui.painter().set(bg, egui::epaint::RectShape::new(full, egui::Rounding::same(theme::CONTROL_RADIUS), fill, stroke));
        }
        resp.on_hover_text("Arrastra para cambiar el orden · doble clic: color · ⌫: eliminar").context_menu(|ui| {
            if ui.button("Cambiar color…").clicked() {
                app.edit_pen_color(i);
                ui.close_menu();
            }
            if ui.button("Renombrar").clicked() {
                app.renaming_pen = Some(i);
                ui.close_menu();
            }
            if ui.button("Usar para dibujar").clicked() {
                app.active_pen = i;
                ui.close_menu();
            }
            ui.separator();
            if i + 1 < n && ui.button("Subir (se dibuja después, queda encima)").clicked() {
                action = Some((i, i + 1));
                ui.close_menu();
            }
            if i > 0 && ui.button("Bajar (se dibuja antes, queda debajo)").clicked() {
                action = Some((i, i - 1));
                ui.close_menu();
            }
            if n > 1 && ui.button("Eliminar").clicked() {
                action = Some((i, usize::MAX));
                ui.close_menu();
            }
        });
    }
    // Arrastre: hueco de inserción según la posición del puntero entre las filas.
    if let Some(from) = dragging.or(dropped) {
        if let Some(y) = ui.ctx().pointer_latest_pos().map(|p| p.y) {
            // `slot` = número de filas (de arriba abajo) por encima del puntero.
            let slot = rows.iter().filter(|(_, r)| r.center().y < y).count();
            let from_row = n - 1 - from;
            let to_row = if slot > from_row { slot - 1 } else { slot };
            if dragging.is_some() && to_row != from_row {
                let line_y = match rows.get(slot) {
                    Some((_, r)) => r.top() - 1.0,
                    None => rows.last().map(|(_, r)| r.bottom() + 1.0).unwrap_or(y),
                };
                let x = rows.first().map(|(_, r)| r.x_range()).unwrap_or(ui.max_rect().x_range());
                ui.painter().hline(x, line_y, egui::Stroke::new(2.0f32, pal.accent));
                ui.painter().circle_filled(egui::pos2(x.min + 2.0, line_y), 3.0, pal.accent);
            }
            if dropped.is_some() && to_row != from_row {
                action = Some((from, n - 1 - to_row));
            }
        }
    }
    match action {
        Some((i, usize::MAX)) => {
            app.doc.remove_pen(i);
            app.active_pen = app.active_pen.min(app.doc.pens.len() - 1);
        }
        Some((from, to)) => {
            let new_of = app.doc.move_pen(from, to);
            let map = |p: usize| new_of.get(p).copied().unwrap_or(p);
            app.active_pen = map(app.active_pen);
            app.color_edit = app.color_edit.map(map);
            app.renaming_pen = app.renaming_pen.map(map);
            let rel = if to > from { "encima de" } else { "debajo de" };
            let other = &app.doc.pens[if to > from { to - 1 } else { to + 1 }].name;
            let msg = format!("{} se dibuja ahora {} {}; sus objetos también.", app.doc.pens[to].name, rel, other);
            app.notify(ui.ctx(), msg);
        }
        None => {}
    }
    ui.horizontal(|ui| {
        if ui.button("+ Rotulador").clicked() {
            app.doc.pens.push(Pen::new(&format!("Rotulador {}", app.doc.pens.len() + 1), [60, 60, 70], 0.8));
            let i = app.doc.pens.len() - 1;
            app.active_pen = i;
            app.pen_focus = true;
            app.edit_pen_color(i);
        }
        ui.label(RichText::new("arriba = encima").small().color(pal.secondary))
            .on_hover_text("El de arriba se dibuja el último y queda encima. Arrastra para reordenar; los objetos se reordenan con su rotulador.");
    });

    ui.add_space(6.0);
    theme::section(ui, "PAPEL (VISTA PREVIA)");
    ui.horizontal(|ui| {
        for (i, (name, c)) in PAPER_TINTS.iter().enumerate() {
            let r = theme::swatch(ui, *c, 20.0).on_hover_text(*name);
            if app.paper_tint == i {
                let pal = theme::current(ui.ctx());
                ui.painter().circle_stroke(r.rect.center(), 13.0, egui::Stroke::new(2.0f32, pal.accent));
            }
            if r.clicked() {
                app.paper_tint = i;
            }
        }
    });

    ui.add_space(6.0);
    ui.separator();
    theme::section(ui, "OBJETOS");
    if app.doc.items.is_empty() {
        ui.label(RichText::new("Todavía no hay nada en el papel.").color(theme::current(ui.ctx()).secondary));
    }
    let mut op: Option<(usize, i32)> = None;
    let mut clicked: Option<(u64, bool)> = None;
    let count = app.doc.items.len();
    for idx in (0..count).rev() {
        let id = app.doc.items[idx].id;
        let is_sel = app.is_selected(id);
        let it = &mut app.doc.items[idx];
        ui.horizontal(|ui| {
            if visibility_toggle(ui, it.visible).on_hover_text("Mostrar u ocultar").clicked() {
                it.visible = !it.visible;
            }
            let pens = &app.doc.pens;
            let color = it.stroke.or(Some(it.fill.pen)).and_then(|p| pens.get(p)).map(|p| p.color).unwrap_or([128, 128, 128]);
            theme::swatch(ui, color, 10.0);
            let mut text = RichText::new(&it.name);
            if !it.visible {
                text = text.weak();
            }
            let resp = ui.add(egui::SelectableLabel::new(is_sel, text));
            if resp.clicked() {
                clicked = Some((id, ui.input(|i| i.modifiers.shift || i.modifiers.command)));
            }
            if it.locked {
                ui.label(RichText::new("bloqueado").small().weak());
            }
            resp.context_menu(|ui| {
                if ui.button(if it.locked { "Desbloquear" } else { "Bloquear" }).clicked() {
                    it.locked = !it.locked;
                    ui.close_menu();
                }
                if idx + 1 < count && ui.button("Traer adelante").clicked() {
                    op = Some((idx, 1));
                    ui.close_menu();
                }
                if idx > 0 && ui.button("Enviar atrás").clicked() {
                    op = Some((idx, -1));
                    ui.close_menu();
                }
                if ui.button("Duplicar").clicked() {
                    op = Some((idx, 2));
                    ui.close_menu();
                }
                if ui.button("Eliminar").clicked() {
                    op = Some((idx, 0));
                    ui.close_menu();
                }
            });
        });
    }
    if let Some((id, add)) = clicked {
        app.pen_focus = false;
        if add {
            app.toggle_selected(id);
        } else {
            app.set_selection(vec![id]);
        }
    }
    match op {
        Some((i, 0)) => {
            let id = app.doc.items.remove(i).id;
            let left: Vec<u64> = app.selection().into_iter().filter(|x| *x != id).collect();
            app.set_selection(left);
        }
        Some((i, 2)) => {
            app.set_selection(vec![app.doc.items[i].id]);
            app.duplicate_selected();
        }
        Some((i, d)) => app.doc.items.swap(i, (i as i32 + d) as usize),
        None => {}
    }
}

// ── Derecha ─────────────────────────────────────────────────────────────────────

pub fn right_panel(app: &mut VectorioApp, ui: &mut Ui) {
    if app.selection().len() > 1 {
        multi_panel(app, ui);
        return;
    }
    let Some(id) = app.selected.filter(|id| app.doc.get(*id).is_some()) else {
        document_settings(app, ui);
        return;
    };
    let pens = app.doc.pens.clone();
    let focus = std::mem::take(&mut app.focus_text);
    let mut duplicate = false;
    let mut delete = false;
    let it = app.doc.get_mut(id).unwrap();
    let fillable = it.is_fillable();

    ui.add(egui::TextEdit::singleline(&mut it.name).font(egui::TextStyle::Heading).frame(false).desired_width(f32::INFINITY));
    ui.add_space(2.0);

    // Contenido específico.
    match &mut it.geometry {
        Geometry::Text(spec) => {
            theme::section(ui, "TEXTO MANUSCRITO");
            let te = ui.add(egui::TextEdit::multiline(&mut spec.text).desired_rows(3).desired_width(f32::INFINITY));
            if focus {
                te.request_focus();
            }
            row(ui, "Letra", |ui| {
                let before = spec.style;
                egui::ComboBox::from_id_salt("font-style").width(ui.available_width()).selected_text(spec.style.label()).show_ui(ui, |ui| {
                    for (i, s) in FontStyle::ALL.into_iter().enumerate() {
                        if i == 8 {
                            ui.separator();
                        }
                        ui.selectable_value(&mut spec.style, s, s.label());
                    }
                });
                // Las letras de Hershey ya traen su inclinación; la propia queda mejor algo inclinada.
                if spec.style != before && spec.style.is_builtin() != before.is_builtin() {
                    spec.slant_deg = if spec.style.is_builtin() { 8.0 } else { 0.0 };
                }
            });
            if spec.style.joins() {
                row(ui, "", |ui| {
                    ui.checkbox(&mut spec.connect, "Unir letras").on_hover_text("Cada palabra en un solo trazo, sin levantar el rotulador");
                });
            }
            row(ui, "Tamaño", |ui| {
                drag(ui, &mut spec.size_mm, 1.0..=200.0, 0.1, " mm").on_hover_text("Altura de las mayúsculas");
            });
            row(ui, "Inclinación", |ui| {
                ui.add(egui::Slider::new(&mut spec.slant_deg, -20.0..=30.0).suffix("°"));
            });
            if spec.style.has_jitter() {
                row(ui, "Pulso", |ui| {
                    ui.add(egui::Slider::new(&mut spec.jitter, 0.0..=2.0)).on_hover_text("Imperfecciones de la mano: tamaño, giro, línea base y temblor");
                });
            }
            row(ui, "Espaciado", |ui| {
                ui.add(egui::Slider::new(&mut spec.letter_spacing, -0.3..=1.0));
            });
            row(ui, "Interlínea", |ui| {
                ui.add(egui::Slider::new(&mut spec.line_height, 1.0..=3.0));
            });
            row(ui, "Alinear", |ui| {
                for (a, l) in [(Align::Left, "Izq."), (Align::Center, "Centro"), (Align::Right, "Der.")] {
                    ui.selectable_value(&mut spec.align, a, l);
                }
            });
            row(ui, "Ancho", |ui| {
                drag(ui, &mut spec.wrap_mm, 0.0..=1000.0, 0.5, " mm").on_hover_text("0 = sin ajuste de línea");
            });
            if ui.button("Otra variación de la letra").clicked() {
                spec.seed = spec.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 1;
            }
        }
        Geometry::Shape(shape) => {
            theme::section(ui, &shape.label().to_uppercase());
            shape_params(ui, shape);
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
    theme::section(ui, "CONTORNO");
    let mut has_stroke = it.stroke.is_some();
    ui.horizontal(|ui| {
        if ui.checkbox(&mut has_stroke, "").changed() {
            it.stroke = if has_stroke { Some(it.fill.pen) } else { None };
        }
        if let Some(pen) = it.stroke.as_mut() {
            pen_combo(ui, "stroke-pen", &pens, pen);
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
            row(ui, "Rotulador", |ui| {
                pen_combo(ui, "fill-pen", &pens, &mut fill.pen);
            });
            if fill.style != FillStyle::Solid {
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
        }
    });
    if !fillable {
        ui.label(RichText::new("Solo las figuras cerradas admiten relleno.").small().color(theme::current(ui.ctx()).secondary));
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
    if duplicate {
        app.duplicate_selected();
    }
    if delete {
        app.delete_selected();
    }
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
        "T · clic en el papel y escribe a mano",
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
    ui.label(RichText::new(format!("{} objetos", ids.len())).heading());
    ui.label(RichText::new("Lo que cambies aquí se aplica a todos. Arrástralos para moverlos juntos.").small().color(pal.secondary));
    let Some(first) = ids.iter().find_map(|id| app.doc.get(*id)).cloned() else { return };
    let fillable: Vec<u64> = ids.iter().copied().filter(|id| app.doc.get(*id).is_some_and(|it| it.is_fillable())).collect();

    ui.add_space(4.0);
    ui.separator();
    theme::section(ui, "CONTORNO");
    let mut stroke = first.stroke;
    let mut has = stroke.is_some();
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui.checkbox(&mut has, "").changed();
        if has {
            let mut pen = stroke.unwrap_or(first.fill.pen);
            if pen_combo(ui, "multi-stroke", &pens, &mut pen).changed() {
                changed = true;
            }
            stroke = Some(pen);
        } else {
            ui.label(RichText::new("Sin contorno").weak());
        }
    });
    if changed {
        for id in &ids {
            if let Some(it) = app.doc.get_mut(*id) {
                // Sin contorno solo si el objeto tiene relleno; si no, desaparecería.
                it.stroke = if has { stroke.or(Some(it.fill.pen)) } else if it.is_fillable() { None } else { it.stroke };
            }
        }
    }

    if !fillable.is_empty() {
        theme::section(ui, "RELLENO");
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
            if f.style != FillStyle::Solid {
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
}
