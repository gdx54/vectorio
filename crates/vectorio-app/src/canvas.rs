//! Lienzo: papel, dibujo con los colores reales de los rotuladores, selección con asas,
//! herramientas de dibujo y simulación del plotter. Todo se dibuja en la GPU (wgpu).

use crate::app::{Tool, VectorioApp, ViewMode, PAPER_TINTS};
use crate::inspector;
use crate::theme::{self, Palette};
use egui::{Color32, CursorIcon, Key, PointerButton, Pos2, Rect, Rounding, Sense, Shape as EShape, Stroke, Vec2};
use vectorio_core::geom::{chaikin, simplify};
use vectorio_core::{p, BBox, Geometry, Item, Placement, Polyline, Shape, TextSpec, P};

pub struct View {
    /// Píxeles (puntos de pantalla) por milímetro.
    pub zoom: f32,
    /// Posición en pantalla del origen del papel, relativa al lienzo.
    pub offset: Vec2,
    pub fitted: bool,
    /// Rectángulo del lienzo en el último fotograma.
    pub rect: Rect,
}

impl Default for View {
    fn default() -> Self {
        View { zoom: 2.0, offset: Vec2::ZERO, fitted: false, rect: Rect::NOTHING }
    }
}

impl View {
    /// Zoom alrededor del centro del lienzo.
    pub fn zoom_by(&mut self, k: f32) {
        let new_zoom = (self.zoom * k).clamp(0.15, 80.0);
        let k = new_zoom / self.zoom;
        let rel = if self.rect.is_positive() { self.rect.size() / 2.0 } else { Vec2::ZERO };
        self.offset = rel - (rel - self.offset) * k;
        self.zoom = new_zoom;
    }

    /// 100 % = tamaño real en una pantalla de 96 ppp.
    pub fn zoom_percent(&self) -> f32 {
        self.zoom * 25.4 / 96.0 * 100.0
    }
}

pub struct Sim {
    pub progress: f32,
    pub playing: bool,
    pub speed: f32,
    pub show_travel: bool,
}

impl Default for Sim {
    fn default() -> Self {
        Sim { progress: 1.0, playing: false, speed: 20.0, show_travel: true }
    }
}

#[derive(Default, Clone)]
pub enum Interaction {
    #[default]
    None,
    Pan,
    /// Mueve todos los objetos seleccionados (sin bloquear) a la vez.
    Move { start: P, orig: Vec<(u64, Placement)> },
    Scale { id: u64, center: P, start_d: f64, orig: Placement },
    Rotate { id: u64, center: P, start_a: f64, orig: Placement },
    Draw(Vec<P>),
    Box(P, P),
    TextBox(P, P),
    ResizeText { id: u64, start: P, width: f64, height: f64, horizontal: bool, vertical: bool },
    /// Recuadro de selección (mm, desde dónde y hasta dónde).
    Marquee(P, P),
}

struct Xf {
    origin: Pos2,
    zoom: f32,
}

impl Xf {
    fn s(&self, q: P) -> Pos2 {
        self.origin + Vec2::new(q.x as f32, q.y as f32) * self.zoom
    }
    fn mm(&self, s: Pos2) -> P {
        let v = (s - self.origin) / self.zoom;
        p(v.x as f64, v.y as f64)
    }
}

pub fn show(app: &mut VectorioApp, ui: &mut egui::Ui) {
    let pal = theme::current(ui.ctx());
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = resp.rect;
    app.view.rect = rect;
    if !app.view.fitted {
        fit(app, rect);
    }
    // Con la ventana Foto → dibujo abierta el lienzo solo se pinta.
    let modal = app.vec_ui.open;
    if !modal {
        navigate(app, ui, &resp);
    }
    let xf = Xf { origin: rect.min + app.view.offset, zoom: app.view.zoom };

    paint_paper(app, &painter, &xf, &pal);
    match app.mode {
        ViewMode::Design => {
            paint_items(app, &painter, &xf, rect);
            if !modal {
                tools(app, ui, &resp, &painter, &xf, &pal);
                pen_drop(app, ui, &painter, &xf, rect);
            }
        }
        ViewMode::Plotter => {
            simulate(app, ui);
            paint_plot(app, &painter, &xf, &pal);
        }
    }
    if app.doc.items.is_empty() && app.mode == ViewMode::Design {
        let c = xf.s(p(app.doc.paper.width_mm / 2.0, app.doc.paper.height_mm / 2.0));
        painter.text(
            c,
            egui::Align2::CENTER_CENTER,
            "Escribe (T), dibuja (P), añade formas (S) o arrastra aquí una foto o un SVG",
            egui::FontId::proportional(14.0),
            Color32::from_gray(150),
        );
    }
}

fn fit(app: &mut VectorioApp, rect: Rect) {
    let left = theme::INSET + inspector::LEFT_W + 28.0 + 16.0;
    let right = theme::INSET + inspector::RIGHT_W + 28.0 + 44.0;
    let usable = Rect::from_min_max(Pos2::new(rect.left() + left, rect.top() + 24.0), Pos2::new(rect.right() - right, rect.bottom() - 70.0));
    let (pw, ph) = (app.doc.paper.width_mm as f32, app.doc.paper.height_mm as f32);
    let zoom = (usable.width().max(100.0) / pw).min(usable.height().max(100.0) / ph);
    app.view.zoom = zoom.max(0.2);
    let size = Vec2::new(pw, ph) * app.view.zoom;
    app.view.offset = usable.center() - rect.min - size / 2.0;
    app.view.fitted = true;
}

fn navigate(app: &mut VectorioApp, ui: &egui::Ui, resp: &egui::Response) {
    let hover = resp.hover_pos();
    let (scroll, zoom_delta) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
    if let Some(pos) = hover {
        if zoom_delta != 1.0 {
            let rel = pos - resp.rect.min;
            let new_zoom = (app.view.zoom * zoom_delta).clamp(0.15, 80.0);
            let k = new_zoom / app.view.zoom;
            app.view.offset = rel - (rel - app.view.offset) * k;
            app.view.zoom = new_zoom;
        } else if scroll != Vec2::ZERO {
            app.view.offset += scroll;
        }
    }
    let space = !ui.ctx().wants_keyboard_input() && ui.input(|i| i.key_down(Key::Space));
    if resp.drag_started_by(PointerButton::Middle) || (space && resp.drag_started_by(PointerButton::Primary)) {
        app.interaction = Interaction::Pan;
    }
    if matches!(app.interaction, Interaction::Pan) {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        app.view.offset += resp.drag_delta();
        if resp.drag_stopped() || !ui.input(|i| i.pointer.any_down()) {
            app.interaction = Interaction::None;
        }
    } else if space && resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }
}

fn tint(app: &VectorioApp) -> Color32 {
    theme::pen_color(PAPER_TINTS[app.paper_tint.min(PAPER_TINTS.len() - 1)].1)
}

fn paint_paper(app: &VectorioApp, painter: &egui::Painter, xf: &Xf, pal: &Palette) {
    let paper = &app.doc.paper;
    let r = Rect::from_min_max(xf.s(P::default()), xf.s(p(paper.width_mm, paper.height_mm)));
    let shadow = egui::epaint::Shadow { offset: Vec2::new(0.0, 6.0), blur: 24.0, spread: 0.0, color: Color32::from_black_alpha(60) };
    painter.add(shadow.as_shape(r, Rounding::same(2.0)));
    painter.rect_filled(r, Rounding::same(2.0), tint(app));
    painter.rect_stroke(r, Rounding::same(2.0), Stroke::new(0.5f32, pal.glass_stroke));
    let m = paper.margin_mm;
    if m > 0.0 {
        let mr = Rect::from_min_max(xf.s(p(m, m)), xf.s(p(paper.width_mm - m, paper.height_mm - m)));
        let pts = [mr.left_top(), mr.right_top(), mr.right_bottom(), mr.left_bottom(), mr.left_top()];
        let col = if app.paper_tint >= 3 { Color32::from_white_alpha(50) } else { Color32::from_black_alpha(40) };
        painter.extend(EShape::dashed_line(&pts, Stroke::new(0.8f32, col), 5.0, 4.0));
    }
}

/// Convierte una polilínea a pantalla descartando puntos a menos de medio píxel.
fn screen_line(line: &[P], xf: &Xf, map: &dyn Fn(P) -> P) -> Vec<Pos2> {
    let mut out: Vec<Pos2> = Vec::with_capacity(line.len());
    for (i, q) in line.iter().enumerate() {
        let s = xf.s(map(*q));
        let last = i + 1 == line.len();
        match out.last() {
            Some(prev) if !last && prev.distance_sq(s) < 0.36 => {}
            _ => out.push(s),
        }
    }
    out
}

fn pen_stroke(app: &VectorioApp, pen: usize, zoom: f32) -> Stroke {
    let Some(pen) = app.doc.pens.get(pen) else {
        return Stroke::new(1.0f32, Color32::GRAY);
    };
    let c = pen.color;
    Stroke::new((pen.width_mm as f32 * zoom).max(0.8), Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 235))
}

fn add_line(shapes: &mut Vec<EShape>, pts: Vec<Pos2>, stroke: Stroke) {
    match pts.len() {
        0 => {}
        1 => shapes.push(EShape::circle_filled(pts[0], stroke.width / 2.0, stroke.color)),
        // Un círculo diminuto puede quedar reducido a sus extremos coincidentes.
        // Conservamos una marca de tinta en vez de enviar una línea de longitud cero.
        2 if pts[0].distance_sq(pts[1]) < 1e-6 => shapes.push(EShape::circle_filled(pts[0], stroke.width / 2.0, stroke.color)),
        _ => shapes.push(EShape::line(pts, stroke)),
    }
}

/// Dibuja cada objeto con sus rellenos (calculados en segundo plano). Mientras se mueve
/// o escala, el último cálculo se transforma al vuelo para que responda sin esperas.
fn paint_items(app: &VectorioApp, painter: &egui::Painter, xf: &Xf, clip: Rect) {
    let mut shapes = Vec::new();
    for it in app.doc.items.iter().filter(|i| i.visible) {
        let bb = it.bbox();
        if !bb.is_empty() {
            let sr = Rect::from_min_max(xf.s(bb.min), xf.s(bb.max)).expand(20.0);
            if !sr.intersects(clip) {
                continue;
            }
        }
        match app.rendered.items.get(&it.id) {
            Some(cache) => {
                let from = cache.snapshot.place;
                let to = it.place;
                let map = move |q: P| if from == to { q } else { to.apply(from.inverse(q)) };
                for (pen, lines) in &cache.parts {
                    let stroke = pen_stroke(app, *pen, xf.zoom);
                    for l in lines {
                        add_line(&mut shapes, screen_line(l, xf, &map), stroke);
                    }
                }
            }
            None => {
                let stroke = pen_stroke(app, it.stroke.unwrap_or(it.fill.pen), xf.zoom);
                for l in it.world_lines() {
                    add_line(&mut shapes, screen_line(&l, xf, &|q| q), stroke);
                }
            }
        }
    }
    painter.extend(shapes);
}

#[cfg(test)]
mod rendering_tests {
    use super::*;

    #[test]
    fn tiny_vectorized_dots_stay_visible_when_zoomed_out() {
        // Puntillismo importado: un círculo de radio 0,15 mm, visto al 39 %.
        let circle: Vec<P> = (0..=6).map(|i| p(0.15, 0.0).rotate(i as f64 / 6.0 * std::f64::consts::TAU)).collect();
        let xf = Xf { origin: Pos2::new(100.0, 100.0), zoom: 1.47 };
        let points = screen_line(&circle, &xf, &|q| q);
        let mut shapes = Vec::new();
        add_line(&mut shapes, points, Stroke::new(0.8f32, Color32::BLACK));
        assert!(matches!(shapes.as_slice(), [EShape::Circle(circle)] if circle.radius > 0.0));
    }
}

/// Una tinta arrastrada desde la paleta se puede soltar sobre un objeto del dibujo.
fn pen_drop(app: &mut VectorioApp, ui: &egui::Ui, painter: &egui::Painter, xf: &Xf, rect: Rect) {
    let Some(drag) = egui::DragAndDrop::payload::<inspector::PenDrag>(ui.ctx()).map(|d| *d) else { return };
    // Solo cuando el puntero está sobre el lienzo y no sobre un panel flotante.
    let Some(pos) = ui.ctx().pointer_latest_pos().filter(|p| rect.contains(*p) && ui.ctx().layer_id_at(*p) == Some(ui.layer_id())) else { return };
    let Some(id) = hit_item(app, xf.mm(pos), 6.0 / xf.zoom as f64) else { return };
    let alt = ui.input(|i| i.modifiers.alt);
    let (Some(fill), Some(pen), Some(it)) = (inspector::pen_drop_fill(app, id, alt), app.doc.pens.get(drag.pen), app.doc.get(id)) else { return };
    let bb = it.bbox();
    painter.rect_stroke(Rect::from_two_pos(xf.s(bb.min), xf.s(bb.max)).expand(4.0), Rounding::same(4.0), Stroke::new(2.0f32, theme::pen_color(pen.color)));
    inspector::set_drop_hint(ui.ctx(), inspector::drop_hint(app, id, fill));
    if ui.input(|i| i.pointer.any_released()) {
        app.doc.assign_pen(id, drag.pen, fill);
        egui::DragAndDrop::clear_payload(ui.ctx());
    }
}

fn hit_item(app: &VectorioApp, q: P, tol: f64) -> Option<u64> {
    app.doc.items.iter().rev().find(|it| it.visible && it.hit(q, tol)).map(|it| it.id)
}

/// Asas de la selección en pantalla: caja, esquina de escala y punto de giro.
fn handles(bb: &BBox, xf: &Xf) -> (Rect, Pos2, Pos2) {
    let r = Rect::from_min_max(xf.s(bb.min), xf.s(bb.max)).expand(6.0);
    (r, r.right_bottom(), Pos2::new(r.center().x, r.top() - 22.0))
}

fn tools(app: &mut VectorioApp, ui: &mut egui::Ui, resp: &egui::Response, painter: &egui::Painter, xf: &Xf, pal: &Palette) {
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    let tol = 6.0 / xf.zoom as f64;
    let panning = matches!(app.interaction, Interaction::Pan) || ui.input(|i| i.key_down(Key::Space)) && !ui.ctx().wants_keyboard_input();

    match app.tool {
        Tool::Draw | Tool::Shape => {
            if resp.hovered() && !panning {
                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
            }
        }
        Tool::Text => {
            if resp.hovered() && !panning {
                ui.ctx().set_cursor_icon(CursorIcon::Text);
            }
        }
        Tool::Select => {}
    }

    // Selección actual y sus asas (escalar y girar solo con un objeto).
    let selection = app.selection();
    let single = selection.len() <= 1;
    let sel = app.selected.filter(|_| single).and_then(|id| app.doc.get(id)).map(|it| (it.id, it.bbox(), it.place, it.locked));
    let add_mod = ui.input(|i| i.modifiers.shift || i.modifiers.command);
    let sel_handles = sel.as_ref().map(|(_, bb, _, _)| handles(bb, xf));
    let near = |a: Pos2, b: Option<Pos2>| b.is_some_and(|b| a.distance(b) < 9.0);

    // Inicio de gestos con el botón principal.
    if resp.drag_started_by(PointerButton::Primary) && !panning {
        app.pen_focus = false;
        if let Some(s) = pointer {
            let q = xf.mm(s);
            match app.tool {
                Tool::Select => {
                    let on_scale = sel_handles.map(|h| h.1);
                    let on_rot = sel_handles.map(|h| h.2);
                    match sel {
                        Some((id, _, place, false)) if app.doc.get(id).is_some_and(|it| matches!(&it.geometry, Geometry::Text(spec) if spec.is_frame()))
                            && sel_handles.is_some_and(|h| near(s, Some(h.0.right_center())) || near(s, Some(h.0.center_bottom())) || near(s, Some(h.1))) => {
                            let Geometry::Text(spec) = &app.doc.get(id).unwrap().geometry else { unreachable!() };
                            let h = sel_handles.unwrap();
                            app.interaction = Interaction::ResizeText { id, start: place.inverse(q), width: spec.wrap_mm, height: spec.frame_height_mm,
                                horizontal: !near(s, Some(h.0.center_bottom())), vertical: !near(s, Some(h.0.right_center())) };
                        }
                        Some((id, bb, place, false)) if near(s, on_scale) => {
                            let c = bb.center();
                            app.interaction = Interaction::Scale { id, center: c, start_d: q.dist(c).max(1e-6), orig: place };
                        }
                        Some((id, bb, place, false)) if near(s, on_rot) => {
                            let c = bb.center();
                            app.interaction = Interaction::Rotate { id, center: c, start_a: (q.y - c.y).atan2(q.x - c.x), orig: place };
                        }
                        _ => {
                            let target = match sel {
                                Some((id, bb, _, _)) if bb.contains(q, tol) && app.doc.get(id).is_some_and(|it| it.hit(q, tol * 2.0)) => Some(id),
                                _ => selection.iter().copied().find(|id| app.doc.get(*id).is_some_and(|it| it.hit(q, tol))).or_else(|| hit_item(app, q, tol)),
                            };
                            match target {
                                Some(id) => {
                                    if add_mod && !app.is_selected(id) {
                                        app.toggle_selected(id);
                                    } else if !app.is_selected(id) {
                                        app.set_selection(vec![id]);
                                    }
                                    let orig: Vec<(u64, Placement)> = app
                                        .selection()
                                        .into_iter()
                                        .filter_map(|id| app.doc.get(id))
                                        .filter(|it| !it.locked)
                                        .map(|it| (it.id, it.place))
                                        .collect();
                                    if !orig.is_empty() {
                                        app.interaction = Interaction::Move { start: q, orig };
                                    }
                                }
                                None => {
                                    if !add_mod {
                                        app.set_selection(vec![]);
                                    }
                                    app.interaction = Interaction::Marquee(q, q);
                                }
                            }
                        }
                    }
                }
                Tool::Draw => app.interaction = Interaction::Draw(vec![q]),
                Tool::Shape => app.interaction = Interaction::Box(q, q),
                Tool::Text => {
                    let start = ui.input(|i| i.pointer.press_origin()).map(|s| xf.mm(s)).unwrap_or(q);
                    app.interaction = Interaction::TextBox(start, q);
                }
            }
        }
    }

    // Gesto en curso.
    if let Some(s) = pointer {
        let q = xf.mm(s);
        let shift = ui.input(|i| i.modifiers.shift);
        match &mut app.interaction {
            Interaction::Move { start, orig } => {
                let mut d = q - *start;
                if shift {
                    if d.x.abs() > d.y.abs() {
                        d.y = 0.0
                    } else {
                        d.x = 0.0
                    }
                }
                let orig = std::mem::take(orig);
                for (id, place) in &orig {
                    if let Some(it) = app.doc.get_mut(*id) {
                        it.place.pos = place.pos + d;
                    }
                }
                if let Interaction::Move { orig: o, .. } = &mut app.interaction {
                    *o = orig;
                }
            }
            Interaction::Scale { id, center, start_d, orig } => {
                let k = (q.dist(*center) / *start_d).clamp(0.01, 100.0);
                let (id, c, orig) = (*id, *center, *orig);
                if let Some(it) = app.doc.get_mut(id) {
                    it.place.scale = orig.scale * k;
                    it.place.pos = c + (orig.pos - c) * k;
                }
            }
            Interaction::ResizeText { id, start, width, height, horizontal, vertical } => {
                if let Some(it) = app.doc.get_mut(*id) {
                    let delta = it.place.inverse(q) - *start;
                    if let Geometry::Text(spec) = &mut it.geometry {
                        if *horizontal { spec.wrap_mm = (*width + delta.x).clamp(15.0, 2000.0); }
                        if *vertical { spec.frame_height_mm = (*height + delta.y).clamp(spec.size_mm * 1.4 + spec.padding() * 2.0, 2000.0); }
                    }
                }
            }
            Interaction::Rotate { id, center, start_a, orig } => {
                let mut da = (q.y - center.y).atan2(q.x - center.x) - *start_a;
                if shift {
                    let step = 15f64.to_radians();
                    da = (da / step).round() * step;
                }
                let (id, c, orig) = (*id, *center, *orig);
                if let Some(it) = app.doc.get_mut(id) {
                    it.place.rotation_deg = (orig.rotation_deg + da.to_degrees()).rem_euclid(360.0);
                    it.place.pos = c + (orig.pos - c).rotate(da);
                }
            }
            Interaction::Draw(pts) => {
                if pts.last().is_none_or(|l| l.dist(q) > 0.15) {
                    pts.push(q);
                }
            }
            Interaction::Box(_, b) | Interaction::TextBox(_, b) | Interaction::Marquee(_, b) => *b = q,
            _ => {}
        }
    }

    // Fin del gesto.
    let released = resp.drag_stopped() || !ui.input(|i| i.pointer.any_down());
    if released {
        match std::mem::take(&mut app.interaction) {
            Interaction::Draw(pts) => finish_draw(app, pts),
            Interaction::Box(a, b) => finish_shape(app, a, b),
            Interaction::TextBox(a, b) => finish_text(app, a, Some(b)),
            Interaction::Marquee(a, b) => {
                let area = BBox { min: p(a.x.min(b.x), a.y.min(b.y)), max: p(a.x.max(b.x), a.y.max(b.y)) };
                let mut ids = if add_mod { app.selection() } else { vec![] };
                for it in app.doc.items.iter().filter(|it| it.visible) {
                    let bb = it.bbox();
                    let touches = !bb.is_empty() && bb.min.x <= area.max.x && bb.max.x >= area.min.x && bb.min.y <= area.max.y && bb.max.y >= area.min.y;
                    if touches && !ids.contains(&it.id) {
                        ids.push(it.id);
                    }
                }
                app.set_selection(ids);
            }
            Interaction::Pan => {}
            other @ (Interaction::Move { .. } | Interaction::Scale { .. } | Interaction::Rotate { .. } | Interaction::ResizeText { .. }) => {
                if !resp.drag_stopped() && ui.input(|i| i.pointer.any_down()) {
                    app.interaction = other;
                }
            }
            Interaction::None => {}
        }
    }

    // Clic simple.
    if resp.clicked_by(PointerButton::Primary) && !panning {
        if let Some(s) = pointer {
            let q = xf.mm(s);
            match app.tool {
                Tool::Select => {
                    app.pen_focus = false;
                    match hit_item(app, q, tol) {
                        Some(id) if add_mod => app.toggle_selected(id),
                        Some(id) => app.set_selection(vec![id]),
                        None if add_mod => {}
                        None => app.set_selection(vec![]),
                    }
                }
                Tool::Text => {
                    if let Some(id) = hit_item(app, q, tol).filter(|id| app.doc.get(*id).is_some_and(|it| matches!(it.geometry, Geometry::Text(_)))) {
                        app.set_selection(vec![id]); app.focus_text = true; app.tool = Tool::Select;
                    } else { finish_text(app, q, None); }
                }
                Tool::Shape => finish_shape(app, q, q),
                Tool::Draw => {}
            }
        }
    }
    if resp.double_clicked() && app.tool == Tool::Select {
        if app.selected_item().is_some_and(|it| matches!(it.geometry, Geometry::Text(_))) {
            app.focus_text = true;
        }
    }

    // Resaltado al pasar por encima y selección.
    if app.tool == Tool::Select && matches!(app.interaction, Interaction::None) {
        if let Some(s) = resp.hover_pos() {
            if let Some(it) = hit_item(app, xf.mm(s), tol).filter(|id| !app.is_selected(*id)).and_then(|id| app.doc.get(id)) {
                let st = Stroke::new(1.5f32, pal.accent.gamma_multiply(0.8));
                let shapes: Vec<EShape> = it.world_lines().iter().map(|l| EShape::line(screen_line(l, xf, &|q| q), st)).collect();
                painter.extend(shapes);
            }
        }
    }
    if !single {
        // Varios objetos: un recuadro fino por objeto y uno común alrededor.
        let mut all = BBox::empty();
        let thin = Stroke::new(1.0f32, pal.accent.gamma_multiply(0.55));
        for it in selection.iter().filter_map(|id| app.doc.get(*id)) {
            let bb = it.bbox();
            if bb.is_empty() {
                continue;
            }
            painter.rect_stroke(Rect::from_min_max(xf.s(bb.min), xf.s(bb.max)).expand(2.0), Rounding::same(2.0), thin);
            all.union(&bb);
        }
        if !all.is_empty() {
            painter.rect_stroke(Rect::from_min_max(xf.s(all.min), xf.s(all.max)).expand(6.0), Rounding::same(3.0), Stroke::new(1.2f32, pal.accent));
        }
    }
    if let Some(it) = app.selected_item().filter(|_| single) {
        let (r, scale_h, rot_h) = handles(&it.bbox(), xf);
        let acc = pal.accent;
        painter.rect_stroke(r, Rounding::same(3.0), Stroke::new(1.2f32, acc));
        if !it.locked {
            painter.line_segment([Pos2::new(r.center().x, r.top()), rot_h], Stroke::new(1.0f32, acc));
            for (h, round) in [(scale_h, false), (rot_h, true)] {
                if round {
                    painter.circle_filled(h, 5.0, Color32::WHITE);
                    painter.circle_stroke(h, 5.0, Stroke::new(1.5f32, acc));
                } else {
                    let hr = Rect::from_center_size(h, Vec2::splat(9.0));
                    painter.rect_filled(hr, Rounding::same(2.0), Color32::WHITE);
                    painter.rect_stroke(hr, Rounding::same(2.0), Stroke::new(1.5f32, acc));
                }
            }
            if let Geometry::Text(spec) = &it.geometry {
                if spec.is_frame() {
                    for handle in [r.right_center(), r.center_bottom()] {
                        let rect = Rect::from_center_size(handle, Vec2::splat(8.0));
                        painter.rect_filled(rect, 1.0, Color32::WHITE);
                        painter.rect_stroke(rect, 1.0, Stroke::new(1.2f32, acc));
                    }
                    let pages = vectorio_core::font::compose(spec).page_count;
                    if pages > 1 {
                        painter.text(r.left_bottom() + Vec2::new(0.0, 16.0), egui::Align2::LEFT_TOP,
                            format!("Página {} de {}", spec.page.min(pages - 1) + 1, pages), egui::FontId::proportional(12.0), acc);
                    }
                    if let Some(s) = resp.hover_pos() {
                        if near(s, Some(r.right_center())) { ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal); }
                        else if near(s, Some(r.center_bottom())) { ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical); }
                    }
                }
            }
            if let Some(s) = resp.hover_pos() {
                if s.distance(scale_h) < 9.0 {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe);
                } else if s.distance(rot_h) < 9.0 {
                    ui.ctx().set_cursor_icon(CursorIcon::Alias);
                }
            }
        }
    }

    // Vista previa de la herramienta.
    match &app.interaction {
        Interaction::Draw(pts) => {
            let st = pen_stroke(app, app.active_pen, xf.zoom);
            painter.add(EShape::line(screen_line(pts, xf, &|q| q), st));
        }
        Interaction::Box(a, b) => {
            let (shape, pos) = boxed_shape(app, *a, *b);
            let pl = Placement { pos, ..Default::default() };
            let line: Polyline = shape.outline().into_iter().map(|q| pl.apply(q)).collect();
            painter.add(EShape::line(screen_line(&line, xf, &|q| q), pen_stroke(app, app.active_pen, xf.zoom)));
            let r = Rect::from_two_pos(xf.s(*a), xf.s(*b));
            painter.rect_stroke(r, Rounding::ZERO, Stroke::new(1.0f32, pal.accent.gamma_multiply(0.5)));
        }
        Interaction::Marquee(a, b) => {
            let r = Rect::from_two_pos(xf.s(*a), xf.s(*b));
            painter.rect_filled(r, Rounding::same(2.0), pal.accent.gamma_multiply(0.08));
            painter.rect_stroke(r, Rounding::same(2.0), Stroke::new(1.0f32, pal.accent.gamma_multiply(0.7)));
        }
        Interaction::TextBox(a, b) => {
            painter.rect_stroke(Rect::from_two_pos(xf.s(*a), xf.s(*b)), 1.0, Stroke::new(1.5f32, pal.accent));
        }
        _ => {}
    }
}

fn finish_text(app: &mut VectorioApp, start: P, end: Option<P>) {
    let paper = &app.doc.paper;
    let (pos, width, height) = match end {
        Some(end) => (p(start.x.min(end.x), start.y.min(end.y)), (end.x - start.x).abs().max(20.0), (end.y - start.y).abs().max(15.0)),
        None => (start, (paper.width_mm - paper.margin_mm - start.x).clamp(50.0, 120.0), (paper.height_mm - paper.margin_mm - start.y).clamp(30.0, 90.0)),
    };
    let mut item = Item::new("Caja de texto", Geometry::Text(TextSpec {
        text: String::new(), size_mm: 4.0, wrap_mm: width, frame_height_mm: height,
        padding_mm: 2.0, hyphenate: true, seed: app.doc.next_id, ..Default::default()
    }), pos);
    item.stroke = Some(app.active_pen);
    item.fill.pen = app.active_pen;
    app.add_item(item);
    app.focus_text = true;
    app.tool = Tool::Select;
}

fn finish_draw(app: &mut VectorioApp, pts: Vec<P>) {
    if pts.len() < 2 {
        return;
    }
    let closed = pts.len() > 8 && pts[0].dist(*pts.last().unwrap()) < 3.0 && vectorio_core::geom::length(&pts) > 15.0;
    let mut line = simplify(&pts, 0.2);
    if closed {
        line.pop();
    }
    let mut line = chaikin(&line, 2, closed);
    if closed {
        line.push(line[0]);
    }
    let bb = BBox::of(std::iter::once(&line));
    let c = bb.center();
    let local: Polyline = line.into_iter().map(|q| q - c).collect();
    let mut item = Item::new("Trazo", Geometry::Path { lines: vec![local], closed }, c);
    item.stroke = Some(app.active_pen);
    item.fill.pen = app.active_pen;
    app.add_item(item);
}

fn boxed_shape(app: &VectorioApp, a: P, b: P) -> (Shape, P) {
    let presets = Shape::presets();
    let mut shape = presets[app.shape_kind.min(presets.len() - 1)].clone();
    let (w, h) = ((b.x - a.x).abs(), (b.y - a.y).abs());
    if w.max(h) < 2.0 {
        return (shape, a);
    }
    shape.fit(w, h);
    (shape, a.lerp(b, 0.5))
}

fn finish_shape(app: &mut VectorioApp, a: P, b: P) {
    let (shape, pos) = boxed_shape(app, a, b);
    let mut item = Item::new(shape.label(), Geometry::Shape(shape), pos);
    item.stroke = Some(app.active_pen);
    item.fill.pen = app.active_pen;
    app.add_item(item);
    app.tool = Tool::Select;
}

// ── Plotter ─────────────────────────────────────────────────────────────────────

fn simulate(app: &mut VectorioApp, ui: &egui::Ui) {
    if !app.sim.playing {
        return;
    }
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    let total = app.rendered.stats.seconds.max(1.0) as f32;
    app.sim.progress += dt * app.sim.speed / total;
    if app.sim.progress >= 1.0 {
        app.sim.progress = 1.0;
        app.sim.playing = false;
    }
    ui.ctx().request_repaint();
}

/// Dibuja las capas de plotter en orden real hasta el progreso indicado, con los
/// desplazamientos en vacío y la punta del rotulador.
fn paint_plot(app: &VectorioApp, painter: &egui::Painter, xf: &Xf, pal: &Palette) {
    let total = app.rendered.stats.draw_mm.max(1e-6);
    let mut budget = app.sim.progress as f64 * total;
    let mut shapes = Vec::new();
    let mut travel = Vec::new();
    let mut cur = P::default();
    let mut tip: Option<(P, usize)> = None;
    let travel_col = if app.paper_tint >= 3 { Color32::from_white_alpha(70) } else { pal.accent.gamma_multiply(0.45) };
    'outer: for layer in &app.rendered.layers {
        let stroke = pen_stroke(app, layer.pen, xf.zoom);
        for path in &layer.paths {
            if budget <= 0.0 {
                break 'outer;
            }
            if app.sim.show_travel && cur.dist(path[0]) > 0.3 {
                travel.push(EShape::line_segment([xf.s(cur), xf.s(path[0])], Stroke::new(0.7f32, travel_col)));
            }
            let len = vectorio_core::geom::length(path);
            if len <= budget {
                add_line(&mut shapes, screen_line(path, xf, &|q| q), stroke);
                budget -= len;
                cur = *path.last().unwrap();
                tip = Some((cur, layer.pen));
            } else {
                let mut part = vec![path[0]];
                for w in path.windows(2) {
                    let d = w[0].dist(w[1]);
                    if d >= budget {
                        part.push(w[0].lerp(w[1], budget / d.max(1e-9)));
                        break;
                    }
                    budget -= d;
                    part.push(w[1]);
                }
                cur = *part.last().unwrap();
                tip = Some((cur, layer.pen));
                add_line(&mut shapes, screen_line(&part, xf, &|q| q), stroke);
                break 'outer;
            }
        }
    }
    painter.extend(travel);
    painter.extend(shapes);
    if app.sim.progress < 1.0 {
        if let Some((q, pen)) = tip {
            let s = xf.s(q);
            let c = app.doc.pens.get(pen).map(|p| theme::pen_color(p.color)).unwrap_or(Color32::GRAY);
            painter.circle_filled(s, 6.0, c);
            painter.circle_stroke(s, 6.0, Stroke::new(2.0f32, Color32::WHITE));
            if let Some(pen) = app.doc.pens.get(pen) {
                painter.text(s + Vec2::new(10.0, -10.0), egui::Align2::LEFT_BOTTOM, &pen.name, egui::FontId::proportional(11.0), pal.secondary);
            }
        }
    }
}
