use super::*;
fn draw(mesh: Mesh) -> Drawing {
    Engine::default()
        .draw(
            Arc::new(mesh),
            &Settings {
                recipe: RecipeKind::XRay,
                ..Default::default()
            },
            &crate::Paper::default(),
            &|| false,
        )
        .unwrap()
}
#[test]
fn sphere_curvatures_have_the_correct_sign_scale_and_gauss_bonnet_integral() {
    let mesh = Arc::new(Primitive::Sphere.mesh());
    let g = fields::Cache::default()
        .geometry(mesh.clone(), &|| false)
        .unwrap();
    assert!(g.mean.iter().all(|v| v.is_finite()));
    let equator = mesh
        .vertices
        .iter()
        .enumerate()
        .filter(|(_, p)| p[2].abs() < 0.7)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let mean = equator.iter().map(|i| g.mean[*i]).sum::<f64>() / equator.len() as f64;
    assert!((mean - 1.).abs() < 0.06, "curvatura esfera {mean}");
    assert!(equator.iter().all(|i| g.gaussian[*i] > 0.8));
    let mut scaled = mesh.as_ref().clone();
    for p in &mut scaled.vertices {
        for v in p {
            *v *= 2.;
        }
    }
    let h = fields::Cache::default()
        .geometry(Arc::new(scaled), &|| false)
        .unwrap();
    for i in equator {
        assert!((h.mean[i] * 2. - g.mean[i]).abs() < 1e-8);
        assert!((h.gaussian[i] * 4. - g.gaussian[i]).abs() < 1e-8);
    }
}
#[test]
fn torus_gaussian_curvature_separates_saddles_from_convex_regions_and_open_planes_stay_flat() {
    let mesh = Arc::new(Primitive::Torus.mesh());
    let g = fields::Cache::default().geometry(mesh, &|| false).unwrap();
    assert!(g.gaussian.iter().any(|v| *v < -0.2));
    assert!(g.gaussian.iter().any(|v| *v > 0.2));
    let mesh = Mesh {
        name: "Plano".into(),
        vertices: vec![
            [-1., -1., 0.],
            [1., -1., 0.],
            [1., 1., 0.],
            [-1., 1., 0.],
            [0., 0., 0.],
        ],
        triangles: vec![[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]],
        objects: vec![],
        face_objects: vec![],
    };
    let g = fields::Cache::default()
        .geometry(Arc::new(mesh), &|| false)
        .unwrap();
    assert!(g.mean.iter().all(|v| v.abs() < 1e-9));
    assert!(g.gaussian.iter().all(|v| v.abs() < 1e-9));
}
#[test]
fn ambient_occlusion_is_reproducible_and_detects_a_covered_plate() {
    let mesh = Arc::new(Mesh {
        name: "Dos placas".into(),
        vertices: vec![
            [-1., -1., 0.],
            [1., -1., 0.],
            [1., 1., 0.],
            [-1., 1., 0.],
            [0., 0., 0.],
            [-2., -2., 0.5],
            [2., -2., 0.5],
            [2., 2., 0.5],
            [-2., 2., 0.5],
        ],
        triangles: vec![
            [0, 1, 4],
            [1, 2, 4],
            [2, 3, 4],
            [3, 0, 4],
            [5, 6, 7],
            [5, 7, 8],
        ],
        objects: vec![],
        face_objects: vec![],
    });
    let mut cache = fields::Cache::default();
    let values = cache
        .values(
            mesh.clone(),
            Measure::AmbientOcclusion,
            64,
            &Settings::default(),
            Camera::default(),
            &|| false,
        )
        .unwrap();
    assert!(values[4] > 0.7, "AO bajo cubierta {}", values[4]);
    assert_eq!(values[5], 0.);
    let same = cache
        .values(
            mesh,
            Measure::AmbientOcclusion,
            64,
            &Settings::default(),
            Camera::default(),
            &|| false,
        )
        .unwrap();
    assert!(Arc::ptr_eq(&values, &same));
}
#[test]
fn scalar_cache_ignores_camera_for_intrinsic_fields_and_invalidates_depth_and_sample_count() {
    let mesh = Arc::new(Primitive::Cube.mesh());
    let mut cache = fields::Cache::default();
    let s = Settings::default();
    let mut camera = Camera::default();
    let a = cache
        .values(
            mesh.clone(),
            Measure::MeanCurvature,
            24,
            &s,
            camera,
            &|| false,
        )
        .unwrap();
    camera.yaw_deg += 90.;
    let b = cache
        .values(
            mesh.clone(),
            Measure::MeanCurvature,
            64,
            &s,
            camera,
            &|| false,
        )
        .unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    let a = cache
        .values(mesh.clone(), Measure::Depth, 24, &s, camera, &|| false)
        .unwrap();
    camera.yaw_deg += 90.;
    let b = cache
        .values(mesh.clone(), Measure::Depth, 24, &s, camera, &|| false)
        .unwrap();
    assert_ne!(a, b);
    let a = cache
        .values(
            mesh.clone(),
            Measure::AmbientOcclusion,
            8,
            &s,
            camera,
            &|| false,
        )
        .unwrap();
    let b = cache
        .values(mesh, Measure::AmbientOcclusion, 16, &s, camera, &|| false)
        .unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
}
fn values(map: ReadingMap) -> fields::Values {
    fields::Values {
        values: Arc::new(vec![0., 1.]),
        range: [0., 1.],
        sorted: vec![],
        config: map,
        light_vertex: None,
        light_weight: 0.,
        dead: 0.,
    }
}
#[test]
fn equalized_diverging_keeps_the_neutral_band_for_real_zeros_only() {
    let config = ReadingMap {
        equalize: true,
        ..Default::default()
    };
    let build = |raw: Vec<f64>| {
        let mut v = values(config.clone());
        v.sorted = raw.clone();
        v.sorted.sort_by(f64::total_cmp);
        let extent = raw.iter().fold(0f64, |m, x| m.max(x.abs()));
        v.range = [-extent, extent];
        v.dead = fields::neutral_width(&v.sorted, extent);
        v
    };
    // Convexo: todo positivo y lejos de cero; ningún valor se queda sin tinta.
    let convex = build((0..100).map(|i| 1. + i as f64 * 0.01).collect());
    assert!((0..100).all(|i| encode::slot(&config, convex.normalize(1. + i as f64 * 0.01), 0).is_some()));
    // Con ceros reales, esos sí caen en la franja neutra y la rampa se recupera con raw_at.
    let mixed = build((0..100).map(|i| i as f64 * 0.02 - 1.).collect());
    assert_eq!(encode::slot(&config, mixed.normalize(0.), 0), None);
    assert!(encode::slot(&config, mixed.normalize(-0.9), 0).is_some());
    for t in [0.1, 0.3, 0.7, 0.9] {
        assert!((mixed.normalize(mixed.raw_at(t)) - t).abs() < 0.02, "{t}");
    }
}
fn sample(point: P, value: f64) -> surface::Sample {
    surface::Sample {
        point,
        world: V3::default(),
        face: 0,
        normal: V3::new(0., 0., 1.),
        value,
    }
}
#[test]
fn ink_boundaries_are_exact_and_retain_every_segment_once() {
    let config = ReadingMap {
        ramp: Ramp::Sequential,
        pens: vec![0, 1, 2, 3],
        ..Default::default()
    };
    let values = values(config);
    let mut out = vec![vec![]; 4];
    encode::segment(
        surface::Segment {
            a: sample(p(0., 0.), 0.),
            b: sample(p(100., 0.), 1.),
        },
        &values,
        0,
        &mut out,
    );
    for (i, lines) in out.iter().enumerate() {
        assert_eq!(
            lines,
            &vec![vec![p(i as f64 * 25., 0.), p((i + 1) as f64 * 25., 0.)]]
        );
    }
}
#[test]
fn ordered_dither_and_density_produce_the_requested_coverage_without_randomness() {
    let mut config = ReadingMap {
        ramp: Ramp::Sequential,
        encoding: Encoding::Dither,
        pens: vec![0, 1],
        ..Default::default()
    };
    assert_eq!(
        (0..16)
            .filter(|i| encode::slot(&config, 0.5, *i) == Some(1))
            .count(),
        8
    );
    config.encoding = Encoding::Density;
    config.pens.truncate(1);
    assert_eq!(
        (0..16)
            .filter(|i| encode::slot(&config, 0.25, *i).is_some())
            .count(),
        4
    );
    assert!((0..16).all(|i| encode::slot(&config, 0., i).is_none()));
    assert!((0..16).all(|i| encode::slot(&config, 1., i) == Some(0)));
}
#[test]
fn sequential_order_uses_oklab_and_diverging_maps_leave_zero_as_paper() {
    let pens = vec![
        crate::Pen::new("Amarillo", [255, 255, 0], 0.2),
        crate::Pen::new("Negro", [0, 0, 0], 0.2),
        crate::Pen::new("Azul", [0, 0, 255], 0.2),
    ];
    let map = ReadingMap {
        ramp: Ramp::Sequential,
        pens: vec![0, 1, 2],
        ..Default::default()
    };
    assert_eq!(map.ordered_pens(&pens), vec![1, 2, 0]);
    assert!((lightness([255, 255, 255]) - 1.).abs() < 1e-6);
    assert_eq!(lightness([0, 0, 0]), 0.);
    let map = ReadingMap::default();
    assert_eq!(encode::slot(&map, 0.5, 0), None);
    assert_eq!(encode::slot(&map, 0., 0), Some(0));
    assert_eq!(encode::slot(&map, 1., 0), Some(1));
}
#[test]
fn every_reading_style_exports_finite_clipped_vector_paths_and_roundtrips() {
    let paper = crate::Paper::default();
    let scene = Settings::default();
    for style in ReadingStyle::ALL {
        let mut recipe = Recipe::new(
            if matches!(style, ReadingStyle::Unfold | ReadingStyle::Section) {
                Primitive::Cube.mesh()
            } else {
                Primitive::Torus.mesh()
            },
            scene.clone(),
        );
        recipe.inks = crate::doc::default_pens();
        recipe.passes = style.passes(&scene);
        let decoded = Recipe::from_json(&recipe.to_json().unwrap()).unwrap();
        let mut engine = Engine::default();
        let drawing = engine.draw_recipe(&decoded, &|| false).unwrap();
        assert!(
            !drawing.parts[0].lines.is_empty(),
            "estilo vacío: {}",
            style.name()
        );
        assert!(
            drawing
                .parts
                .iter()
                .flat_map(|p| &p.lines)
                .flatten()
                .all(|p| p.x.is_finite()
                    && p.y.is_finite()
                    && p.x >= paper.margin_mm - 1e-6
                    && p.x <= paper.width_mm - paper.margin_mm + 1e-6
                    && p.y >= paper.margin_mm - 1e-6
                    && p.y <= paper.height_mm - paper.margin_mm + 1e-6),
            "{}",
            style.name()
        );
        let doc = drawing.document(paper.clone(), recipe.inks.clone(), []);
        let svg = crate::svg_out::export(&doc, &doc.layers());
        assert!(svg.contains("<path") && !svg.contains("<image"));
        let again = engine.draw_recipe(&decoded, &|| false).unwrap();
        assert_eq!(drawing.parts[0].lines, again.parts[0].lines);
    }
}
#[test]
fn legend_is_real_vector_geometry_in_a_reserved_strip_and_does_not_erase_the_map() {
    let mut recipe = Recipe::new(Primitive::Torus.mesh(), Settings::default());
    recipe.inks = crate::doc::default_pens();
    recipe.passes = ReadingStyle::Thermal.passes(&recipe.settings);
    let map = recipe.passes[0].reading.as_mut().unwrap();
    map.legend = true;
    map.date = "2026-11-16".into();
    let d = Engine::default().draw_recipe(&recipe, &|| false).unwrap();
    let boundary = recipe.paper.height_mm - recipe.paper.margin_mm - 26.;
    assert!(d.parts[0]
        .lines
        .iter()
        .flatten()
        .any(|p| p.y < boundary - 10.));
    assert!(d.parts[0]
        .lines
        .iter()
        .flatten()
        .any(|p| p.y > boundary + 4.));
    let doc = d.document(recipe.paper.clone(), recipe.inks, []);
    let svg = crate::svg_out::export(&doc, &doc.layers());
    assert!(!svg.contains("<text") && !svg.contains("<image"));
}
#[test]
fn old_passes_can_receive_a_ramp_without_losing_geometry_on_palette_changes() {
    let scene = Settings {
        recipe: RecipeKind::SurfaceWaves,
        ..Default::default()
    };
    let source = Arc::new(Primitive::Torus.mesh());
    let mut passes = scene.passes();
    passes.truncate(1);
    passes[0].reading = Some(ReadingMap {
        measure: Measure::Height,
        ramp: Ramp::Sequential,
        pens: vec![0, 1, 2],
        ..Default::default()
    });
    let mut engine = Engine::default();
    let a = engine
        .draw_passes(
            source.clone(),
            &scene,
            &passes,
            &crate::Paper::default(),
            &|| false,
        )
        .unwrap();
    let renders = engine.renders;
    passes[0].reading.as_mut().unwrap().pens = vec![4, 5, 6];
    let b = engine
        .draw_passes(source, &scene, &passes, &crate::Paper::default(), &|| false)
        .unwrap();
    assert_eq!(engine.renders, renders);
    assert_eq!(a.parts[0].lines, b.parts[0].lines);
    assert_eq!(engine.readings.results.len(), 1);
}
#[test]
fn invalid_ramps_effects_and_mid_computation_cancellation_are_rejected() {
    let mut map = ReadingMap::default();
    map.pens.clear();
    assert!(map.validate().is_err());
    let mut params = ReadingParams {
        facets: 1200,
        effect: ReadingEffect::Unfold { tabs_mm: 2. },
    };
    assert!(params.validate().is_err());
    params.facets = 128;
    assert!(params.validate().is_ok());
    let count = std::cell::Cell::new(0);
    let cancel = || {
        count.set(count.get() + 1);
        count.get() > 10
    };
    let d = draw(Primitive::Torus.mesh());
    let mut cache = fields::Cache::default();
    assert!(cache
        .values(
            d.mesh,
            Measure::AmbientOcclusion,
            128,
            &Settings::default(),
            d.camera,
            &cancel
        )
        .is_err());
}

#[test]
fn curvature_gauss_bonnet_and_thickness_have_physical_scale() {
    let mesh = Arc::new(Primitive::Sphere.mesh());
    let mut cache = fields::Cache::default();
    let g = cache.geometry(mesh.clone(), &|| false).unwrap();
    let points = mesh.points();
    let mut mass = vec![0.; points.len()];
    for t in &mesh.triangles {
        let a = points[t[0]];
        let b = points[t[1]];
        let c = points[t[2]];
        let area = (b - a).cross(c - a).length() * 0.5;
        for i in t {
            mass[*i] += area / 3.;
        }
    }
    let integrated = g
        .gaussian
        .iter()
        .zip(&mass)
        .map(|(k, a)| k * a)
        .sum::<f64>();
    assert!((integrated - 4. * std::f64::consts::PI).abs() < 1e-7);
    let thickness = cache
        .values(
            mesh.clone(),
            Measure::Thickness,
            24,
            &Settings::default(),
            Camera::default(),
            &|| false,
        )
        .unwrap();
    let mean = thickness.iter().sum::<f64>() / thickness.len() as f64;
    assert!((mean - 2.).abs() < 0.08, "espesor esfera {mean}");
}
#[test]
fn aspect_wraps_at_north_without_inventing_a_half_turn() {
    let map = ReadingMap {
        measure: Measure::Aspect,
        ramp: Ramp::Sequential,
        pens: vec![0, 1, 2, 3],
        ..Default::default()
    };
    let mut values = values(map);
    values.range = [0., 360.];
    let mut out = vec![vec![]; 4];
    encode::segment(
        surface::Segment {
            a: sample(p(0., 0.), 350.),
            b: sample(p(20., 0.), 370.),
        },
        &values,
        0,
        &mut out,
    );
    assert!(out[1].is_empty() && out[2].is_empty());
    assert_eq!(out[3], vec![vec![p(0., 0.), p(10., 0.)]]);
    assert_eq!(out[0], vec![vec![p(10., 0.), p(20., 0.)]]);
}
#[test]
fn unfolded_cube_keeps_coplanar_faces_connected_and_uses_a_single_physical_scale() {
    let d = draw(Primitive::Cube.mesh());
    let map = ReadingMap {
        pens: vec![0, 1, 2],
        ..Default::default()
    };
    let (groups, scale) = unfold::draw(&d, &map, 2., &crate::Paper::default(), &|| false).unwrap();
    assert!(scale > 0.);
    assert!(!groups[0].is_empty());
    assert!(!groups[1].is_empty() || !groups[2].is_empty());
    // No diagonal de triangulación de un cuadrado se convierte en línea de corte.
    assert!(groups[0]
        .iter()
        .flat_map(|l| l.windows(2))
        .all(|p| p[0].dist(p[1]) <= 2. * scale + 1e-6));
}
#[test]
fn section_hatching_preserves_the_torus_hole_and_stereo_keeps_the_two_cameras_separate() {
    let s = Settings {
        elevation: 89.99,
        ..Default::default()
    };
    let source = Arc::new(Primitive::Torus.mesh());
    let mut engine = Engine::default();
    let pass = ReadingStyle::Section.passes(&s);
    let d = engine
        .draw_passes(source.clone(), &s, &pass, &crate::Paper::default(), &|| {
            false
        })
        .unwrap();
    let center = d.camera.project(V3::default()).0;
    let hatch = &d.parts[0].families[2].lines;
    assert!(!hatch.is_empty());
    assert!(hatch.iter().flat_map(|line| line.windows(2)).all(|pair| {
        let midpoint = (pair[0] + pair[1]) * 0.5;
        midpoint.dist(center) > d.camera.scale * 0.25
    }));
    let s = Settings::default();
    let mut passes = ReadingStyle::Anaglyph.passes(&s);
    let PassKind::Reading(params) = &mut passes[0].kind else {
        panic!()
    };
    params.effect = ReadingEffect::Anaglyph {
        angle_deg: 3.,
        side_by_side: true,
    };
    let d = engine
        .draw_passes(source, &s, &passes, &crate::Paper::default(), &|| false)
        .unwrap();
    let half = crate::Paper::default().width_mm * 0.5;
    assert!(d.parts[0].families[0]
        .lines
        .iter()
        .flatten()
        .all(|p| p.x < half));
    assert!(d.parts[0].families[1]
        .lines
        .iter()
        .flatten()
        .all(|p| p.x > half));
}
#[test]
fn custom_palette_is_sorted_when_a_drawing_becomes_a_document() {
    let s = Settings::default();
    let source = Arc::new(Primitive::Torus.mesh());
    let mut passes = ReadingStyle::Thermal.passes(&s);
    passes[0].reading.as_mut().unwrap().pens = vec![0, 1, 2];
    let drawing = Engine::default()
        .draw_passes(source, &s, &passes, &crate::Paper::default(), &|| false)
        .unwrap();
    let palette = vec![
        crate::Pen::new("Blanco", [255, 255, 255], 0.2),
        crate::Pen::new("Negro", [0, 0, 0], 0.2),
        crate::Pen::new("Gris", [128, 128, 128], 0.2),
    ];
    let doc = drawing.document(crate::Paper::default(), palette, []);
    assert_eq!(doc.items[0].stroke, Some(1));
    assert_eq!(doc.items[1].stroke, Some(2));
    assert_eq!(doc.items[2].stroke, Some(0));
}
