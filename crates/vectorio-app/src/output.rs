//! Espacio de salida: análisis físico, planificación de secado y simulación temporal.
use crate::{app::{Rendered, VectorioApp}, canvas::Sim, theme};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::sync::{mpsc, Arc};
use vectorio_core::{output::{self, HeatGrid, OutputPlan, OutputSettings, OutputStep}, Document, P};

#[derive(Clone, PartialEq)]
struct Key { rev: u64, settings: OutputSettings }

#[derive(Clone, Copy, PartialEq, Default)]
enum Map { #[default] Total, Before, After, Route }

#[derive(Default)]
struct HeatSummary { max: u16, inked: usize, above: usize, levels: [usize; 6] }

struct PenSpan { start_s: f64, end_s: f64, pen: Option<usize> }

pub struct OutputUi {
    key: Option<Key>,
    job: Option<(Key, mpsc::Receiver<Result<OutputPlan, String>>)>,
    pub plan: Option<Arc<OutputPlan>>,
    error: Option<String>,
    heat_meshes: Vec<egui::Mesh>,
    heat_view: Option<(Pos2, f32, usize, egui::Mesh)>,
    summaries: Vec<HeatSummary>,
    timeline: Option<egui::TextureHandle>,
    timeline_dark: bool,
    pen_spans: Vec<PenSpan>,
    map: Map,
    show_drawing: bool,
    show_travel: bool,
    zoom: f32,
    offset: Vec2,
    fitted: bool,
    viewport: Vec2,
}

impl Default for OutputUi {
    fn default() -> Self {
        Self { key: None, job: None, plan: None, error: None, heat_meshes: vec![], heat_view: None, summaries: vec![], timeline: None, timeline_dark: false, pen_spans: vec![], map: Map::Total,
            show_drawing: false, show_travel: true, zoom: 1.0, offset: Vec2::ZERO,
            fitted: false, viewport: Vec2::ZERO }
    }
}

pub fn duration(seconds: f64) -> String {
    let s = seconds.ceil().max(0.0) as u64;
    if s >= 3600 { format!("{} h {:02} min", s / 3600, s / 60 % 60) }
    else if s >= 60 { format!("{} min {:02} s", s / 60, s % 60) }
    else { format!("{s} s") }
}
fn distance(mm: f64) -> String {
    if mm >= 1000.0 { format!("{:.1} m", mm / 1000.0).replace('.', ",") } else { format!("{mm:.0} mm") }
}
fn pct(n: usize, total: usize) -> String {
    format!("{:.1} %", n as f64 * 100.0 / total.max(1) as f64).replace('.', ",")
}
fn heat_color(i: usize) -> Color32 {
    let [r, g, b, a] = output::HEAT_COLORS[i]; Color32::from_rgba_unmultiplied(r, g, b, a)
}

impl OutputUi {
    pub fn ready(&self, rev: u64, cfg: &OutputSettings) -> bool {
        self.plan.is_some() && self.job.is_none() && self.key.as_ref() == Some(&Key { rev, settings: cfg.clone() })
    }
    fn update(&mut self, ctx: &egui::Context, doc: &Document, rendered: &Rendered, current: bool, sim: &mut Sim) {
        if let Some((_, rx)) = &self.job {
            match rx.try_recv() {
                Ok(result) => {
                    self.key = self.job.take().map(|j| j.0);
                    self.error = result.as_ref().err().cloned();
                    self.plan = result.ok().map(Arc::new);
                    self.heat_meshes.clear();
                    self.heat_view = None;
                    self.summaries.clear();
                    self.timeline = None;
                    self.pen_spans.clear();
                    if let Some(plan) = &self.plan {
                        for grid in [&plan.total_heat, &plan.wet_before, &plan.wet_after] {
                            self.heat_meshes.push(heat_mesh(grid));
                            self.summaries.push(HeatSummary { max: grid.max(), inked: grid.inked(),
                                above: grid.at_least(plan.settings.max_wet_passes.saturating_add(1)), levels: grid.levels() });
                        }
                        self.timeline = Some(ctx.load_texture("output-timeline", timeline_image(ctx, plan), egui::TextureOptions::NEAREST));
                        self.timeline_dark = ctx.style().visuals.dark_mode;
                        self.pen_spans = pen_spans(plan);
                    }
                    sim.progress = 1.0; sim.playing = false;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.key = self.job.take().map(|j| j.0);
                    self.error = Some("El análisis no ha terminado. Pulsa Recalcular para intentarlo de nuevo.".into());
                    self.plan = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.timeline_dark != ctx.style().visuals.dark_mode {
            self.timeline_dark = ctx.style().visuals.dark_mode;
            if let Some(plan) = &self.plan {
                if let Some(timeline) = &mut self.timeline { timeline.set(timeline_image(ctx, plan), egui::TextureOptions::NEAREST); }
            }
            ctx.request_repaint();
        }
        let key = Key { rev: rendered.rev, settings: doc.output.clone() };
        if current && self.job.is_none() && self.key.as_ref() != Some(&key) {
            let (tx, rx) = mpsc::channel();
            let (layers, paper, pens, motion, cfg, ctx) =
                (rendered.layers.clone(), doc.paper.clone(), doc.pens.clone(), doc.plot.clone(), doc.output.clone(), ctx.clone());
            std::thread::spawn(move || {
                let result = output::prepare(&layers, &paper, &pens, &motion, &cfg);
                let _ = tx.send(result); ctx.request_repaint();
            });
            self.job = Some((key, rx)); self.error = None; sim.playing = false;
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui, doc: &mut Document, sim: &mut Sim) {
        let pal = theme::current(ui.ctx());
        theme::section(ui, "SECADO POR ZONAS");
        ui.checkbox(&mut doc.output.drying, "Proteger las zonas húmedas");
        ui.add_enabled_ui(doc.output.drying, |ui| {
            ui.label("Tiempo de secado por pasada");
            ui.add(egui::Slider::new(&mut doc.output.dry_seconds, 1.0..=180.0).suffix(" s"));
            ui.label("Máximo de pasadas aún húmedas");
            ui.add(egui::Slider::new(&mut doc.output.max_wet_passes, 1..=10));
            ui.checkbox(&mut doc.output.reorder, "Alternar zonas mientras seca");
            ui.label("Tramo máximo al proteger");
            ui.add(egui::Slider::new(&mut doc.output.chunk_mm, 5.0..=100.0).suffix(" mm"))
                .on_hover_text("Cuando hace falta proteger una zona, divide trazos largos para alternar o esperar con la pluma levantada. Las rutas que ya cumplen el límite se conservan intactas.");
        });
        ui.add_space(8.0);
        theme::section(ui, "HUELLA DE LA TINTA");
        ui.label("Empape alrededor del trazo");
        ui.add(egui::Slider::new(&mut doc.output.soak_mm, 0.0..=2.0).max_decimals(2).suffix(" mm"))
            .on_hover_text("Como en Linear: se añade al radio físico del rotulador al calcular qué zonas se pisan.");
        ui.label(RichText::new("El grosor se toma de cada tinta del documento.").small().color(pal.secondary));
        ui.add_space(8.0);
        egui::CollapsingHeader::new("Tiempos y precisión").show(ui, |ui| {
            setting(ui, "Dibujar", &mut doc.plot.draw_speed, 1.0..=200.0, " mm/s");
            setting(ui, "Viajar", &mut doc.plot.travel_speed, 1.0..=400.0, " mm/s");
            setting(ui, "Subir/bajar", &mut doc.plot.lift_time, 0.0..=3.0, " s");
            setting(ui, "Pausa al cambiar tinta", &mut doc.output.pen_change_seconds, 0.0..=300.0, " s");
            setting(ui, "Celda del mapa", &mut doc.output.cell_mm, 0.1..=2.0, " mm");
            ui.label(RichText::new("Cambios manuales con esta pausa mínima prevista. Se conserva el orden de la paleta.").small().color(pal.secondary));
        });
        self.details(ui, sim);
        ui.add_space(8.0);
        ui.label(RichText::new("Ajusta secado y empape con una prueba de tu papel y rotuladores.").small().color(pal.secondary))
            .on_hover_text("Estimación conservadora: cada huella se considera húmeda hasta que seca el final del trazo.");
    }

    fn actions(&mut self, ui: &mut egui::Ui, doc: &Document, ready: bool) {
        let pal = theme::current(ui.ctx());
        ui.separator();
        ui.add_space(6.0);
        if ui.button("Recalcular salida").clicked() { self.key = None; }
        let has_drawing = self.plan.as_ref().is_some_and(|p| p.stats.strokes > 0);
        ui.add_enabled_ui(ready && has_drawing, |ui| {
            if theme::accent_button(ui, "Guardar plan con esperas…").clicked() { self.export_plan(); }
            if ui.button("Exportar SVG de la ruta…").clicked() { self.export_svg(doc); }
        });
        ui.label(RichText::new("JSON: ruta y pausas. SVG: recorrido sin esperas.").small().color(pal.secondary));
        if let Some(error) = &self.error { ui.colored_label(pal.text, error); }
    }

    pub fn export_plan(&mut self) {
        let Some(plan) = &self.plan else { return };
        let Some(path) = rfd::FileDialog::new().set_title("Guardar plan de salida con secado")
            .add_filter("Plan de salida Vectorio", &["json"]).set_file_name("vectorio-salida.json").save_file() else { return };
        let result = serde_json::to_vec_pretty(plan.as_ref()).map_err(|e| e.to_string())
            .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()));
        self.error = result.err().map(|e| format!("No se pudo guardar el plan: {e}"));
    }
    pub fn export_svg(&mut self, doc: &Document) {
        let Some(plan) = &self.plan else { return };
        let Some(path) = rfd::FileDialog::new().add_filter("SVG", &["svg"])
            .set_file_name("vectorio-ruta.svg").save_file() else { return };
        let svg = vectorio_core::svg_out::export(doc, &plan.layers());
        self.error = std::fs::write(path, svg).err().map(|e| format!("No se pudo exportar: {e}"));
    }

    fn summary(&self, ui: &mut egui::Ui, doc: &Document, ready: bool) {
        let pal = theme::current(ui.ctx());
        ui.label(RichText::new("Salida").size(17.0).strong().color(pal.text));
        ui.label(RichText::new(format!("EasyDraw · {}", doc.paper.name)).small().color(pal.secondary))
            .on_hover_text(format!("{:.0} × {:.0} mm · preparación sin conexión al plotter", doc.paper.width_mm, doc.paper.height_mm));
        if let Some(plan) = &self.plan {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(duration(plan.stats.total_seconds)).size(21.0).strong().color(pal.text));
                ui.label(RichText::new("estimados").small().color(pal.secondary));
            });
            egui::Grid::new("output-summary-grid").num_columns(2).spacing([16.0, 3.0]).show(ui, |ui| {
                for (label, value) in [("Dibujo", distance(plan.stats.draw_mm)), ("Viajes", distance(plan.stats.travel_mm)),
                    ("Secado", format!("{} · {} pausas", duration(plan.stats.wait_seconds), plan.stats.waits)),
                    ("Cambios de tinta", plan.stats.pen_changes.to_string())] {
                    ui.label(RichText::new(label).small().color(pal.secondary));
                    ui.label(RichText::new(value).small().color(pal.text)); ui.end_row();
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            if !ready && self.error.is_none() { ui.spinner(); ui.label("Calculando huella y recorrido…"); }
            else if let Some(plan) = &self.plan {
                let limit = plan.settings.max_wet_passes;
                if self.summaries.len() == 3 {
                    ui.label(RichText::new(format!("Húmedo: {} → {} pasadas\nSobre el límite de {limit}: {} → {}",
                        self.summaries[1].max, self.summaries[2].max,
                        pct(self.summaries[1].above, self.summaries[0].inked),
                        pct(self.summaries[2].above, self.summaries[0].inked))).small().color(pal.secondary));
                }
                if plan.stats.outside_points > 0 { ui.label(RichText::new("Hay trazos fuera del papel").strong().color(pal.text)); }
            }
        });
        ui.add_space(6.0);
        ui.separator();
    }

    fn playback(&mut self, ui: &mut egui::Ui, sim: &mut Sim, ready: bool) {
        let Some(plan) = self.plan.clone() else { return };
        let total = plan.stats.total_seconds;
        let pal = theme::current(ui.ctx());
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(ready && total > 0.0, egui::Button::new(if sim.playing { "Pausar" } else { "Simular salida" })).clicked() {
                if sim.progress >= 1.0 { sim.progress = 0.0; }
                sim.playing = !sim.playing; self.map = Map::Route;
            }
            ui.add_enabled_ui(ready, |ui| {
                ui.scope(|ui| { ui.spacing_mut().slider_width = 110.0; ui.add(egui::Slider::new(&mut sim.progress, 0.0..=1.0).show_value(false)); });
                egui::ComboBox::from_id_salt("output-speed").selected_text(format!("×{:.0}", sim.speed)).width(62.0).show_ui(ui, |ui| {
                    for v in [1.0, 5.0, 20.0, 60.0, 200.0] { ui.selectable_value(&mut sim.speed, v, format!("×{v:.0}")); }
                });
            });
            ui.label(format!("{} / {}", duration(sim.progress as f64 * total), duration(total)));
            ui.checkbox(&mut self.show_travel, "Ver viajes");
        });
        let (response, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 82.0), Sense::click_and_drag());
        let r = response.rect;
        let route = Rect::from_min_max(Pos2::new(r.left() + 62.0, r.top() + 10.0), Pos2::new(r.right(), r.top() + 34.0));
        let inks = Rect::from_min_max(Pos2::new(route.left(), r.top() + 42.0), Pos2::new(route.right(), r.top() + 60.0));
        let time_x = |t: f64| route.left() + route.width() * (t / total.max(1e-9)) as f32;
        for (label, track) in [("Acciones", route), ("Tintas", inks)] {
            painter.text(Pos2::new(r.left(), track.center().y), egui::Align2::LEFT_CENTER, label,
                egui::FontId::proportional(11.0), pal.secondary);
            painter.rect_filled(track, 0.0, pal.control_hover);
        }
        if let Some(texture) = &self.timeline { painter.image(texture.id(), route, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
        let ink_painter = painter.with_clip_rect(inks);
        for span in &self.pen_spans {
            let band = Rect::from_min_max(Pos2::new(time_x(span.start_s), inks.top()), Pos2::new(time_x(span.end_s), inks.bottom()));
            let (color, label) = span.pen.map_or((pal.control_hover, "Cambio"), |pen| (theme::pen_color(plan.pens[pen].color), plan.pens[pen].name.as_str()));
            ink_painter.rect_filled(band, 0.0, color);
            let light = color.r() as u32 * 299 + color.g() as u32 * 587 + color.b() as u32 * 114 > 140_000;
            let galley = ink_painter.layout_no_wrap(label.into(), egui::FontId::proportional(11.0), if light { Color32::BLACK } else { Color32::WHITE });
            if galley.size().x + 12.0 <= band.width() {
                ink_painter.galley(band.center() - galley.size() / 2.0, galley, Color32::WHITE);
            }
        }
        for track in [route, inks] { painter.rect_stroke(track, 0.0, Stroke::new(0.5_f32, pal.separator)); }
        for tick in 0..=4 {
            let fraction = tick as f64 / 4.0;
            let x = route.left() + route.width() * fraction as f32;
            painter.line_segment([Pos2::new(x, inks.bottom() + 2.0), Pos2::new(x, inks.bottom() + 5.0)], Stroke::new(1.0_f32, pal.secondary));
            let align = match tick { 0 => egui::Align2::LEFT_TOP, 4 => egui::Align2::RIGHT_TOP, _ => egui::Align2::CENTER_TOP };
            painter.text(Pos2::new(x, inks.bottom() + 7.0), align, duration(total * fraction), egui::FontId::proportional(10.0), pal.secondary);
        }
        if ready && (response.clicked() || response.dragged() || response.drag_stopped()) {
            if let Some(pos) = response.interact_pointer_pos().filter(|p| p.x >= route.left()) {
                sim.progress = ((pos.x - route.left()) / route.width()).clamp(0.0, 1.0); self.map = Map::Route;
            }
        }
        // Contorno oscuro + centro blanco: el cabezal se ve tanto en negro como
        // sobre amarillo, papel claro y el resto de las tintas. Recorre las dos pistas.
        let x = (route.left() + route.width() * sim.progress).clamp(route.left() + 3.0, route.right() - 3.0);
        let marker = [Pos2::new(x, route.top() - 4.0), Pos2::new(x, inks.bottom() + 3.0)];
        painter.line_segment(marker, Stroke::new(6.0_f32, Color32::from_rgb(28, 28, 32)));
        painter.line_segment(marker, Stroke::new(3.0_f32, Color32::WHITE));
        painter.add(egui::Shape::convex_polygon(vec![Pos2::new(x - 5.0, route.top() - 7.0),
            Pos2::new(x + 5.0, route.top() - 7.0), Pos2::new(x, route.top() - 1.0)], Color32::WHITE, Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 32))));
        if let Some(pos) = response.hover_pos().filter(|p| p.x >= route.left()) {
            let t = ((pos.x - route.left()) / route.width()).clamp(0.0, 1.0) as f64 * total;
            let pen = self.pen_spans.iter().find(|s| s.start_s <= t && t <= s.end_s)
                .and_then(|s| s.pen).map_or("Cambio de tinta", |pen| plan.pens[pen].name.as_str());
            response.on_hover_text(format!("{} · {pen}\nPulsa o arrastra en cualquiera de las dos pistas para recorrer la salida.", duration(t)));
        }
        let now = sim.progress as f64 * total;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Gris: viaje · violeta: secado · ocre: cambio").small().color(pal.secondary));
            if let Some(step) = plan.steps.get(plan.steps.partition_point(|s| s.times().1 <= now)) {
                let text = match *step {
                    OutputStep::Wait { end_s, .. } => format!("Secando con la pluma levantada · faltan {}", duration(end_s - now)),
                    OutputStep::ChangePen { pen, .. } => format!("Cambiar a {}", plan.pens[pen].name),
                    OutputStep::Draw { stroke, .. } => format!("Dibujando con {}", plan.pens[plan.strokes[stroke].pen].name),
                    OutputStep::Travel { .. } => "Viajando con la pluma levantada".into(),
                };
                ui.label(RichText::new(text).small().color(pal.text));
            }
        });
    }

    fn details(&mut self, ui: &mut egui::Ui, sim: &mut Sim) {
        let Some(plan) = self.plan.clone() else { return };
        let total = plan.stats.total_seconds;
        let now = sim.progress as f64 * total;
        egui::CollapsingHeader::new("Desglose y pausas del plan").show(ui, |ui| {
            ui.label(format!("Dibujar {} · viajar {} · subir/bajar {} · secar {} · cambios {} · {} trazos",
                duration(plan.stats.draw_seconds), duration(plan.stats.travel_seconds), duration(plan.stats.lift_seconds),
                duration(plan.stats.wait_seconds), duration(plan.stats.change_seconds), plan.stats.strokes));
            ui.label(format!("Mismo dibujo sin protección: {} y {} de viajes. Preparado: {} y {} de viajes.",
                duration(plan.baseline_stats.total_seconds), distance(plan.baseline_stats.travel_mm), duration(total), distance(plan.stats.travel_mm)));
            egui::ScrollArea::vertical().id_salt("output-events").max_height(70.0).show(ui, |ui| {
                for step in &plan.steps {
                    let (a, b) = step.times();
                    let label = match *step {
                        OutputStep::Wait { at, .. } => format!("{} · secar {} · X {:.1}, Y {:.1} mm", duration(a), duration(b - a), at.x, at.y),
                        OutputStep::ChangePen { pen, .. } => format!("{} · cambiar a {}", duration(a), plan.pens[pen].name),
                        _ => continue,
                    };
                    if ui.selectable_label(now >= a && now < b, label).clicked() { sim.progress = (a / total.max(1e-9)) as f32; self.map = Map::Route; }
                }
            });
        });
    }

    fn map(&mut self, ui: &mut egui::Ui, doc: &Document, sim: &Sim, ready: bool) {
        let pal = theme::current(ui.ctx());
        theme::glass_panel(ui, &pal, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                for (mode, label, hint) in [
                    (Map::Total, "Tinta total", "Todas las pasadas que se pisan. Esperar no reduce la cantidad total de tinta."),
                    (Map::Before, "Húmedo · sin secado", "Máximo de pasadas aún húmedas en cada zona, sin protección."),
                    (Map::After, "Húmedo · preparado", "Máximo de pasadas aún húmedas con la ruta y las esperas preparadas."),
                    (Map::Route, "Recorrido", "Simula dibujo, viajes, secado y cambios manuales de tinta.")
                ] { ui.selectable_value(&mut self.map, mode, label).on_hover_text(hint); }
            });
            ui.horizontal(|ui| {
                if self.map != Map::Route { ui.checkbox(&mut self.show_drawing, "Ver dibujo"); }
                if ui.button("−").on_hover_text("Alejar").clicked() { self.zoom_by(0.8); }
                if ui.button("+").on_hover_text("Acercar").clicked() { self.zoom_by(1.25); }
                if ui.button("Ajustar").clicked() { self.fitted = false; }
                if !ready && self.plan.is_some() { ui.label(RichText::new("Recalculando…").small().color(pal.secondary)); }
            });
        });
        ui.add_space(8.0);
        let legend_height = if self.map == Map::Route { 0.0 } else if ui.available_width() < 760.0 { 100.0 } else { 80.0 };
        let size = Vec2::new(ui.available_width(), (ui.available_height() - legend_height).max(80.0));
        let (resp, _) = ui.allocate_painter(size, Sense::click_and_drag());
        let rect = resp.rect;
        // El papel tiene su propia capa de pintura: los fondos de los paneles
        // que se reconstruyen al cambiar de tema nunca lo cubren parcialmente.
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("output-paper-paint"))).with_clip_rect(rect);
        if !self.fitted || self.viewport != size {
            self.zoom = ((size.x - 44.0) / doc.paper.width_mm as f32).min((size.y - 36.0) / doc.paper.height_mm as f32).max(0.01);
            self.offset = (size - Vec2::new(doc.paper.width_mm as f32, doc.paper.height_mm as f32) * self.zoom) / 2.0;
            self.fitted = true; self.viewport = size;
        }
        if let Some(pos) = resp.hover_pos() {
            let (scroll, factor) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
            if factor != 1.0 { let rel = pos - rect.min; let z = (self.zoom * factor).clamp(0.02, 80.0); self.offset = rel - (rel - self.offset) * z / self.zoom; self.zoom = z; }
            else { self.offset += scroll; }
        }
        if resp.dragged() { self.offset += resp.drag_delta(); }
        if resp.double_clicked() { self.fitted = false; }
        let origin = rect.min + self.offset;
        let paper_rect = Rect::from_min_size(origin, Vec2::new(doc.paper.width_mm as f32, doc.paper.height_mm as f32) * self.zoom);
        let shadow = egui::epaint::Shadow { offset: Vec2::new(0.0, 6.0), blur: 24.0, spread: 0.0, color: Color32::from_black_alpha(60) };
        painter.add(shadow.as_shape(paper_rect, egui::Rounding::same(2.0)));
        painter.rect_filled(paper_rect, 2.0, Color32::WHITE);
        painter.rect_stroke(paper_rect, 2.0, Stroke::new(0.7_f32, Color32::from_gray(170)));
        if let Some(plan) = &self.plan {
            if self.map != Map::Route {
                let idx = match self.map { Map::Total => 0, Map::Before => 1, _ => 2 };
                if let Some(source) = self.heat_meshes.get(idx) {
                    if self.heat_view.as_ref().is_none_or(|(o, z, i, _)| *o != origin || *z != self.zoom || *i != idx) {
                        let mut mesh = source.clone();
                        for vertex in &mut mesh.vertices { vertex.pos = origin + vertex.pos.to_vec2() * self.zoom; }
                        self.heat_view = Some((origin, self.zoom, idx, mesh));
                    }
                    if let Some((_, _, _, mesh)) = &self.heat_view {
                        painter.with_clip_rect(rect.intersect(paper_rect)).add(egui::Shape::Mesh(mesh.clone()));
                    }
                }
                if self.show_drawing { draw_route(&painter, plan, origin, self.zoom, f64::INFINITY, false, true); }
            } else { draw_route(&painter, plan, origin, self.zoom, sim.progress as f64 * plan.stats.total_seconds, self.show_travel, false); }
            if self.map != Map::Route {
                let grid = match self.map { Map::Total => &plan.total_heat, Map::Before => &plan.wet_before, _ => &plan.wet_after };
                let idx = match self.map { Map::Total => 0, Map::Before => 1, _ => 2 };
                if let Some(summary) = self.summaries.get(idx) {
                    theme::glass_panel(ui, &pal, |ui| { ui.set_width(ui.available_width()); legend(ui, grid, summary); });
                }
            }
        }
        if doc.items.is_empty() {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Añade texto, formas o una foto en Diseño.\nAquí verás la huella de tinta y el plan de salida.",
                egui::FontId::proportional(14.0), Color32::from_gray(110));
        }
    }
    pub fn zoom_by(&mut self, k: f32) {
        let z = (self.zoom * k).clamp(0.02, 80.0);
        self.offset = self.viewport / 2.0 - (self.viewport / 2.0 - self.offset) * z / self.zoom;
        self.zoom = z;
    }
    pub fn fit(&mut self) { self.fitted = false; }
    pub fn actual(&mut self) { self.zoom_by((96.0 / 25.4) / self.zoom); }
}

pub fn show(app: &mut VectorioApp, ctx: &egui::Context) {
    let current = app.rendered.rev == app.rev && !app.tracing() && !app.is_computing();
    let ready = current && app.output_ui.ready(app.rev, &app.doc.output);
    app.output_ui.update(ctx, &app.doc, &app.rendered, current, &mut app.sim);
    if app.sim.playing && ready {
        app.output_ui.map = Map::Route;
        let total = app.output_ui.plan.as_ref().map_or(1.0, |p| p.stats.total_seconds).max(1.0);
        app.sim.progress = (app.sim.progress + ctx.input(|i| i.stable_dt).min(0.1) * app.sim.speed / total as f32).min(1.0);
        if app.sim.progress >= 1.0 { app.sim.playing = false; }
        ctx.request_repaint();
    } else if !ready { app.sim.playing = false; }
    let pal = theme::current(ctx);
    egui::CentralPanel::default().frame(egui::Frame::none().fill(pal.canvas).inner_margin(egui::Margin::same(theme::INSET))).show(ctx, |ui| {
        egui::SidePanel::right("output-inspector").default_width(300.0).width_range(280.0..=360.0).resizable(true)
            .frame(egui::Frame::none().inner_margin(egui::Margin { left: 14.0, ..Default::default() })).show_inside(ui, |ui| {
                theme::glass_panel(ui, &pal, |ui| {
                    ui.set_min_height(ui.available_height());
                    egui::TopBottomPanel::top("output-summary").frame(egui::Frame::none()).show_inside(ui, |ui| app.output_ui.summary(ui, &app.doc, ready));
                    egui::TopBottomPanel::bottom("output-actions").frame(egui::Frame::none()).show_inside(ui, |ui| app.output_ui.actions(ui, &app.doc, ready));
                    egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("output-settings-scroll").show(ui, |ui| app.output_ui.settings(ui, &mut app.doc, &mut app.sim));
                    });
                });
            });
        egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| {
            egui::TopBottomPanel::bottom("output-playback").frame(egui::Frame::none().inner_margin(egui::Margin { top: 10.0, ..Default::default() })).show_inside(ui, |ui| {
                theme::glass_panel(ui, &pal, |ui| { ui.set_width(ui.available_width()); app.output_ui.playback(ui, &mut app.sim, ready); });
            });
            egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| app.output_ui.map(ui, &app.doc, &app.sim, ready));
        });
    });
    // El cálculo de ready se repite en el siguiente fotograma tras recibir un trabajo.
    if current && !ready && app.output_ui.job.is_none() && app.output_ui.error.is_none() { ctx.request_repaint(); }
}

fn setting(ui: &mut egui::Ui, label: &str, value: &mut f64, range: std::ops::RangeInclusive<f64>, suffix: &str) {
    ui.horizontal(|ui| { ui.label(label); ui.add(egui::DragValue::new(value).range(range).speed(0.1).suffix(suffix)); });
}
fn legend(ui: &mut egui::Ui, grid: &HeatGrid, summary: &HeatSummary) {
    let pal = theme::current(ui.ctx());
    ui.horizontal_wrapped(|ui| {
        for (i, count) in summary.levels.into_iter().enumerate() {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
            ui.painter().rect_filled(r, 2.0, heat_color(i));
            ui.label(format!("{} · {}", output::HEAT_LABELS[i], pct(count, summary.inked)));
            ui.add_space(4.0);
        }
    });
    ui.label(RichText::new(format!("Pasadas por celda de {:.2} mm · máximo {} · % de superficie dibujada", grid.cell_mm, summary.max)).small().color(pal.secondary));
}

fn draw_route(painter: &egui::Painter, plan: &OutputPlan, origin: Pos2, zoom: f32, now: f64, travel: bool, faint: bool) {
    let at = |q: P| origin + Vec2::new(q.x as f32, q.y as f32) * zoom;
    let mut shapes = Vec::new();
    let mut tip = None;
    for step in &plan.steps {
        let (a, b) = step.times();
        if a > now { break; }
        match *step {
            OutputStep::Draw { stroke, .. } => {
                let s = &plan.strokes[stroke];
                let pen = &plan.pens[s.pen];
                let color = if faint { Color32::from_black_alpha(50) } else { theme::pen_color(pen.color) };
                let mut points = vec![at(s.path[0])];
                let fraction = ((now - a) / (b - a).max(1e-9)).clamp(0.0, 1.0);
                let mut remaining = vectorio_core::geom::length(&s.path) * fraction;
                for pair in s.path.windows(2) {
                    let d = pair[0].dist(pair[1]);
                    let q = pair[0].lerp(pair[1], (remaining / d.max(1e-9)).min(1.0));
                    let point = at(q);
                    if points.last().is_none_or(|p| p.distance_sq(point) > 0.36) || remaining <= d { points.push(point); }
                    if remaining <= d { break; }
                    remaining -= d;
                }
                tip = points.last().copied();
                let width = (pen.width_mm as f32 * zoom).max(0.6);
                if points.len() > 1 && points.iter().any(|p| p.distance_sq(points[0]) > 1e-6) {
                    shapes.push(egui::Shape::line(points, Stroke::new(width, color)));
                } else { shapes.push(egui::Shape::circle_filled(points[0], width / 2.0, color)); }
            }
            OutputStep::Travel { from, to, .. } => {
                let end = from.lerp(to, ((now - a) / (b - a).max(1e-9)).clamp(0.0, 1.0));
                if travel { shapes.push(egui::Shape::line_segment([at(from), at(end)], Stroke::new(0.7_f32, Color32::from_rgb(124, 145, 181)))); }
                tip = Some(at(end));
            }
            OutputStep::Wait { at: q, .. } => { tip = Some(at(q)); }
            OutputStep::ChangePen { .. } => { tip = Some(at(P::default())); }
        }
    }
    painter.extend(shapes);
    if !faint && now < plan.stats.total_seconds {
        if let Some(tip) = tip { painter.circle_stroke(tip, 5.0, Stroke::new(2.0_f32, Color32::from_rgb(53, 33, 108))); }
    }
}

/// Celdas de color nativas, agrupadas en franjas iguales. Se conserva el máximo
/// de cada bloque igual que en la imagen de Linear, sin una textura dependiente
/// del tema. La malla transformada se reutiliza mientras no cambie el encuadre.
fn heat_mesh(grid: &HeatGrid) -> egui::Mesh {
    let (w, h, pixels) = grid.pixels(2048);
    let dx = grid.cols as f32 * grid.cell_mm as f32 / w as f32;
    let dy = grid.rows as f32 * grid.cell_mm as f32 / h as f32;
    let mut mesh = egui::Mesh::default();
    for y in 0..h {
        let mut x = 0;
        while x < w {
            let color = pixels[y * w + x];
            let start = x;
            x += 1;
            while x < w && pixels[y * w + x] == color { x += 1; }
            if color[3] > 0 {
                let [r, g, b, a] = color;
                mesh.add_colored_rect(Rect::from_min_max(Pos2::new(start as f32 * dx, y as f32 * dy),
                    Pos2::new(x as f32 * dx, (y + 1) as f32 * dy)), Color32::from_rgba_unmultiplied(r, g, b, a));
            }
        }
    }
    mesh
}

fn timeline_image(ctx: &egui::Context, plan: &OutputPlan) -> egui::ColorImage {
    const WIDTH: usize = 2048;
    let pal = theme::current(ctx);
    let mut pixels = vec![pal.control_hover; WIDTH];
    let mut priority = vec![0; WIDTH];
    for step in &plan.steps {
        let (a, b) = step.times();
        let (color, rank) = match *step {
            OutputStep::Draw { stroke, .. } => (theme::pen_color(plan.pens[plan.strokes[stroke].pen].color), 1),
            OutputStep::Wait { .. } => (Color32::from_rgb(112, 67, 166), 3),
            OutputStep::ChangePen { .. } => (Color32::from_rgb(166, 119, 36), 2),
            _ => continue,
        };
        let start = (a / plan.stats.total_seconds.max(1e-9) * WIDTH as f64) as usize;
        let end = (b / plan.stats.total_seconds.max(1e-9) * WIDTH as f64).ceil() as usize;
        for x in start.min(WIDTH)..end.min(WIDTH) { if rank >= priority[x] { pixels[x] = color; priority[x] = rank; } }
    }
    egui::ColorImage { size: [WIDTH, 1], pixels }
}

/// La tinta sigue activa durante viajes y secado; el cambio manual se deja neutro.
fn pen_spans(plan: &OutputPlan) -> Vec<PenSpan> {
    let mut out: Vec<PenSpan> = Vec::new();
    let mut active = plan.strokes.first().map(|s| s.pen);
    for step in &plan.steps {
        let (a, b) = step.times();
        let pen = match *step {
            OutputStep::ChangePen { .. } => None,
            OutputStep::Draw { stroke, .. } => { active = Some(plan.strokes[stroke].pen); active }
            _ => active,
        };
        if b > a {
            if let Some(last) = out.last_mut().filter(|s| s.pen == pen && (s.end_s - a).abs() < 1e-6) { last.end_s = b; }
            else { out.push(PenSpan { start_s: a, end_s: b, pen }); }
        }
        if let OutputStep::ChangePen { pen, .. } = *step { active = Some(pen); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exports_and_simulation_stay_visible_in_small_and_large_windows() {
        fn text_rect(shape: &egui::Shape, label: &str) -> Option<Rect> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == label => Some(Rect::from_min_size(t.pos, t.galley.size())),
                egui::Shape::Vec(v) => v.iter().find_map(|s| text_rect(s, label)),
                _ => None,
            }
        }
        fn has_heat(shape: &egui::Shape, heat: &egui::Mesh) -> bool {
            match shape {
                egui::Shape::Mesh(mesh) => mesh == heat && !mesh.indices.is_empty(),
                egui::Shape::Vec(v) => v.iter().any(|s| has_heat(s, heat)),
                _ => false,
            }
        }
        for (size, appearance) in [Vec2::new(980.0, 620.0), Vec2::new(1440.0, 900.0)].into_iter()
            .flat_map(|size| [theme::Appearance::Light, theme::Appearance::Dark].map(|appearance| (size, appearance))) {
            let ctx = egui::Context::default();
            let mut app = VectorioApp::blank(&ctx);
            appearance.apply(&ctx);
            let mut item = vectorio_core::Item::new("Cruces", vectorio_core::Geometry::Path {
                lines: vec![vec![vectorio_core::p(30.0, 30.0), vectorio_core::p(80.0, 30.0)]; 8], closed: false }, P::default());
            item.stroke = Some(0); app.doc.add(item);
            app.rendered.rev = app.rev;
            app.rendered.layers = app.doc.layers();
            let plan = output::prepare(&app.rendered.layers, &app.doc.paper, &app.doc.pens, &app.doc.plot, &app.doc.output).unwrap();
            let (tx, rx) = mpsc::channel(); tx.send(Ok(plan)).unwrap();
            app.output_ui.job = Some((Key { rev: app.rev, settings: app.doc.output.clone() }, rx));
            let screen = Rect::from_min_size(Pos2::ZERO, size);
            let mut result = None;
            for _ in 0..3 {
                result = Some(ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| {
                    egui::TopBottomPanel::top("test-toolbar").exact_height(52.0).show(ctx, |ui| { ui.label("Salida"); });
                    show(&mut app, ctx);
                }));
            }
            for label in ["Guardar plan con esperas…", "Exportar SVG de la ruta…", "Simular salida", "Húmedo · preparado", "Acciones", "Tintas"] {
                let (clip, rect) = result.as_ref().unwrap().shapes.iter()
                    .find_map(|s| text_rect(&s.shape, label).map(|r| (s.clip_rect, r))).expect(label);
                assert!(screen.contains_rect(rect) && clip.contains_rect(rect), "{label} fuera de {size:?}: {rect:?}, clip {clip:?}");
            }
            assert!(app.output_ui.ready(app.rev, &app.doc.output), "rev {}, key {:?}, cfg {:?}, job {}", app.rev,
                app.output_ui.key.as_ref().map(|k| (&k.settings, k.rev)), app.doc.output, app.output_ui.job.is_some());
            let changed = ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| {
                egui::TopBottomPanel::top("test-toolbar").exact_height(52.0).show(ctx, |ui| {
                    ui.label("Salida");
                    if appearance == theme::Appearance::Light { theme::Appearance::Dark.apply(ctx); }
                    else { theme::Appearance::Light.apply(ctx); }
                });
                show(&mut app, ctx);
            });
            let heat = &app.output_ui.heat_view.as_ref().unwrap().3;
            assert!(changed.shapes.iter().any(|s| has_heat(&s.shape, heat)), "El mapa debe seguir pintado al cambiar de tema");
            app.doc.output.dry_seconds += 1.0;
            assert!(!app.output_ui.ready(app.rev, &app.doc.output), "No se puede exportar el plan con ajustes anteriores");
        }
    }
}
