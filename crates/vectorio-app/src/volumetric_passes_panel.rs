//! Lista de pasadas y editor contextual. Un único plegable Avanzado, sin anidar.
use super::*;
pub(super) fn show(ui: &mut egui::Ui, state: &mut VolumetricUi, app: &mut VectorioApp) {
    let pal = theme::current(ui.ctx());
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    ui.spacing_mut().slider_width = (ui.available_width() - 65.).max(75.);
    ui.horizontal(|ui| {
        ui.weak(tr!("Pasadas"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(tr!("Ver todas")).clicked() {
                state.selected = None;
            }
        });
    });
    ui.add_space(8.);
    if state.passes.is_empty() {
        ui.weak(tr!("Añade una pasada para empezar."));
    }
    let mut toggle = None;
    let mut reorder = None;
    for (index, entry) in state.passes.iter().enumerate() {
        let selected = state.selected == Some(entry.id);
        let row = egui::Frame::none()
            .fill(if selected {
                pal.control
            } else {
                Color32::TRANSPARENT
            })
            .rounding(6.)
            .inner_margin(egui::Margin::symmetric(3., 4.))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.;
                    let (eye, response) =
                        ui.allocate_exact_size(egui::vec2(22., 24.), Sense::click());
                    let stroke = Stroke::new(
                        1.2f32,
                        if entry.pass.enabled {
                            pal.text
                        } else {
                            pal.secondary
                        },
                    );
                    let c = eye.center();
                    ui.painter().add(egui::Shape::closed_line(
                        vec![
                            c + egui::vec2(-8., 0.),
                            c + egui::vec2(-4., -4.),
                            c + egui::vec2(4., -4.),
                            c + egui::vec2(8., 0.),
                            c + egui::vec2(4., 4.),
                            c + egui::vec2(-4., 4.),
                        ],
                        stroke,
                    ));
                    ui.painter().circle_stroke(c, 2., stroke);
                    if !entry.pass.enabled {
                        ui.painter().line_segment(
                            [c + egui::vec2(-8., 7.), c + egui::vec2(8., -7.)],
                            stroke,
                        );
                    }
                    let eye_label = if entry.pass.enabled {
                        tr!("Ocultar pasada")
                    } else {
                        tr!("Mostrar pasada")
                    };
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            ui.is_enabled(),
                            &eye_label,
                        )
                    });
                    theme::focus_ring(ui, &response);
                    if response.on_hover_text(eye_label).clicked() {
                        toggle = Some(index);
                    }
                    let (swatch, _) = ui.allocate_exact_size(egui::vec2(12., 24.), Sense::hover());
                    let color = app
                        .doc
                        .pens
                        .get(entry.pass.pen)
                        .map_or(Color32::BLACK, |p| theme::pen_color(p.color));
                    if let Some(map) = &entry.pass.reading {
                        let pens = map.ordered_pens(&app.doc.pens);
                        let height = 12. / pens.len().max(1) as f32;
                        for (i, pen) in pens.iter().enumerate() {
                            let color = app
                                .doc
                                .pens
                                .get(*pen)
                                .map_or(Color32::BLACK, |p| theme::pen_color(p.color));
                            ui.painter().rect_filled(
                                Rect::from_min_size(
                                    swatch.center() + egui::vec2(-4.5, -6. + i as f32 * height),
                                    egui::vec2(9., height),
                                ),
                                0.,
                                color,
                            );
                        }
                    } else {
                        ui.painter().circle_filled(swatch.center(), 4.5, color);
                    }
                    let label = pass_name(&entry.pass.name);
                    let response = ui.add_sized(
                        [ui.available_width() - 16., 24.],
                        egui::Label::new(RichText::new(label).color(if entry.pass.enabled {
                            pal.text
                        } else {
                            pal.secondary
                        }))
                        .truncate()
                        .sense(Sense::click_and_drag()),
                    );
                    response.dnd_set_drag_payload(entry.id);
                    if response.clicked() {
                        state.selected = Some(entry.id);
                    }
                    let (handle, drag) =
                        ui.allocate_exact_size(egui::vec2(12., 24.), Sense::drag());
                    for y in [-3., 0., 3.] {
                        ui.painter().line_segment(
                            [
                                handle.center() + egui::vec2(-3., y),
                                handle.center() + egui::vec2(3., y),
                            ],
                            Stroke::new(1.0f32, pal.secondary),
                        );
                    }
                    drag.dnd_set_drag_payload(entry.id);
                });
            })
            .response;
        if let Some(payload) = row.dnd_release_payload::<u64>() {
            reorder = Some((*payload, index));
        }
    }
    if let Some(i) = toggle {
        state.checkpoint();
        state.passes[i].pass.enabled = !state.passes[i].pass.enabled;
    }
    if let Some((id, index)) = reorder {
        state.move_pass(id, index);
    }
    let _add_menu = ui.menu_button(tr!("Añadir pasada"), |ui| {
        for kind in 0..10 {
            let pass = make_pass(kind, &state.settings);
            if ui.button(type_name(&pass.kind)).clicked() {
                state.checkpoint();
                let id = state.next_pass_id;
                state.next_pass_id += 1;
                state.passes.push(UiPass { id, pass });
                state.selected = Some(id);
                ui.close_menu();
            }
        }
        ui.separator();
        ui.menu_button(tr!("Añadir mapa de lectura"), |ui| {
            for style in ReadingStyle::ALL {
                if ui.button(crate::i18n::tr_key(style.name())).clicked() {
                    state.checkpoint();
                    let mut pass = style.passes(&state.settings).remove(0);
                    if let Some(map) = &mut pass.reading {
                        for pen in &mut map.pens {
                            *pen = (*pen).min(app.doc.pens.len().saturating_sub(1));
                        }
                    }
                    let id = state.next_pass_id;
                    state.next_pass_id += 1;
                    state.passes.push(UiPass { id, pass });
                    state.selected = Some(id);
                    ui.close_menu();
                }
            }
        });
    });
    state.menu_open |= egui::menu::BarState::load(ui.ctx(), ui.id()).is_some();
    ui.add_space(10.);
    ui.separator();
    ui.add_space(6.);
    let Some(index) = state
        .selected
        .and_then(|id| state.passes.iter().position(|p| p.id == id))
    else {
        ui.weak(tr!("Selecciona una pasada para editarla."));
        return;
    };
    let snapshot = state.snapshot();
    let before = state.passes[index].pass.clone();
    let id = state.passes[index].id;
    let mut delete = false;
    let mut duplicate = false;
    ui.horizontal(|ui| {
        ui.weak(type_name(&before.kind));
        let _actions_menu = ui.menu_button(tr!("Acciones"), |ui| {
            {
                let pass = &mut state.passes[index].pass;
                if !matches!(pass.kind, PassKind::Reading(_)) {
                    let mut enabled = pass.reading.is_some();
                    if ui.checkbox(&mut enabled, tr!("Mapa de lectura")).changed() {
                        pass.reading = enabled.then(|| ReadingMap {
                            pens: vec![pass.pen, pass.pen],
                            ..Default::default()
                        });
                    }
                }
            }

            {
                let pass = &mut state.passes[index].pass;
                if !reading_ui::fixed(&pass.kind) {
                    ui.menu_button(tr!("Aplicar gradiente de tintas"), |ui| {
                        for (label, measure) in [
                            (tm!("Altura"), Measure::Height),
                            (tm!("Profundidad"), Measure::Depth),
                            (tm!("Luz"), Measure::Light),
                        ] {
                            if ui.button(crate::i18n::tr_key(label)).clicked() {
                                let other = (pass.pen + 1) % app.doc.pens.len().max(1);
                                pass.reading = Some(ReadingMap {
                                    measure,
                                    encoding: Encoding::Dither,
                                    ramp: Ramp::Sequential,
                                    pens: vec![pass.pen, other],
                                    ..Default::default()
                                });
                                ui.close_menu();
                            }
                        }
                    });
                }
            }
            duplicate = ui.button(tr!("Duplicar pasada")).clicked();
            delete = ui.button(tr!("Eliminar pasada")).clicked();
            if duplicate || delete {
                ui.close_menu();
            }
            if ui
                .add_enabled(index > 0, egui::Button::new(tr!("Subir pasada")))
                .clicked()
            {
                reorder = Some((id, index - 1));
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    index + 1 < state.passes.len(),
                    egui::Button::new(tr!("Bajar pasada")),
                )
                .clicked()
            {
                reorder = Some((id, index + 1));
                ui.close_menu();
            }
        });
        state.menu_open |= egui::menu::BarState::load(ui.ctx(), ui.id()).is_some();
    });
    let planar = planar_share(ui.ctx(), &state.mesh);
    let pass = &mut state.passes[index].pass;
    ui.add(
        egui::TextEdit::singleline(&mut pass.name)
            .desired_width(ui.available_width())
            .hint_text(tr!("Nombre de la pasada")),
    );
    // Tres bloques fijos: qué se lee del modelo, cómo se ve en el papel y el resto plegado.
    block(ui, &tr!("Qué dibuja"));
    ui.push_id(("reading-what", id), |ui| reading_ui::what(ui, pass));
    // Dónde dibuja según la luz, antes de los parámetros propios del tipo.
    if !reading_ui::transformed(&pass.kind) {
        mask_controls(ui, &mut pass.mask);
    }
    ui.push_id(id, |ui| parameters(ui, &mut pass.kind));
    if let Some(warning) = suitability(&pass.kind, planar) {
        ui.colored_label(theme::current(ui.ctx()).secondary, warning);
    }
    block(ui, &tr!("Cómo se ve"));
    if pass.reading.is_none() {
        ui.label(tr!("Tinta"));
        egui::ComboBox::from_id_salt(("vol-pass-pen", id))
            .selected_text(
                app.doc
                    .pens
                    .get(pass.pen)
                    .map_or_else(|| tr!("Sin tinta"), crate::i18n::pen_name),
            )
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (i, pen) in app.doc.pens.iter().enumerate() {
                    ui.selectable_value(&mut pass.pen, i, crate::i18n::pen_name(pen));
                }
            });
    }
    state.menu_open |= ui
        .push_id(("reading", id), |ui| reading_ui::look(ui, pass, app, id))
        .inner;
    if !matches!(pass.kind, PassKind::ToneHatch(_) | PassKind::Reading(_)) {
        ui.label(tr!("Visibilidad"));
        egui::ComboBox::from_id_salt(("vol-visibility", id))
            .selected_text(match pass.visibility {
                Visibility::Visible => tr!("Líneas visibles"),
                Visibility::All => tr!("Todas las líneas"),
                Visibility::Hidden => tr!("Ocultas discontinuas"),
            })
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut pass.visibility,
                    Visibility::Visible,
                    tr!("Líneas visibles"),
                );
                ui.selectable_value(
                    &mut pass.visibility,
                    Visibility::All,
                    tr!("Todas las líneas"),
                );
                if !matches!(
                    pass.kind,
                    PassKind::Hatch { .. } | PassKind::Ground(_) | PassKind::ToneHatch(_)
                ) {
                    ui.selectable_value(
                        &mut pass.visibility,
                        Visibility::Hidden,
                        tr!("Ocultas discontinuas"),
                    );
                }
            });
        if pass.visibility == Visibility::Hidden {
            scene_panel::slider(ui, &tr!("Guion (mm)"), &mut pass.dash.dash_mm, 0.1..=10.);
            scene_panel::slider(ui, &tr!("Hueco (mm)"), &mut pass.dash.gap_mm, 0.1..=10.);
        }
    }
    if !pass.supports_halo() {
        pass.finish.halo_mm = 0.;
    }
    ui.push_id(("finish", id), |ui| finishing_ui::controls(ui, pass, app));
    if pass.reading.is_some() {
        ui.add_space(6.);
        egui::CollapsingHeader::new(tr!("Avanzado"))
            .id_salt(("vol-pass-advanced", id))
            .show(ui, |ui| {
                ui.push_id(("reading-advanced", id), |ui| reading_ui::advanced(ui, pass));
            });
    }
    if *pass != before {
        if state.undo.len() >= 64 {
            state.undo.remove(0);
        }
        state.undo.push(snapshot);
        state.redo.clear();
    }
    if duplicate {
        state.checkpoint();
        let mut entry = state.passes[index].clone();
        entry.id = state.next_pass_id;
        state.next_pass_id += 1;
        state.selected = Some(entry.id);
        state.passes.insert(index + 1, entry);
    }
    if delete {
        state.checkpoint();
        state.passes.remove(index);
        state.selected = None;
    } else if let Some((id, index)) = reorder {
        state.move_pass(id, index);
    }
}
fn block(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.);
    ui.label(egui::RichText::new(title).strong());
    ui.add_space(2.);
}
/// Fracción plana de la malla, calculada una vez por modelo.
fn planar_share(ctx: &egui::Context, mesh: &Arc<Mesh>) -> f64 {
    let key = egui::Id::new(("vol-planar-share", Arc::as_ptr(mesh) as usize, mesh.triangles.len()));
    if let Some(v) = ctx.data(|d| d.get_temp::<f64>(key)) {
        return v;
    }
    let v = mesh.planar_share();
    ctx.data_mut(|d| d.insert_temp(key, v));
    v
}
/// Desarrollo y Sección técnica piden caras planas; en una forma orgánica se avisa.
pub(super) fn suitability(kind: &PassKind, planar: f64) -> Option<String> {
    if planar >= 0.6 {
        return None;
    }
    match kind {
        PassKind::Reading(ReadingParams { effect: ReadingEffect::Unfold { .. }, .. }) => Some(tr!(
            "Este modelo es curvo u orgánico: el desarrollo saldrá en muchas piezas pequeñas. Funciona mejor con piezas de caras planas."
        )),
        PassKind::Reading(ReadingParams { effect: ReadingEffect::Section { .. }, .. }) => Some(tr!(
            "Este modelo es curvo u orgánico: la sección técnica se lee mejor en piezas de caras planas."
        )),
        _ => None,
    }
}
pub(super) fn type_name(kind: &PassKind) -> String {
    match kind {
        PassKind::Reading(p) => reading_ui::name(&p.effect),
        PassKind::Outline(_) => tr!("Contorno"),
        PassKind::Edges { .. } => tr!("Aristas"),
        PassKind::Sections(_) => tr!("Secciones"),
        PassKind::Field(_) => tr!("Curvas de campo"),
        PassKind::Waves(_) => tr!("Ondas"),
        PassKind::Mesh(_) => tr!("Malla"),
        PassKind::Hatch { .. } => tr!("Trama"),
        PassKind::Ground(_) => tr!("Sombra arrojada"),
        PassKind::ToneHatch(p) => match p.mode {
            HatchMode::Parallel => tr!("Trama por niveles"),
            HatchMode::Form => tr!("Trama de forma"),
        },
    }
}
fn make_pass(index: usize, scene: &Settings) -> Pass {
    let base = Settings::default();
    let mut pass = match index {
        0 => Pass {
            name: String::new(),
            enabled: true,
            pen: 0,
            visibility: Visibility::Visible,
            mask: TonalMask::All,
            dash: DashPattern::default(),
            reading: None,
            finish: FinishSettings::default(),
            kind: PassKind::Outline(OutlineParams::Surface {
                facets: scene.art.facets,
            }),
        },
        1 => {
            let mut s = base.clone();
            s.recipe = RecipeKind::XRay;
            s.passes().remove(2)
        }
        2 => {
            let mut s = base.clone();
            s.recipe = RecipeKind::Strata;
            s.passes().remove(0)
        }
        3 => base.passes().remove(0),
        4 => {
            let mut s = base.clone();
            s.recipe = RecipeKind::Hybrid;
            s.passes().remove(0)
        }
        5 => {
            let mut s = base.clone();
            s.recipe = RecipeKind::TriMesh;
            s.art.mesh_inks = 1;
            s.passes().remove(0)
        }
        6 => {
            let mut s = base;
            s.recipe = RecipeKind::FlatShadow;
            s.shadow = scene.shadow.clone();
            s.passes().remove(0)
        }
        8 | 9 => Pass {
            name: String::new(),
            enabled: true,
            pen: 0,
            visibility: Visibility::Visible,
            mask: TonalMask::All,
            dash: DashPattern::default(),
            reading: None,
            finish: FinishSettings::default(),
            kind: PassKind::ToneHatch(ToneHatchParams {
                mode: if index == 8 {
                    HatchMode::Parallel
                } else {
                    HatchMode::Form
                },
                ..Default::default()
            }),
        },
        _ => Pass {
            name: String::new(),
            enabled: true,
            pen: 0,
            visibility: Visibility::Visible,
            mask: TonalMask::All,
            dash: DashPattern::default(),
            reading: None,
            finish: FinishSettings::default(),
            kind: PassKind::Ground(scene.shadow.clone()),
        },
    };
    pass.name = type_name(&pass.kind);
    pass.enabled = true;
    pass
}
fn family(ui: &mut egui::Ui, value: &mut CurveFamily) {
    ui.label(tr!("Familia de curvas"));
    egui::ComboBox::from_id_salt("family")
        .selected_text(match value {
            CurveFamily::Primary => tr!("Principales"),
            CurveFamily::Secondary => tr!("Secundarias"),
            CurveFamily::All => tr!("Todas"),
        })
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.selectable_value(value, CurveFamily::Primary, tr!("Principales"));
            ui.selectable_value(value, CurveFamily::Secondary, tr!("Secundarias"));
            ui.selectable_value(value, CurveFamily::All, tr!("Todas"));
        });
}
fn parameters(ui: &mut egui::Ui, kind: &mut PassKind) {
    match kind {
        PassKind::Reading(p) => reading_ui::effect_controls(ui, p),
        PassKind::Field(_) | PassKind::Sections(_) => {
            let sections = matches!(kind, PassKind::Sections(_));
            let p = match kind {
                PassKind::Field(p) | PassKind::Sections(p) => p,
                _ => unreachable!(),
            };
            ui.label(tr!("Campo"));
            egui::ComboBox::from_id_salt("field")
                .selected_text(match p.field {
                    Field::Height => tr!("Altura del volumen"),
                    Field::CameraDepth => tr!("Profundidad de cámara"),
                    Field::Distance => tr!("Distancia al centro"),
                })
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut p.field, Field::Height, tr!("Altura del volumen"));
                    ui.selectable_value(
                        &mut p.field,
                        Field::CameraDepth,
                        tr!("Profundidad de cámara"),
                    );
                    ui.selectable_value(&mut p.field, Field::Distance, tr!("Distancia al centro"));
                });
            ui.label(tr!("Cortes"));
            ui.add(egui::Slider::new(&mut p.cuts, 1..=160));
            ui.horizontal_wrapped(|ui| {
                for (label, n) in [
                    ("X", [1., 0., 0.]),
                    ("Y", [0., 1., 0.]),
                    ("Z", [0., 0., 1.]),
                ] {
                    ui.selectable_value(&mut p.direction, n, label);
                }
            });
            if sections {
                scene_panel::slider(
                    ui,
                    &tr!("Desplazar estratos (mm)"),
                    &mut p.shift_mm,
                    -5.0..=5.,
                );
            }
            family(ui, &mut p.family);
            egui::CollapsingHeader::new(tr!("Avanzado")).show(ui, |ui| {
                scene_panel::slider(ui, &tr!("Separación 3D"), &mut p.spacing, 0.0..=1.);
                scene_panel::slider(ui, &tr!("Desfase de cortes"), &mut p.offset, -1.0..=1.);
                let end = p.range[1] - 0.001;
                scene_panel::slider(ui, &tr!("Inicio del rango"), &mut p.range[0], 0.0..=end);
                let start = p.range[0] + 0.001;
                scene_panel::slider(ui, &tr!("Final del rango"), &mut p.range[1], start..=1.);
                ui.label(tr!("Curva principal cada"));
                ui.add(egui::Slider::new(&mut p.major_every, 1..=160));
                ui.label(tr!("Dirección personalizada"));
                for (i, label) in ["X", "Y", "Z"].into_iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.add(
                            egui::DragValue::new(&mut p.direction[i])
                                .range(-1.0..=1.)
                                .speed(0.01),
                        );
                    });
                }
            });
        }
        PassKind::Edges { angle } => {
            scene_panel::slider(ui, &tr!("Ángulo de arista"), angle, 0.0..=180.)
        }
        PassKind::Outline(OutlineParams::Surface { facets }) => {
            ui.label(tr!("Triángulos"));
            ui.add(egui::Slider::new(facets, 64..=20_000).logarithmic(true));
        }
        PassKind::Outline(OutlineParams::Shadow(p)) | PassKind::Ground(p) => shadow(ui, p),
        PassKind::Hatch { settings, middle } => {
            if *middle {
                ui.checkbox(&mut settings.middle_tone, tr!("Añadir tono intermedio"));
            }
            shadow(ui, settings);
        }
        PassKind::ToneHatch(p) => {
            egui::ComboBox::from_id_salt("tone-hatch-mode")
                .selected_text(if p.mode == HatchMode::Parallel {
                    tr!("Trama cruzada")
                } else {
                    tr!("Sigue la forma")
                })
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut p.mode, HatchMode::Parallel, tr!("Trama cruzada"));
                    ui.selectable_value(&mut p.mode, HatchMode::Form, tr!("Sigue la forma"));
                });
            ui.label(tr!("Niveles de luz"));
            ui.add(egui::Slider::new(&mut p.levels, 2..=4));
            if p.mode == HatchMode::Parallel {
                scene_panel::slider(
                    ui,
                    &tr!("Separación de trazos (mm)"),
                    &mut p.spacing_mm,
                    0.08..=4.,
                );
                egui::CollapsingHeader::new(tr!("Avanzado")).show(ui, |ui| {
                    for (i, angle) in p.angles.iter_mut().take(p.levels).enumerate() {
                        scene_panel::slider(
                            ui,
                            &tr!("Ángulo del nivel %{n}", n = i + 1),
                            angle,
                            -180.0..=180.,
                        );
                    }
                });
            } else {
                ui.label(tr!("Cortes"));
                ui.add(egui::Slider::new(&mut p.cuts, 4..=160));
                ui.horizontal_wrapped(|ui| {
                    for (label, direction) in [
                        ("X", [1., 0., 0.]),
                        ("Y", [0., 1., 0.]),
                        ("Z", [0., 0., 1.]),
                    ] {
                        ui.selectable_value(&mut p.direction, direction, label);
                    }
                });
            }
        }
        PassKind::Waves(p) => {
            ui.label(tr!("Curvas"));
            ui.add(egui::Slider::new(&mut p.lines, 4..=160));
            egui::ComboBox::from_id_salt("wave-pattern")
                .selected_text(if p.pattern == WavePattern::Waves {
                    tr!("Ondas geodésicas")
                } else {
                    tr!("Interferencias")
                })
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut p.pattern,
                        WavePattern::Waves,
                        tr!("Ondas geodésicas"),
                    );
                    ui.selectable_value(
                        &mut p.pattern,
                        WavePattern::Interference,
                        tr!("Interferencias"),
                    );
                });
            family(ui, &mut p.family);
            scene_panel::slider(ui, &tr!("Deformación"), &mut p.deformation, 0.0..=0.6);
            egui::CollapsingHeader::new(tr!("Avanzado")).show(ui, |ui| {
                ui.label(tr!("Triángulos"));
                ui.add(egui::Slider::new(&mut p.facets, 64..=20_000).logarithmic(true));
                scene_panel::slider(
                    ui,
                    &tr!("Azimut de origen"),
                    &mut p.origin_azimuth,
                    -180.0..=180.,
                );
                scene_panel::slider(
                    ui,
                    &tr!("Elevación de origen"),
                    &mut p.origin_elevation,
                    -90.0..=90.,
                );
                scene_panel::slider(ui, &tr!("Frecuencia"), &mut p.frequency, 0.5..=12.);
                ui.label(tr!("Curva principal cada"));
                ui.add(egui::Slider::new(&mut p.major_every, 1..=160));
                ui.checkbox(&mut p.outline, tr!("Añadir contorno"));
            });
        }
        PassKind::Mesh(p) => {
            ui.label(tr!("Repartir las tintas por"));
            egui::ComboBox::from_id_salt("mesh-color")
                .selected_text(match p.coloring {
                    MeshColor::Orientation => tr!("Orientación de caras"),
                    MeshColor::Height => tr!("Altura del volumen"),
                    MeshColor::Depth => tr!("Profundidad de cámara"),
                    MeshColor::Objects => tr!("Objetos del modelo"),
                })
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut p.coloring,
                        MeshColor::Orientation,
                        tr!("Orientación de caras"),
                    );
                    ui.selectable_value(
                        &mut p.coloring,
                        MeshColor::Height,
                        tr!("Altura del volumen"),
                    );
                    ui.selectable_value(
                        &mut p.coloring,
                        MeshColor::Depth,
                        tr!("Profundidad de cámara"),
                    );
                    ui.selectable_value(
                        &mut p.coloring,
                        MeshColor::Objects,
                        tr!("Objetos del modelo"),
                    );
                });
            ui.label(tr!("Familias de tinta"));
            ui.add(egui::Slider::new(&mut p.inks, 1..=256));
            p.group = p.group.min(p.inks - 1);
            let mut group = p.group + 1;
            ui.label(tr!("Familia de esta pasada"));
            ui.add(egui::Slider::new(&mut group, 1..=p.inks));
            p.group = group - 1;
            ui.label(tr!("Triángulos"));
            ui.add(egui::Slider::new(&mut p.facets, 64..=20_000).logarithmic(true));
        }
    }
}
fn shadow(ui: &mut egui::Ui, p: &mut ShadowSettings) {
    scene_panel::slider(
        ui,
        &tr!("Separación de trazos (mm)"),
        &mut p.hatch_spacing_mm,
        0.08..=2.,
    );
    scene_panel::slider(
        ui,
        &tr!("Ángulo de trazos"),
        &mut p.hatch_angle,
        -180.0..=180.,
    );
    scene_panel::slider(ui, &tr!("Cantidad de sombra"), &mut p.threshold, 0.0..=0.9);
    ui.checkbox(&mut p.ground, tr!("Sombra en el suelo"));
    egui::CollapsingHeader::new(tr!("Avanzado")).show(ui, |ui| {
        ui.label(tr!("Facetas"));
        ui.add(egui::Slider::new(&mut p.facets, 64..=8000).logarithmic(true));
        scene_panel::slider(ui, &tr!("Separar del suelo"), &mut p.ground_gap, 0.0..=2.);
    });
}

fn pass_name(name: &str) -> String {
    const NAMES: &[&str] = &[
        tm!("Contorno"),
        tm!("Trama de forma"),
        tm!("Trama cruzada"),
        tm!("Aristas visibles"),
        tm!("Aristas ocultas"),
        tm!("Curvas en sombra"),
        tm!("Curvas principales"),
        tm!("Curvas secundarias"),
        tm!("Estructura"),
        tm!("Sombra única · objeto y suelo"),
        tm!("Tono intermedio"),
        tm!("Contorno opcional"),
        tm!("Ondas principales"),
        tm!("Ondas secundarias"),
        tm!("Silueta"),
        tm!("Ondas de superficie"),
        tm!("Malla · grupo 1"),
        tm!("Malla · grupo 2"),
        tm!("Malla · grupo 3"),
    ];
    if NAMES.contains(&name) {
        crate::i18n::tr_key(name)
    } else {
        name.to_owned()
    }
}

fn mask_controls(ui: &mut egui::Ui, mask: &mut TonalMask) {
    ui.label(tr!("Máscara tonal"));
    egui::ComboBox::from_id_salt("tonal-mask")
        .selected_text(match mask {
            TonalMask::All => tr!("Toda la superficie"),
            TonalMask::Light => tr!("Solo en luz"),
            TonalMask::Shadow => tr!("Solo en sombra"),
            TonalMask::Band { .. } => tr!("Banda de luz"),
        })
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.selectable_value(mask, TonalMask::All, tr!("Toda la superficie"));
            ui.selectable_value(mask, TonalMask::Light, tr!("Solo en luz"));
            ui.selectable_value(mask, TonalMask::Shadow, tr!("Solo en sombra"));
            if ui
                .selectable_label(matches!(mask, TonalMask::Band { .. }), tr!("Banda de luz"))
                .clicked()
                && !matches!(mask, TonalMask::Band { .. })
            {
                *mask = TonalMask::Band { min: 0.2, max: 0.6 };
            }
        });
    if let TonalMask::Band { min, max } = mask {
        scene_panel::slider(ui, &tr!("Luz mínima"), min, 0.0..=(*max - 0.01).max(0.));
        scene_panel::slider(ui, &tr!("Luz máxima"), max, (*min + 0.01).min(1.)..=1.);
    }
}
