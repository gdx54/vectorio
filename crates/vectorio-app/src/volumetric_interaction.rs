//! Manipulación en unidades físicas del dibujo y orientación de la luz.
use super::*;

pub(super) fn move_on_paper(settings: &mut Settings, delta: egui::Vec2, points_per_mm: f32) {
    if !points_per_mm.is_finite() || points_per_mm <= 0. || !delta.is_finite() {
        return;
    }
    settings.position_mm[0] = (settings.position_mm[0]
        + f64::from(delta.x) / f64::from(points_per_mm))
    .clamp(-1000., 1000.);
    settings.position_mm[1] = (settings.position_mm[1]
        + f64::from(delta.y) / f64::from(points_per_mm))
    .clamp(-1000., 1000.);
}
pub(super) fn zoom_on_paper(
    settings: &mut Settings,
    factor: f64,
    anchor: vectorio_core::P,
    paper: &Paper,
) {
    if !factor.is_finite() || factor <= 0. || !anchor.x.is_finite() || !anchor.y.is_finite() {
        return;
    }
    let next = (settings.zoom * factor).clamp(0.05, 3.);
    let factor = next / settings.zoom;
    for (i, (coordinate, center)) in [
        (anchor.x, paper.width_mm * 0.5),
        (anchor.y, paper.height_mm * 0.5),
    ]
    .into_iter()
    .enumerate()
    {
        let relative = coordinate - center;
        settings.position_mm[i] =
            (relative + (settings.position_mm[i] - relative) * factor).clamp(-1000., 1000.);
    }
    settings.zoom = next;
}
/// Coeficientes calculados una vez por fotograma, incluso con dibujos muy densos.
#[derive(Clone, Copy)]
pub(super) struct Composition {
    old_center: vectorio_core::P,
    center: vectorio_core::P,
    scale: f64,
    sin: f64,
    cos: f64,
    identity: bool,
}
impl Composition {
    pub fn between(old: &Settings, old_paper: &Paper, new: &Settings, paper: &Paper) -> Self {
        let extent = |p: &Paper| p.width_mm.min(p.height_mm) - 2. * p.margin_mm;
        let (sin, cos) = (new.paper_rotation - old.paper_rotation)
            .to_radians()
            .sin_cos();
        Self {
            old_center: p(
                old_paper.width_mm * 0.5 + old.position_mm[0],
                old_paper.height_mm * 0.5 + old.position_mm[1],
            ),
            center: p(
                paper.width_mm * 0.5 + new.position_mm[0],
                paper.height_mm * 0.5 + new.position_mm[1],
            ),
            scale: new.zoom / old.zoom * extent(paper) / extent(old_paper),
            sin,
            cos,
            identity: old.zoom == new.zoom
                && old.position_mm == new.position_mm
                && old.paper_rotation == new.paper_rotation
                && old_paper == paper,
        }
    }
    pub fn point(self, point: vectorio_core::P) -> vectorio_core::P {
        if self.identity {
            return point;
        }
        let q = point - self.old_center;
        self.center
            + p(
                q.x * self.cos - q.y * self.sin,
                q.x * self.sin + q.y * self.cos,
            ) * self.scale
    }
}
pub(super) fn orbit(settings: &mut Settings, delta: egui::Vec2) {
    settings.yaw = (settings.yaw - f64::from(delta.x) * 0.4 + 180.).rem_euclid(360.) - 180.;
    settings.elevation = (settings.elevation + f64::from(delta.y) * 0.3).clamp(0.01, 89.99);
}
fn sun_position(center: egui::Pos2, radius: f32, azimuth: f64, elevation: f64) -> egui::Pos2 {
    let (sin, cos) = azimuth.to_radians().sin_cos();
    let distance = elevation.to_radians().cos() as f32 * radius;
    center + egui::vec2(cos as f32, -sin as f32) * distance
}
fn light_at(
    position: egui::Pos2,
    center: egui::Pos2,
    radius: f32,
    previous_azimuth: f64,
) -> (f64, f64) {
    let delta = position - center;
    let azimuth = if delta.length_sq() > 1e-6 {
        f64::from(-delta.y).atan2(f64::from(delta.x)).to_degrees()
    } else {
        previous_azimuth
    };
    let cosine =
        f64::from(delta.length() / radius).clamp(85f64.to_radians().cos(), 5f64.to_radians().cos());
    (azimuth, cosine.acos().to_degrees().clamp(5., 85.))
}
pub(super) fn light_gizmo(ui: &mut egui::Ui, state: &mut VolumetricUi) {
    let pal = theme::current(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 90.), Sense::hover());
    let response = ui.interact(
        rect,
        ui.id().with("vol-light-sphere"),
        Sense::click_and_drag(),
    );
    let center = rect.center();
    let radius = 34.;
    if ui.is_enabled() {
        if response.clicked() || response.dragged() {
            response.request_focus();
            if let Some(position) = response.interact_pointer_pos() {
                let (azimuth, elevation) =
                    light_at(position, center, radius, state.settings.shadow.azimuth);
                state.set_light(azimuth, elevation);
            }
        }
        if response.has_focus() {
            let delta = keyboard_delta(ui, response.id);
            if delta != egui::Vec2::ZERO {
                state.set_light(
                    state.settings.shadow.azimuth + f64::from(delta.x),
                    state.settings.shadow.elevation - f64::from(delta.y),
                );
            }
        }
    }
    let painter = ui.painter_at(rect);
    painter.circle_filled(center, radius, pal.control);
    painter.circle_stroke(center, radius, Stroke::new(1.0f32, pal.secondary));
    painter.circle_stroke(
        center,
        radius * std::f32::consts::FRAC_1_SQRT_2,
        Stroke::new(0.6f32, pal.secondary.gamma_multiply(0.35)),
    );
    for d in [egui::vec2(1., 0.), egui::vec2(0., 1.)] {
        painter.line_segment(
            [center - d * radius, center + d * radius],
            Stroke::new(0.6f32, pal.secondary.gamma_multiply(0.25)),
        );
    }
    let sun = sun_position(
        center,
        radius,
        state.settings.shadow.azimuth,
        state.settings.shadow.elevation,
    );
    painter.line_segment([center, sun], Stroke::new(1.0f32, pal.text));
    painter.circle_filled(sun, 5., pal.text);
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.;
        let d = egui::vec2(a.cos(), a.sin());
        painter.line_segment([sun + d * 7., sun + d * 9.], Stroke::new(1.0f32, pal.text));
    }
    if response.has_focus() {
        state.spatial_focus = Some(response.id);
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            tr!("Orientar la luz"),
        )
    });
    theme::focus_ring(ui, &response);
    response.on_hover_text(tr!("Arrastra el sol · flechas para ajustar la luz."));
}
pub(super) fn keyboard_delta(ui: &mut egui::Ui, id: egui::Id) -> egui::Vec2 {
    ui.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            id,
            egui::EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                ..Default::default()
            },
        );
    });
    ui.input_mut(|input| {
        let modifiers = if input.modifiers.shift {
            egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
        };
        let step = if input.modifiers.shift { 10. } else { 2. };
        let x = i32::from(input.consume_key(modifiers, egui::Key::ArrowRight))
            - i32::from(input.consume_key(modifiers, egui::Key::ArrowLeft));
        let y = i32::from(input.consume_key(modifiers, egui::Key::ArrowDown))
            - i32::from(input.consume_key(modifiers, egui::Key::ArrowUp));
        egui::vec2(x as f32, y as f32) * step
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scale_keeps_the_anchor_fixed_even_at_the_zoom_limits() {
        let paper = Paper::default();
        let anchor = p(143.2, 178.6);
        let mut settings = Settings {
            position_mm: [18., -12.],
            paper_rotation: 28.,
            ..Default::default()
        };
        for factor in [1.37, 100., 0.001, 1.4] {
            let before = settings.clone();
            zoom_on_paper(&mut settings, factor, anchor, &paper);
            let mapped = Composition::between(&before, &paper, &settings, &paper).point(anchor);
            assert!(mapped.dist(anchor) < 1e-10);
            assert!((0.05..=3.).contains(&settings.zoom));
        }
    }
    #[test]
    fn pointer_light_mapping_is_invertible_and_stays_above_the_horizon() {
        let center = egui::pos2(65., 45.);
        let radius = 34.;
        for azimuth in [-175., -90., 0., 90., 135.] {
            for elevation in [5., 25., 45., 85.] {
                let point = sun_position(center, radius, azimuth, elevation);
                let (az, el) = light_at(point, center, radius, 12.);
                assert!((az - azimuth).abs() < 0.001);
                assert!((el - elevation).abs() < 0.001);
            }
        }
        assert_eq!(light_at(center, center, radius, 123.), (123., 85.));
        assert_eq!(
            light_at(center + egui::vec2(500., 0.), center, radius, 123.),
            (0., 5.)
        );
    }
    #[test]
    fn movement_uses_the_physical_paper_scale_and_rejects_invalid_deltas() {
        let mut settings = Settings::default();
        move_on_paper(&mut settings, egui::vec2(40., -20.), 2.);
        assert_eq!(settings.position_mm, [20., -10.]);
        move_on_paper(&mut settings, egui::vec2(5., 6.), 0.);
        move_on_paper(&mut settings, egui::vec2(f32::NAN, 6.), 2.);
        assert_eq!(settings.position_mm, [20., -10.]);
        move_on_paper(&mut settings, egui::vec2(1e8, -1e8), 1.);
        assert_eq!(settings.position_mm, [1000., -1000.]);
        settings.validate().unwrap();
    }
}
