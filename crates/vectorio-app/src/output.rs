//! Espacio de salida: análisis físico, planificación de secado y simulación temporal.
use crate::{app::{Rendered, VectorioApp, PAPER_TINTS}, canvas::Sim, theme};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use crate::fmt::{distance, duration, pct};
use std::sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc};
use vectorio_core::{output::{self, HeatGrid, OutputError, OutputPlan, OutputSettings, OutputStep}, Document, P};

#[derive(Clone, PartialEq)]
struct Key { rev: u64, settings: OutputSettings }

#[derive(Clone, Copy, PartialEq, Default)]
enum Map { #[default] Total, Before, After, Route }

#[derive(Default)]
struct HeatSummary { max: u16, inked: usize, above: usize, levels: [usize; 6] }

struct Job { key: Key, cancel: Arc<AtomicBool>, rx: mpsc::Receiver<Result<OutputPlan, OutputError>> }

/// Pausa entre el último cambio y el cálculo, para no encadenar análisis al arrastrar un deslizador.
const DEBOUNCE_S: f64 = 0.25;

fn error_text(error: &OutputError) -> String {
    match error {
        OutputError::InvalidSettings => tr!("Revisa el papel, las velocidades y los tiempos de secado: deben ser valores finitos y positivos."),
        OutputError::MissingPen => tr!("Hay un trazo sin rotulador válido."),
        OutputError::InvalidPenWidth => tr!("El grosor del rotulador debe ser positivo."),
        OutputError::InvalidCoordinates => tr!("Hay coordenadas no válidas en el dibujo."),
        OutputError::InvalidLength => tr!("Hay un trazado con una longitud no válida."),
        OutputError::Cancelled => tr!("El cálculo se interrumpió."),
    }
}

struct PenSpan { start_s: f64, end_s: f64, pen: Option<usize> }

pub struct OutputUi {
    key: Option<Key>,
    job: Option<Job>,
    /// Cambio pendiente de calcular y desde cuándo (segundos de egui): se espera a que
    /// dejen de cambiar los valores antes de lanzar un cálculo completo.
    pending: Option<(Key, f64)>,
    pub plan: Option<Arc<OutputPlan>>,
    error: Option<String>,
    /// Resultado de la última exportación; la aplicación lo muestra como aviso.
    notice: Option<String>,
    /// Mapas de tinta total, húmedo sin secado y húmedo preparado, como texturas.
    heat_textures: Vec<egui::TextureHandle>,
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
        Self { key: None, job: None, pending: None, plan: None, error: None, notice: None, heat_textures: vec![], summaries: vec![], timeline: None, timeline_dark: false, pen_spans: vec![], map: Map::Total,
            show_drawing: false, show_travel: true, zoom: 1.0, offset: Vec2::ZERO,
            fitted: false, viewport: Vec2::ZERO }
    }
}

fn heat_color(i: usize) -> Color32 {
    let [r, g, b, a] = output::HEAT_COLORS[i]; theme::rgba(r, g, b, a)
}

impl OutputUi {
    pub fn ready(&self, rev: u64, cfg: &OutputSettings) -> bool {
        self.plan.is_some() && self.job.is_none() && self.key.as_ref() == Some(&Key { rev, settings: cfg.clone() })
    }
    fn update(&mut self, ctx: &egui::Context, doc: &Document, rendered: &Rendered, current: bool, sim: &mut Sim) {
        let key = Key { rev: rendered.rev, settings: doc.output.clone() };
        // Un cálculo con valores que ya no son los actuales se interrumpe y se descarta.
        if self.job.as_ref().is_some_and(|j| j.key != key) {
            if let Some(job) = self.job.take() { job.cancel.store(true, Ordering::Relaxed); }
        }
        if let Some(job) = &self.job {
            match job.rx.try_recv() {
                Ok(result) => {
                    self.key = self.job.take().map(|j| j.key);
                    let had_plan = self.plan.is_some();
                    self.error = result.as_ref().err().map(error_text);
                    self.plan = result.ok().map(Arc::new);
                    self.heat_textures.clear();
                    self.summaries.clear();
                    self.timeline = None;
                    self.pen_spans.clear();
                    if let Some(plan) = &self.plan {
                        for grid in [&plan.total_heat, &plan.wet_before, &plan.wet_after] {
                            self.heat_textures.push(ctx.load_texture(format!("output-heat-{}", self.heat_textures.len()), heat_image(grid),
                                egui::TextureOptions { minification: egui::TextureFilter::Linear, ..egui::TextureOptions::NEAREST }));
                            self.summaries.push(HeatSummary { max: grid.max(), inked: grid.inked(),
                                above: grid.at_least(plan.settings.max_wet_passes.saturating_add(1)), levels: grid.levels() });
                        }
                        self.timeline = Some(ctx.load_texture("output-timeline", timeline_image(ctx, plan), egui::TextureOptions::NEAREST));
                        self.timeline_dark = ctx.style().visuals.dark_mode;
                        self.pen_spans = pen_spans(plan);
                    }
                    // Con un plan anterior se conserva el punto de la simulación; solo el primero salta al final.
                    if !had_plan || self.plan.is_none() { sim.progress = 1.0; }
                    sim.playing = false;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.key = self.job.take().map(|j| j.key);
                    self.error = Some(tr!("El análisis no ha terminado. Pulsa Recalcular para intentarlo de nuevo."));
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
        if !current || self.job.is_some() || self.key.as_ref() == Some(&key) {
            self.pending = None;
            return;
        }
        // El primer análisis sale al instante; los siguientes esperan a que cesen los cambios.
        let now = ctx.input(|i| i.time);
        let since = match &self.pending { Some((k, t)) if *k == key => *t, _ => now };
        self.pending = Some((key.clone(), since));
        let wait = if self.plan.is_some() { DEBOUNCE_S - (now - since) } else { 0.0 };
        if wait > 0.0 { ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait)); return; }
        self.pending = None;
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        // En el orden del lienzo, los trazos de cada objeto (ya calculados) se recorren
        // de abajo arriba; si no, sirven las capas por tinta del dibujo.
        let stacked = doc.output.canvas_order.then(|| {
            let per_item: Vec<_> = doc.items.iter().map(|it| rendered.items.get(&it.id).map(|plot| plot.parts.clone()).unwrap_or_default()).collect();
            (doc.clone(), per_item)
        });
        let (layers, paper, pens, motion, cfg, flag, ctx) =
            (if stacked.is_some() { vec![] } else { rendered.layers.clone() }, doc.paper.clone(), doc.pens.clone(), doc.plot.clone(), doc.output.clone(), cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let layers = match stacked { Some((doc, per_item)) => doc.canvas_layers_from(per_item), None => layers };
            let result = output::prepare_cancellable(&layers, &paper, &pens, &motion, &cfg, &flag);
            let _ = tx.send(result); ctx.request_repaint();
        });
        self.job = Some(Job { key, cancel, rx }); self.error = None; sim.playing = false;
    }

    fn settings(&mut self, ui: &mut egui::Ui, doc: &mut Document, sim: &mut Sim) {
        let pal = theme::current(ui.ctx());
        theme::section(ui, &tr!("ORDEN DE LAS TINTAS"));
        ui.checkbox(&mut doc.output.canvas_order, tr!("Seguir el orden del lienzo"))
            .on_hover_text(tr!("Traza los objetos de abajo arriba, como se ven en Diseño: lo que queda encima se dibuja al final. Útil con tintas opacas, como los rotuladores acrílicos; puede cambiar de tinta más veces."));
        let order = if doc.output.canvas_order { tr!("De abajo arriba, como en el lienzo. Los objetos seguidos con la misma tinta se trazan juntos.") }
            else { tr!("Una tinta tras otra, en el orden de la paleta: los menos cambios posibles.") };
        ui.label(RichText::new(order).small().color(pal.tertiary));
        ui.add_space(8.0);
        theme::section(ui, &tr!("SECADO POR ZONAS"));
        ui.checkbox(&mut doc.output.drying, tr!("Proteger las zonas húmedas"));
        ui.add_enabled_ui(doc.output.drying, |ui| {
            ui.label(tr!("Tiempo de secado por pasada"));
            ui.add(egui::Slider::new(&mut doc.output.dry_seconds, 1.0..=180.0).suffix(" s"));
            ui.label(tr!("Máximo de pasadas aún húmedas"));
            ui.add(egui::Slider::new(&mut doc.output.max_wet_passes, 1..=10));
            ui.checkbox(&mut doc.output.reorder, tr!("Alternar zonas mientras seca"));
            ui.label(tr!("Tramo máximo al proteger"));
            ui.add(egui::Slider::new(&mut doc.output.chunk_mm, 5.0..=100.0).suffix(" mm"))
                .on_hover_text(tr!("Cuando hace falta proteger una zona, divide trazos largos para alternar o esperar con la pluma levantada. Las rutas que ya cumplen el límite se conservan intactas."));
        });
        ui.add_space(8.0);
        theme::section(ui, &tr!("HUELLA DE LA TINTA"));
        ui.label(tr!("Empape alrededor del trazo"));
        ui.add(egui::Slider::new(&mut doc.output.soak_mm, 0.0..=2.0).max_decimals(2).suffix(" mm"))
            .on_hover_text(tr!("Como en Linear: se añade al radio físico del rotulador al calcular qué zonas se pisan."));
        ui.label(RichText::new(tr!("El grosor se toma de cada tinta del documento.")).small().color(pal.tertiary));
        ui.add_space(8.0);
        egui::CollapsingHeader::new(tr!("Tiempos y precisión")).show(ui, |ui| {
            setting(ui, &tr!("Dibujar"), &mut doc.plot.draw_speed, 1.0..=200.0, " mm/s");
            setting(ui, &tr!("Viajar"), &mut doc.plot.travel_speed, 1.0..=400.0, " mm/s");
            setting(ui, &tr!("Subir/bajar"), &mut doc.plot.lift_time, 0.0..=3.0, " s");
            setting(ui, &tr!("Pausa al cambiar tinta"), &mut doc.output.pen_change_seconds, 0.0..=300.0, " s");
            setting(ui, &tr!("Celda del mapa"), &mut doc.output.cell_mm, 0.1..=2.0, " mm");
            let note = if doc.output.canvas_order { tr!("Cambios manuales con esta pausa mínima prevista. Se sigue el orden del lienzo.") }
                else { tr!("Cambios manuales con esta pausa mínima prevista. Se conserva el orden de la paleta.") };
            ui.label(RichText::new(note).small().color(pal.tertiary));
        });
        self.details(ui, sim);
        ui.add_space(8.0);
        ui.label(RichText::new(tr!("Ajusta secado y empape con una prueba de tu papel y rotuladores.")).small().color(pal.tertiary))
            .on_hover_text(tr!("Estimación conservadora: cada huella se considera húmeda hasta que seca el final del trazo."));
    }

    fn actions(&mut self, ui: &mut egui::Ui, doc: &Document, ready: bool) {
        let pal = theme::current(ui.ctx());
        ui.separator();
        ui.add_space(6.0);
        if ui.button(tr!("Recalcular salida")).clicked() { self.key = None; }
        let has_drawing = self.plan.as_ref().is_some_and(|p| p.stats.strokes > 0);
        ui.add_enabled_ui(ready && has_drawing, |ui| {
            if theme::accent_button(ui, &tr!("Guardar plan con esperas…")).clicked() { self.export_plan(); }
            if ui.button(tr!("Exportar SVG de la ruta…")).clicked() { self.export_svg(doc); }
        });
        ui.label(RichText::new(tr!("JSON: ruta y pausas. SVG: recorrido sin esperas.")).small().color(pal.tertiary));
        if let Some(error) = &self.error { ui.colored_label(pal.text, error); }
    }

    pub fn export_plan(&mut self) {
        let Some(plan) = &self.plan else { return };
        let Some(path) = rfd::FileDialog::new().set_title(tr!("Guardar plan de salida con secado"))
            .add_filter(tr!("Plan de salida Vectorio"), &["json"]).set_file_name("vectorio-salida.json").save_file() else { return };
        let result = serde_json::to_vec_pretty(plan.as_ref()).map_err(|e| e.to_string())
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()));
        self.finish_export(result.map(|()| tr!("Plan guardado en %{ruta}", ruta = path.display())), tr!("No se pudo guardar el plan: %{error}"));
    }
    pub fn export_svg(&mut self, doc: &Document) {
        let Some(plan) = &self.plan else { return };
        let Some(path) = rfd::FileDialog::new().add_filter("SVG", &["svg"])
            .set_file_name("vectorio-ruta.svg").save_file() else { return };
        let svg = vectorio_core::svg_out::export(doc, &plan.layers());
        let result = std::fs::write(&path, svg).map_err(|e| e.to_string());
        self.finish_export(result.map(|()| tr!("SVG de la ruta exportado en %{ruta}", ruta = path.display())), tr!("No se pudo exportar: %{error}"));
    }
    /// Éxito o fallo de una exportación: aviso en pantalla y, si falla, también en el inspector.
    fn finish_export(&mut self, result: Result<String, String>, failure: String) {
        match result {
            Ok(message) => { self.error = None; self.notice = Some(message); }
            Err(e) => { let message = failure.replace("%{error}", &e); self.error = Some(message.clone()); self.notice = Some(message); }
        }
    }

    fn summary(&self, ui: &mut egui::Ui, doc: &Document, ready: bool, plotter: &str) {
        let pal = theme::current(ui.ctx());
        ui.label(RichText::new(tr!("Salida")).size(17.0).strong().color(pal.text));
        ui.label(RichText::new(format!("{plotter} · {}", crate::i18n::tr_key(&doc.paper.name))).small().color(pal.tertiary))
            .on_hover_text(tr!("%{w} × %{h} mm · preparación sin conexión al plotter", w = format!("{:.0}", doc.paper.width_mm), h = format!("{:.0}", doc.paper.height_mm)));
        if let Some(plan) = &self.plan {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(duration(plan.stats.total_seconds)).size(21.0).strong().color(pal.text));
                ui.label(RichText::new(tr!("estimados")).small().color(pal.tertiary));
            });
            egui::Grid::new("output-summary-grid").num_columns(2).spacing([16.0, 3.0]).show(ui, |ui| {
                for (label, value) in [(tr!("Dibujo"), distance(plan.stats.draw_mm)), (tr!("Viajes"), distance(plan.stats.travel_mm)),
                    (tr!("Secado"), tr!("%{time} · %{n} pausas", time = duration(plan.stats.wait_seconds), n = plan.stats.waits)),
                    (tr!("Cambios de tinta"), plan.stats.pen_changes.to_string())] {
                    ui.label(RichText::new(label).small().color(pal.tertiary));
                    ui.label(RichText::new(value).small().color(pal.text)); ui.end_row();
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            if !ready && self.error.is_none() { ui.spinner(); ui.label(tr!("Calculando huella y recorrido…")); }
            else if let Some(plan) = &self.plan {
                let limit = plan.settings.max_wet_passes;
                if self.summaries.len() == 3 {
                    ui.label(RichText::new(tr!("Húmedo: %{a} → %{b} pasadas\nSobre el límite de %{limit}: %{c} → %{d}",
                        a = self.summaries[1].max, b = self.summaries[2].max, limit = limit,
                        c = pct(self.summaries[1].above, self.summaries[0].inked),
                        d = pct(self.summaries[2].above, self.summaries[0].inked))).small().color(pal.tertiary));
                }
                if plan.stats.outside_points > 0 { ui.label(RichText::new(tr!("Hay trazos fuera del papel")).strong().color(pal.text)); }
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
            if ui.add_enabled(ready && total > 0.0, egui::Button::new(if sim.playing { tr!("Pausar") } else { tr!("Simular salida") })).clicked() {
                if sim.progress >= 1.0 { sim.progress = 0.0; }
                sim.playing = !sim.playing; self.map = Map::Route;
            }
            ui.add_enabled_ui(ready, |ui| {
                ui.scope(|ui| { ui.spacing_mut().slider_width = 110.0; ui.add(egui::Slider::new(&mut sim.progress, 0.0..=1.0).show_value(false)); });
                egui::ComboBox::from_id_salt("output-speed").icon(crate::theme::combo_icon).selected_text(format!("×{:.0}", sim.speed)).width(62.0).show_ui(ui, |ui| {
                    for v in [1.0, 5.0, 20.0, 60.0, 200.0] { ui.selectable_value(&mut sim.speed, v, format!("×{v:.0}")); }
                });
            });
            ui.label(format!("{} / {}", duration(sim.progress as f64 * total), duration(total)));
            ui.checkbox(&mut self.show_travel, tr!("Ver viajes"));
        });
        let (response, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 82.0), Sense::click_and_drag());
        let r = response.rect;
        let route = Rect::from_min_max(Pos2::new(r.left() + 62.0, r.top() + 10.0), Pos2::new(r.right(), r.top() + 34.0));
        let inks = Rect::from_min_max(Pos2::new(route.left(), r.top() + 42.0), Pos2::new(route.right(), r.top() + 60.0));
        let time_x = |t: f64| route.left() + route.width() * (t / total.max(1e-9)) as f32;
        for (label, track) in [(tr!("Acciones"), route), (tr!("Tintas"), inks)] {
            painter.text(Pos2::new(r.left(), track.center().y), egui::Align2::LEFT_CENTER, &label,
                egui::FontId::proportional(11.0), pal.secondary);
            painter.rect_filled(track, 0.0, pal.control_hover);
        }
        if let Some(texture) = &self.timeline { painter.image(texture.id(), route, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE); }
        let ink_painter = painter.with_clip_rect(inks);
        for span in &self.pen_spans {
            let band = Rect::from_min_max(Pos2::new(time_x(span.start_s), inks.top()), Pos2::new(time_x(span.end_s), inks.bottom()));
            let (color, label) = span.pen.map_or((pal.control_hover, tr!("Cambio")), |pen| (theme::pen_color(plan.pens[pen].color), crate::i18n::pen_name(&plan.pens[pen])));
            ink_painter.rect_filled(band, 0.0, color);
            let light = color.r() as u32 * 299 + color.g() as u32 * 587 + color.b() as u32 * 114 > 140_000;
            let galley = ink_painter.layout_no_wrap(label, egui::FontId::proportional(11.0), if light { Color32::BLACK } else { Color32::WHITE });
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
                .and_then(|s| s.pen).map_or(tr!("Cambio de tinta"), |pen| crate::i18n::pen_name(&plan.pens[pen]));
            response.on_hover_text(tr!("%{time} · %{pen}\nPulsa o arrastra en cualquiera de las dos pistas para recorrer la salida.", time = duration(t), pen = pen));
        }
        let now = sim.progress as f64 * total;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tr!("Gris: viaje · violeta: secado · ocre: cambio")).small().color(pal.tertiary));
            if let Some(step) = plan.steps.get(plan.steps.partition_point(|s| s.times().1 <= now)) {
                let text = match *step {
                    OutputStep::Wait { end_s, .. } => tr!("Secando con la pluma levantada · faltan %{time}", time = duration(end_s - now)),
                    OutputStep::ChangePen { pen, .. } => tr!("Cambiar a %{pen}", pen = crate::i18n::pen_name(&plan.pens[pen])),
                    OutputStep::Draw { stroke, .. } => tr!("Dibujando con %{pen}", pen = crate::i18n::pen_name(&plan.pens[plan.strokes[stroke].pen])),
                    OutputStep::Travel { .. } => tr!("Viajando con la pluma levantada"),
                };
                ui.label(RichText::new(text).small().color(pal.text));
            }
        });
    }

    fn details(&mut self, ui: &mut egui::Ui, sim: &mut Sim) {
        let Some(plan) = self.plan.clone() else { return };
        let total = plan.stats.total_seconds;
        let now = sim.progress as f64 * total;
        egui::CollapsingHeader::new(tr!("Desglose y pausas del plan")).show(ui, |ui| {
            ui.label(tr!("Dibujar %{draw} · viajar %{travel} · subir/bajar %{lift} · secar %{wait} · cambios %{change} · %{n} trazos",
                draw = duration(plan.stats.draw_seconds), travel = duration(plan.stats.travel_seconds), lift = duration(plan.stats.lift_seconds),
                wait = duration(plan.stats.wait_seconds), change = duration(plan.stats.change_seconds), n = plan.stats.strokes));
            ui.label(tr!("Mismo dibujo sin protección: %{t1} y %{d1} de viajes. Preparado: %{t2} y %{d2} de viajes.",
                t1 = duration(plan.baseline_stats.total_seconds), d1 = distance(plan.baseline_stats.travel_mm), t2 = duration(total), d2 = distance(plan.stats.travel_mm)));
            egui::ScrollArea::vertical().id_salt("output-events").max_height(70.0).show(ui, |ui| {
                for step in &plan.steps {
                    let (a, b) = step.times();
                    let label = match *step {
                        OutputStep::Wait { at, .. } => tr!("%{start} · secar %{wait} · X %{x}, Y %{y} mm", start = duration(a), wait = duration(b - a), x = crate::fmt::decimal(at.x, 1), y = crate::fmt::decimal(at.y, 1)),
                        OutputStep::ChangePen { pen, .. } => tr!("%{start} · cambiar a %{pen}", start = duration(a), pen = crate::i18n::pen_name(&plan.pens[pen])),
                        _ => continue,
                    };
                    if ui.selectable_label(now >= a && now < b, label).clicked() { sim.progress = (a / total.max(1e-9)) as f32; self.map = Map::Route; }
                }
            });
        });
    }

    /// Selector de mapa y zoom: una cápsula flotante bajo la barra, sobre el papel.
    fn controls(&mut self, ctx: &egui::Context, ready: bool, enabled: bool) {
        let pal = theme::current(ctx);
        let width = (ctx.screen_rect().width() - PANEL_W - 3.0 * theme::INSET).max(300.0);
        egui::Area::new(egui::Id::new("output-controls")).enabled(enabled).fixed_pos(Pos2::new(theme::INSET, theme::TOOLBAR_H + theme::INSET)).order(egui::Order::Middle).show(ctx, |ui| {
            theme::glass_panel(ui, &pal, |ui| {
                ui.set_max_width(width - 2.0 * theme::PANEL_PAD);
                ui.horizontal_wrapped(|ui| {
                    for (mode, label, hint) in [
                        (Map::Total, tr!("Tinta total"), tr!("Todas las pasadas que se pisan. Esperar no reduce la cantidad total de tinta.")),
                        (Map::Before, tr!("Húmedo · sin secado"), tr!("Máximo de pasadas aún húmedas en cada zona, sin protección.")),
                        (Map::After, tr!("Húmedo · preparado"), tr!("Máximo de pasadas aún húmedas con la ruta y las esperas preparadas.")),
                        (Map::Route, tr!("Recorrido"), tr!("Simula dibujo, viajes, secado y cambios manuales de tinta."))
                    ] { ui.selectable_value(&mut self.map, mode, label).on_hover_text(hint); }
                });
                ui.horizontal(|ui| {
                    if self.map != Map::Route { ui.checkbox(&mut self.show_drawing, tr!("Ver dibujo")); }
                    if ui.button("−").on_hover_text(tr!("Alejar")).clicked() { self.zoom_by(0.8); }
                    if ui.button("+").on_hover_text(tr!("Acercar")).clicked() { self.zoom_by(1.25); }
                    if ui.button(tr!("Ajustar")).clicked() { self.fitted = false; }
                    if !ready && self.plan.is_some() { ui.label(RichText::new(tr!("Recalculando…")).small().color(pal.tertiary)); }
                });
            });
        });
    }

    /// Leyenda del mapa de calor y reproducción, flotando abajo.
    fn bottom(&mut self, ctx: &egui::Context, sim: &mut Sim, ready: bool, enabled: bool) {
        let pal = theme::current(ctx);
        let width = (ctx.screen_rect().width() - PANEL_W - 3.0 * theme::INSET).max(300.0);
        egui::Area::new(egui::Id::new("output-bottom")).enabled(enabled).anchor(egui::Align2::LEFT_BOTTOM, Vec2::new(theme::INSET, -theme::INSET)).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_width(width);
            if self.map != Map::Route {
                if let Some(plan) = &self.plan {
                    let (grid, idx) = match self.map { Map::Total => (&plan.total_heat, 0), Map::Before => (&plan.wet_before, 1), _ => (&plan.wet_after, 2) };
                    if let Some(summary) = self.summaries.get(idx) {
                        theme::glass_panel(ui, &pal, |ui| { ui.set_width(width - 2.0 * theme::PANEL_PAD); legend(ui, grid, summary); });
                        ui.add_space(8.0);
                    }
                }
            }
            theme::glass_panel(ui, &pal, |ui| { ui.set_width(width - 2.0 * theme::PANEL_PAD); self.playback(ui, sim, ready); });
        });
    }

    /// Lienzo del mapa: ocupa toda la ventana y los paneles flotan encima. El papel se
    /// ajusta al hueco que queda libre entre ellos.
    fn map(&mut self, ui: &mut egui::Ui, doc: &Document, sim: &Sim, ready: bool, paper_tint: usize) {
        let _ = ready;
        let size = ui.max_rect().size();
        let (resp, _) = ui.allocate_painter(size, Sense::click_and_drag());
        let rect = resp.rect;
        let heat = self.map != Map::Route;
        let usable = Rect::from_min_max(
            rect.min + Vec2::new(theme::INSET, theme::TOOLBAR_H + theme::INSET + 78.0),
            rect.max - Vec2::new(PANEL_W + 2.0 * theme::INSET, theme::INSET + 180.0 + if heat { 90.0 } else { 0.0 }),
        );
        // El papel tiene su propia capa de pintura: los fondos de los paneles
        // que se reconstruyen al cambiar de tema nunca lo cubren parcialmente.
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("output-paper-paint"))).with_clip_rect(rect);
        if !self.fitted || self.viewport != size {
            self.zoom = ((usable.width() - 44.0) / doc.paper.width_mm as f32).min((usable.height() - 36.0) / doc.paper.height_mm as f32).max(0.01);
            self.offset = usable.center() - rect.min - Vec2::new(doc.paper.width_mm as f32, doc.paper.height_mm as f32) * self.zoom / 2.0;
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
        // Los mapas de calor se leen sobre blanco; el recorrido usa el color de papel elegido.
        let [r, g, b] = PAPER_TINTS[paper_tint.min(PAPER_TINTS.len() - 1)].1;
        let paper_color = if self.map == Map::Route { Color32::from_rgb(r, g, b) } else { Color32::WHITE };
        let dark_paper = paper_color.r() as u32 * 299 + paper_color.g() as u32 * 587 + paper_color.b() as u32 * 114 < 110_000;
        theme::set_backdrop(ui.ctx(), Some((paper_rect, paper_color)));
        painter.rect_filled(paper_rect, 2.0, paper_color);
        painter.rect_stroke(paper_rect, 2.0, Stroke::new(0.7_f32, Color32::from_gray(if dark_paper { 90 } else { 170 })));
        if let Some(plan) = &self.plan {
            if self.map != Map::Route {
                let idx = match self.map { Map::Total => 0, Map::Before => 1, _ => 2 };
                if let Some(texture) = self.heat_textures.get(idx) {
                    // La rejilla cubre el papel entero: la textura se estira sobre ella.
                    let grid = [&plan.total_heat, &plan.wet_before, &plan.wet_after][idx];
                    let area = Rect::from_min_size(origin, Vec2::new(grid.cols as f32, grid.rows as f32) * grid.cell_mm as f32 * self.zoom);
                    painter.with_clip_rect(rect.intersect(paper_rect))
                        .image(texture.id(), area, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                }
                if self.show_drawing { draw_route(&painter, plan, origin, self.zoom, f64::INFINITY, RouteStyle { travel: false, faint: true, dark_paper: false }); }
            } else { draw_route(&painter, plan, origin, self.zoom, sim.progress as f64 * plan.stats.total_seconds, RouteStyle { travel: self.show_travel, faint: false, dark_paper }); }
        }
        if doc.items.is_empty() {
            painter.text(paper_rect.center(), egui::Align2::CENTER_CENTER, tr!("Añade texto, formas o una foto en Diseño.\nAquí verás la huella de tinta y el plan de salida."),
                egui::FontId::proportional(14.0), Color32::from_gray(if dark_paper { 170 } else { 110 }));
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
    let ready = app.output_ready();
    app.output_ui.update(ctx, &app.doc, &app.rendered, current, &mut app.sim);
    if app.sim.playing && ready {
        app.output_ui.map = Map::Route;
        let total = app.output_ui.plan.as_ref().map_or(1.0, |p| p.stats.total_seconds).max(1.0);
        app.sim.progress = (app.sim.progress + ctx.input(|i| i.stable_dt).min(0.1) * app.sim.speed / total as f32).min(1.0);
        if app.sim.progress >= 1.0 { app.sim.playing = false; }
        ctx.request_repaint();
    } else if !ready { app.sim.playing = false; }
    if let Some(message) = app.output_ui.notice.take() { app.notify(ctx, message); }
    let pal = theme::current(ctx);
    egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
        theme::paint_canvas(ui.painter(), ctx.screen_rect(), &pal);
        if app.document_modal() { ui.disable(); }
        app.output_ui.map(ui, &app.doc, &app.sim, ready, app.prefs.paper_tint);
    });
    let enabled = !app.document_modal();
    app.output_ui.controls(ctx, ready, enabled);
    app.output_ui.bottom(ctx, &mut app.sim, ready, enabled);
    let top = theme::TOOLBAR_H + theme::INSET;
    let height = ctx.screen_rect().height() - top - theme::INSET;
    egui::Area::new(egui::Id::new("output-inspector")).enabled(enabled).fixed_pos(Pos2::new(ctx.screen_rect().right() - PANEL_W - theme::INSET, top)).order(egui::Order::Middle).show(ctx, |ui| {
        theme::glass_panel(ui, &pal, |ui| {
            ui.set_width(PANEL_W - 2.0 * theme::PANEL_PAD);
            ui.set_height(height - 2.0 * theme::PANEL_PAD);
            egui::TopBottomPanel::top("output-summary").frame(egui::Frame::none()).show_inside(ui, |ui| app.output_ui.summary(ui, &app.doc, ready, app.prefs.plotter()));
            egui::TopBottomPanel::bottom("output-actions").frame(egui::Frame::none()).show_inside(ui, |ui| {
                ui.separator();
                ui.add_space(6.0);
                crate::machine_ui::actions(app, ui, ready);
                app.output_ui.actions(ui, &app.doc, ready);
            });
            egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("output-settings-scroll").show(ui, |ui| {
                    app.output_ui.settings(ui, &mut app.doc, &mut app.sim);
                    ui.add_space(8.0);
                    crate::machine_ui::settings(app, ui);
                });
            });
        });
    });
    // El cálculo de ready se repite en el siguiente fotograma tras recibir un trabajo.
    if current && !ready && app.output_ui.job.is_none() && app.output_ui.error.is_none() { ctx.request_repaint(); }
}

fn setting(ui: &mut egui::Ui, label: &str, value: &mut f64, range: std::ops::RangeInclusive<f64>, suffix: &str) {
    ui.horizontal(|ui| { ui.label(label); ui.add(egui::DragValue::new(value).range(range).speed(0.1).suffix(suffix)); });
}
const PANEL_W: f32 = 320.0;

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
    ui.label(RichText::new(tr!("Pasadas por celda de %{cell} mm · máximo %{max} · % de superficie dibujada", cell = crate::fmt::decimal(grid.cell_mm, 2), max = summary.max)).small().color(pal.tertiary));
}

#[derive(Clone, Copy)]
struct RouteStyle { travel: bool, faint: bool, dark_paper: bool }

fn draw_route(painter: &egui::Painter, plan: &OutputPlan, origin: Pos2, zoom: f32, now: f64, style: RouteStyle) {
    let RouteStyle { travel, faint, dark_paper } = style;
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
        if let Some(tip) = tip { painter.circle_stroke(tip, 5.0, Stroke::new(2.0_f32, if dark_paper { Color32::from_rgb(246, 240, 149) } else { Color32::from_rgb(53, 33, 108) })); }
    }
}

/// Imagen del mapa: un píxel por bloque de celdas, conservando el máximo de cada bloque
/// igual que en Linear. Los colores no dependen del tema, así que no hay que rehacerla
/// al cambiar de aspecto y al dibujar solo se estira una textura.
fn heat_image(grid: &HeatGrid) -> egui::ColorImage {
    let (w, h, pixels) = grid.pixels(2048);
    egui::ColorImage { size: [w, h], pixels: pixels.into_iter().map(|[r, g, b, a]| theme::rgba(r, g, b, a)).collect() }
}

fn timeline_image(ctx: &egui::Context, plan: &OutputPlan) -> egui::ColorImage {
    const WIDTH: usize = 2048;
    let pal = theme::current(ctx);
    let mut pixels = vec![pal.solid(pal.control_hover); WIDTH];
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
        visible_in("es", &["Guardar plan con esperas…", "Exportar SVG de la ruta…", "Simular salida", "Húmedo · preparado", "Acciones", "Tintas", "Conectar", "Sin conectar"]);
    }
    #[test]
    fn the_english_texts_also_fit_without_overflowing() {
        visible_in("en", &["Save plan with waits…", "Export route SVG…", "Simulate output", "Wet · prepared", "Actions", "Pens", "Connect", "Not connected"]);
    }
    fn visible_in(locale: &str, labels: &[&str]) {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale(locale);
        fn text_rect(shape: &egui::Shape, label: &str) -> Option<Rect> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == label => Some(Rect::from_min_size(t.pos, t.galley.size())),
                egui::Shape::Vec(v) => v.iter().find_map(|s| text_rect(s, label)),
                _ => None,
            }
        }
        fn has_heat(shape: &egui::Shape, heat: egui::TextureId) -> bool {
            match shape {
                egui::Shape::Mesh(mesh) => mesh.texture_id == heat && !mesh.indices.is_empty(),
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
            app.output_ui.job = Some(Job { key: Key { rev: app.rev, settings: app.doc.output.clone() }, cancel: Arc::new(AtomicBool::new(false)), rx });
            let screen = Rect::from_min_size(Pos2::ZERO, size);
            let mut result = None;
            for _ in 0..3 {
                result = Some(ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| {
                    show(&mut app, ctx);
                }));
            }
            for &label in labels {
                let (clip, rect) = result.as_ref().unwrap().shapes.iter()
                    .find_map(|s| text_rect(&s.shape, label).map(|r| (s.clip_rect, r))).expect(label);
                assert!(screen.contains_rect(rect) && clip.contains_rect(rect), "{label} fuera de {size:?}: {rect:?}, clip {clip:?}");
            }
            assert!(app.output_ui.ready(app.rev, &app.doc.output), "rev {}, key {:?}, cfg {:?}, job {}", app.rev,
                app.output_ui.key.as_ref().map(|k| (&k.settings, k.rev)), app.doc.output, app.output_ui.job.is_some());
            let changed = ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| {
                if appearance == theme::Appearance::Light { theme::Appearance::Dark.apply(ctx); }
                else { theme::Appearance::Light.apply(ctx); }
                show(&mut app, ctx);
            });
            let heat = app.output_ui.heat_textures[0].id();
            assert!(changed.shapes.iter().any(|s| has_heat(&s.shape, heat)), "El mapa debe seguir pintado al cambiar de tema");
            app.doc.output.dry_seconds += 1.0;
            assert!(!app.output_ui.ready(app.rev, &app.doc.output), "No se puede exportar el plan con ajustes anteriores");
        }
        rust_i18n::set_locale("es");
    }
    #[test]
    fn canvas_order_follows_the_objects_bottom_to_top_only_when_asked() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        for (i, pen) in [1, 0, 1].into_iter().enumerate() {
            let y = 30.0 + 10.0 * i as f64;
            let mut item = vectorio_core::Item::new("Raya", vectorio_core::Geometry::Path {
                lines: vec![vec![vectorio_core::p(30.0, y), vectorio_core::p(80.0, y)]], closed: false }, P::default());
            item.stroke = Some(pen);
            doc.add(item);
        }
        let per_item = doc.plot_items();
        let items = doc.items.iter().zip(&per_item).map(|(it, parts)| (it.id, crate::app::ItemPlot { snapshot: it.clone(), parts: parts.clone() })).collect();
        let rendered = Rendered { rev: 1, layers: doc.layers_from(per_item), items, ..Default::default() };
        let plan_pens = |doc: &Document| {
            let mut ui = OutputUi::default();
            let mut sim = Sim::default();
            for _ in 0..500 {
                let _ = ctx.run(egui::RawInput::default(), |ctx| ui.update(ctx, doc, &rendered, true, &mut sim));
                if ui.ready(rendered.rev, &doc.output) { break; }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let plan = ui.plan.expect("plan");
            (plan.layers().iter().map(|l| l.pen).collect::<Vec<_>>(), plan.stats.pen_changes)
        };
        // Por defecto, una tinta tras otra en el orden de la paleta.
        assert_eq!(plan_pens(&doc), (vec![0, 1], 1));
        // Con la casilla, como en el lienzo: el rojo de encima se traza al final.
        doc.output.canvas_order = true;
        assert_eq!(plan_pens(&doc), (vec![1, 0, 1], 2));
    }
    #[test]
    fn rapid_changes_wait_before_recalculating_and_cancel_the_stale_job() {
        let ctx = egui::Context::default();
        let mut doc = Document::default();
        let rendered = Rendered::default();
        let plan = output::prepare(&[], &doc.paper, &doc.pens, &doc.plot, &doc.output).unwrap();
        let mut ui = OutputUi { plan: Some(Arc::new(plan)), key: Some(Key { rev: rendered.rev, settings: doc.output.clone() }), ..Default::default() };
        let mut sim = Sim { progress: 0.4, ..Sim::default() };
        doc.output.dry_seconds += 5.0;
        let _ = ctx.run(egui::RawInput::default(), |ctx| ui.update(ctx, &doc, &rendered, true, &mut sim));
        assert!(ui.job.is_none() && ui.pending.is_some(), "un cambio con plan previo espera antes de calcular");
        // Un cálculo en curso con valores antiguos se interrumpe al llegar otros nuevos.
        let cancel = Arc::new(AtomicBool::new(false));
        let (_tx, rx) = mpsc::channel();
        ui.job = Some(Job { key: Key { rev: rendered.rev, settings: OutputSettings::default() }, cancel: cancel.clone(), rx });
        let _ = ctx.run(egui::RawInput::default(), |ctx| ui.update(ctx, &doc, &rendered, true, &mut sim));
        assert!(cancel.load(Ordering::Relaxed) && ui.job.is_none());
        assert_eq!(sim.progress, 0.4);
    }
}
