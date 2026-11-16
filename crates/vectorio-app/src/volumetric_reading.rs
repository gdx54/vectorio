//! Controles de medida, codificación y rampa; una sola edición por pasada.
use super::*;
const _MEASURES: [&str; 11] = [
    tm!("Altura"),
    tm!("Profundidad"),
    tm!("Distancia geodésica"),
    tm!("Luz"),
    tm!("Curvatura media"),
    tm!("Curvatura gaussiana"),
    tm!("Pendiente"),
    tm!("Orientación de pendiente"),
    tm!("Oclusión ambiental"),
    tm!("Espesor"),
    tm!("Objetos"),
];
const _STYLES: [&str; 17] = [
    tm!("Mapa de curvatura"),
    tm!("Mapa gaussiano"),
    tm!("Mapa de luz"),
    tm!("Mapa de pendientes"),
    tm!("Luz ambiente"),
    tm!("Mapa térmico"),
    tm!("Hachuras de relieve"),
    tm!("Mapa de agujas"),
    tm!("Líneas de horizonte"),
    tm!("Crestas y valles"),
    tm!("Anaglifo"),
    tm!("Desarrollo"),
    tm!("Estela de giro"),
    tm!("Sección técnica"),
    tm!("Isolíneas de curvatura"),
    tm!("Isolíneas de luz"),
    tm!("Mezcla óptica térmica"),
];
pub(super) fn name(effect: &ReadingEffect) -> String {
    crate::i18n::tr_key(match effect {
        ReadingEffect::Surface => tm!("Mapa de lectura"),
        ReadingEffect::Relief { .. } => tm!("Hachuras de relieve"),
        ReadingEffect::Needles { .. } => tm!("Mapa de agujas"),
        ReadingEffect::Horizons { .. } => tm!("Líneas de horizonte"),
        ReadingEffect::Ridges { .. } => tm!("Crestas y valles"),
        ReadingEffect::Anaglyph { .. } => tm!("Anaglifo"),
        ReadingEffect::Unfold { .. } => tm!("Desarrollo"),
        ReadingEffect::Spin { .. } => tm!("Estela de giro"),
        ReadingEffect::Section { .. } => tm!("Sección técnica"),
    })
}
pub(super) fn fixed(kind: &PassKind) -> bool {
    matches!(
        kind,
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Anaglyph { .. }
                | ReadingEffect::Unfold { .. }
                | ReadingEffect::Spin { .. }
                | ReadingEffect::Section { .. }
                | ReadingEffect::Ridges { .. },
            ..
        })
    )
}
pub(super) fn transformed(kind: &PassKind) -> bool {
    matches!(
        kind,
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Unfold { .. }
                | ReadingEffect::Anaglyph { .. }
                | ReadingEffect::Spin { .. }
                | ReadingEffect::Section { .. }
                | ReadingEffect::Horizons { .. },
            ..
        })
    )
}

const _INK_SETS: [&str; 5] = [
    tm!("Grafito"),
    tm!("Cuatricromía"),
    tm!("Colores primarios"),
    tm!("Térmica"),
    tm!("Tierra y añil"),
];
fn ink_sets(
    ui: &mut egui::Ui,
    map: &mut ReadingMap,
    doc: &mut vectorio_core::Document,
    minimum: usize,
) {
    let selected = InkSet::ALL.into_iter().find(|s| s.matches(map, &doc.pens));
    let mut chosen = None;
    ui.label(tr!("Juego de tintas"));
    egui::ComboBox::from_id_salt("reading-ink-set")
        .selected_text(
            selected.map_or_else(|| tr!("Personalizado"), |s| crate::i18n::tr_key(s.name())),
        )
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for set in InkSet::ALL {
                let pens = set.pens();
                let valid = map.encoding == Encoding::Density || pens.len() >= minimum;
                let r = ui.add_enabled(
                    valid,
                    egui::Button::new(crate::i18n::tr_key(set.name()))
                        .selected(selected == Some(set)),
                );
                if r.clicked() {
                    chosen = Some(set);
                    ui.close_menu();
                }
                r.on_hover_ui(|ui| {
                    ui.horizontal(|ui| {
                        for pen in pens {
                            theme::swatch(ui, pen.color, 16.);
                        }
                    });
                });
            }
        });
    if let Some(set) = chosen {
        map.pens = set.apply(doc);
        if map.encoding == Encoding::Density {
            map.pens.truncate(1);
        }
        map.bind_palette(&doc.pens);
    }
}
fn measure(ui: &mut egui::Ui, id: &str, value: &mut Measure) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(crate::i18n::tr_key(value.name()))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for kind in Measure::ALL {
                ui.selectable_value(value, kind, crate::i18n::tr_key(kind.name()));
            }
        });
}
pub(super) fn controls(ui: &mut egui::Ui, pass: &mut Pass, app: &mut VectorioApp) -> bool {
    let Some(map) = &mut pass.reading else {
        return false;
    };
    let fixed = fixed(&pass.kind);
    let minimum = match &pass.kind {
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Anaglyph { .. } | ReadingEffect::Ridges { .. },
            ..
        }) => 2,
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Unfold { .. } | ReadingEffect::Section { .. },
            ..
        }) => 3,
        _ => 1,
    };
    if let PassKind::Reading(ReadingParams {
        effect: ReadingEffect::Section {
            show_dimensions, ..
        },
        ..
    }) = &mut pass.kind
    {
        ui.checkbox(show_dimensions, tr!("Mostrar cotas XYZ"));
    }
    if !fixed {
        ui.label(tr!("Medida"));
        let before = map.measure;
        measure(ui, "read-measure", &mut map.measure);
        if before != map.measure {
            map.secondary = None;
            map.range = None;
            map.threshold = None;
            map.equalize = false;
            map.invert = false;
            map.ramp = if map.measure == Measure::Objects {
                map.encoding = Encoding::Bands;
                Ramp::Categorical
            } else if matches!(
                map.measure,
                Measure::MeanCurvature | Measure::GaussianCurvature
            ) {
                Ramp::Diverging
            } else {
                Ramp::Sequential
            };
            if map.ramp == Ramp::Diverging && map.pens.len() < 2 {
                map.pens.push(pass.pen);
            }
        }
        if map.measure != Measure::Objects {
            ui.label(tr!("Codificación"));
            let before = map.encoding;
            egui::ComboBox::from_id_salt("read-encoding")
                .selected_text(match map.encoding {
                    Encoding::Bands => tr!("Fronteras exactas"),
                    Encoding::Dither => tr!("Transición difuminada"),
                    Encoding::Density => tr!("Densidad de trazos"),
                    Encoding::Optical => tr!("Mezcla óptica"),
                    Encoding::Isolines => tr!("Isolíneas del campo"),
                })
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut map.encoding,
                        Encoding::Bands,
                        tr!("Fronteras exactas"),
                    );
                    ui.selectable_value(
                        &mut map.encoding,
                        Encoding::Dither,
                        tr!("Transición difuminada"),
                    );
                    ui.selectable_value(
                        &mut map.encoding,
                        Encoding::Density,
                        tr!("Densidad de trazos"),
                    );
                    if matches!(
                        pass.kind,
                        PassKind::Reading(ReadingParams {
                            effect: ReadingEffect::Surface,
                            ..
                        })
                    ) {
                        ui.selectable_value(
                            &mut map.encoding,
                            Encoding::Optical,
                            tr!("Mezcla óptica"),
                        );
                        ui.selectable_value(
                            &mut map.encoding,
                            Encoding::Isolines,
                            tr!("Isolíneas del campo"),
                        );
                    }
                });
            if map.encoding == Encoding::Density {
                map.ramp = Ramp::Sequential;
                map.pens.truncate(1);
            } else if before == Encoding::Density {
                map.pens.push(pass.pen);
            }
            if map.encoding != Encoding::Density {
                ui.label(if map.encoding == Encoding::Optical {
                    tr!("Distribución del campo")
                } else {
                    tr!("Rampa")
                });
                egui::ComboBox::from_id_salt("read-ramp")
                    .selected_text(if map.ramp == Ramp::Sequential {
                        if map.encoding == Encoding::Optical {
                            tr!("Secuencial")
                        } else {
                            tr!("Por luminosidad")
                        }
                    } else {
                        tr!("Divergente · papel en cero")
                    })
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut map.ramp,
                            Ramp::Sequential,
                            if map.encoding == Encoding::Optical {
                                tr!("Secuencial")
                            } else {
                                tr!("Por luminosidad")
                            },
                        );
                        ui.selectable_value(
                            &mut map.ramp,
                            Ramp::Diverging,
                            tr!("Divergente · papel en cero"),
                        );
                    });
                if map.ramp == Ramp::Diverging && map.pens.len() < 2 {
                    map.pens.push(pass.pen);
                }
            }
        }
    }
    let min = minimum.max(if map.ramp == Ramp::Diverging { 2 } else { 1 });
    ink_sets(ui, map, &mut app.doc, min);
    if map.encoding == Encoding::Optical {
        ui.label(tr!("Mapa de color"));
        egui::ComboBox::from_id_salt("optical-colors")
            .selected_text(match map.optical.colors {
                ColorMap::Thermal => tr!("Térmica"),
                ColorMap::Gray => tr!("Grises"),
                ColorMap::CoolWarm => tr!("Frío y cálido"),
            })
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut map.optical.colors, ColorMap::Thermal, tr!("Térmica"));
                ui.selectable_value(&mut map.optical.colors, ColorMap::Gray, tr!("Grises"));
                ui.selectable_value(
                    &mut map.optical.colors,
                    ColorMap::CoolWarm,
                    tr!("Frío y cálido"),
                );
            });
    }
    if map.encoding == Encoding::Isolines {
        ui.label(tr!("Niveles del campo"));
        ui.add(egui::Slider::new(&mut map.levels, 2..=96));
        ui.checkbox(&mut map.label_values, tr!("Rotular valores"));
    }
    let family_id = ui.id().with("reading-active-family");
    let mut active = ui
        .ctx()
        .data(|d| d.get_temp::<usize>(family_id))
        .unwrap_or(0)
        .min(map.pens.len().saturating_sub(1));
    let tint_response = ui.menu_button(tr!("Tintas · %{n}", n = map.pens.len()), |ui| {
        ui.set_min_width(240.);
        ui.set_max_width(280.);
        if map.encoding != Encoding::Density || fixed {
            let mut count = map.pens.len().max(min);
            ui.label(tr!("Familias de tinta"));
            if ui.add(egui::Slider::new(&mut count, min..=8)).changed() {
                map.pens.resize_with(count, || pass.pen);
                active = active.min(count - 1);
            }
        }
        ui.horizontal_wrapped(|ui| {
            for (i, pen) in map.pens.iter().enumerate() {
                let label = tr!("Familia %{n}", n = i + 1);
                if ui.selectable_label(active == i, &label).clicked() {
                    active = i;
                }
                if let Some(p) = app.doc.pens.get(*pen) {
                    theme::swatch(ui, p.color, 10.).on_hover_text(crate::i18n::pen_name(p));
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("reading-palette")
            .max_height(260.)
            .show(ui, |ui| {
                for (index, pen) in app.doc.pens.iter().enumerate() {
                    ui.horizontal(|ui| {
                        theme::swatch(ui, pen.color, 14.);
                        if ui
                            .selectable_label(map.pens[active] == index, crate::i18n::pen_name(pen))
                            .clicked()
                        {
                            map.pens[active] = index;
                        }
                    });
                }
            });
    });
    ui.ctx().data_mut(|d| d.insert_temp(family_id, active));
    map.bind_palette(&app.doc.pens);
    let menu_open = egui::menu::BarState::load(ui.ctx(), ui.id()).is_some();
    if map.ramp == Ramp::Sequential && map.encoding != Encoding::Optical && !fixed {
        tint_response
            .response
            .on_hover_text(tr!("De oscuro a claro, según las tintas reales."));
    }
    let ordered = map.ordered_pens(&app.doc.pens);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 8.), Sense::hover());
    if map.encoding == Encoding::Optical {
        for i in 0..64 {
            let color = theme::pen_color(map.target_color(i as f64 / 63.));
            ui.painter().rect_filled(
                Rect::from_min_size(
                    rect.min + egui::vec2(i as f32 * rect.width() / 64., 0.),
                    egui::vec2(rect.width() / 64. + 0.5, 8.),
                ),
                0.,
                color,
            );
        }
    } else {
        let width = rect.width() / ordered.len().max(1) as f32;
        for (i, pen) in ordered.iter().enumerate() {
            let color = app
                .doc
                .pens
                .get(*pen)
                .map_or(Color32::BLACK, |p| theme::pen_color(p.color));
            ui.painter().rect_filled(
                Rect::from_min_size(
                    rect.min + egui::vec2(i as f32 * width, 0.),
                    egui::vec2(width, 8.),
                ),
                0.,
                color,
            );
        }
    }
    if !fixed {
        if map.encoding != Encoding::Isolines
            && matches!(
                pass.kind,
                PassKind::Reading(ReadingParams {
                    effect: ReadingEffect::Surface
                        | ReadingEffect::Relief { .. }
                        | ReadingEffect::Needles { .. },
                    ..
                })
            )
        {
            scene_panel::slider(
                ui,
                &if map.encoding == Encoding::Optical {
                    tr!("Escala de trama (mm)")
                } else {
                    tr!("Separación de trazos (mm)")
                },
                &mut map.spacing_mm,
                0.1..=8.,
            );
        }
        if map.measure != Measure::Objects {
            ui.checkbox(&mut map.invert, tr!("Invertir medida"));
        }
    }
    let before = map.legend;
    ui.checkbox(&mut map.legend, tr!("Cartela en el papel"));
    if map.legend && !before && map.date.is_empty() {
        map.date = today();
    }
    egui::CollapsingHeader::new(tr!("Avanzado"))
        .id_salt("read-advanced")
        .show(ui, |ui| {
            if !fixed && map.measure != Measure::Objects {
                ui.checkbox(&mut map.equalize, tr!("Ecualizar por histograma"));
                let mut combine = map.secondary.is_some();
                if ui
                    .checkbox(&mut combine, tr!("Combinar otra medida"))
                    .changed()
                {
                    map.secondary = combine.then_some(Measure::Slope);
                }
                if let Some(second) = &mut map.secondary {
                    measure(ui, "read-second", second);
                    if *second == Measure::Objects {
                        *second = Measure::Slope;
                    }
                    for (i, weight) in map.weights.iter_mut().enumerate() {
                        scene_panel::slider(
                            ui,
                            &tr!("Peso de medida %{n}", n = i + 1),
                            weight,
                            -10.0..=10.,
                        );
                    }
                }
                let mut manual = map.range.is_some();
                if ui.checkbox(&mut manual, tr!("Rango manual")).changed() {
                    map.range = manual.then_some([-1., 1.]);
                }
                if let Some(range) = &mut map.range {
                    if map.ramp == Ramp::Diverging {
                        let mut extent = range[0].abs().max(range[1].abs()).max(0.01);
                        ui.label(tr!("Límite absoluto"));
                        ui.add(
                            egui::DragValue::new(&mut extent)
                                .speed(0.01)
                                .range(0.01..=1e6),
                        );
                        *range = [-extent, extent];
                    } else {
                        ui.label(tr!("Mínimo / máximo"));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut range[0]).speed(0.01));
                            ui.add(egui::DragValue::new(&mut range[1]).speed(0.01));
                        });
                        if range[1] <= range[0] {
                            range[1] = range[0] + 0.01;
                        }
                    }
                }
                let mut binary = map.threshold.is_some();
                if ui.checkbox(&mut binary, tr!("Umbral de medida")).changed() {
                    map.threshold = binary.then_some(0.);
                }
                if let Some(threshold) = &mut map.threshold {
                    ui.add(egui::DragValue::new(threshold).speed(0.01));
                }
                if matches!(
                    pass.kind,
                    PassKind::Reading(ReadingParams {
                        effect: ReadingEffect::Surface,
                        ..
                    })
                ) {
                    scene_panel::slider(
                        ui,
                        &tr!("Ángulo de trazos"),
                        &mut map.angle,
                        -180.0..=180.,
                    );
                }
                if map.measure == Measure::AmbientOcclusion
                    || map.secondary == Some(Measure::AmbientOcclusion)
                {
                    ui.label(tr!("Muestras de oclusión ambiental"));
                    ui.add(egui::Slider::new(&mut map.samples, 8..=128));
                }
            }
            if let PassKind::Reading(params) = &mut pass.kind {
                ui.label(tr!("Facetas"));
                let maximum = if matches!(params.effect, ReadingEffect::Unfold { .. }) {
                    512
                } else {
                    8000
                };
                ui.add(egui::Slider::new(&mut params.facets, 64..=maximum).logarithmic(true));
            }
            if map.legend {
                ui.label(tr!("Fecha de la cartela (UTC)"));
                ui.text_edit_singleline(&mut map.date);
            }
        });
    menu_open
}

pub(super) fn effect_controls(ui: &mut egui::Ui, params: &mut ReadingParams) {
    match &mut params.effect {
        ReadingEffect::Surface => {}
        ReadingEffect::Relief { length_mm } => {
            scene_panel::slider(ui, &tr!("Longitud de hachura (mm)"), length_mm, 0.2..=12.)
        }
        ReadingEffect::Needles { size_mm } => {
            scene_panel::slider(ui, &tr!("Longitud de aguja (mm)"), size_mm, 0.2..=12.)
        }
        ReadingEffect::Horizons { height_mm, rows } => {
            scene_panel::slider(ui, &tr!("Relieve del perfil (mm)"), height_mm, -80.0..=80.);
            ui.label(tr!("Perfiles"));
            ui.add(egui::Slider::new(rows, 4..=240));
        }
        ReadingEffect::Ridges { threshold } => {
            scene_panel::slider(ui, &tr!("Umbral de crestas"), threshold, 0.0..=1.)
        }
        ReadingEffect::Anaglyph {
            angle_deg,
            side_by_side,
        } => {
            scene_panel::slider(
                ui,
                &tr!("Separación de cámaras (grados)"),
                angle_deg,
                0.5..=8.,
            );
            ui.checkbox(side_by_side, tr!("Par estereoscópico"));
        }
        ReadingEffect::Unfold { tabs_mm } => {
            scene_panel::slider(ui, &tr!("Pestañas de pegado (mm)"), tabs_mm, 0.5..=8.)
        }
        ReadingEffect::Spin { angle_deg, copies } => {
            scene_panel::slider(
                ui,
                &tr!("Giro entre vistas (grados)"),
                angle_deg,
                -180.0..=180.,
            );
            ui.label(tr!("Vistas"));
            ui.add(egui::Slider::new(copies, 2..=12));
        }
        ReadingEffect::Section {
            direction, offset, ..
        } => {
            ui.horizontal_wrapped(|ui| {
                for (label, n) in [
                    ("X", [1., 0., 0.]),
                    ("Y", [0., 1., 0.]),
                    ("Z", [0., 0., 1.]),
                ] {
                    ui.selectable_value(direction, n, label);
                }
            });
            scene_panel::slider(ui, &tr!("Posición del corte"), offset, -0.95..=0.95);
        }
    }
}
fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86400) as i64;
    date_from_days(days)
}
fn date_from_days(days: i64) -> String {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legend_dates_are_stable_iso_dates() {
        assert_eq!(date_from_days(0), "1970-01-01");
        assert_eq!(date_from_days(19782), "2024-02-29");
    }
}
#[cfg(test)]
mod palette_tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        app: &mut VectorioApp,
        pass: &mut Pass,
        time: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(980., 620.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(240.);
                    ui.set_max_width(240.);
                    controls(ui, pass, app);
                });
            },
        )
    }
    fn point(out: &egui::FullOutput, label: &str) -> egui::Pos2 {
        out.shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    let r = Rect::from_min_size(t.pos, t.galley.size());
                    (t.galley.text() == label && s.clip_rect.contains(r.center()))
                        .then_some(r.center())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("No aparece {label}"))
    }
    fn click(
        ctx: &egui::Context,
        app: &mut VectorioApp,
        pass: &mut Pass,
        time: &mut f64,
        pos: egui::Pos2,
    ) -> egui::FullOutput {
        *time += 0.05;
        let _ = frame(
            ctx,
            app,
            pass,
            *time,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        *time += 0.05;
        frame(
            ctx,
            app,
            pass,
            *time,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        )
    }
    #[test]
    fn each_document_color_can_be_selected_in_each_family_without_nested_popup_dismissal() {
        let _lock = crate::i18n::locale_lock();
        for lang in ["es", "en"] {
            rust_i18n::set_locale(lang);
            let ctx = egui::Context::default();
            let mut app = VectorioApp::blank(&ctx);
            let mut pass = ReadingStyle::Thermal.passes(&Settings::default()).remove(0);
            let map = pass.reading.as_mut().unwrap();
            map.pens = vec![0, 1, 2];
            let mut time = 0.;
            let out = frame(&ctx, &mut app, &mut pass, time, vec![]);
            let pos = point(&out, &tr!("Tintas · %{n}", n = 3));
            let _ = click(&ctx, &mut app, &mut pass, &mut time, pos);
            time += 0.1;
            let mut out = frame(&ctx, &mut app, &mut pass, time, vec![]);
            for family in 0..3 {
                let pos = point(&out, &tr!("Familia %{n}", n = family + 1));
                let _ = click(&ctx, &mut app, &mut pass, &mut time, pos);
                time += 0.1;
                out = frame(&ctx, &mut app, &mut pass, time, vec![]);
                for pen in 0..app.doc.pens.len() {
                    let pos = point(&out, &crate::i18n::pen_name(&app.doc.pens[pen]));
                    let _ = click(&ctx, &mut app, &mut pass, &mut time, pos);
                    time += 0.1;
                    out = frame(&ctx, &mut app, &mut pass, time, vec![]);
                    assert_eq!(
                        pass.reading.as_ref().unwrap().pens[family],
                        pen,
                        "familia {family}, tinta {pen} ({lang})"
                    );
                }
            }
        }
        rust_i18n::set_locale("es");
    }
}
#[cfg(test)]
mod extra_controls_tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        app: &mut VectorioApp,
        pass: &mut Pass,
        time: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                time: Some(time),
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(980., 620.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(240.);
                    ui.set_max_width(240.);
                    controls(ui, pass, app);
                });
            },
        )
    }
    fn point(out: &egui::FullOutput, label: &str) -> egui::Pos2 {
        out.shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    (t.galley.text() == label).then_some(t.pos + t.galley.size() * 0.5)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("Falta {label}"))
    }
    fn click(
        ctx: &egui::Context,
        app: &mut VectorioApp,
        pass: &mut Pass,
        time: f64,
        pos: egui::Pos2,
    ) -> egui::FullOutput {
        let _ = frame(
            ctx,
            app,
            pass,
            time,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        frame(
            ctx,
            app,
            pass,
            time + 0.05,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        )
    }
    #[test]
    fn the_section_dimensions_checkbox_removes_the_xyz_annotation() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let mut pass = ReadingStyle::Section.passes(&Settings::default()).remove(0);
        let out = frame(&ctx, &mut app, &mut pass, 0., vec![]);
        let pos = point(&out, "Mostrar cotas XYZ");
        let _ = click(&ctx, &mut app, &mut pass, 0.1, pos);
        assert!(matches!(
            pass.kind,
            PassKind::Reading(ReadingParams {
                effect: ReadingEffect::Section {
                    show_dimensions: false,
                    ..
                },
                ..
            })
        ));
    }
    #[test]
    fn a_preset_applies_all_families_and_they_remain_individually_editable() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let mut pass = ReadingStyle::OpticalThermal
            .passes(&Settings::default())
            .remove(0);
        let original = app.doc.pens.clone();
        let out = frame(&ctx, &mut app, &mut pass, 0., vec![]);
        let pos = point(&out, "Personalizado");
        let _ = click(&ctx, &mut app, &mut pass, 0.1, pos);
        let out = frame(&ctx, &mut app, &mut pass, 0.3, vec![]);
        let pos = point(&out, "Cuatricromía");
        let _ = click(&ctx, &mut app, &mut pass, 0.4, pos);
        let map = pass.reading.as_ref().unwrap();
        assert!(InkSet::Cmyk.matches(map, &app.doc.pens));
        assert_eq!(map.optical.inks.len(), 4);
        assert_eq!(&app.doc.pens[..original.len()], original);
        pass.reading.as_mut().unwrap().pens[0] = 3;
        let _ = frame(&ctx, &mut app, &mut pass, 0.7, vec![]);
        assert!(!InkSet::Cmyk.matches(pass.reading.as_ref().unwrap(), &app.doc.pens));
        assert_eq!(
            pass.reading.as_ref().unwrap().optical.inks[0].color,
            app.doc.pens[3].color
        );
    }
}
