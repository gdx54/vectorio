//! Ajustes físicos de acabado, conservando el editor contextual existente.
use super::*;
const _STYLES: [&str; 4] = [
    tm!("Ondas con halo"),
    tm!("Eco de contorno"),
    tm!("Trazo a mano"),
    tm!("Ondas con gradiente"),
];
pub(super) fn controls(ui: &mut egui::Ui, pass: &mut Pass, app: &VectorioApp) {
    let halo = pass.supports_halo();
    let echo = pass.supports_echo();
    egui::CollapsingHeader::new(tr!("Acabado artístico"))
        .id_salt("vol-finishing")
        .default_open(pass.finish.post() || pass.finish.halo_mm > 0.)
        .show(ui, |ui| {
            if halo {
                scene_panel::slider(
                    ui,
                    &tr!("Halo al ocultarse (mm)"),
                    &mut pass.finish.halo_mm,
                    0.0..=5.,
                );
            }
            if echo {
                ui.label(tr!("Ecos del contorno"));
                ui.add(egui::Slider::new(&mut pass.finish.echo_count, 0..=12));
                if pass.finish.echo_count > 0 {
                    scene_panel::slider(
                        ui,
                        &tr!("Desplazamiento del eco (mm)"),
                        &mut pass.finish.echo_mm,
                        -4.0..=4.,
                    );
                    if pass.finish.echo_mm.abs() < 0.02 {
                        pass.finish.echo_mm = 0.02;
                    }
                    if pass.reading.is_some() {
                        ui.label(tr!("Tinta del eco"));
                        egui::ComboBox::from_id_salt("echo-pen")
                            .selected_text(
                                app.doc
                                    .pens
                                    .get(pass.pen)
                                    .map_or_else(|| tr!("Sin tinta"), crate::i18n::pen_name),
                            )
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for (i, pen) in app.doc.pens.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut pass.pen,
                                        i,
                                        crate::i18n::pen_name(pen),
                                    );
                                }
                            });
                    }
                }
            }
            scene_panel::slider(
                ui,
                &tr!("Temblor de mano (mm)"),
                &mut pass.finish.jitter_mm,
                0.0..=1.,
            );
            if pass.finish.jitter_mm > 0. {
                scene_panel::slider(
                    ui,
                    &tr!("Suavidad del temblor (mm)"),
                    &mut pass.finish.wavelength_mm,
                    0.5..=20.,
                );
            }
            scene_panel::slider(
                ui,
                &tr!("Longitud irregular"),
                &mut pass.finish.irregularity,
                0.0..=1.,
            );
            scene_panel::slider(
                ui,
                &tr!("Pequeños huecos"),
                &mut pass.finish.gap_probability,
                0.0..=0.5,
            );
            if pass.finish.gap_probability > 0. {
                scene_panel::slider(
                    ui,
                    &tr!("Tamaño del hueco (mm)"),
                    &mut pass.finish.gap_mm,
                    0.1..=3.,
                );
            }
            if pass.finish.variation() {
                ui.label(tr!("Semilla del acabado"));
                ui.add(egui::DragValue::new(&mut pass.finish.seed));
            }
        });
}
