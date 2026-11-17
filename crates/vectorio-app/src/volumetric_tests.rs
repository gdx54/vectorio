use super::*;
#[test]
fn saved_palettes_apply_to_the_target_pass_and_undo_restores_its_families() {
    let mut state = VolumetricUi::default();
    let mut doc = vectorio_core::Document::default();
    state.replace_passes(ReadingStyle::OpticalThermal.passes(&state.settings), "Prueba".into(), false);
    let id = state.passes[0].id;
    let before = state.passes[0].pass.clone();
    let original = doc.pens.clone();
    let palette = crate::palettes::presets().remove(1);
    state.apply_palette(id, &palette, &mut doc).unwrap();
    let map = state.passes[0].pass.reading.as_ref().unwrap();
    assert!(palette.matches(&map.pens, &doc.pens));
    assert_eq!(map.optical.inks.len(), 4);
    assert_eq!(&doc.pens[..original.len()], original);
    state.passes[0].pass.validate().unwrap();
    let applied = state.passes[0].pass.clone();
    state.undo_stack(false);
    assert_eq!(state.passes[0].pass, before);
    state.undo_stack(true);
    assert_eq!(state.passes[0].pass, applied);
    assert!(state.apply_palette(u64::MAX, &palette, &mut doc).is_err());
    assert_eq!(state.passes[0].pass, applied);
}

#[test]
fn palettes_respect_section_family_minimum_and_density_uses_only_one_ink() {
    let mut state = VolumetricUi::default();
    let mut doc = vectorio_core::Document::default();
    state.replace_passes(ReadingStyle::Section.passes(&state.settings), "Sección".into(), false);
    let before = doc.clone();
    let pass = state.passes[0].clone();
    let palette = crate::palettes::presets().remove(0);
    assert!(state.apply_palette(pass.id, &palette, &mut doc).is_err());
    assert_eq!(doc, before);
    assert_eq!(state.passes[0].pass, pass.pass);
    let map = state.passes[0].pass.reading.as_mut().unwrap();
    map.encoding = Encoding::Density;
    map.ramp = Ramp::Sequential;
    // Una vista de sección sigue necesitando sus tres familias aunque el campo tenga densidad.
    assert!(state.apply_palette(pass.id, &palette, &mut doc).is_err());
    state.replace_passes(ReadingStyle::Thermal.passes(&state.settings), "Densidad".into(), false);
    let map = state.passes[0].pass.reading.as_mut().unwrap();
    map.encoding = Encoding::Density;
    map.ramp = Ramp::Sequential;
    map.pens.truncate(1);
    let count = doc.pens.len();
    state.apply_palette(state.passes[0].id, &crate::palettes::presets().remove(1), &mut doc).unwrap();
    assert_eq!(state.passes[0].pass.reading.as_ref().unwrap().pens.len(), 1);
    assert!(doc.pens.len() <= count + 1, "no añade las tintas descartadas de la paleta");
    state.passes[0].pass.validate().unwrap();
}
#[test]
fn worker_rejects_stale_replies_and_inks_do_not_invalidate_geometry() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    state.request(&ctx, &app.doc.paper);
    let first = state.version;
    for entry in &mut state.passes {
        if let PassKind::Field(p) = &mut entry.pass.kind {
            p.cuts = 12;
        }
    }
    state.request(&ctx, &app.doc.paper);
    let latest = state.version;
    assert!(latest > first);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !state.ready && std::time::Instant::now() < deadline {
        state.poll(&mut app);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(state.ready);
    assert!(state.current(&app.doc.paper));
    assert_eq!(
        state
            .drawing
            .as_ref()
            .unwrap()
            .curves
            .iter()
            .map(|c| c.level)
            .max(),
        Some(11)
    );
    for entry in &mut state.passes {
        entry.pass.pen = 2;
    }
    state.request(&ctx, &app.doc.paper);
    assert_eq!(state.version, latest);
    state.settings.yaw += 15.;
    assert!(!state.current(&app.doc.paper));
    state.request(&ctx, &app.doc.paper);
    assert!(!state.ready);
}
#[test]
fn stale_reply_cannot_replace_the_latest_drawing() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    let (tx, _rx) = mpsc::channel();
    let (out, input) = mpsc::channel();
    state.worker = Some(Worker {
        tx,
        rx: input,
        latest: Arc::new(AtomicU64::new(2)),
    });
    state.version = 2;
    out.send(Reply {
        version: 1,
        result: Err("obsoleto".into()),
    })
    .unwrap();
    state.poll(&mut app);
    assert!(state.error.is_none());
    assert!(!state.ready);
}
#[test]
fn small_large_windows_and_both_languages_keep_actions_visible() {
    let _lock = crate::i18n::locale_lock();
    for lang in ["es", "en"] {
        rust_i18n::set_locale(lang);
        for kind in [
            RecipeKind::Atlas,
            RecipeKind::FlatShadow,
            RecipeKind::SurfaceWaves,
            RecipeKind::TriMesh,
            RecipeKind::Hybrid,
        ] {
            for (width, height) in [(980., 620.), (1440., 900.)] {
                let ctx = egui::Context::default();
                let mut app = VectorioApp::blank(&ctx);
                app.mode = ViewMode::Volumetric;
                let mut state = VolumetricUi::default();
                state.replace_passes(
                    Settings {
                        recipe: kind,
                        ..Default::default()
                    }
                    .passes(),
                    String::new(),
                    false,
                );
                prepare(&mut state, &app.doc.paper);
                app.volumetric_ui = state;
                for frame in 0..3 {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, height),
                            )),
                            ..Default::default()
                        },
                        |ctx| show(&mut app, ctx),
                    );
                    let mut found = false;
                    for clipped in &output.shapes {
                        if let egui::Shape::Text(text) = &clipped.shape {
                            if text.galley.text() == tr!("Colocar en el lienzo") {
                                let bounds = Rect::from_min_size(text.pos, text.galley.size());
                                assert!(
                                    bounds.top() >= theme::TOOLBAR_H && bounds.bottom() <= height
                                );
                                assert!(bounds.left() >= 0. && bounds.right() <= width);
                                found = true;
                            }
                        }
                    }
                    if frame > 0 {
                        assert!(
                            found,
                            "acción no visible en {width}x{height} {lang}: {:?}",
                            output
                                .shapes
                                .iter()
                                .filter_map(|s| if let egui::Shape::Text(t) = &s.shape {
                                    Some(t.galley.text())
                                } else {
                                    None
                                })
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
    }
    rust_i18n::set_locale("es");
}

#[test]
fn placed_paths_roundtrip_through_the_existing_document_and_svg() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    let drawing = Engine::default()
        .draw(state.mesh.clone(), &state.settings, &app.doc.paper, &|| {
            false
        })
        .unwrap();
    let items = drawing.items([0, 2, 1]);
    for item in items {
        app.doc.add(item);
    }
    let layers = app.doc.layers();
    assert!(!layers.is_empty());
    let svg = vectorio_core::svg_out::export(&app.doc, &layers);
    assert!(svg.contains("inkscape:groupmode"));
    let restored = vectorio_core::Document::from_json(&app.doc.to_json()).unwrap();
    assert_eq!(restored.items, app.doc.items);
    state.settings.direction = [0.; 3];
    assert!(Engine::default()
        .draw(state.mesh.clone(), &state.settings, &app.doc.paper, &|| {
            false
        })
        .is_err());
}

#[test]
fn importing_stl_works_and_a_bad_file_keeps_the_previous_model() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("modelo.stl");
    std::fs::write(
        &path,
        include_bytes!("../../vectorio-core/tests/fixtures/volumetric/cube-binary.stl"),
    )
    .unwrap();
    state.load(&ctx, path.clone());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while state.loading.is_some() && std::time::Instant::now() < deadline {
        state.poll(&mut app);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(state.primitive, None);
    assert_eq!(state.mesh.triangles.len(), 12);
    assert_eq!(state.report.boundary_edges, 0);
    assert!(state.load_error.is_none());
    let previous = state.mesh.clone();
    std::fs::write(&path, b"solid truncado").unwrap();
    state.load(&ctx, path);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while state.loading.is_some() && std::time::Instant::now() < deadline {
        state.poll(&mut app);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(Arc::ptr_eq(&state.mesh, &previous));
    assert!(state.load_error.is_some());
    state.request(&ctx, &app.doc.paper);
    assert!(state.load_error.is_some());
    assert!(app.doc.items.is_empty());
    state.choose_primitive(Primitive::Torus);
    assert_eq!(state.primitive, Some(Primitive::Torus));
    assert_eq!(state.mesh.triangles.len(), 3072);
}

#[test]
fn essential_errors_have_readable_contrast_on_both_panel_themes() {
    fn luminance(color: Color32) -> f64 {
        let c = color.to_array();
        let linear = |v: u8| {
            let v = v as f64 / 255.;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        linear(c[0]) * 0.2126 + linear(c[1]) * 0.7152 + linear(c[2]) * 0.0722
    }
    for (dark, background) in [
        (true, Color32::from_rgb(38, 38, 42)),
        (false, Color32::from_rgb(238, 238, 238)),
    ] {
        let a = luminance(error_color(dark));
        let b = luminance(background);
        assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
    }
}

#[test]
fn dropping_obj_or_stl_from_every_workspace_opens_the_3d_model_and_keeps_the_document() {
    for (mode, extension) in [
        ViewMode::Design,
        ViewMode::Create,
        ViewMode::Photo,
        ViewMode::Output,
        ViewMode::Volumetric,
    ]
    .into_iter()
    .flat_map(|m| [(m, "OBJ"), (m, "STL")])
    {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        app.mode = mode;
        app.volumetric_ui.layout = Layout::Paper;
        app.doc.add(vectorio_core::Item::new(
            "Conservar",
            vectorio_core::Geometry::Path {
                lines: vec![vec![p(10., 10.), p(20., 20.)]],
                closed: false,
            },
            p(0., 0.),
        ));
        let before = app.doc.clone();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("modelo.{extension}"));
        let bytes: &[u8] = if extension == "STL" {
            include_bytes!("../../vectorio-core/tests/fixtures/volumetric/cube-binary.stl")
        } else {
            include_bytes!("../../vectorio-core/tests/fixtures/volumetric/cube.obj")
        };
        std::fs::write(&path, bytes).unwrap();
        let _ = ctx.run(
            egui::RawInput {
                dropped_files: vec![egui::DroppedFile {
                    path: Some(path),
                    ..Default::default()
                }],
                ..Default::default()
            },
            |ctx| app.dropped_files(ctx),
        );
        assert_eq!(app.mode, ViewMode::Volumetric);
        assert!(app.volumetric_ui.layout == Layout::Paper);
        assert!(!app.vec_ui.has_photo());
        let mut state = std::mem::take(&mut app.volumetric_ui);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while state.loading.is_some() && std::time::Instant::now() < deadline {
            state.poll(&mut app);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(state.loading.is_none() && state.load_error.is_none());
        assert_eq!(state.mesh.triangles.len(), 12);
        assert_eq!(app.doc, before);
    }
}
#[test]
fn a_new_drop_supersedes_an_import_already_in_progress() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../vectorio-core/tests/fixtures/volumetric");
    state.open_model(&ctx, root.join("no-existe.stl"));
    state.open_model(&ctx, root.join("cube.obj"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while state.loading.is_some() && std::time::Instant::now() < deadline {
        state.poll(&mut app);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(state.loading.is_none() && state.load_error.is_none());
    assert_eq!(state.mesh.name, "cube");
    assert_eq!(state.mesh.triangles.len(), 12);
}

#[test]
fn pass_metadata_and_selection_do_not_invalidate_geometry() {
    let ctx = egui::Context::default();
    let paper = Paper::default();
    let mut state = VolumetricUi::default();
    state.request(&ctx, &paper);
    let version = state.version;
    state.passes[0].pass.name = "Prueba".into();
    state.passes[0].pass.pen = 5;
    state.passes[1].pass.enabled = false;
    state.passes.reverse();
    state.selected = Some(state.passes[0].id);
    state.request(&ctx, &paper);
    assert_eq!(state.version, version);
    state.passes[0].pass.visibility = Visibility::All;
    state.request(&ctx, &paper);
    assert!(state.version > version);
}
#[test]
fn arbitrary_stack_and_style_undo_preserve_pass_ids_and_inks() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    let before = state.snapshot();
    let mut settings = Settings {
        recipe: RecipeKind::TriMesh,
        ..Default::default()
    };
    settings.art.mesh_inks = 6;
    settings.art.coloring = MeshColor::Height;
    let mut passes = settings.passes();
    app.doc.pens = (0..6)
        .map(|i| Pen::new(&format!("Tinta {i}"), [i * 30, 30, 80], 0.2))
        .collect();
    for (i, p) in passes.iter_mut().enumerate() {
        p.pen = i;
    }
    state.replace_passes(passes, "Seis tintas".into(), true);
    prepare(&mut state, &app.doc.paper);
    let recipe = state.recipe(&app);
    let restored = Recipe::from_json(&recipe.to_json().unwrap()).unwrap();
    assert_eq!(restored.passes.len(), 6);
    let drawing = state.materialized(&app.doc.pens).unwrap();
    assert_eq!(
        drawing
            .items([])
            .iter()
            .filter_map(|i| i.stroke)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        6
    );
    let source = state.passes.clone();
    state.undo_stack(false);
    assert_eq!(state.passes, before.passes);
    state.undo_stack(true);
    assert_eq!(state.passes, source);
    let exported = state.materialized(&app.doc.pens).unwrap();
    state.selected = Some(state.passes[0].id);
    assert_eq!(
        exported.parts.iter().map(|p| &p.lines).collect::<Vec<_>>(),
        state
            .materialized(&app.doc.pens)
            .unwrap()
            .parts
            .iter()
            .map(|p| &p.lines)
            .collect::<Vec<_>>()
    );
}
pub(super) fn prepare(state: &mut VolumetricUi, paper: &Paper) {
    let request = state.geometry_request(paper);
    let passes: Vec<_> = request.passes.iter().map(|(_, p)| p.clone()).collect();
    state.drawing = Some(
        Engine::default()
            .draw_passes(
                state.mesh.clone(),
                &request.settings,
                &passes,
                paper,
                &|| false,
            )
            .unwrap(),
    );
    state.drawn_scene = Some((
        request.settings.clone(),
        reading_content_paper(&request.paper, request.passes.iter().map(|(_, p)| p)),
    ));
    state.drawn_ids = request.passes.iter().map(|(id, _)| *id).collect();
    state.requested = Some(request);
    state.ready = true;
}

#[test]
fn menu_undo_in_volume_restores_the_stack_without_changing_the_document() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    app.mode = ViewMode::Volumetric;
    let doc = app.doc.clone();
    let before = app.volumetric_ui.passes.clone();
    app.volumetric_ui.replace_passes(
        Settings {
            recipe: RecipeKind::SurfaceWaves,
            ..Default::default()
        }
        .passes(),
        "Ondas".into(),
        true,
    );
    app.run(&ctx, crate::commands::Command::Undo);
    assert_eq!(app.volumetric_ui.passes, before);
    assert_eq!(app.doc, doc);
    app.run(&ctx, crate::commands::Command::Redo);
    assert!(matches!(
        app.volumetric_ui.passes[0].pass.kind,
        PassKind::Waves(_)
    ));
}

#[test]
fn loading_a_custom_v2_recipe_restores_the_actual_stack_instead_of_its_legacy_mode() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    let mut recipe = Recipe::new(Primitive::Cube.mesh(), Settings::default());
    recipe.inks = app.doc.pens.clone();
    recipe.passes = Settings {
        recipe: RecipeKind::SurfaceWaves,
        ..Default::default()
    }
    .passes();
    recipe.passes[0].name = "Onda personalizada".into();
    recipe.passes[0].pen = 2;
    recipe.passes.reverse();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("custom.json");
    std::fs::write(&path, recipe.to_json().unwrap()).unwrap();
    let mut state = VolumetricUi::default();
    state.load(&ctx, path);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while state.loading.is_some() && std::time::Instant::now() < deadline {
        state.poll(&mut app);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(
        state
            .passes
            .iter()
            .map(|p| p.pass.clone())
            .collect::<Vec<_>>(),
        recipe.passes
    );
    prepare(&mut state, &app.doc.paper);
    assert!(state
        .materialized(&app.doc.pens)
        .unwrap()
        .parts
        .iter()
        .any(|p| p.pen == 2 && p.name == "Onda personalizada"));
}

fn gesture_app() -> (egui::Context, VectorioApp) {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    app.mode = ViewMode::Volumetric;
    prepare(&mut app.volumetric_ui, &app.doc.paper);
    (ctx, app)
}
fn pointer_frame(
    ctx: &egui::Context,
    app: &mut VectorioApp,
    time: f64,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440., 900.),
            )),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ctx| show(app, ctx),
    )
}
fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}
#[test]
fn dragging_paper_is_physical_and_one_undo_step_even_after_a_pause() {
    let (ctx, mut app) = gesture_app();
    let initial = app.volumetric_ui.settings.clone();
    let start = egui::pos2(650., 400.);
    pointer_frame(&ctx, &mut app, 0., vec![egui::Event::PointerMoved(start)]);
    pointer_frame(&ctx, &mut app, 0.1, vec![pointer_button(start, true)]);
    let middle = start + egui::vec2(25., 12.);
    pointer_frame(&ctx, &mut app, 0.2, vec![egui::Event::PointerMoved(middle)]);
    pointer_frame(&ctx, &mut app, 1.2, vec![]);
    let end = middle + egui::vec2(25., 12.);
    pointer_frame(&ctx, &mut app, 1.3, vec![egui::Event::PointerMoved(end)]);
    pointer_frame(&ctx, &mut app, 1.4, vec![pointer_button(end, false)]);
    let moved = app.volumetric_ui.settings.clone();
    assert!(moved.position_mm[0] > 0. && moved.position_mm[1] > 0.);
    assert!((moved.position_mm[0] / moved.position_mm[1] - 50. / 24.).abs() < 1e-5);
    assert_eq!(app.volumetric_ui.undo.len(), 1);
    app.volumetric_ui.undo_stack(false);
    assert_eq!(app.volumetric_ui.settings, initial);
    app.volumetric_ui.undo_stack(true);
    assert_eq!(app.volumetric_ui.settings, moved);
    let restored = Recipe::from_json(&app.volumetric_ui.recipe(&app).to_json().unwrap()).unwrap();
    assert_eq!(restored.settings, moved);
}
#[test]
fn orbit_respects_camera_lock_and_does_not_repeat_stationary_motion() {
    for locked in [false, true] {
        let (ctx, mut app) = gesture_app();
        app.volumetric_ui.locked = locked;
        let initial = app.volumetric_ui.settings.clone();
        let start = egui::pos2(1145., 797.);
        let end = start + egui::vec2(20., 8.);
        pointer_frame(&ctx, &mut app, 0., vec![egui::Event::PointerMoved(start)]);
        pointer_frame(&ctx, &mut app, 0.1, vec![pointer_button(start, true)]);
        pointer_frame(&ctx, &mut app, 0.2, vec![egui::Event::PointerMoved(end)]);
        let at_end = app.volumetric_ui.settings.clone();
        pointer_frame(&ctx, &mut app, 0.3, vec![]);
        pointer_frame(&ctx, &mut app, 0.4, vec![pointer_button(end, false)]);
        assert_eq!(app.volumetric_ui.settings, at_end);
        if locked {
            assert_eq!(at_end, initial);
            assert!(app.volumetric_ui.undo.is_empty());
        } else {
            assert!((at_end.yaw - initial.yaw + 8.).abs() < 1e-9);
            assert!((at_end.elevation - initial.elevation - 2.4).abs() < 1e-9);
            assert_eq!(app.volumetric_ui.undo.len(), 1);
        }
    }
}
#[test]
fn wheel_and_pinch_are_local_to_the_paper_and_never_change_the_model_scale() {
    let (ctx, mut app) = gesture_app();
    let initial = app.volumetric_ui.settings.clone();
    let outside = egui::pos2(1250., 200.);
    pointer_frame(
        &ctx,
        &mut app,
        0.,
        vec![
            egui::Event::PointerMoved(outside),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., 80.),
                modifiers: Default::default(),
            },
        ],
    );
    assert_eq!(app.volumetric_ui.settings.zoom, initial.zoom);
    let inside = egui::pos2(750., 420.);
    pointer_frame(&ctx, &mut app, 0.1, vec![egui::Event::PointerMoved(inside)]);
    pointer_frame(
        &ctx,
        &mut app,
        0.2,
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0., 80.),
            modifiers: Default::default(),
        }],
    );
    let wheel_zoom = app.volumetric_ui.settings.zoom;
    assert!(wheel_zoom > initial.zoom);
    pointer_frame(&ctx, &mut app, 0.5, vec![egui::Event::Zoom(1.2)]);
    assert!(app.volumetric_ui.settings.zoom > wheel_zoom);
    assert_eq!(app.volumetric_ui.settings.model_scale, initial.model_scale);
}
#[test]
fn common_light_updates_all_shadow_passes_and_undo_restores_recipe_values() {
    let (ctx, mut app) = gesture_app();
    app.volumetric_ui.replace_passes(
        Settings {
            recipe: RecipeKind::FlatShadow,
            ..Default::default()
        }
        .passes(),
        "Sombra plana".into(),
        false,
    );
    let before = app.volumetric_ui.snapshot();
    let epoch = app.volumetric_ui.history_epoch;
    app.volumetric_ui.set_light(-83., 62.);
    let _ = ctx.run(Default::default(), |ctx| {
        app.volumetric_ui
            .record_scene_edit(before.clone(), epoch, ctx)
    });
    let recipe = app.volumetric_ui.recipe(&app);
    let restored = Recipe::from_json(&recipe.to_json().unwrap()).unwrap();
    for pass in &restored.passes {
        let shadow = match &pass.kind {
            PassKind::Hatch { settings, .. }
            | PassKind::Outline(OutlineParams::Shadow(settings)) => settings,
            _ => continue,
        };
        assert_eq!((shadow.azimuth, shadow.elevation), (-83., 62.));
    }
    app.volumetric_ui.undo_stack(false);
    assert_eq!(app.volumetric_ui.settings, before.settings);
    assert_eq!(app.volumetric_ui.passes, before.passes);
    assert!(app.doc.items.is_empty());
}

#[test]
fn a_floating_pass_sheet_consumes_drag_and_wheel_before_the_paper() {
    let _locale = crate::i18n::locale_lock();
    rust_i18n::set_locale("es");
    let (ctx, mut app) = gesture_app();
    app.volumetric_ui.passes_open = true;
    let run = |app: &mut VectorioApp, time, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(980., 620.),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| show(app, ctx),
        )
    };
    run(&mut app, 0., vec![]);
    let output = run(&mut app, 0.1, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == "Curvas principales" => {
                Some(t.pos + t.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("fila visible en la hoja de pasadas");
    let before = app.volumetric_ui.settings.clone();
    run(
        &mut app,
        0.2,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., 80.),
                modifiers: Default::default(),
            },
        ],
    );
    run(&mut app, 0.3, vec![pointer_button(point, true)]);
    run(
        &mut app,
        0.4,
        vec![egui::Event::PointerMoved(point + egui::vec2(20., 0.))],
    );
    run(
        &mut app,
        0.5,
        vec![pointer_button(point + egui::vec2(20., 0.), false)],
    );
    assert_eq!(app.volumetric_ui.settings.position_mm, before.position_mm);
    assert_eq!(app.volumetric_ui.settings.zoom, before.zoom);
}
#[test]
fn the_light_sphere_accepts_drag_and_keyboard_without_a_drawing() {
    let ctx = egui::Context::default();
    let mut state = VolumetricUi::default();
    let run = |state: &mut VolumetricUi, time, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(360., 240.),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none())
                    .show(ctx, |ui| {
                        ui.allocate_new_ui(
                            egui::UiBuilder::new().max_rect(Rect::from_min_size(
                                egui::pos2(20., 20.),
                                egui::vec2(134., 90.),
                            )),
                            |ui| interaction::light_gizmo(ui, state),
                        );
                    });
            },
        )
    };
    let first = run(&mut state, 0., vec![]);
    let start = first
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle) if circle.radius == 34. => Some(circle.center),
            _ => None,
        })
        .expect("esfera visible");
    let end = start + egui::vec2(28., -7.);
    run(&mut state, 0.05, vec![egui::Event::PointerMoved(start)]);
    run(&mut state, 0.1, vec![pointer_button(start, true)]);
    run(&mut state, 0.2, vec![egui::Event::PointerMoved(end)]);
    run(&mut state, 0.3, vec![pointer_button(end, false)]);
    assert!(
        (state.settings.shadow.azimuth - 7f64.atan2(28.).to_degrees()).abs() < 1e-6,
        "az={} el={} inicio={start:?} final={end:?}",
        state.settings.shadow.azimuth,
        state.settings.shadow.elevation
    );
    let elevation = state.settings.shadow.elevation;
    run(
        &mut state,
        0.4,
        vec![egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!((state.settings.shadow.elevation - elevation - 2.).abs() < 1e-9);
}

#[test]
fn command_z_after_focusing_the_paper_undoes_the_gesture_instead_of_disappearing() {
    let (ctx, mut app) = gesture_app();
    let initial = app.volumetric_ui.settings.clone();
    let point = egui::pos2(650., 400.);
    pointer_frame(&ctx, &mut app, 0., vec![egui::Event::PointerMoved(point)]);
    pointer_frame(&ctx, &mut app, 0.1, vec![pointer_button(point, true)]);
    pointer_frame(&ctx, &mut app, 0.2, vec![pointer_button(point, false)]);
    assert!(app.volumetric_ui.scene_control_focused(&ctx));
    pointer_frame(&ctx, &mut app, 0.3, vec![pointer_button(point, true)]);
    pointer_frame(
        &ctx,
        &mut app,
        0.4,
        vec![egui::Event::PointerMoved(point + egui::vec2(30., 10.))],
    );
    pointer_frame(
        &ctx,
        &mut app,
        0.5,
        vec![pointer_button(point + egui::vec2(30., 10.), false)],
    );
    assert_ne!(app.volumetric_ui.settings.position_mm, initial.position_mm);
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440., 900.),
            )),
            time: Some(0.6),
            events: vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
            ..Default::default()
        },
        |ctx| {
            app.shortcuts(ctx);
            show(&mut app, ctx);
        },
    );
    assert_eq!(app.volumetric_ui.settings, initial);
    assert!(app.doc.items.is_empty());
}

#[test]
fn the_light_drag_inside_a_scrollable_scene_changes_both_angles_and_keeps_arrow_focus() {
    let (ctx, mut app) = gesture_app();
    app.volumetric_ui.scene_open = Some(2);
    pointer_frame(&ctx, &mut app, 0., vec![]);
    let output = pointer_frame(&ctx, &mut app, 0.1, vec![]);
    let center = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle) if circle.radius == 34. => Some(circle.center),
            _ => None,
        })
        .expect("esfera en la hoja de escena");
    let start = center + egui::vec2(-17., -17.);
    let end = center + egui::vec2(25., 0.);
    pointer_frame(&ctx, &mut app, 0.2, vec![egui::Event::PointerMoved(start)]);
    pointer_frame(&ctx, &mut app, 0.3, vec![pointer_button(start, true)]);
    pointer_frame(&ctx, &mut app, 0.4, vec![egui::Event::PointerMoved(end)]);
    pointer_frame(&ctx, &mut app, 0.5, vec![pointer_button(end, false)]);
    assert!(
        app.volumetric_ui.settings.shadow.azimuth.abs() < 0.001,
        "el sol debe moverse dentro del panel: {}",
        app.volumetric_ui.settings.shadow.azimuth
    );
    let elevation = app.volumetric_ui.settings.shadow.elevation;
    for (time, pressed) in [(0.6, true), (0.7, false), (0.8, true)] {
        pointer_frame(
            &ctx,
            &mut app,
            time,
            vec![egui::Event::Key {
                key: egui::Key::ArrowDown,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
    }
    assert!((app.volumetric_ui.settings.shadow.elevation - elevation + 4.).abs() < 1e-6);
    assert!(app.volumetric_ui.scene_control_focused(&ctx));
}

#[test]
fn tonal_styles_keep_masks_and_output_actions_accessible_in_both_layouts_and_languages() {
    let _lock = crate::i18n::locale_lock();
    for lang in ["es", "en"] {
        rust_i18n::set_locale(lang);
        for style in TonalStyle::ALL {
            for (width, height) in [(980., 620.), (1440., 900.)] {
                let ctx = egui::Context::default();
                let mut app = VectorioApp::blank(&ctx);
                app.mode = ViewMode::Volumetric;
                let mut state = VolumetricUi::default();
                state.replace_passes(style.passes(&state.settings), style.name().into(), false);
                state.selected = Some(state.passes[0].id);
                state.passes_open = width < 1100.;
                prepare(&mut state, &app.doc.paper);
                app.volumetric_ui = state;
                let mut labels = Vec::new();
                for _ in 0..3 {
                    let output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, height),
                            )),
                            ..Default::default()
                        },
                        |ctx| show(&mut app, ctx),
                    );
                    labels = output
                        .shapes
                        .iter()
                        .filter_map(|s| {
                            if let egui::Shape::Text(t) = &s.shape {
                                Some((
                                    t.galley.text().to_owned(),
                                    Rect::from_min_size(t.pos, t.galley.size()),
                                ))
                            } else {
                                None
                            }
                        })
                        .collect();
                }
                for text in [
                    tr!("Máscara tonal"),
                    tr!("Colocar en el lienzo"),
                    tr!("Exportar SVG"),
                ] {
                    let (_, bounds) = labels
                        .iter()
                        .find(|(label, _)| *label == text)
                        .unwrap_or_else(|| panic!("{text} en {} {lang} {width}", style.name()));
                    assert!(
                        bounds.left() >= 0.
                            && bounds.right() <= width
                            && bounds.top() >= 0.
                            && bounds.bottom() <= height
                    );
                }
            }
        }
    }
    rust_i18n::set_locale("es");
}
#[test]
fn applying_a_tonal_style_and_editing_its_mask_roundtrips_with_scene_undo() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    let before = state.snapshot();
    let saved = styles::SavedStyle {
        name: TonalStyle::Crosshatch.name().into(),
        passes: TonalStyle::Crosshatch.passes(&state.settings),
        inks: app.doc.pens.clone(),
    };
    styles::apply(&mut state, &mut app, &saved);
    let style = state.snapshot();
    state.checkpoint();
    state.passes[0].pass.mask = TonalMask::Band { min: 0.1, max: 0.7 };
    state.set_light(-45., 35.);
    let encoded = state.recipe(&app).to_json().unwrap();
    let restored = Recipe::from_json(&encoded).unwrap();
    assert_eq!(
        restored.passes[0].mask,
        TonalMask::Band { min: 0.1, max: 0.7 }
    );
    assert_eq!(restored.settings.shadow.elevation, 35.);
    state.undo_stack(false);
    assert_eq!(state.passes, style.passes);
    assert_eq!(state.settings, style.settings);
    state.undo_stack(false);
    assert_eq!(state.passes, before.passes);
    assert_eq!(state.settings, before.settings);
}

#[test]
fn reading_ink_assignments_are_metadata_and_materialize_as_real_document_inks() {
    let ctx = egui::Context::default();
    let app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    state.replace_passes(
        ReadingStyle::Thermal.passes(&state.settings),
        "Mapa térmico".into(),
        false,
    );
    prepare(&mut state, &app.doc.paper);
    let request = state.geometry_request(&app.doc.paper);
    let initial = state.materialized(&app.doc.pens).unwrap();
    state.passes[0].pass.reading.as_mut().unwrap().pens = vec![7, 6, 5, 4];
    assert!(state.geometry_request(&app.doc.paper) == request);
    let changed = state.materialized(&app.doc.pens).unwrap();
    assert_eq!(initial.parts[0].lines, changed.parts[0].lines);
    assert!(changed.parts[0]
        .families
        .iter()
        .all(|f| [7, 6, 5, 4].contains(&f.pen)));
    let doc = changed.document(app.doc.paper.clone(), app.doc.pens.clone(), []);
    assert!(doc
        .items
        .iter()
        .all(|i| [7, 6, 5, 4].contains(&i.stroke.unwrap())));
}
#[test]
fn reading_styles_and_ink_references_survive_recipe_style_save_and_undo() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    let original = state.snapshot();
    let mut inks = app.doc.pens.clone();
    inks.push(Pen::new("Cian de prueba", [0, 170, 190], 0.25));
    let mut passes = ReadingStyle::Anaglyph.passes(&state.settings);
    passes[0].reading.as_mut().unwrap().pens = vec![1, inks.len() - 1];
    let saved = styles::SavedStyle {
        name: "Par de cámaras".into(),
        passes,
        inks,
    };
    let decoded: styles::SavedStyle =
        serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    styles::apply(&mut state, &mut app, &decoded);
    let recipe = Recipe::from_json(&state.recipe(&app).to_json().unwrap()).unwrap();
    let map = recipe.passes[0].reading.as_ref().unwrap();
    assert_eq!(recipe.inks[map.pens[1]].color, [0, 170, 190]);
    state.undo_stack(false);
    assert_eq!(state.passes, original.passes);
    state.undo_stack(true);
    assert_eq!(state.passes[0].pass.reading, recipe.passes[0].reading);
}
#[test]
fn reading_controls_and_output_actions_fit_both_languages_and_window_sizes() {
    let _lock = crate::i18n::locale_lock();
    for lang in ["es", "en"] {
        rust_i18n::set_locale(lang);
        for (width, height) in [(980., 620.), (1440., 900.)] {
            for style in [
                ReadingStyle::Thermal,
                ReadingStyle::Ambient,
                ReadingStyle::Anaglyph,
                ReadingStyle::Section,
                ReadingStyle::Suggestive,
                ReadingStyle::ApparentRidges,
                ReadingStyle::PrincipalFlow,
            ] {
                let ctx = egui::Context::default();
                let mut app = VectorioApp::blank(&ctx);
                app.mode = ViewMode::Volumetric;
                let mut state = VolumetricUi::default();
                state.replace_passes(style.passes(&state.settings), style.name().into(), false);
                state.selected = Some(state.passes[0].id);
                state.passes_open = width < 1100.;
                prepare(&mut state, &app.doc.paper);
                app.volumetric_ui = state;
                let mut output = None;
                for _ in 0..3 {
                    output = Some(ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, height),
                            )),
                            ..Default::default()
                        },
                        |ctx| show(&mut app, ctx),
                    ));
                }
                let output = output.unwrap();
                for text in [tr!("Colocar en el lienzo"), tr!("Exportar SVG")] {
                    let bounds = output
                        .shapes
                        .iter()
                        .find_map(|s| {
                            if let egui::Shape::Text(t) = &s.shape {
                                (t.galley.text() == text)
                                    .then_some(Rect::from_min_size(t.pos, t.galley.size()))
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    assert!(
                        bounds.left() >= 0. && bounds.right() <= width && bounds.bottom() <= height
                    );
                }
            }
        }
    }
    rust_i18n::set_locale("es");
}

#[test]
fn zoom_with_a_legend_still_keeps_the_physical_anchor_fixed() {
    let mut state = VolumetricUi::default();
    state.settings.paper_rotation = 27.;
    let mut passes = ReadingStyle::Thermal.passes(&state.settings);
    passes[0].reading.as_mut().unwrap().legend = true;
    state.replace_passes(passes, "Mapa térmico".into(), false);
    let paper = Paper::default();
    let mut engine = Engine::default();
    let passes = state
        .passes
        .iter()
        .map(|p| p.pass.clone())
        .collect::<Vec<_>>();
    let before = engine
        .draw_passes(
            state.mesh.clone(),
            &state.settings,
            &passes,
            &paper,
            &|| false,
        )
        .unwrap();
    let world = before.mesh.points()[0];
    let rotation = state.settings.paper_rotation.to_radians();
    let project = |d: &Drawing| {
        let point = d.camera.project(world).0;
        d.camera.paper_center + (point - d.camera.paper_center).rotate(rotation)
    };
    let anchor = project(&before);
    let effective = state.composition_paper(&paper);
    interaction::zoom_on_paper(&mut state.settings, 1.3, anchor, &effective);
    let after = engine
        .draw_passes(
            state.mesh.clone(),
            &state.settings,
            &passes,
            &paper,
            &|| false,
        )
        .unwrap();
    assert!(project(&after).dist(anchor) < 1e-8);
}

#[test]
fn escape_closes_the_reading_ink_menu_before_leaving_volume() {
    let _lock = crate::i18n::locale_lock();
    rust_i18n::set_locale("es");
    let (ctx, mut app) = gesture_app();
    app.volumetric_ui.replace_passes(
        ReadingStyle::Thermal.passes(&Settings::default()),
        "Mapa térmico".into(),
        false,
    );
    app.volumetric_ui.selected = Some(app.volumetric_ui.passes[0].id);
    prepare(&mut app.volumetric_ui, &app.doc.paper);
    pointer_frame(&ctx, &mut app, 0., vec![]);
    let output = pointer_frame(&ctx, &mut app, 0.1, vec![]);
    let position = output
        .shapes
        .iter()
        .find_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                (t.galley.text() == "Tintas · 4").then_some(t.pos + t.galley.size() * 0.5)
            } else {
                None
            }
        })
        .unwrap();
    pointer_frame(
        &ctx,
        &mut app,
        0.2,
        vec![egui::Event::PointerMoved(position)],
    );
    pointer_frame(&ctx, &mut app, 0.3, vec![pointer_button(position, true)]);
    pointer_frame(&ctx, &mut app, 0.4, vec![pointer_button(position, false)]);
    pointer_frame(&ctx, &mut app, 0.45, vec![]);
    assert!(app.volumetric_ui.menu_open(&ctx));
    let key = |pressed| egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    };
    for (time, event) in [(0.5, key(true)), (0.6, key(false))] {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440., 900.),
                )),
                time: Some(time),
                events: vec![event],
                ..Default::default()
            },
            |ctx| {
                app.shortcuts(ctx);
                if app.mode == ViewMode::Volumetric {
                    show(&mut app, ctx);
                }
            },
        );
        assert_eq!(app.mode, ViewMode::Volumetric);
    }
    let _ = ctx.run(
        egui::RawInput {
            events: vec![key(true)],
            ..Default::default()
        },
        |ctx| app.shortcuts(ctx),
    );
    assert_eq!(app.mode, ViewMode::Design);
}

#[test]
fn finishing_style_undo_and_recipe_preserve_every_effect_parameter() {
    let ctx = egui::Context::default();
    let mut app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    let original = state.snapshot();
    let saved = styles::SavedStyle {
        name: ArtisticStyle::Hand.name().into(),
        passes: ArtisticStyle::Hand.passes(&state.settings),
        inks: app.doc.pens.clone(),
    };
    styles::apply(&mut state, &mut app, &saved);
    state.checkpoint();
    state.passes[0].pass.finish.seed = 188;
    state.passes[0].pass.finish.jitter_mm = 0.21;
    let recipe = Recipe::from_json(&state.recipe(&app).to_json().unwrap()).unwrap();
    assert_eq!(recipe.passes[0].finish, state.passes[0].pass.finish);
    state.undo_stack(false);
    assert_eq!(state.passes[0].pass.finish, saved.passes[0].finish);
    state.undo_stack(false);
    assert_eq!(state.passes, original.passes);
    state.undo_stack(true);
    assert_eq!(state.passes[0].pass.finish, saved.passes[0].finish);
}
#[test]
fn artistic_gradient_ink_assignment_is_metadata_and_keeps_the_dither_geometry() {
    let ctx = egui::Context::default();
    let app = VectorioApp::blank(&ctx);
    let mut state = VolumetricUi::default();
    state.replace_passes(
        ArtisticStyle::Gradient.passes(&state.settings),
        ArtisticStyle::Gradient.name().into(),
        false,
    );
    prepare(&mut state, &app.doc.paper);
    let initial = state.geometry_request(&app.doc.paper);
    let lines = state.materialized(&app.doc.pens).unwrap().parts[0]
        .lines
        .clone();
    state.passes[0].pass.reading.as_mut().unwrap().pens = vec![0, 1, 2];
    assert!(initial == state.geometry_request(&app.doc.paper));
    assert_eq!(
        lines,
        state.materialized(&app.doc.pens).unwrap().parts[0].lines
    );
}
#[test]
fn artistic_finishing_controls_and_actions_fit_both_layouts_and_languages() {
    let _lock = crate::i18n::locale_lock();
    for lang in ["es", "en"] {
        rust_i18n::set_locale(lang);
        for (width, height) in [(980., 620.), (1440., 900.)] {
            for style in ArtisticStyle::ALL {
                let ctx = egui::Context::default();
                let mut app = VectorioApp::blank(&ctx);
                app.mode = ViewMode::Volumetric;
                let mut state = VolumetricUi::default();
                state.replace_passes(style.passes(&state.settings), style.name().into(), false);
                state.selected = Some(state.passes[0].id);
                state.passes_open = width < 1100.;
                prepare(&mut state, &app.doc.paper);
                app.volumetric_ui = state;
                let mut output = None;
                for _ in 0..3 {
                    output = Some(ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, height),
                            )),
                            ..Default::default()
                        },
                        |ctx| show(&mut app, ctx),
                    ));
                }
                let output = output.unwrap();
                for text in [tr!("Colocar en el lienzo"), tr!("Exportar SVG")] {
                    let bounds = output
                        .shapes
                        .iter()
                        .find_map(|s| {
                            if let egui::Shape::Text(t) = &s.shape {
                                (t.galley.text() == text)
                                    .then_some(Rect::from_min_size(t.pos, t.galley.size()))
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    assert!(
                        bounds.left() >= 0. && bounds.right() <= width && bounds.bottom() <= height
                    );
                }
            }
        }
    }
    rust_i18n::set_locale("es");
}
