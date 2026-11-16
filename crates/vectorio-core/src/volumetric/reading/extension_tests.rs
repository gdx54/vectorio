use super::*;
fn recipe(style: ReadingStyle, primitive: Primitive) -> Recipe {
    let mut r = Recipe::new(primitive.mesh(), Settings::default());
    r.passes = style.passes(&r.settings);
    r.inks = crate::doc::default_pens();
    r
}
#[test]
fn optical_color_solver_matches_image_separation_without_a_raster() {
    let pens = [[0, 160, 200], [210, 45, 140], [247, 200, 30], [28, 28, 32]];
    let mut image = image::RgbImage::new(3, 1);
    for (i, color) in [[255, 255, 255], [0, 0, 0], [255, 0, 0]]
        .into_iter()
        .enumerate()
    {
        image.put_pixel(i as u32, 0, image::Rgb(color));
    }
    let old = crate::mix::coverage(&image, &pens);
    for (i, color) in [[255, 255, 255], [0, 0, 0], [255, 0, 0]]
        .into_iter()
        .enumerate()
    {
        let c = crate::mix::color_coverage(color, &pens);
        for j in 0..pens.len() {
            assert!((c[j] - old[j].data[i] as f64).abs() < 1e-6);
            assert!((0.0..=1.).contains(&c[j]));
        }
    }
    assert!(crate::mix::color_coverage([255, 255, 255], &pens)
        .iter()
        .all(|v| *v == 0.));
}
#[test]
fn preset_ink_sets_reuse_matching_pens_and_preserve_existing_items_and_colors() {
    let mut doc = crate::Document::default();
    let colors = doc.pens.clone();
    let id = doc.add(crate::Item::new(
        "Vecino",
        crate::Geometry::Path {
            lines: vec![],
            closed: false,
        },
        p(0., 0.),
    ));
    let neighbor = doc.get(id).unwrap().clone();
    for set in InkSet::ALL {
        let pens = set.apply(&mut doc);
        let count = doc.pens.len();
        assert_eq!(pens, set.apply(&mut doc));
        assert_eq!(count, doc.pens.len());
        let map = ReadingMap {
            pens,
            ..Default::default()
        };
        assert!(set.matches(&map, &doc.pens));
    }
    assert_eq!(&doc.pens[..colors.len()], colors);
    assert_eq!(doc.get(id).unwrap(), &neighbor);
}
#[test]
fn scalar_isolines_interpolate_height_in_world_space_under_perspective() {
    let mesh = Mesh {
        name: "Plano inclinado".into(),
        vertices: vec![[-1., -1., -1.], [-1., 1., -1.], [1., 1., 1.], [1., -1., 1.]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        objects: vec![],
        face_objects: vec![],
    };
    for perspective in [false, true] {
        let mut r = Recipe::new(mesh.clone(), Settings::default());
        r.settings.projection.perspective = perspective;
        r.settings.projection.distance = 0.8;
        r.settings.paper_rotation = 17.;
        r.passes = vec![Pass::new(
            "Isolíneas",
            PassKind::Reading(ReadingParams {
                facets: 64,
                effect: ReadingEffect::Surface,
            }),
            0,
        )];
        r.passes[0].reading = Some(ReadingMap {
            measure: Measure::Height,
            encoding: Encoding::Isolines,
            ramp: Ramp::Sequential,
            pens: vec![0, 1],
            levels: 2,
            range: Some([-0.5, 0.5]),
            ..Default::default()
        });
        let d = Engine::default().draw_recipe(&r, &|| false).unwrap();
        assert_eq!(d.parts[0].lines.len(), 2);
        let project = |q| {
            let p = d.camera.project(q).0;
            d.camera.paper_center + (p - d.camera.paper_center).rotate(17f64.to_radians())
        };
        for (slot, level) in [-1. / 6., 1. / 6.].into_iter().enumerate() {
            let a = project(V3::new(level, -1., level));
            let b = project(V3::new(level, 1., level));
            let direction = b - a;
            for q in d.parts[0].families[slot].lines.iter().flatten() {
                let cross = (q.x - a.x) * direction.y - (q.y - a.y) * direction.x;
                assert!(cross.abs() / direction.len() < 1e-5, "{q:?}, {level}");
            }
        }
    }
}
#[test]
fn new_encodings_are_vectorial_clipped_deterministic_and_old_recipes_load_defaults() {
    for style in [
        ReadingStyle::ScalarContours,
        ReadingStyle::LightContours,
        ReadingStyle::OpticalThermal,
    ] {
        let mut r = recipe(style, Primitive::Torus);
        r.passes[0].reading.as_mut().unwrap().legend = true;
        r.passes[0].reading.as_mut().unwrap().label_values = style != ReadingStyle::OpticalThermal;
        let mut engine = Engine::default();
        let d = engine.draw_recipe(&r, &|| false).unwrap();
        let again = engine.draw_recipe(&r, &|| false).unwrap();
        assert_eq!(d.parts[0].lines, again.parts[0].lines);
        assert!(!d.parts[0].lines.is_empty(), "{style:?}");
        for q in d.parts[0].lines.iter().flatten() {
            assert!(q.x.is_finite() && q.y.is_finite());
            assert!(
                q.x >= r.paper.margin_mm - 1e-6
                    && q.x <= r.paper.width_mm - r.paper.margin_mm + 1e-6
                    && q.y >= r.paper.margin_mm - 1e-6
                    && q.y <= r.paper.height_mm - r.paper.margin_mm + 1e-6
            );
        }
        let document = d.document(r.paper.clone(), r.inks.clone(), []);
        let svg = crate::svg_out::export(&document, &document.layers());
        assert!(!svg.contains("<image") && !svg.contains("<text"));
        assert_eq!(r, Recipe::from_json(&r.to_json().unwrap()).unwrap());
        assert!(engine.draw_recipe(&r, &|| true).is_err());
    }
    let r = recipe(ReadingStyle::Thermal, Primitive::Cube);
    let mut json = serde_json::to_value(&r).unwrap();
    let map = json["passes"][0]["reading"].as_object_mut().unwrap();
    for field in ["optical", "levels", "label_values"] {
        map.remove(field);
    }
    let restored = Recipe::from_json(&json.to_string()).unwrap();
    let map = restored.passes[0].reading.as_ref().unwrap();
    assert_eq!(map.levels, 16);
    assert!(!map.label_values);
    assert!(map.optical.inks.is_empty());
}
#[test]
fn changing_physical_inks_reseparates_optical_strokes_without_rebuilding_mesh_geometry() {
    let mut r = recipe(ReadingStyle::OpticalThermal, Primitive::Cube);
    r.passes[0].reading.as_mut().unwrap().pens = vec![0, 1, 2];
    let mut engine = Engine::default();
    let a = engine.draw_recipe(&r, &|| false).unwrap();
    let renders = engine.renders;
    r.inks[1].color = [0, 180, 110];
    let b = engine.draw_recipe(&r, &|| false).unwrap();
    assert_eq!(engine.renders, renders);
    assert_ne!(a.parts[0].lines, b.parts[0].lines);
    let second = b.parts[0].lines.clone();
    r.passes[0].name = "Otro nombre".into();
    let c = engine.draw_recipe(&r, &|| false).unwrap();
    assert_eq!(c.parts[0].lines, second);
    assert_eq!(engine.renders, renders);
}
#[test]
fn isolines_support_scalar_measures_and_reject_incompatible_effects_and_objects() {
    let mut r = recipe(ReadingStyle::ScalarContours, Primitive::Assembly);
    for measure in Measure::ALL.into_iter().filter(|m| *m != Measure::Objects) {
        let map = r.passes[0].reading.as_mut().unwrap();
        map.measure = measure;
        map.ramp = Ramp::Sequential;
        map.samples = 8;
        map.levels = 6;
        let d = Engine::default().draw_recipe(&r, &|| false).unwrap();
        assert!(d.parts[0]
            .lines
            .iter()
            .flatten()
            .all(|q| q.x.is_finite() && q.y.is_finite()));
    }
    let mut pass = r.passes.remove(0);
    pass.reading.as_mut().unwrap().measure = Measure::Objects;
    assert!(pass.validate().is_err());
    pass.reading.as_mut().unwrap().measure = Measure::Height;
    pass.kind = PassKind::Edges { angle: 25. };
    assert!(pass.validate().is_err());
}
#[test]
fn section_dimensions_can_be_hidden_and_old_recipes_keep_them_enabled() {
    let mut r = recipe(ReadingStyle::Section, Primitive::Cube);
    let mut engine = Engine::default();
    let with = engine.draw_recipe(&r, &|| false).unwrap();
    if let PassKind::Reading(p) = &mut r.passes[0].kind {
        if let ReadingEffect::Section {
            show_dimensions, ..
        } = &mut p.effect
        {
            *show_dimensions = false;
        }
    }
    let without = engine.draw_recipe(&r, &|| false).unwrap();
    assert!(without.parts[0].lines.len() < with.parts[0].lines.len());
    assert_eq!(
        without.parts[0].families[1].lines,
        with.parts[0].families[1].lines
    );
    assert_eq!(
        without.parts[0].families[2].lines,
        with.parts[0].families[2].lines
    );
    assert_eq!(r, Recipe::from_json(&r.to_json().unwrap()).unwrap());
    let mut json = serde_json::to_value(&r).unwrap();
    json["passes"][0]["kind"]["Reading"]["effect"]["Section"]
        .as_object_mut()
        .unwrap()
        .remove("show_dimensions");
    let old = Recipe::from_json(&json.to_string()).unwrap();
    assert!(matches!(
        &old.passes[0].kind,
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Section {
                show_dimensions: true,
                ..
            },
            ..
        })
    ));
}
#[test]
fn a_diverging_optical_map_reserves_unprinted_paper_at_zero() {
    let mut map = ReadingMap {
        encoding: Encoding::Optical,
        ramp: Ramp::Diverging,
        pens: vec![0, 1, 2],
        ..Default::default()
    };
    map.bind_palette(&crate::doc::default_pens());
    assert_eq!(map.target_color(0.5), [255, 255, 255]);
    let table = optical::Table::new(&map, &|| false).unwrap();
    assert!((0..3).all(|i| table.coverage(i, 0.5) == 0.));
}
