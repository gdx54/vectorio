use super::*;
use std::sync::Arc;
fn length(lines: &[crate::Polyline]) -> f64 {
    lines.iter().map(|l| crate::geom::length(l)).sum()
}
#[test]
fn phase_d_styles_roundtrip_and_export_finite_clipped_vector_paths() {
    let source = Arc::new(Primitive::Torus.mesh());
    let scene = Settings {
        paper_rotation: 27.,
        position_mm: [12., -7.],
        ..Default::default()
    };
    let paper = crate::Paper::default();
    for style in TonalStyle::ALL {
        let mut recipe = Recipe::new(source.as_ref().clone(), scene.clone());
        recipe.inks = crate::doc::default_pens();
        recipe.passes = style.passes(&scene);
        let decoded = Recipe::from_json(&recipe.to_json().unwrap()).unwrap();
        assert_eq!(decoded, recipe);
        let mut engine = Engine::default();
        let drawing = engine.draw_recipe(&decoded, &|| false).unwrap();
        assert!(
            drawing.parts.iter().all(|p| !p.lines.is_empty()),
            "{}",
            style.name()
        );
        assert!(drawing
            .parts
            .iter()
            .flat_map(|part| &part.lines)
            .flatten()
            .all(|p| p.x.is_finite()
                && p.y.is_finite()
                && p.x >= paper.margin_mm - 1e-7
                && p.y >= paper.margin_mm - 1e-7
                && p.x <= paper.width_mm - paper.margin_mm + 1e-7
                && p.y <= paper.height_mm - paper.margin_mm + 1e-7));
        let doc = drawing.document(paper.clone(), recipe.inks.clone(), []);
        let svg = crate::svg_out::export(&doc, &doc.layers());
        assert!(svg.contains("<path"));
        assert!(!svg.contains("<image"));
        let repeat = engine.draw_recipe(&decoded, &|| false).unwrap();
        assert_eq!(
            svg,
            crate::svg_out::export(
                &repeat.document(paper.clone(), recipe.inks, []),
                &doc.layers()
            )
        );
    }
}
#[test]
fn light_and_shadow_partition_waves_mesh_and_field_without_recalculating_their_geometry() {
    let source = Arc::new(Primitive::Torus.mesh());
    let scene = Settings::default();
    let paper = crate::Paper::default();
    for recipe in [
        RecipeKind::Atlas,
        RecipeKind::SurfaceWaves,
        RecipeKind::TriMesh,
    ] {
        let settings = Settings {
            recipe,
            ..scene.clone()
        };
        let mut pass = settings.passes().remove(0);
        pass.pen = 0;
        let mut engine = Engine::default();
        let full = engine
            .draw_passes(source.clone(), &scene, &[pass.clone()], &paper, &|| false)
            .unwrap();
        let renders = engine.renders;
        pass.mask = TonalMask::Light;
        let lit = engine
            .draw_passes(source.clone(), &scene, &[pass.clone()], &paper, &|| false)
            .unwrap();
        pass.mask = TonalMask::Shadow;
        let dark = engine
            .draw_passes(source.clone(), &scene, &[pass.clone()], &paper, &|| false)
            .unwrap();
        assert!(!lit.parts[0].lines.is_empty() && !dark.parts[0].lines.is_empty());
        let sum = length(&lit.parts[0].lines) + length(&dark.parts[0].lines);
        // Simplificación previa de curvas puede alejar micras de una silueta.
        assert!(
            (sum - length(&full.parts[0].lines)).abs() < 0.5,
            "{recipe:?}: {sum} / {}",
            length(&full.parts[0].lines)
        );
        assert_eq!(engine.renders, renders);
        assert_eq!(engine.tones_builds(), 1);
    }
}
#[test]
fn tonal_effect_cache_ignores_names_inks_order_and_enabled_but_invalidates_masks_and_light() {
    let source = Arc::new(Primitive::Cube.mesh());
    let mut scene = Settings::default();
    let paper = crate::Paper::default();
    let mut passes = TonalStyle::Crosshatch.passes(&scene);
    let mut engine = Engine::default();
    let initial = engine
        .draw_passes(source.clone(), &scene, &passes, &paper, &|| false)
        .unwrap();
    let finishes = engine.finishes;
    passes[0].name = "Otra tinta".into();
    passes[0].pen = 6;
    passes.reverse();
    let reversed = engine
        .draw_passes(source.clone(), &scene, &passes, &paper, &|| false)
        .unwrap();
    assert_eq!(initial.parts[0].lines, reversed.parts[1].lines);
    assert_eq!(engine.finishes, finishes);
    passes[1].enabled = false;
    engine
        .draw_passes(source.clone(), &scene, &passes, &paper, &|| false)
        .unwrap();
    passes[1].enabled = true;
    engine
        .draw_passes(source.clone(), &scene, &passes, &paper, &|| false)
        .unwrap();
    assert_eq!(engine.finishes, finishes);
    passes[1].mask = TonalMask::Band { min: 0.2, max: 0.8 };
    engine
        .draw_passes(source.clone(), &scene, &passes, &paper, &|| false)
        .unwrap();
    assert_eq!(engine.finishes, finishes + 1);
    scene.shadow.azimuth += 90.;
    let moved = engine
        .draw_passes(source, &scene, &passes, &paper, &|| false)
        .unwrap();
    assert_ne!(moved.parts[1].lines, reversed.parts[1].lines);
}
#[test]
fn hidden_edges_are_occluded_intervals_in_a_separate_ink_with_real_mm_gaps() {
    let source = Arc::new(Primitive::Cube.mesh());
    let scene = Settings::default();
    let paper = crate::Paper::default();
    let mut passes = TonalStyle::Technical.passes(&scene);
    passes[1].pen = 7;
    let mut engine = Engine::default();
    let drawing = engine
        .draw_passes(source, &scene, &passes, &paper, &|| false)
        .unwrap();
    assert!(length(&drawing.parts[0].lines) > 0. && length(&drawing.parts[1].lines) > 0.);
    assert!(drawing.parts[1]
        .lines
        .iter()
        .all(|l| crate::geom::length(l) <= 2. + 1e-7));
    assert!(drawing.parts[1]
        .lines
        .iter()
        .any(|l| (crate::geom::length(l) - 2.).abs() < 1e-7));
    assert_eq!(drawing.items([])[1].stroke, Some(7));
}
#[test]
fn form_hatch_retains_curved_object_sections_and_changes_with_orientation() {
    let source = Arc::new(Primitive::Sphere.mesh());
    let scene = Settings::default();
    let paper = crate::Paper::default();
    let mut pass = TonalStyle::Engraving.passes(&scene).remove(0);
    let mut engine = Engine::default();
    let a = engine
        .draw_passes(source.clone(), &scene, &[pass.clone()], &paper, &|| false)
        .unwrap();
    assert!(a.parts[0].lines.iter().any(|l| l.len() > 4));
    let PassKind::ToneHatch(params) = &mut pass.kind else {
        panic!()
    };
    params.direction = [1., 0., 0.];
    let b = engine
        .draw_passes(source, &scene, &[pass], &paper, &|| false)
        .unwrap();
    assert_ne!(a.parts[0].lines, b.parts[0].lines);
}
#[test]
fn previous_v2_without_tonal_fields_loads_and_new_parameters_validate() {
    let recipe = Recipe::new(Primitive::Cube.mesh(), Settings::default());
    let mut json = serde_json::to_value(&recipe).unwrap();
    for pass in json["passes"].as_array_mut().unwrap() {
        pass.as_object_mut().unwrap().remove("mask");
        pass.as_object_mut().unwrap().remove("dash");
    }
    json["settings"]
        .as_object_mut()
        .unwrap()
        .remove("hidden_lines");
    let restored = Recipe::from_json(&json.to_string()).unwrap();
    assert_eq!(restored, recipe);
    let mut pass = TonalStyle::Crosshatch.passes(&recipe.settings).remove(0);
    pass.mask = TonalMask::Band { min: 0.8, max: 0.2 };
    assert!(pass.validate().is_err());
    pass.mask = TonalMask::All;
    pass.dash.gap_mm = 0.;
    assert!(pass.validate().is_err());
    pass.dash = DashPattern::default();
    let PassKind::ToneHatch(params) = &mut pass.kind else {
        panic!()
    };
    params.levels = 5;
    assert!(pass.validate().is_err());
    assert!(Engine::default()
        .draw_passes(
            Arc::new(recipe.mesh),
            &recipe.settings,
            &[],
            &recipe.paper,
            &|| true
        )
        .is_err());
}

#[test]
fn hidden_intervals_are_the_exact_complement_of_visible_intervals() {
    let camera = Camera {
        scale: 1.,
        paper_center: crate::p(10., 10.),
        world_center: V3::default(),
        ..Default::default()
    };
    let (right, down, away) = camera.axes();
    let world = |x: f64, y: f64, z: f64| right * (x - 10.) + down * (y - 10.) + away * z;
    let area = crate::BBox {
        min: crate::p(0., 0.),
        max: crate::p(20., 20.),
    };
    let render = |hidden, depth| {
        let mut scene = crate::scene::Scene::new(camera, area);
        scene.set_intersections(false);
        scene.set_hidden_lines(hidden);
        scene.occluder(&[world(4., 4., 0.), world(16., 4., 0.), world(10., 16., 0.)]);
        scene.line(&[world(0., 8., depth), world(20., 8., depth)]);
        scene.render().parts.remove(0).lines
    };
    let visible = render(false, 2.);
    let hidden = render(true, 2.);
    assert!((length(&visible) - 12.).abs() < 1e-7);
    assert!((length(&hidden) - 8.).abs() < 1e-7);
    assert_eq!(hidden.len(), 1);
    assert!((hidden[0][0].x - 6.).abs() < 1e-7);
    assert!((hidden[0].last().unwrap().x - 14.).abs() < 1e-7);
    assert!(render(true, -2.).is_empty());
}

#[test]
fn form_section_planes_follow_model_rotation_and_nonuniform_scale() {
    let scene = Settings {
        model_rotation: [90., 0., 0.],
        model_scale: [2., 1., 0.5],
        ..Default::default()
    };
    let pass = TonalStyle::Engraving.passes(&scene).remove(0);
    let (program, _) = super::passes::program(&scene, &pass);
    let normal = v3(program.direction);
    assert!((normal - V3::new(0., -1., 0.)).length() < 1e-12);
    // Un punto que pertenecía al plano del objeto sigue dentro tras transformarlo.
    let object = V3::new(0.4, 0.2, 0.);
    let world = V3::new(object.x * 2., -object.z * 0.5, object.y);
    assert!(normal.dot(world).abs() < 1e-12);
}
