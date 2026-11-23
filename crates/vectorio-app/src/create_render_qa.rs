//! Captura de los propios triángulos de egui sin abrir ni reiniciar ventanas.
use super::*;
use image::{Rgba, RgbaImage};
use std::collections::HashMap;

#[test]
#[ignore = "Captura visual local de Crear sin tocar la sesión abierta"]
fn render_create_panels() {
    let _lock = crate::i18n::locale_lock();
    rust_i18n::set_locale("es");
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/phase5");
    std::fs::create_dir_all(&directory).unwrap();
    let catalog = std::env::var_os("VECTORIO_QA_SCENE_CATALOG").is_some();
    for (width, height) in [(980, 620), (1440, 900)] {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        app.mode = ViewMode::Create;
        app.prefs.create.generator = "festival_city".into();
        if catalog {
            app.prefs.appearance = if width == 980 { theme::Appearance::Dark } else { theme::Appearance::Light };
            ctx.options_mut(|o| o.theme_preference = if width == 980 { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        }
        let ballpoint = std::env::var_os("VECTORIO_QA_BALLPOINT").is_some();
        let pen_count = std::env::var("VECTORIO_QA_BALLPOINT").ok().and_then(|s| s.parse::<usize>().ok()).filter(|n| (4..=5).contains(n)).unwrap_or(4);
        if ballpoint {
            app.prefs.create.inks.insert("festival_city".into(), Some(InkChoice { palette: "ballpoint".into(), count: pen_count }));
            let params = app.prefs.create.params.entry("festival_city".into()).or_default();
            params.set("signs", ParamValue::Bool(false));
            params.set("event", ParamValue::Text("parade".into()));
        }
        let dome_world = std::env::var("VECTORIO_QA_DOMES").ok();
        if let Some(world) = &dome_world {
            app.prefs.create.generator = "domes".into();
            app.prefs.create.params.entry("domes".into()).or_default().set("world", ParamValue::Text(world.clone()));
            if world == "space" {
                app.prefs.create.inks.insert("domes".into(), Some(InkChoice { palette: "orbit".into(), count: 5 }));
            }
        }
        let glass_case = std::env::var("VECTORIO_QA_GLASS").ok();
        let custom_generator=std::env::var("VECTORIO_QA_GENERATOR").ok();
        if let Some(id)=&custom_generator{app.prefs.create.generator=id.clone();}
        let mut preferences = crate::prefs_ui::PrefsUi::default();
        if let Some(case) = &glass_case {
            ctx.options_mut(|o| o.theme_preference = if case.contains("light") { egui::ThemePreference::Light } else { egui::ThemePreference::Dark });
            preferences.show_window(&app.prefs);
        }
        let mut textures = HashMap::new();
        let mut output = None;
        let mut catalog_point = None;
        for frame in 0..12 {
            let events = if catalog && (frame == 3 || frame == 4) { catalog_point.map_or_else(Vec::new, |pos| vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton {pos,button:egui::PointerButton::Primary,pressed:frame==3,modifiers:Default::default()}]) } else { vec![] };
            let input = egui::RawInput { events, screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width as f32, height as f32))), time: Some(frame as f64 / 10.0), ..Default::default() };
            let result = ctx.run(input, |ctx| {
                if glass_case.is_some() { glass_fixture(ctx, &app, &mut preferences); }
                else {if std::env::var_os("VECTORIO_QA_COHERENCE").is_some(){app.toolbar(ctx,&theme::current(ctx));} show(&mut app, ctx); }
            });
            if catalog && frame < 3 {
                catalog_point=result.shapes.iter().find_map(|s|if let egui::Shape::Text(t)=&s.shape{
                    t.galley.text().starts_with("Ciudad en fiesta ·").then_some(t.pos+t.galley.size()*0.5)
                }else{None});
            }
            for (id, delta) in &result.textures_delta.set {
                let size = delta.image.size();
                let pixels: Vec<_> = match &delta.image {
                    egui::ImageData::Color(image) => image.pixels.clone(),
                    egui::ImageData::Font(image) => image.srgba_pixels(None).collect(),
                };
                if delta.pos.is_none() {
                    textures.insert(*id, RgbaImage::new(size[0] as u32, size[1] as u32));
                }
                let texture = textures.get_mut(id).unwrap();
                let offset = delta.pos.unwrap_or([0, 0]);
                for (i, color) in pixels.iter().enumerate() {
                    texture.put_pixel((offset[0] + i % size[0]) as u32, (offset[1] + i / size[0]) as u32, Rgba(color.to_array()));
                }
            }
            output = Some(result);
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let output = output.unwrap();
        let mut canvas = RgbaImage::from_pixel(width, height, Rgba([245, 245, 245, 255]));
        for primitive in ctx.tessellate(output.shapes, output.pixels_per_point) {
            if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                let texture = &textures[&mesh.texture_id];
                for triangle in mesh.indices.as_chunks::<3>().0 {
                    let vertices = [mesh.vertices[triangle[0] as usize], mesh.vertices[triangle[1] as usize], mesh.vertices[triangle[2] as usize]];
                    raster(&mut canvas, texture, vertices, primitive.clip_rect);
                }
            }
        }
        let name = if catalog { Some("catalogo-escenas".into()) } else { custom_generator }.or_else(||glass_case.as_ref().map(|case| format!("glass-{case}")))
            .or_else(|| dome_world.as_ref().map(|world| format!("cupulas-{world}")))
            .unwrap_or_else(|| if ballpoint { format!("boligrafo-{pen_count}") } else { "crear".into() });
        canvas.save(directory.join(format!("{name}-{width}.png"))).unwrap();
    }
}

fn raster(canvas: &mut RgbaImage, texture: &RgbaImage, vertices: [egui::epaint::Vertex; 3], clip: Rect) {
    let bounds = Rect::from_points(&vertices.map(|v| v.pos)).intersect(clip);
    let a = vertices[1].pos - vertices[0].pos;
    let b = vertices[2].pos - vertices[0].pos;
    let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
    let determinant = cross(a, b);
    if determinant.abs() < 1e-8 {
        return;
    }
    for y in bounds.top().max(0.0) as u32..(bounds.bottom().ceil().max(0.0) as u32).min(canvas.height()) {
        for x in bounds.left().max(0.0) as u32..(bounds.right().ceil().max(0.0) as u32).min(canvas.width()) {
            let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5) - vertices[0].pos;
            let w1 = cross(p, b) / determinant;
            let w2 = cross(a, p) / determinant;
            let weights = [1.0 - w1 - w2, w1, w2];
            if weights.iter().any(|w| *w < 0.0) {
                continue;
            }
            let uv = vertices.iter().zip(weights).fold(Vec2::ZERO, |sum, (v, w)| sum + v.uv.to_vec2() * w);
            let tx = ((uv.x * texture.width() as f32) as u32).min(texture.width() - 1);
            let ty = ((uv.y * texture.height() as f32) as u32).min(texture.height() - 1);
            let texel = texture.get_pixel(tx, ty);
            let color: Vec<f32> = (0..4).map(|c| vertices.iter().zip(weights).map(|(v, w)| v.color.to_array()[c] as f32 * w / 255.0).sum::<f32>() * texel[c] as f32 / 255.0).collect();
            let dest = canvas.get_pixel_mut(x, y);
            for c in 0..3 {
                dest[c] = (color[c] * 255.0 + dest[c] as f32 * (1.0 - color[3])).clamp(0.0, 255.0) as u8;
            }
        }
    }
}


fn glass_fixture(ctx: &egui::Context, app: &VectorioApp, preferences: &mut crate::prefs_ui::PrefsUi) {
    let pal = theme::current(ctx);
    let paper = ctx.screen_rect().expand(20.0);
    theme::set_backdrop(ctx, Some((paper, egui::Color32::WHITE)));
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.painter().rect_filled(paper, 0.0, egui::Color32::from_gray(220));
        for i in -30..80 {
            let x = i as f32 * 22.0;
            ui.painter().line_segment([egui::pos2(x, 0.0), egui::pos2(x + 900.0, 900.0)], Stroke::new(1.5f32, egui::Color32::from_rgb(35, 50, 105)));
        }
    });
    preferences.show(ctx, &app.prefs, &app.doc, &pal);
    egui::Area::new(egui::Id::new("glass-fixture-toolbar")).fixed_pos(egui::pos2(280.0, 12.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        ui.horizontal(|ui| {
            crate::app::capsule(ui, &pal, |ui| {
                ui.add_enabled(false, egui::Button::new("Deshacer"));
                ui.add_enabled(false, egui::Button::new("Rehacer"));
            });
            crate::app::capsule(ui, &pal, |ui| {
                for label in ["Diseño", "Escenas", "Salida"] {
                    crate::app::text_segment(ui, &pal, label, label == "Escenas");
                }
            });
            theme::accent_button(ui, "Exportar");
        });
    });
}
