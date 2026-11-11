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
    /// Base translúcida de los paneles flotantes (parte inferior del degradado).
    pub glass: Color32,
    /// Parte superior del vidrio, algo más clara y densa.
    pub glass_top: Color32,
    /// Brillo del borde interior: intenso arriba, casi nulo abajo.
    pub rim_top: Color32,
    pub rim_bottom: Color32,
    /// Superficies que deben leerse siempre igual: barra superior, menús y hojas.
    pub bar: Color32,
    /// Tarjetas de tinta sobre el vidrio.
    pub card: Color32,
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
            glass: Color32::from_rgba_unmultiplied(34, 34, 38, 150),
            glass_top: Color32::from_rgba_unmultiplied(66, 66, 72, 182),
            rim_top: Color32::from_rgba_unmultiplied(255, 255, 255, 64),
            rim_bottom: Color32::from_rgba_unmultiplied(255, 255, 255, 8),
            bar: Color32::from_rgba_unmultiplied(44, 44, 48, 228),
            card: Color32::from_rgba_unmultiplied(64, 64, 70, 226),
            glass_stroke: Color32::from_rgba_unmultiplied(255, 255, 255, 24),
            text: Color32::from_rgb(242, 242, 247),
            secondary: Color32::from_rgb(152, 152, 160),
            accent: Color32::from_rgb(10, 132, 255),
            accent_soft: Color32::from_rgb(43, 62, 87),
            control: Color32::from_rgb(58, 58, 62),
            control_hover: Color32::from_rgb(70, 70, 75),
            separator: Color32::from_rgba_unmultiplied(255, 255, 255, 20),
            shadow: Color32::from_black_alpha(110),
        }
    } else {
        Palette {
            canvas: Color32::from_rgb(232, 232, 236),
            glass: Color32::from_rgba_unmultiplied(242, 242, 247, 150),
            glass_top: Color32::from_rgba_unmultiplied(255, 255, 255, 192),
            rim_top: Color32::from_rgba_unmultiplied(255, 255, 255, 235),
            rim_bottom: Color32::from_rgba_unmultiplied(255, 255, 255, 50),
            bar: Color32::from_rgba_unmultiplied(250, 250, 252, 232),
            card: Color32::from_rgba_unmultiplied(255, 255, 255, 232),
            glass_stroke: Color32::from_rgba_unmultiplied(0, 0, 0, 22),
            text: Color32::from_rgb(29, 29, 31),
            secondary: Color32::from_rgb(110, 110, 115),
            accent: Color32::from_rgb(0, 122, 255),
            accent_soft: Color32::from_rgb(224, 235, 250),
            control: Color32::from_rgb(239, 239, 242),
            control_hover: Color32::from_rgb(228, 228, 233),
            separator: Color32::from_rgba_unmultiplied(0, 0, 0, 18),
            shadow: Color32::from_black_alpha(40),
        }
    }
}

pub fn current(ctx: &egui::Context) -> Palette {
    palette(ctx.style().visuals.dark_mode)
}

/// Marco opaco para hojas modales y ventanas que deben leerse sin distracciones.
pub fn glass_frame(pal: &Palette) -> egui::Frame {
    egui::Frame::none()
        .fill(pal.bar)
        .stroke(Stroke::new(1.0f32, pal.glass_stroke))
        .rounding(Rounding::same(PANEL_RADIUS))
        .inner_margin(egui::Margin::same(PANEL_MARGIN + 4.0))
        .shadow(Shadow { offset: Vec2::new(0.0, 10.0), blur: 30.0, spread: 0.0, color: pal.shadow })
}

/// Panel flotante de vidrio líquido. egui no puede desenfocar lo que hay debajo, así
/// que el efecto combina un degradado translúcido, un borde especular y una sombra
/// suave: el dibujo se intuye a través del panel sin restar legibilidad.
pub fn glass_panel<R>(ui: &mut egui::Ui, pal: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    glass_panel_with(ui, pal, PANEL_RADIUS, egui::Margin::same(PANEL_MARGIN + 4.0), add)
}

pub fn glass_panel_with<R>(ui: &mut egui::Ui, pal: &Palette, radius: f32, margin: egui::Margin, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let background = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Frame::none().inner_margin(margin).show(ui, add);
    ui.painter().set(background, glass_shape(inner.response.rect, radius, pal));
    inner
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Contorno de un rectángulo redondeado en sentido horario, con arcos de ocho tramos.
fn rounded_outline(rect: egui::Rect, radius: f32) -> Vec<egui::Pos2> {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
    let corners = [
        (rect.right_top() + Vec2::new(-r, r), -90.0f32),
        (rect.right_bottom() + Vec2::new(-r, -r), 0.0),
        (rect.left_bottom() + Vec2::new(r, -r), 90.0),
        (rect.left_top() + Vec2::new(r, r), 180.0),
    ];
    let mut points = Vec::with_capacity(36);
    for (center, start) in corners {
        for i in 0..=8 {
            let a = (start + i as f32 * 90.0 / 8.0).to_radians();
            points.push(center + Vec2::new(a.cos(), a.sin()) * r);
        }
    }
    points
}

pub fn glass_shape(rect: egui::Rect, radius: f32, pal: &Palette) -> egui::Shape {
    let shade = |y: f32| ((y - rect.top()) / rect.height().max(1.0)).powf(0.6);
    let outer = rounded_outline(rect, radius);
    let inner = rounded_outline(rect.shrink(1.25), (radius - 1.25).max(0.0));
    // Relleno: abanico desde el centro con color por vértice (degradado vertical).
    let mut fill = egui::Mesh::default();
    fill.colored_vertex(rect.center(), mix(pal.glass_top, pal.glass, shade(rect.center().y)));
    for q in &outer { fill.colored_vertex(*q, mix(pal.glass_top, pal.glass, shade(q.y))); }
    let n = outer.len() as u32;
    for i in 0..n { fill.add_triangle(0, 1 + i, 1 + (i + 1) % n); }
    // Borde especular: banda fina entre el contorno exterior y el interior.
    let mut rim = egui::Mesh::default();
    for (o, i) in outer.iter().zip(&inner) {
        let color = mix(pal.rim_top, pal.rim_bottom, shade(o.y));
        rim.colored_vertex(*o, color);
        rim.colored_vertex(*i, color);
    }
    for k in 0..n {
        let (a, b) = (2 * k, 2 * ((k + 1) % n));
        rim.add_triangle(a, a + 1, b);
        rim.add_triangle(a + 1, b + 1, b);
    }
    let shadow = Shadow { offset: Vec2::new(0.0, 8.0), blur: 26.0, spread: 0.0, color: pal.shadow.gamma_multiply(0.7) };
    egui::Shape::Vec(vec![
        shadow.as_shape(rect, Rounding::same(radius)).into(),
        egui::Shape::mesh(fill),
        egui::Shape::mesh(rim),
        egui::Shape::rect_stroke(rect, Rounding::same(radius), Stroke::new(1.0f32, pal.glass_stroke)),
    ])
}

fn visuals(dark: bool) -> egui::Visuals {
    let pal = palette(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = pal.canvas;
    v.window_fill = pal.bar;
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

pub fn close_button(ui: &mut egui::Ui) -> egui::Response {
    let pal = current(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::click());
    ui.painter().circle_filled(rect.center(), 12.0, if response.hovered() { pal.control_hover } else { pal.control });
    let center = rect.center();
    let stroke = Stroke::new(1.5f32, pal.secondary);
    ui.painter().line_segment([center + Vec2::new(-4.0, -4.0), center + Vec2::new(4.0, 4.0)], stroke);
    ui.painter().line_segment([center + Vec2::new(-4.0, 4.0), center + Vec2::new(4.0, -4.0)], stroke);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Cerrar"));
    response.on_hover_text("Cerrar sin guardar (Esc)")
}
