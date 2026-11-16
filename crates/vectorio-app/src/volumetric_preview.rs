//! Vista del volumen y su traducción a papel, con cámara compartida.
use super::*;

pub(super) struct Workspace {
    pub styles: Rect,
    pub scene: Rect,
    pub passes: Option<Rect>,
    pub paper: Rect,
    pub footer: Rect,
    pub compact: bool,
}
impl Workspace {
    pub fn new(rect: Rect, collapsed: bool) -> Self {
        let margin = theme::INSET;
        let top = theme::TOOLBAR_H + margin;
        let styles = Rect::from_min_max(
            egui::pos2(margin, top),
            egui::pos2(rect.right() - margin, top + 40.),
        );
        let footer = Rect::from_min_max(
            egui::pos2(margin, rect.bottom() - margin - 44.),
            egui::pos2(rect.right() - margin, rect.bottom() - margin),
        );
        let compact = rect.width() < 1100.;
        let scene_width = if compact || collapsed { 40. } else { 176. };
        let scene = Rect::from_min_max(
            egui::pos2(margin, styles.bottom() + 12.),
            egui::pos2(margin + scene_width, footer.top() - 12.),
        );
        let passes = (!compact).then(|| {
            Rect::from_min_max(
                egui::pos2(rect.right() - margin - 220., scene.top()),
                egui::pos2(rect.right() - margin, scene.bottom()),
            )
        });
        let right = passes.map_or(rect.right() - margin, |r| r.left() - 12.);
        let paper = Rect::from_min_max(
            egui::pos2(scene.right() + 12., scene.top()),
            egui::pos2(right, scene.bottom()),
        );
        Self {
            styles,
            scene,
            passes,
            paper,
            footer,
            compact,
        }
    }
}
pub(super) fn previews(ui: &mut egui::Ui, state: &mut VolumetricUi, app: &VectorioApp) {
    let mut space = ui.max_rect();
    let pal = theme::current(ui.ctx());
    ui.allocate_new_ui(
        egui::UiBuilder::new().max_rect(Rect::from_min_size(
            space.min,
            egui::vec2(space.width(), 28.),
        )),
        |ui| {
            ui.horizontal(|ui| {
                ui.weak(tr!("Papel"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.selectable_value(&mut state.layout, Layout::Dual, tr!("Vista dividida"));
                    ui.selectable_value(&mut state.layout, Layout::Paper, tr!("Papel"));
                    if state.scene_collapsed && ui.small_button(tr!("Escena")).clicked() {
                        state.scene_collapsed = false;
                    }
                });
            });
        },
    );
    space.min.y += 32.;
    let warning = state.load_error.as_ref().or(state.error.as_ref());
    if let Some(error) = warning {
        ui.allocate_new_ui(
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                space.min,
                egui::vec2(space.width(), 32.),
            )),
            |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(error_color(ui.visuals().dark_mode), error);
                    if ui.small_button(tr!("Recalcular")).clicked() {
                        state.requested = None;
                    }
                });
            },
        );
        space.min.y += 36.;
    }
    if state.loading.is_some() || (!state.current(&app.doc.paper) && state.error.is_none()) {
        ui.allocate_new_ui(
            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                space.min,
                egui::vec2(space.width(), 3.),
            )),
            |ui| {
                ui.add(
                    egui::ProgressBar::new(0.35)
                        .desired_width(space.width())
                        .desired_height(3.)
                        .animate(true),
                );
            },
        );
    }
    if state.layout == Layout::Dual {
        let model = Rect::from_min_size(
            space.min,
            egui::vec2((space.width() - 12.) * 0.38, space.height()),
        );
        let paper = Rect::from_min_max(egui::pos2(model.right() + 12., space.top()), space.max);
        model_preview(ui, state, app, model);
        paper_preview(ui, state, app, paper);
    } else {
        let paper = Rect::from_min_max(space.min, egui::pos2(space.right(), space.bottom() - 84.));
        paper_preview(ui, state, app, paper);
        let inset = Rect::from_min_size(
            egui::pos2(space.right() - 92., space.bottom() - 72.),
            egui::vec2(92., 72.),
        );
        ui.painter().rect_filled(inset, 8., pal.control);
        model_preview(ui, state, app, inset);
    }
}
fn model_preview(ui: &mut egui::Ui, state: &mut VolumetricUi, app: &VectorioApp, rect: Rect) {
    let pal = theme::current(ui.ctx());
    let painter = ui.painter().with_clip_rect(rect);
    let small = rect.width() < 200.;
    if !small {
        painter.text(
            rect.min,
            egui::Align2::LEFT_TOP,
            tr!("Volumen 3D"),
            egui::FontId::proportional(12.),
            pal.secondary,
        );
    }
    let viewport = if small {
        rect.shrink(6.)
    } else {
        rect.shrink2(egui::vec2(12., 32.))
    };
    let response = ui.interact(viewport, ui.id().with("vol-orbit"), Sense::click_and_drag());
    if response.double_clicked() {
        state.layout = if state.layout == Layout::Dual {
            Layout::Paper
        } else {
            Layout::Dual
        };
    }
    if response.clicked() {
        response.request_focus();
    }
    if ui.is_enabled() && !state.locked {
        if response.dragged() {
            interaction::orbit(&mut state.settings, response.drag_delta());
        }
        if response.has_focus() {
            let delta = interaction::keyboard_delta(ui, response.id);
            state.settings.yaw =
                (state.settings.yaw - f64::from(delta.x) + 180.).rem_euclid(360.) - 180.;
            state.settings.elevation =
                (state.settings.elevation - f64::from(delta.y)).clamp(0.01, 89.99);
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(if response.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
    }
    if response.has_focus() {
        state.spatial_focus = Some(response.id);
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            tr!("Orbitar cámara"),
        )
    });
    theme::focus_ring(ui, &response);
    response.on_hover_text(if state.locked {
        tr!("Cámara bloqueada")
    } else {
        tr!("Arrastra para orbitar · doble clic para ampliar.")
    });
    let Some(drawing) = &state.drawing else {
        return;
    };
    let mesh = &drawing.display_mesh;
    let camera = Camera {
        yaw_deg: state.settings.yaw,
        elev_deg: state.settings.elevation,
        scale: 1.,
        world_center: drawing.camera.world_center,
        paper_center: p(0., 0.),
        perspective: drawing.camera.perspective,
    };
    let pts = mesh.points();
    let projected: Vec<_> = pts.iter().map(|q| camera.project(*q).0).collect();
    let lo = projected
        .iter()
        .fold(p(f64::INFINITY, f64::INFINITY), |a, q| {
            p(a.x.min(q.x), a.y.min(q.y))
        });
    let hi = projected
        .iter()
        .fold(p(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, q| {
            p(a.x.max(q.x), a.y.max(q.y))
        });
    let center = (lo + hi) * 0.5;
    let scale = (viewport.width() as f64 / (hi.x - lo.x).max(1e-9))
        .min(viewport.height() as f64 / (hi.y - lo.y).max(1e-9))
        * 0.88;
    let project = |q: V3| {
        let (p, z) = camera.project(q);
        (
            viewport.center()
                + egui::vec2((p.x - center.x) as f32, (p.y - center.y) as f32) * scale as f32,
            z,
        )
    };
    if let Some(shadow) = drawing
        .shadow
        .as_ref()
        .filter(|s| s.faces.len() == mesh.triangles.len())
    {
        let ink = |role: usize| {
            app.doc
                .pens
                .get(state.passes.get(role).map_or(0, |entry| entry.pass.pen))
                .map_or(Color32::BLACK, |p| theme::pen_color(p.color))
        };
        for poly in &shadow.ground_world {
            polygon(&painter, poly.iter().map(|q| project(*q).0), ink(0));
        }
        let mut order: Vec<_> = mesh
            .triangles
            .iter()
            .enumerate()
            .map(|(i, t)| (t.iter().map(|j| project(pts[*j]).1).sum::<f64>(), i))
            .collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, i) in order {
            let face = &shadow.faces[i];
            let color = if face.dark {
                ink(0)
            } else if face.middle {
                ink(1)
            } else {
                Color32::WHITE
            };
            polygon(
                &painter,
                mesh.triangles[i].iter().map(|j| project(pts[*j]).0),
                color,
            );
            for patch in &face.cast {
                polygon(&painter, patch.iter().map(|q| project(*q).0), ink(0));
            }
        }
    } else {
        let mut tris: Vec<_> = mesh
            .triangles
            .iter()
            .map(|t| {
                let tri = t.map(|i| pts[i]);
                let depth = tri.iter().map(|q| project(*q).1).sum::<f64>();
                (depth, tri)
            })
            .collect();
        tris.sort_by(|a, b| b.0.total_cmp(&a.0));
        let opacity = 1.;
        let mut solid = egui::Mesh::default();
        for (_, tri) in tris {
            let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalized();
            let tone = (n.dot(V3::new(0.3, -0.5, 0.8).normalized()).abs() * 70. + 90.) as u8;
            let color = Color32::from_rgba_unmultiplied(tone, tone, tone, (255. * opacity) as u8);
            let base = solid.vertices.len() as u32;
            for point in tri {
                solid.colored_vertex(project(point).0, color);
            }
            solid.add_triangle(base, base + 1, base + 2);
        }
        // Triángulos directos: el teselador de polígonos con antialias puede crear
        // ingletes enormes al proyectar una cara casi de canto.
        painter.add(egui::Shape::mesh(solid));
        // La referencia 3D muestra el sólido. Los trazos definitivos y la selección
        // pertenecen al papel, con una parte por pasada y tintas independientes.
    }

    if state.axes && !small {
        let base = egui::pos2(viewport.left() + 28., viewport.bottom() - 28.);
        for (name, axis, color) in [
            ("X", V3::new(1., 0., 0.), Color32::from_rgb(190, 65, 55)),
            ("Y", V3::new(0., 1., 0.), Color32::from_rgb(50, 135, 80)),
            ("Z", V3::new(0., 0., 1.), Color32::from_rgb(60, 100, 190)),
        ] {
            let (p, _) = camera.project(camera.world_center + axis);
            let end = base + egui::vec2(p.x as f32, p.y as f32) * 24.;
            painter.line_segment([base, end], Stroke::new(1.0f32, color));
            painter.text(
                end,
                egui::Align2::CENTER_CENTER,
                name,
                egui::FontId::proportional(10.),
                color,
            );
        }
    }
}
fn paper_preview(ui: &mut egui::Ui, state: &mut VolumetricUi, app: &VectorioApp, rect: Rect) {
    let painter = ui.painter().with_clip_rect(rect);
    let paper = &app.doc.paper;
    let composition_paper = state.composition_paper(paper);
    let viewport = rect.shrink(8.);
    let scale =
        (viewport.width() / paper.width_mm as f32).min(viewport.height() / paper.height_mm as f32);
    let sheet = Rect::from_center_size(
        viewport.center(),
        egui::vec2(paper.width_mm as f32, paper.height_mm as f32) * scale,
    );
    let response = ui.interact(
        sheet,
        ui.id().with("vol-paper-drawing"),
        Sense::click_and_drag(),
    );
    if response.clicked() {
        response.request_focus();
    }
    if ui.is_enabled() {
        let touch = ui.input(|i| i.multi_touch());
        if response.dragged() && touch.is_none() {
            interaction::move_on_paper(&mut state.settings, response.drag_delta(), scale);
        }
        if let Some(cursor) = response.hover_pos() {
            let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.raw_scroll_delta.y));
            let factor = if zoom != 1. {
                f64::from(zoom)
            } else {
                (f64::from(scroll) * 0.0025).exp()
            };
            if factor != 1. {
                let anchor = p(
                    f64::from(cursor.x - sheet.left()) / f64::from(scale),
                    f64::from(cursor.y - sheet.top()) / f64::from(scale),
                );
                interaction::zoom_on_paper(&mut state.settings, factor, anchor, &composition_paper);
            }
            if let Some(touch) = touch {
                interaction::move_on_paper(&mut state.settings, touch.translation_delta, scale);
            }
            ui.ctx().set_cursor_icon(if response.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }
        if response.has_focus() {
            interaction::move_on_paper(
                &mut state.settings,
                interaction::keyboard_delta(ui, response.id) * 0.5,
                1.,
            );
        }
    }
    if response.has_focus() {
        state.spatial_focus = Some(response.id);
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            tr!("Mover dibujo en el papel"),
        )
    });
    response.on_hover_text(tr!("Arrastra para mover · rueda o pellizco para escalar."));
    painter.rect_filled(sheet, 0., Color32::WHITE);
    let area = sheet.shrink(paper.margin_mm as f32 * scale);
    painter.rect_stroke(area, 0., Stroke::new(0.5f32, Color32::from_gray(215)));
    let plot = painter.with_clip_rect(area);
    let current = state.current(&app.doc.paper);
    let composition = state.drawn_scene.as_ref().map(|(old, old_paper)| {
        interaction::Composition::between(old, old_paper, &state.settings, &composition_paper)
    });
    for entry in state.passes.iter().filter(|p| p.pass.enabled) {
        let Some(part) = state.part(entry.id) else {
            continue;
        };
        for (ink, lines) in part.colors(&entry.pass, &app.doc.pens) {
            let base = app
                .doc
                .pens
                .get(ink)
                .map_or(Color32::BLACK, |pen| theme::pen_color(pen.color));
            let stroke_width =
                (app.doc.pens.get(ink).map_or(0.1764, |p| p.width_mm) as f32 * scale).max(0.5);
            let selected = state.selected == Some(entry.id);
            let opacity: f32 = if current { 1. } else { 0.35 };
            let emphasis: f32 = if state.selected.is_some() && !selected {
                0.18
            } else {
                1.
            };
            let color = base.gamma_multiply(opacity * emphasis);
            for line in lines {
                let fixed_legend = state.drawn_scene.as_ref().is_some_and(|(_, old_paper)| {
                    line.iter()
                        .all(|point| point.y > old_paper.height_mm - old_paper.margin_mm + 0.5)
                });
                plot.add(egui::Shape::line(
                    line.iter()
                        .map(|point| {
                            let point = if fixed_legend {
                                *point
                            } else {
                                composition.map_or(*point, |composition| composition.point(*point))
                            };
                            sheet.min + egui::vec2(point.x as f32, point.y as f32) * scale
                        })
                        .collect(),
                    Stroke::new(
                        if selected {
                            stroke_width.max(1.2)
                        } else {
                            stroke_width
                        },
                        color,
                    ),
                ));
            }
        }
    }
}

// Abanico de polígonos convexos, sin contorno ni costuras de antialias entre caras.
fn polygon(painter: &egui::Painter, points: impl Iterator<Item = egui::Pos2>, color: Color32) {
    let mut mesh = egui::Mesh::default();
    for point in points {
        mesh.colored_vertex(point, color);
    }
    for i in 1..mesh.vertices.len().saturating_sub(1) {
        mesh.add_triangle(0, i as u32, i as u32 + 1);
    }
    if !mesh.indices.is_empty() {
        painter.add(egui::Shape::mesh(mesh));
    }
}
