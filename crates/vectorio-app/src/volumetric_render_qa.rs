//! Capturas del espacio nuevo a partir de los triángulos reales de egui.
use super::*;
use egui::Vec2;
use image::{Rgba, RgbaImage};
use std::collections::HashMap;
#[test]
#[ignore = "Captura visual de VOLUMETRIC sin abrir ventanas"]
fn render_volumetric_workspace() {
    let _lock = crate::i18n::locale_lock();
    rust_i18n::set_locale("es");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/volumetric");
    std::fs::create_dir_all(&dir).unwrap();
    let model_path = std::env::var_os("VECTORIO_QA_MODEL").map(std::path::PathBuf::from);
    let mut cases = vec![
        (980, 620, true, "workspace"),
        (1440, 900, false, "workspace"),
        (980, 620, true, "error"),
        (1440, 900, false, "imported"),
        (1440, 900, false, "selected"),
        (980, 620, true, "passes-sheet"),
        (1440, 900, false, "loading"),
        (1440, 900, false, "light-gizmo"),
        (980, 620, true, "light-gizmo-small"),
        (1440, 900, false, "paper-moved"),
    ];
    if std::env::var_os("VECTORIO_QA_TONE").is_some() {
        cases = vec![
            (1440, 900, false, "grabado"),
            (1440, 900, false, "aguafuerte"),
            (1440, 900, false, "tecnico"),
            (1440, 900, false, "topografia"),
            (980, 620, true, "grabado"),
            (980, 620, true, "aguafuerte"),
            (980, 620, true, "tecnico"),
            (980, 620, true, "topografia"),
        ];
    }
    if std::env::var_os("VECTORIO_QA_READING").is_some() {
        cases = vec![
            (1440, 900, false, "lecture-curvature"),
            (1440, 900, false, "lecture-ambient"),
            (1440, 900, false, "lecture-thermal"),
            (1440, 900, false, "lecture-relief"),
            (1440, 900, false, "lecture-horizons"),
            (1440, 900, false, "lecture-stereo"),
            (1440, 900, false, "lecture-unfold"),
            (1440, 900, false, "lecture-section"),
            (980, 620, true, "lecture-thermal"),
            (980, 620, true, "lecture-ambient"),
        ];
    }
    if std::env::var_os("VECTORIO_QA_FINISH").is_some() {
        cases = vec![
            (1440, 900, false, "finish-halo"),
            (1440, 900, false, "finish-echo"),
            (1440, 900, false, "finish-hand"),
            (1440, 900, false, "finish-gradient"),
            (980, 620, true, "finish-echo"),
            (980, 620, true, "finish-gradient"),
        ];
    }
    if std::env::var_os("VECTORIO_QA_CONTINUITY").is_some() {
        cases = vec![
            (1440, 900, false, "continuity-perspective"),
            (1440, 900, false, "continuity-four"),
            (980, 620, true, "continuity-turn"),
            (980, 620, true, "continuity-edit"),
        ];
    }
    if std::env::var_os("VECTORIO_QA_COHERENCE").is_some() {
        cases = vec![
            (1440, 900, false, "coherence-workspace"),
            (1440, 900, false, "coherence-catalog"),
            (980, 620, true, "coherence-catalog"),
            (1440, 900, false, "coherence-palette"),
            (980, 620, true, "coherence-palette"),
        ];
    }
    if std::env::var_os("VECTORIO_QA_EXTRA").is_some() {
        cases = vec![
            (1440, 900, false, "extra-optical"),
            (980, 620, true, "extra-optical"),
            (1440, 900, false, "extra-isolines"),
            (980, 620, true, "extra-section"),
            (1440, 900, false, "extra-light"),
        ];
    }
    let features = std::env::var_os("VECTORIO_QA_FEATURES").is_some();
    if features {
        cases = vec![
            (1440, 900, false, "features-suggestive"),
            (980, 620, true, "features-suggestive"),
            (1440, 900, false, "features-apparent"),
            (980, 620, true, "features-apparent"),
        ];
        if model_path.is_some() {
            cases.extend([
                (1440, 900, false, "features-spider-suggestive"),
                (980, 620, true, "features-spider-suggestive"),
                (1440, 900, false, "features-spider-apparent"),
                (980, 620, true, "features-spider-apparent"),
            ]);
        }
    }
    if std::env::var_os("VECTORIO_QA_FLOW").is_some() {
        cases = vec![(1440,900,false,"features-flow"),(980,620,true,"features-flow"), (1440,900,false,"features-flow-minimum")];
        if model_path.is_some() {
            cases.extend([(1440,900,false,"features-spider-flow"),(980,620,true,"features-spider-flow")]);
        }
    }
    if model_path.is_some() && !features && std::env::var_os("VECTORIO_QA_FLOW").is_none() {
        cases.extend([(1440, 900, false, "large"), (980, 620, true, "drop")]);
    }
    let shadow = std::env::var_os("VECTORIO_QA_SHADOW").is_some();
    if shadow {
        cases.extend([(1440, 900, false, "paths"), (1440, 900, false, "tones")]);
    }
    let art = std::env::var_os("VECTORIO_QA_ART").is_some();
    if art {
        cases = vec![
            (1440, 900, false, "ondas"),
            (1440, 900, false, "malla"),
            (1440, 900, false, "combinado"),
            (980, 620, true, "malla"),
            (980, 620, true, "combinado"),
            (1440, 900, false, "interferencias"),
            (1440, 900, false, "tintas"),
        ];
        if model_path.is_some() {
            cases.extend([
                (1440, 900, false, "ondas-grande"),
                (1440, 900, false, "malla-grande"),
                (1440, 900, false, "combinado-grande"),
            ]);
        }
    }
    for (width, height, dark, case) in cases {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        app.mode = ViewMode::Volumetric;
        ctx.options_mut(|o| {
            o.theme_preference = if dark {
                egui::ThemePreference::Dark
            } else {
                egui::ThemePreference::Light
            }
        });
        let mut state = VolumetricUi::default();
        if shadow {
            state.settings.recipe = RecipeKind::FlatShadow;
        }
        if art {
            state.settings.recipe = if case.starts_with("malla") || case == "tintas" {
                RecipeKind::TriMesh
            } else if case.starts_with("combinado") {
                RecipeKind::Hybrid
            } else {
                RecipeKind::SurfaceWaves
            };
            if case == "interferencias" {
                state.settings.art.pattern = WavePattern::Interference;
            }
        }
        if case == "paths" {
            state.settings.shadow.hatch_spacing_mm = 0.7;
        }
        if case == "tones" {
            state.settings.shadow.middle_tone = true;
            state.settings.shadow.outline = true;
        }
        if case == "error" {
            state.load_error =
                Some("El STL está truncado. Revisa el archivo o vuelve a exportarlo.".into());
        }
        if case == "imported" {
            state.mesh = Arc::new(
                parse_stl(
                    include_bytes!("../../vectorio-core/tests/fixtures/volumetric/cube-binary.stl"),
                    "cube-binary",
                )
                .unwrap()
                .0,
            );
            state.primitive = None;
        }

        if case == "large" || case == "drop" || case.ends_with("-grande") {
            state.mesh = Arc::new(load_mesh(model_path.as_ref().unwrap()).unwrap().0);
            state.primitive = None;
        }
        if case == "drop" {
            state.layout = Layout::Paper;
        }
        if case == "selected" {
            state.settings.recipe = RecipeKind::Hybrid;
        }
        if case.starts_with("light-gizmo") {
            state.settings.recipe = RecipeKind::FlatShadow;
            if case.ends_with("small") {
                state.scene_open = Some(2);
            }
        }
        if case == "paper-moved" {
            state.settings.position_mm = [30., -12.];
            state.settings.zoom = 1.35;
        }
        state.replace_passes(state.settings.passes(), "Estilo de prueba".into(), false);
        if let Some(style) = match case {
            "grabado" => Some(TonalStyle::Engraving),
            "aguafuerte" => Some(TonalStyle::Crosshatch),
            "tecnico" => Some(TonalStyle::Technical),
            "topografia" => Some(TonalStyle::ShadowTopography),
            _ => None,
        } {
            state.replace_passes(style.passes(&state.settings), style.name().into(), false);
            state.selected = Some(state.passes[0].id);
            if width < 1100 {
                state.passes_open = true;
            }
        }
        if let Some(style) = match case {
            "lecture-curvature" => Some(ReadingStyle::Curvature),
            "lecture-ambient" => Some(ReadingStyle::Ambient),
            "lecture-thermal" => Some(ReadingStyle::Thermal),
            "lecture-relief" => Some(ReadingStyle::Relief),
            "lecture-horizons" => Some(ReadingStyle::Horizons),
            "lecture-stereo" => Some(ReadingStyle::Anaglyph),
            "lecture-unfold" => Some(ReadingStyle::Unfold),
            "lecture-section" => Some(ReadingStyle::Section),
            _ => None,
        } {
            if matches!(style, ReadingStyle::Unfold | ReadingStyle::Section) {
                state.choose_primitive(Primitive::Cube);
            }
            let mut passes = style.passes(&state.settings);
            if style == ReadingStyle::Thermal {
                passes[0].reading.as_mut().unwrap().legend = true;
                passes[0].reading.as_mut().unwrap().date = "2026-11-16".into();
            }
            if style == ReadingStyle::Anaglyph {
                app.doc.pens.push(Pen::new("Cian", [0, 170, 190], 0.25));
                passes[0].reading.as_mut().unwrap().pens = vec![1, app.doc.pens.len() - 1];
            }
            state.replace_passes(passes, style.name().into(), false);
            state.selected = Some(state.passes[0].id);
            if width < 1100 {
                state.passes_open = true;
            }
        }
        if let Some(style) = match case {
            "finish-halo" => Some(ArtisticStyle::Halo),
            "finish-echo" => Some(ArtisticStyle::Echo),
            "finish-hand" => Some(ArtisticStyle::Hand),
            "finish-gradient" => Some(ArtisticStyle::Gradient),
            _ => None,
        } {
            state.replace_passes(style.passes(&state.settings), style.name().into(), false);
            state.selected = Some(state.passes[0].id);
            if width < 1100 {
                state.passes_open = true;
            }
        }
        if case.starts_with("continuity-") {
            state.choose_primitive(Primitive::Assembly);
            state.settings.projection.perspective = true;
            state.settings.projection.distance = 0.8;
            if case == "continuity-four" {
                state.settings.sheet = SheetViews::Orthographic;
                state.scene_open = Some(3);
            }
            if case == "continuity-turn" {
                state.settings.sheet = SheetViews::Turntable;
                state.settings.view_count = 6;
                state.scene_open = Some(3);
            }
            if case == "continuity-edit" {
                state.editing = Some(99);
                state.scene_open = Some(1);
            }
            state.replace_passes(
                TonalStyle::Technical.passes(&state.settings),
                "Plano técnico".into(),
                false,
            );
        }
        if case.starts_with("coherence-") {
            state.replace_passes(
                ReadingStyle::Thermal.passes(&state.settings),
                ReadingStyle::Thermal.name().into(),
                false,
            );
            state.passes[0].pass.reading.as_mut().unwrap().pens = vec![3, 4, 6];
            state.selected = Some(state.passes[0].id);
            state.passes_open = width < 1100;
        }
        if case.starts_with("extra-") {
            let style = match case {
                "extra-optical" => ReadingStyle::OpticalThermal,
                "extra-isolines" => ReadingStyle::ScalarContours,
                "extra-section" => ReadingStyle::Section,
                _ => ReadingStyle::LightContours,
            };
            if case == "extra-section" {
                state.choose_primitive(Primitive::Cube);
            }
            state.replace_passes(style.passes(&state.settings), style.name().into(), false);
            let map = state.passes[0].pass.reading.as_mut().unwrap();
            if case == "extra-optical" {
                map.pens = InkSet::Cmyk.apply(&mut app.doc);
                map.legend = true;
                map.date = "2026-11-16".into();
            }
            if case == "extra-isolines" {
                map.label_values = true;
            }
            map.bind_palette(&app.doc.pens);
            if case == "extra-section" {
                if let PassKind::Reading(ReadingParams {
                    effect:
                        ReadingEffect::Section {
                            show_dimensions, ..
                        },
                    ..
                }) = &mut state.passes[0].pass.kind
                {
                    *show_dimensions = false;
                }
            }
            state.selected = Some(state.passes[0].id);
            state.passes_open = width < 1100;
        }
        if case.starts_with("features-") {
            if case.contains("spider") {
                state.mesh = Arc::new(load_mesh(model_path.as_ref().unwrap()).unwrap().0);
                state.primitive = None;
            } else {
                state.choose_primitive(Primitive::Torus);
            }
            let style = if case.contains("flow") { ReadingStyle::PrincipalFlow } else if case.ends_with("suggestive") { ReadingStyle::Suggestive } else { ReadingStyle::ApparentRidges };
            state.replace_passes(style.passes(&state.settings), style.name().into(), false);
            if case.ends_with("minimum") {
                if let PassKind::Reading(ReadingParams {effect:ReadingEffect::PrincipalFlow {minimum,..},..}) = &mut state.passes[0].pass.kind {*minimum=true;}
            }
            state.selected = Some(state.passes[0].id);
            state.passes_open = width < 1100;
        }
        tests::prepare(&mut state, &app.doc.paper);
        if case == "selected" || case == "passes-sheet" {
            state.selected = Some(state.passes[0].id);
        }
        if case == "passes-sheet" {
            state.passes_open = true;
        }
        app.volumetric_ui = state;
        let mut textures = HashMap::new();
        let mut output = None;
        let mut advanced = None;
        for frame in 0..10 {
            if case == "loading" {
                app.volumetric_ui.settings.yaw += 0.01;
            }
            if case == "coherence-catalog" && frame == 5 {
                ctx.data_mut(|d| {
                    d.insert_temp(
                        egui::Id::new("vol-style-catalog").with("query"),
                        "curvatura".to_string(),
                    )
                });
            }
            let result = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width as f32, height as f32),
                    )),
                    events: if (case == "tones" || case == "tintas" || case == "interferencias")
                        && frame == 3
                        || case == "light-gizmo" && (frame == 2 || frame == 5)
                        || (case == "coherence-catalog" || case == "coherence-palette")
                            && frame == 3
                    {
                        advanced.map_or_else(Vec::new, |pos| {
                            vec![
                                egui::Event::PointerMoved(pos),
                                egui::Event::PointerButton {
                                    pos,
                                    button: egui::PointerButton::Primary,
                                    pressed: true,
                                    modifiers: Default::default(),
                                },
                                egui::Event::PointerButton {
                                    pos,
                                    button: egui::PointerButton::Primary,
                                    pressed: false,
                                    modifiers: Default::default(),
                                },
                            ]
                        })
                    } else if (case == "tintas" || case == "interferencias") && frame == 5 {
                        vec![
                            egui::Event::PointerMoved(egui::pos2(240., 600.)),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                delta: egui::vec2(0., -480.),
                                modifiers: Default::default(),
                            },
                        ]
                    } else {
                        vec![]
                    },
                    time: Some(frame as f64 / 10.),
                    ..Default::default()
                },
                |ctx| {
                    if case.starts_with("coherence-") || case.starts_with("extra-") {
                        app.toolbar(ctx, &theme::current(ctx));
                    }
                    show(&mut app, ctx)
                },
            );
            for (id, delta) in &result.textures_delta.set {
                let size = delta.image.size();
                let pixels: Vec<_> = match &delta.image {
                    egui::ImageData::Color(i) => i.pixels.clone(),
                    egui::ImageData::Font(i) => i.srgba_pixels(None).collect(),
                };
                if delta.pos.is_none() {
                    textures.insert(*id, RgbaImage::new(size[0] as u32, size[1] as u32));
                }
                let texture = textures.get_mut(id).unwrap();
                let offset = delta.pos.unwrap_or([0, 0]);
                for (i, color) in pixels.iter().enumerate() {
                    texture.put_pixel(
                        (offset[0] + i % size[0]) as u32,
                        (offset[1] + i / size[0]) as u32,
                        Rgba(color.to_array()),
                    );
                }
            }
            for clipped in &result.shapes {
                if let egui::Shape::Text(t) = &clipped.shape {
                    if t.galley.text()
                        == if case == "coherence-catalog" {
                            tr!("Mapa térmico")
                        } else if case == "coherence-palette" {
                            tr!("Tintas · %{n}", n = 3)
                        } else if case == "light-gizmo" {
                            if frame < 3 {
                                tr!("Cámara")
                            } else {
                                tr!("Luz")
                            }
                        } else if case == "tintas" {
                            tr!("Composición y tintas")
                        } else if case == "interferencias" {
                            tr!("Origen y variación")
                        } else {
                            tr!("Acabado y trazos")
                        }
                    {
                        if case != "coherence-catalog" || t.pos.y < 100. {
                            advanced = Some(t.pos + egui::vec2(30., 6.));
                        }
                    }
                }
            }
            output = Some(result);
        }
        let output = output.unwrap();
        let mut canvas = RgbaImage::from_pixel(width, height, Rgba([245, 245, 245, 255]));
        for primitive in ctx.tessellate(output.shapes, output.pixels_per_point) {
            if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                let texture = &textures[&mesh.texture_id];
                for tri in mesh.indices.as_chunks::<3>().0 {
                    raster(
                        &mut canvas,
                        texture,
                        [
                            mesh.vertices[tri[0] as usize],
                            mesh.vertices[tri[1] as usize],
                            mesh.vertices[tri[2] as usize],
                        ],
                        primitive.clip_rect,
                    );
                }
            }
        }
        canvas
            .save(dir.join(format!(
                "{}{case}-{width}.png",
                if shadow { "shadow-" } else { "" }
            )))
            .unwrap();
    }
}
pub(crate) fn raster(
    canvas: &mut RgbaImage,
    texture: &RgbaImage,
    vertices: [egui::epaint::Vertex; 3],
    clip: Rect,
) {
    let bounds = Rect::from_points(&vertices.map(|v| v.pos)).intersect(clip);
    let a = vertices[1].pos - vertices[0].pos;
    let b = vertices[2].pos - vertices[0].pos;
    let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
    let determinant = cross(a, b);
    if determinant.abs() < 1e-8 {
        return;
    }
    for y in
        bounds.top().max(0.0) as u32..(bounds.bottom().ceil().max(0.0) as u32).min(canvas.height())
    {
        for x in bounds.left().max(0.0) as u32
            ..(bounds.right().ceil().max(0.0) as u32).min(canvas.width())
        {
            let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5) - vertices[0].pos;
            let w1 = cross(p, b) / determinant;
            let w2 = cross(a, p) / determinant;
            let weights = [1.0 - w1 - w2, w1, w2];
            if weights.iter().any(|w| *w < 0.0) {
                continue;
            }
            let uv = vertices
                .iter()
                .zip(weights)
                .fold(Vec2::ZERO, |sum, (v, w)| sum + v.uv.to_vec2() * w);
            let tx = ((uv.x * texture.width() as f32) as u32).min(texture.width() - 1);
            let ty = ((uv.y * texture.height() as f32) as u32).min(texture.height() - 1);
            let texel = texture.get_pixel(tx, ty);
            let color: Vec<f32> = (0..4)
                .map(|c| {
                    vertices
                        .iter()
                        .zip(weights)
                        .map(|(v, w)| v.color.to_array()[c] as f32 * w / 255.0)
                        .sum::<f32>()
                        * texel[c] as f32
                        / 255.0
                })
                .collect();
            let dest = canvas.get_pixel_mut(x, y);
            for c in 0..3 {
                dest[c] =
                    (color[c] * 255.0 + dest[c] as f32 * (1.0 - color[3])).clamp(0.0, 255.0) as u8;
            }
        }
    }
}
