//! Escena compartida. Solo Modelo › Avanzado introduce un segundo nivel plegable.
use super::*;
pub(super) fn section_name(index: usize) -> String {
    crate::i18n::tr_key([tm!("Modelo"), tm!("Cámara"), tm!("Luz"), tm!("Papel")][index.min(3)])
}
pub(super) fn rail(ui: &mut egui::Ui, state: &mut VolumetricUi) {
    let pal = theme::current(ui.ctx());
    theme::glass_panel_with(
        ui,
        &pal,
        theme::PANEL_RADIUS,
        egui::Margin::same(5.),
        theme::Material::Panel,
        |ui| {
            for index in 0..4 {
                let (rect, response) = ui.allocate_exact_size(egui::vec2(30., 34.), Sense::click());
                if response.hovered() || state.scene_open == Some(index) {
                    ui.painter().rect_filled(rect.shrink(1.), 7., pal.control);
                }
                let center = rect.center();
                let stroke = Stroke::new(1.4f32, pal.text);
                match index {
                    0 => {
                        let pts = [
                            center + egui::vec2(0., -9.),
                            center + egui::vec2(8., -4.),
                            center + egui::vec2(8., 5.),
                            center + egui::vec2(0., 10.),
                            center + egui::vec2(-8., 5.),
                            center + egui::vec2(-8., -4.),
                        ];
                        ui.painter()
                            .add(egui::Shape::closed_line(pts.to_vec(), stroke));
                        for a in [pts[1], pts[3], pts[5]] {
                            ui.painter().line_segment([center, a], stroke);
                        }
                    }
                    1 => {
                        ui.painter().rect_stroke(
                            Rect::from_center_size(center, egui::vec2(19., 13.)),
                            3.,
                            stroke,
                        );
                        ui.painter().circle_stroke(center, 4., stroke);
                    }
                    2 => {
                        ui.painter().circle_stroke(center, 4., stroke);
                        for n in 0..8 {
                            let angle = n as f32 * std::f32::consts::TAU / 8.;
                            let d = egui::vec2(angle.cos(), angle.sin());
                            ui.painter()
                                .line_segment([center + d * 7., center + d * 10.], stroke);
                        }
                    }
                    _ => {
                        ui.painter().rect_stroke(
                            Rect::from_center_size(center, egui::vec2(15., 20.)),
                            1.,
                            stroke,
                        );
                        ui.painter().line_segment(
                            [center + egui::vec2(-4., -4.), center + egui::vec2(4., -4.)],
                            stroke,
                        );
                    }
                }
                let label = section_name(index);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label)
                });
                theme::focus_ring(ui, &response);
                if response.on_hover_text(&label).clicked() {
                    state.scene_open = if state.scene_open == Some(index) {
                        None
                    } else {
                        Some(index)
                    };
                }
            }
        },
    );
}
pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut VolumetricUi,
    app: &mut VectorioApp,
    only: Option<usize>,
) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    ui.spacing_mut().slider_width = (ui.available_width() - 68.).max(65.);
    if only.is_none() {
        ui.horizontal(|ui| {
            ui.weak(tr!("Escena"));
            if ui.small_button(tr!("Plegar")).clicked() {
                state.scene_collapsed = true;
            }
        });
        ui.add_space(8.);
    }
    for index in 0..4 {
        if only.is_some_and(|i| i != index) {
            continue;
        }
        let mut body = |ui: &mut egui::Ui| match index {
            0 => model(ui, state),
            1 => camera(ui, state),
            2 => light(ui, state),
            _ => paper(ui, state, app),
        };
        if only.is_some() {
            body(ui);
        } else {
            egui::CollapsingHeader::new(section_name(index))
                .default_open(index == 0 || index == 1)
                .show(ui, body);
        }
    }
    ui.add_space(12.);
    if ui.button(tr!("Abrir receta…")).clicked() {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
        {
            state.load(ui.ctx(), path);
        }
    }
    if ui.button(tr!("Guardar receta…")).clicked() {
        state.save_recipe(app);
    }
}
fn model(ui: &mut egui::Ui, state: &mut VolumetricUi) {
    ui.weak(tr!("%{n} triángulos", n = state.mesh.triangles.len()));
    egui::ComboBox::from_id_salt("vol-model")
        .selected_text(
            state
                .primitive
                .map_or_else(|| state.mesh.name.clone(), primitive_name),
        )
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for primitive in [
                Primitive::Cube,
                Primitive::Sphere,
                Primitive::Cylinder,
                Primitive::Cone,
                Primitive::Plane,
                Primitive::Torus,
                Primitive::Assembly,
            ] {
                if ui
                    .selectable_label(
                        state.primitive == Some(primitive),
                        primitive_name(primitive),
                    )
                    .clicked()
                {
                    state.choose_primitive(primitive);
                }
            }
        });
    if ui
        .add_enabled(
            state.loading.is_none(),
            egui::Button::new(tr!("Importar OBJ / STL…")),
        )
        .clicked()
    {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("OBJ / STL", &["obj", "stl"])
            .pick_file()
        {
            state.load(ui.ctx(), path);
        }
    }
    egui::ComboBox::from_id_salt("vol-detail")
        .selected_text(match state.settings.mesh_detail {
            MeshDetail::Interactive => tr!("Calidad interactiva"),
            MeshDetail::Detailed => tr!("Calidad detallada"),
            MeshDetail::Original => tr!("Malla original completa"),
        })
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut state.settings.mesh_detail,
                MeshDetail::Interactive,
                tr!("Calidad interactiva"),
            );
            ui.selectable_value(
                &mut state.settings.mesh_detail,
                MeshDetail::Detailed,
                tr!("Calidad detallada"),
            );
            ui.selectable_value(
                &mut state.settings.mesh_detail,
                MeshDetail::Original,
                tr!("Malla original completa"),
            );
        });
    if state.report.removed > 0 {
        ui.label(tr!(
            "%{n} caras degeneradas o repetidas omitidas.",
            n = state.report.removed
        ));
    }
    if state.report.boundary_edges > 0 {
        ui.label(tr!(
            "Malla abierta: %{n} aristas de frontera.",
            n = state.report.boundary_edges
        ));
    }
    if state.report.non_manifold_edges > 0 {
        ui.label(tr!(
            "%{n} aristas no manifold; pueden quedar contornos abiertos.",
            n = state.report.non_manifold_edges
        ));
    }
    if let Some(drawing) = &state.drawing {
        if drawing.mesh.triangles.len() < state.mesh.triangles.len() {
            ui.weak(tr!("Malla de trabajo: %{n} triángulos. El original se conserva.", n = drawing.mesh.triangles.len())).on_hover_text(tr!("La calidad elegida se usa en el dibujo y el SVG. Original conserva todo el detalle y tarda más."));
        }
    }
    egui::CollapsingHeader::new(tr!("Avanzado"))
        .id_salt("vol-model-advanced")
        .show(ui, |ui| {
            for (i, label) in [ts!("Ancho"), ts!("Fondo"), ts!("Alto")]
                .into_iter()
                .enumerate()
            {
                slider(ui, label, &mut state.settings.model_scale[i], 0.05..=10.);
            }
            for (i, label) in ["X", "Y", "Z"].into_iter().enumerate() {
                slider(
                    ui,
                    &tr!("Rotación %{axis}", axis = label),
                    &mut state.settings.model_rotation[i],
                    -180.0..=180.,
                );
            }
            slider(
                ui,
                &tr!("Simplificar (mm)"),
                &mut state.settings.simplify_mm,
                0.0..=1.,
            );
            ui.label(tr!("Semilla"));
            ui.add(egui::DragValue::new(&mut state.settings.seed));
        });
}
fn primitive_name(p: Primitive) -> String {
    match p {
        Primitive::Cube => tr!("Cubo / prisma"),
        Primitive::Sphere => tr!("Esfera"),
        Primitive::Cylinder => tr!("Cilindro"),
        Primitive::Cone => tr!("Cono"),
        Primitive::Plane => tr!("Plano"),
        Primitive::Torus => tr!("Toroide"),
        Primitive::Assembly => tr!("Composición de volúmenes"),
    }
}
fn camera(ui: &mut egui::Ui, state: &mut VolumetricUi) {
    ui.add_enabled_ui(
        !state.locked && state.settings.sheet != SheetViews::Orthographic,
        |ui| {
            ui.checkbox(
                &mut state.settings.projection.perspective,
                tr!("Perspectiva"),
            );
            if state.settings.projection.perspective {
                slider(
                    ui,
                    &tr!("Distancia focal"),
                    &mut state.settings.projection.distance,
                    0.6..=12.,
                );
                slider(
                    ui,
                    &tr!("Plano cercano"),
                    &mut state.settings.projection.near,
                    0.01..=1.,
                );
            }
        },
    );
    ui.horizontal_wrapped(|ui| {
        for (label, yaw, elevation) in [
            (ts!("Planta"), 0., 89.99),
            (ts!("Alzado"), -90., 0.01),
            (ts!("Perfil"), 0., 0.01),
            (ts!("Isométrica"), 45., 35.26438968),
        ] {
            if ui
                .add_enabled(!state.locked, egui::Button::new(label).small())
                .clicked()
            {
                state.settings.yaw = yaw;
                state.settings.elevation = elevation;
            }
        }
    });
    ui.add_enabled_ui(!state.locked, |ui| {
        slider(ui, &tr!("Azimut"), &mut state.settings.yaw, -180.0..=180.);
        slider(
            ui,
            &tr!("Elevación"),
            &mut state.settings.elevation,
            0.01..=89.99,
        );
    });
    ui.checkbox(&mut state.locked, tr!("Bloquear cámara"));
    ui.checkbox(&mut state.axes, tr!("Mostrar ejes"));
}
fn light(ui: &mut egui::Ui, state: &mut VolumetricUi) {
    interaction::light_gizmo(ui, state);
    let old = state.settings.shadow.clone();
    slider(
        ui,
        &tr!("Dirección de luz"),
        &mut state.settings.shadow.azimuth,
        -180.0..=180.,
    );
    slider(
        ui,
        &tr!("Altura de luz"),
        &mut state.settings.shadow.elevation,
        5.0..=85.,
    );
    if state.settings.shadow != old {
        state.set_light(
            state.settings.shadow.azimuth,
            state.settings.shadow.elevation,
        );
    }
}
fn paper(ui: &mut egui::Ui, state: &mut VolumetricUi, app: &mut VectorioApp) {
    ui.label(tr!("Vistas en la hoja"));
    egui::ComboBox::from_id_salt("vol-sheet-views")
        .width(ui.available_width())
        .selected_text(match state.settings.sheet {
            SheetViews::Single => tr!("Una vista"),
            SheetViews::Orthographic => tr!("Planta, alzado, perfil e isométrica"),
            SheetViews::Turntable => tr!("Giro completo"),
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut state.settings.sheet,
                SheetViews::Single,
                tr!("Una vista"),
            );
            ui.selectable_value(
                &mut state.settings.sheet,
                SheetViews::Orthographic,
                tr!("Planta, alzado, perfil e isométrica"),
            );
            ui.selectable_value(
                &mut state.settings.sheet,
                SheetViews::Turntable,
                tr!("Giro completo"),
            );
        });
    if state.settings.sheet != SheetViews::Single {
        if state.settings.sheet == SheetViews::Turntable {
            ui.label(tr!("Número de vistas"));
            ui.add(egui::Slider::new(&mut state.settings.view_count, 2..=12));
        }
        slider(
            ui,
            &tr!("Separación de vistas (mm)"),
            &mut state.settings.view_gap_mm,
            0.0..=30.,
        );
    }
    egui::ComboBox::from_id_salt("vol-paper")
        .selected_text(crate::i18n::tr_key(&app.doc.paper.name))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for paper in Paper::presets() {
                let name = crate::i18n::tr_key(&paper.name);
                ui.selectable_value(&mut app.doc.paper, paper, name);
            }
        });
    let max_margin = (app.doc.paper.width_mm.min(app.doc.paper.height_mm) * 0.4).min(60.);
    slider(
        ui,
        &tr!("Margen (mm)"),
        &mut app.doc.paper.margin_mm,
        0.0..=max_margin,
    );
    slider(ui, &tr!("Escala"), &mut state.settings.zoom, 0.05..=3.);
    for (i, label) in [ts!("Posición X (mm)"), ts!("Posición Y (mm)")]
        .into_iter()
        .enumerate()
    {
        ui.label(label);
        ui.add(
            egui::DragValue::new(&mut state.settings.position_mm[i])
                .speed(0.1)
                .range(-1000.0..=1000.)
                .fixed_decimals(1),
        );
    }
    slider(
        ui,
        &tr!("Giro en el papel"),
        &mut state.settings.paper_rotation,
        -180.0..=180.,
    );
    if ui.button(tr!("Centrar dibujo")).clicked() {
        state.settings.zoom = 0.88;
        state.settings.position_mm = [0.; 2];
        state.settings.paper_rotation = 0.;
    }
}
pub(super) fn slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
) {
    ui.label(label);
    ui.add(egui::Slider::new(value, range));
}
