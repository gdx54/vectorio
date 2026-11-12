//! Ventana de Ajustes (⌘,): los cambios se aplican al momento, sin botón de aceptar.

use crate::app::{choice, segmented, text_segment, PAPER_TINTS};
use crate::prefs::{Language, NewDocument, Preferences, SCALE_RANGE};
use crate::theme::{self, Appearance, Palette};
use egui::{RichText, Sense, Vec2};
use vectorio_core::{Document, Paper};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab { #[default] General, NewDocument, Output }

#[derive(Default)]
pub struct PrefsUi {
    pub open: bool,
    tab: Tab,
    /// Valor del deslizador de tamaño mientras se arrastra: la interfaz solo cambia al soltar.
    scale_draft: f32,
}

fn row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        let pal = theme::current(ui.ctx());
        ui.add_sized([150.0, 22.0], egui::Label::new(RichText::new(label).color(pal.secondary)).truncate());
        add(ui);
    });
}

fn drag(ui: &mut egui::Ui, label: &str, value: &mut f64, range: std::ops::RangeInclusive<f64>, speed: f64, suffix: &str) {
    row(ui, label, |ui| { ui.add(egui::DragValue::new(value).range(range).speed(speed).suffix(suffix).max_decimals(2)); });
}

impl PrefsUi {
    pub fn show_window(&mut self, prefs: &Preferences) {
        self.open = true;
        self.scale_draft = prefs.ui_scale;
    }

    /// Dibuja la ventana si está abierta y devuelve las preferencias nuevas si algo cambió.
    pub fn show(&mut self, ctx: &egui::Context, prefs: &Preferences, doc: &Document, pal: &Palette) -> Option<Preferences> {
        if !self.open { return None; }
        let screen = ctx.screen_rect();
        let veil = egui::Area::new(egui::Id::new("prefs-veil")).fixed_pos(screen.min).order(egui::Order::Middle).show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(screen, 0.0, theme::veil(ui.visuals().dark_mode));
            resp.clicked()
        });
        ctx.move_to_top(veil.response.layer_id);
        if veil.inner { self.open = false; }
        let mut edited = prefs.clone();
        let mut scale = self.scale_draft;
        let mut tab = self.tab;
        let mut close = false;
        egui::Area::new(egui::Id::new("prefs-sheet")).anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, theme::TOOLBAR_H / 2.0))
            .order(egui::Order::Foreground).show(ctx, |ui| {
                theme::glass_sheet(ui, pal, |ui| {
                    ui.set_width(500.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr!("Ajustes")).size(15.0).strong().color(pal.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(tr!("Esc para cerrar")).color(pal.secondary));
                        });
                    });
                    ui.add_space(8.0);
                    segmented(ui, pal, |ui| {
                        for (value, label) in [(Tab::General, tr!("General")), (Tab::NewDocument, tr!("Documento nuevo")), (Tab::Output, tr!("Salida"))] {
                            if text_segment(ui, pal, &label, tab == value).clicked() { tab = value; }
                        }
                    });
                    ui.add_space(10.0);
                    egui::ScrollArea::vertical().max_height((screen.height() - 220.0).max(180.0)).auto_shrink([false, true]).show(ui, |ui| match tab {
                        Tab::General => general(ui, pal, &mut edited, &mut scale),
                        Tab::NewDocument => new_document(ui, pal, &mut edited, doc),
                        Tab::Output => output(ui, pal, &mut edited),
                    });
                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::accent_button(ui, &tr!("Listo")).clicked() { close = true; }
                    });
                });
            });
        self.tab = tab;
        self.scale_draft = scale;
        if close { self.open = false; }
        // El tamaño solo se aplica al soltar el deslizador, para que no se mueva bajo el cursor.
        let released = !ctx.input(|i| i.pointer.any_down());
        if released && (scale - edited.ui_scale).abs() > f32::EPSILON { edited.ui_scale = scale; }
        let edited = edited.sanitized();
        (edited != *prefs).then_some(edited)
    }
}

fn general(ui: &mut egui::Ui, pal: &Palette, prefs: &mut Preferences, scale: &mut f32) {
    theme::section(ui, &tr!("APARIENCIA"));
    let labels = [tr!("Sistema"), tr!("Claro"), tr!("Oscuro")];
    choice(ui, &mut prefs.appearance, &[(Appearance::System, &labels[0]), (Appearance::Light, &labels[1]), (Appearance::Dark, &labels[2])]);
    ui.add_space(10.0);
    theme::section(ui, &tr!("IDIOMA"));
    let labels = Language::ALL.map(|l| l.label());
    choice(ui, &mut prefs.language, &[(Language::System, &labels[0]), (Language::Spanish, &labels[1]), (Language::English, &labels[2])]);
    ui.add_space(10.0);
    theme::section(ui, &tr!("TAMAÑO DE LA INTERFAZ"));
    ui.add(egui::Slider::new(scale, SCALE_RANGE).custom_formatter(|v, _| format!("{:.0} %", v * 100.0))
        .custom_parser(|s| s.trim().trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0)));
    ui.label(RichText::new(tr!("Se aplica al soltar el deslizador.")).small().color(pal.tertiary));
    ui.add_space(10.0);
    theme::section(ui, &tr!("AYUDA"));
    ui.checkbox(&mut prefs.show_tips, tr!("Mostrar ideas para empezar en el panel de documento"));
}

fn new_document(ui: &mut egui::Ui, pal: &Palette, prefs: &mut Preferences, current: &Document) {
    let nd = &mut prefs.new_document;
    theme::section(ui, &tr!("PAPEL"));
    row(ui, &tr!("Tamaño"), |ui| {
        egui::ComboBox::from_id_salt("prefs-paper").icon(crate::theme::combo_icon).selected_text(crate::i18n::tr_key(&nd.paper.name)).show_ui(ui, |ui| {
            for preset in Paper::presets() {
                let selected = preset.name == nd.paper.name;
                if ui.selectable_label(selected, crate::i18n::tr_key(&preset.name)).clicked() && !selected {
                    nd.paper = Paper { margin_mm: nd.paper.margin_mm, ..preset };
                }
            }
        });
    });
    let before = (nd.paper.width_mm, nd.paper.height_mm);
    drag(ui, &tr!("Ancho"), &mut nd.paper.width_mm, 20.0..=2000.0, 1.0, " mm");
    drag(ui, &tr!("Alto"), &mut nd.paper.height_mm, 20.0..=2000.0, 1.0, " mm");
    if before != (nd.paper.width_mm, nd.paper.height_mm) { nd.paper.name = tm!("Personalizado").into(); }
    drag(ui, &tr!("Margen"), &mut nd.paper.margin_mm, 0.0..=60.0, 0.2, " mm");
    ui.add_space(8.0);
    theme::section(ui, &tr!("COLOR DEL PAPEL (VISTA PREVIA)"));
    ui.horizontal(|ui| {
        for (i, (name, color)) in PAPER_TINTS.iter().enumerate() {
            let response = theme::swatch(ui, *color, 22.0).on_hover_text(crate::i18n::tr_key(name));
            if prefs.paper_tint == i { ui.painter().circle_stroke(response.rect.center(), 13.5, egui::Stroke::new(2.0_f32, pal.accent)); }
            if response.clicked() { prefs.paper_tint = i; }
        }
    });
    let nd = &mut prefs.new_document;
    ui.add_space(8.0);
    theme::section(ui, &tr!("PLOTTER"));
    drag(ui, &tr!("Dibujo"), &mut nd.plot.draw_speed, 1.0..=300.0, 0.5, " mm/s");
    drag(ui, &tr!("Viaje"), &mut nd.plot.travel_speed, 1.0..=500.0, 0.5, " mm/s");
    drag(ui, &tr!("Subir/bajar"), &mut nd.plot.lift_time, 0.0..=3.0, 0.01, " s");
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.button(tr!("Usar los de este documento")).on_hover_text(tr!("Papel, velocidades y ajustes de salida del proyecto abierto")).clicked() {
            *nd = NewDocument::from_document(current);
        }
        if ui.button(tr!("Restablecer")).clicked() { *nd = NewDocument::default(); }
    });
    ui.label(RichText::new(tr!("Solo afecta a los documentos nuevos; cada proyecto guarda los suyos.")).small().color(pal.tertiary));
}

fn output(ui: &mut egui::Ui, pal: &Palette, prefs: &mut Preferences) {
    theme::section(ui, &tr!("PLOTTER"));
    row(ui, &tr!("Nombre"), |ui| { ui.add(egui::TextEdit::singleline(&mut prefs.plotter_name).desired_width(180.0)); });
    ui.add_space(8.0);
    theme::section(ui, &tr!("VALORES INICIALES DE SALIDA"));
    let out = &mut prefs.new_document.output;
    drag(ui, &tr!("Tiempo de secado"), &mut out.dry_seconds, 1.0..=180.0, 0.5, " s");
    row(ui, &tr!("Pasadas húmedas"), |ui| { ui.add(egui::DragValue::new(&mut out.max_wet_passes).range(1..=10)); });
    drag(ui, &tr!("Empape"), &mut out.soak_mm, 0.0..=2.0, 0.01, " mm");
    drag(ui, &tr!("Cambio de tinta"), &mut out.pen_change_seconds, 0.0..=300.0, 1.0, " s");
    ui.label(RichText::new(tr!("Solo afectan a documentos nuevos; cada proyecto guarda los suyos.")).small().color(pal.tertiary));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_text(shape: &egui::Shape, label: &str, screen: egui::Rect) -> bool {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == label => screen.contains_rect(egui::Rect::from_min_size(t.pos, t.galley.size())),
            egui::Shape::Vec(v) => v.iter().any(|s| has_text(s, label, screen)),
            _ => false,
        }
    }

    #[test]
    fn every_tab_fits_the_smallest_window_and_untouched_settings_report_no_change() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(980.0, 620.0));
        for tab in [Tab::General, Tab::NewDocument, Tab::Output] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let pal = theme::current(&ctx);
            let prefs = Preferences::default();
            let doc = Document::default();
            let mut ui = PrefsUi { tab, ..Default::default() };
            ui.show_window(&prefs);
            let mut changed = None;
            let mut out = None;
            for _ in 0..2 {
                out = Some(ctx.run(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ctx| { changed = ui.show(ctx, &prefs, &doc, &pal); }));
            }
            assert!(changed.is_none(), "ajustes sin tocar no deben cambiar nada");
            assert!(out.unwrap().shapes.iter().any(|s| has_text(&s.shape, "Listo", screen)), "«Listo» fuera de la ventana");
        }
    }
}
