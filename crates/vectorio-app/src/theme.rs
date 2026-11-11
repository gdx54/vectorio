//! Aspecto inspirado en macOS 27: barra de herramientas uniforme, paneles flotantes de
//! vidrio legible (translúcidos pero con tinte suficiente), radios concéntricos y
//! tipografía del sistema. Sigue el modo claro/oscuro del sistema.

use egui::{epaint::Shadow, Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Vec2};

/// Radio de los paneles flotantes; los controles internos usan el radio concéntrico
/// (radio del panel menos su margen).
pub const PANEL_RADIUS: f32 = 18.0;
pub const PANEL_MARGIN: f32 = 10.0;
pub const CONTROL_RADIUS: f32 = PANEL_RADIUS - PANEL_MARGIN;
/// Separación entre los paneles y el borde de la ventana.
pub const INSET: f32 = 10.0;
pub const TOOLBAR_H: f32 = 52.0;
/// Hueco para los botones de cerrar/minimizar/ampliar.
pub const TRAFFIC_LIGHTS_W: f32 = 78.0;

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Color32,
    pub glass: Color32,
    pub glass_stroke: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub separator: Color32,
    pub shadow: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            canvas: Color32::from_rgb(30, 30, 33),
            glass: Color32::from_rgba_unmultiplied(44, 44, 48, 228),
            glass_stroke: Color32::from_rgba_unmultiplied(255, 255, 255, 24),
            text: Color32::from_rgb(242, 242, 247),
            secondary: Color32::from_rgb(152, 152, 160),
            accent: Color32::from_rgb(10, 132, 255),
            accent_soft: Color32::from_rgba_unmultiplied(10, 132, 255, 60),
            control: Color32::from_rgba_unmultiplied(255, 255, 255, 18),
            control_hover: Color32::from_rgba_unmultiplied(255, 255, 255, 32),
            separator: Color32::from_rgba_unmultiplied(255, 255, 255, 20),
            shadow: Color32::from_black_alpha(110),
        }
    } else {
        Palette {
            canvas: Color32::from_rgb(232, 232, 236),
            glass: Color32::from_rgba_unmultiplied(250, 250, 252, 232),
            glass_stroke: Color32::from_rgba_unmultiplied(0, 0, 0, 22),
            text: Color32::from_rgb(29, 29, 31),
            secondary: Color32::from_rgb(110, 110, 115),
            accent: Color32::from_rgb(0, 122, 255),
            accent_soft: Color32::from_rgba_unmultiplied(0, 122, 255, 40),
            control: Color32::from_rgba_unmultiplied(0, 0, 0, 12),
            control_hover: Color32::from_rgba_unmultiplied(0, 0, 0, 22),
            separator: Color32::from_rgba_unmultiplied(0, 0, 0, 18),
            shadow: Color32::from_black_alpha(40),
        }
    }
}

pub fn current(ctx: &egui::Context) -> Palette {
    palette(ctx.style().visuals.dark_mode)
}

/// Marco de un panel flotante de vidrio.
pub fn glass_frame(pal: &Palette) -> egui::Frame {
    egui::Frame::none()
        .fill(pal.glass)
        .stroke(Stroke::new(1.0f32, pal.glass_stroke))
        .rounding(Rounding::same(PANEL_RADIUS))
        .inner_margin(egui::Margin::same(PANEL_MARGIN + 4.0))
        .shadow(Shadow { offset: Vec2::new(0.0, 10.0), blur: 30.0, spread: 0.0, color: pal.shadow })
}

fn visuals(dark: bool) -> egui::Visuals {
    let pal = palette(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = pal.canvas;
    v.window_fill = pal.glass;
    v.window_stroke = Stroke::new(1.0f32, pal.glass_stroke);
    v.window_rounding = Rounding::same(PANEL_RADIUS);
    v.window_shadow = Shadow { offset: Vec2::new(0.0, 16.0), blur: 40.0, spread: 0.0, color: pal.shadow };
    v.popup_shadow = Shadow { offset: Vec2::new(0.0, 8.0), blur: 20.0, spread: 0.0, color: pal.shadow };
    v.menu_rounding = Rounding::same(CONTROL_RADIUS + 2.0);
    v.extreme_bg_color = if dark { Color32::from_rgb(28, 28, 30) } else { Color32::WHITE };
    v.faint_bg_color = pal.control;
    v.selection.bg_fill = pal.accent_soft;
    v.selection.stroke = Stroke::new(1.0f32, pal.accent);
    v.hyperlink_color = pal.accent;
    v.override_text_color = Some(pal.text);
    let r = Rounding::same(CONTROL_RADIUS);
    for (w, fill, stroke) in [
        (&mut v.widgets.noninteractive, Color32::TRANSPARENT, pal.separator),
        (&mut v.widgets.inactive, pal.control, Color32::TRANSPARENT),
        (&mut v.widgets.hovered, pal.control_hover, Color32::TRANSPARENT),
        (&mut v.widgets.active, pal.accent_soft, Color32::TRANSPARENT),
        (&mut v.widgets.open, pal.control_hover, Color32::TRANSPARENT),
    ] {
        w.rounding = r;
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0f32, stroke);
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0f32, pal.text);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0f32, pal.text);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0f32, pal.text);
    v.widgets.active.fg_stroke = Stroke::new(1.0f32, pal.accent);
    v.slider_trailing_fill = true;
    v
}

fn style(dark: bool) -> egui::Style {
    let mut s = egui::Style { visuals: visuals(dark), ..Default::default() };
    s.spacing.item_spacing = Vec2::new(8.0, 7.0);
    s.spacing.button_padding = Vec2::new(10.0, 5.0);
    s.spacing.interact_size = Vec2::new(36.0, 26.0);
    s.spacing.slider_width = 140.0;
    s.spacing.combo_width = 150.0;
    s.text_styles = [
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(17.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
    ]
    .into();
    s
}

/// Tipografía del sistema (SF Pro) si está disponible; si no, la de egui.
fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    for (name, path) in [("sf", "/System/Library/Fonts/SFNS.ttf"), ("sfmono", "/System/Library/Fonts/SFNSMono.ttf")] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(name.into(), egui::FontData::from_owned(bytes));
            let family = if name == "sf" { FontFamily::Proportional } else { FontFamily::Monospace };
            fonts.families.entry(family).or_default().insert(0, name.into());
        }
    }
    fonts
}

pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_style_of(egui::Theme::Light, style(false));
    ctx.set_style_of(egui::Theme::Dark, style(true));
    ctx.options_mut(|o| o.theme_preference = egui::ThemePreference::System);
}

/// Título de sección en el estilo de las barras laterales de macOS.
pub fn section(ui: &mut egui::Ui, text: &str) {
    let pal = current(ui.ctx());
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text).size(11.5).color(pal.secondary).strong());
}

/// Botón con estilo de acento (acción principal).
pub fn accent_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let pal = current(ui.ctx());
    ui.add(
        egui::Button::new(egui::RichText::new(text).color(Color32::WHITE))
            .fill(pal.accent)
            .rounding(Rounding::same(CONTROL_RADIUS)),
    )
}

/// Muestra de color redonda.
pub fn swatch(ui: &mut egui::Ui, color: [u8; 3], size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
    let pal = current(ui.ctx());
    ui.painter().circle_filled(rect.center(), size / 2.0, Color32::from_rgb(color[0], color[1], color[2]));
    ui.painter().circle_stroke(rect.center(), size / 2.0, Stroke::new(1.0f32, pal.glass_stroke));
    resp
}

pub fn pen_color(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
