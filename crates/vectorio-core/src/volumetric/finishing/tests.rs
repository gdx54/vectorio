use super::*;
use std::sync::Arc;
fn length(lines: &[Polyline]) -> f64 {
    lines.iter().map(|l| crate::geom::length(l)).sum()
}
fn square(lo: f64, hi: f64) -> Polyline {
    vec![p(lo, lo), p(hi, lo), p(hi, hi), p(lo, hi), p(lo, lo)]
}
#[test]
fn halo_uses_depth_and_physical_arc_length_across_vertices() {
    let camera = Camera {
        scale: 1.,
        paper_center: p(10., 10.),
        world_center: V3::default(),
        ..Default::default()
    };
    let (right, down, away) = camera.axes();
    let world = |x: f64, y: f64, z: f64| right * (x - 10.) + down * (y - 10.) + away * z;
    let render = |depth, halo| {
        let mut scene = crate::scene::Scene::new(
            camera,
            BBox {
                min: p(0., 0.),
                max: p(20., 20.),
            },
        );
        scene.set_intersections(false);
        scene.set_line_halo(halo);
        scene.occluder(&[world(4., 4., 0.), world(16., 4., 0.), world(10., 16., 0.)]);
        scene.line(&[
            world(0., 8., depth),
            world(5.5, 8., depth),
            world(8., 8., depth),
            world(12., 8., depth),
            world(14.5, 8., depth),
            world(20., 8., depth),
        ]);
        scene.render().parts.remove(0).lines
    };
    let original = render(2., 0.);
    let halo = render(2., 1.);
    assert!((length(&original) - 12.).abs() < 1e-7);
    assert!((length(&halo) - 10.).abs() < 1e-7);
    assert!((halo[0].last().unwrap().x - 5.).abs() < 1e-7);
    assert!((halo[1][0].x - 15.).abs() < 1e-7);
    assert!((length(&render(-2., 1.)) - 20.).abs() < 1e-7);
}
#[test]
fn closed_halo_does_not_depend_on_where_the_polyline_starts() {
    let camera = Camera {
        scale: 1.,
        paper_center: p(10., 10.),
        world_center: V3::default(),
        ..Default::default()
    };
    let (r, d, a) = camera.axes();
    let world = |p: crate::P, z| r * (p.x - 10.) + d * (p.y - 10.) + a * z;
    let points = [p(10., 8.), p(18., 8.), p(18., 18.), p(10., 18.)];
    let render = |shift: usize| {
        let mut scene = crate::scene::Scene::new(
            camera,
            BBox {
                min: p(0., 0.),
                max: p(20., 20.),
            },
        );
        scene.set_intersections(false);
        scene.set_line_halo(1.);
        scene.occluder(&[
            world(p(4., 4.), 0.),
            world(p(16., 4.), 0.),
            world(p(10., 16.), 0.),
        ]);
        let mut line = (0..4)
            .map(|i| world(points[(i + shift) % 4], 2.))
            .collect::<Vec<_>>();
        line.push(line[0]);
        scene.line(&line);
        scene.render().parts.remove(0).lines
    };
    assert!((length(&render(0)) - length(&render(2))).abs() < 1e-7);
}
#[test]
fn echo_expands_outer_contours_shrinks_holes_and_ignores_winding() {
    let input = vec![square(10., 30.), square(16., 24.)];
    let out = echo::copies(&input, 1, 1., &|| false).unwrap();
    let a = BBox::of(std::iter::once(&out[0]));
    let b = BBox::of(std::iter::once(&out[1]));
    assert_eq!((a.min, a.max), (p(9., 9.), p(31., 31.)));
    assert_eq!((b.min, b.max), (p(17., 17.), p(23., 23.)));
    let reversed = input
        .iter()
        .map(|l| l.iter().rev().copied().collect())
        .collect::<Vec<_>>();
    let reversed = echo::copies(&reversed, 1, 1., &|| false).unwrap();
    assert_eq!(BBox::of(&out), BBox::of(&reversed));
    let inward = echo::copies(&[square(10., 30.)], 1, -1., &|| false).unwrap();
    assert_eq!(
        BBox::of(&inward),
        BBox {
            min: p(11., 11.),
            max: p(29., 29.)
        }
    );
}
#[test]
fn echo_joins_unordered_segments_and_supports_a_point_two_mm_double_outline() {
    let pieces = vec![
        vec![p(20., 10.), p(20., 20.)],
        vec![p(10., 20.), p(10., 10.)],
        vec![p(20., 20.), p(10., 20.)],
        vec![p(10., 10.), p(20., 10.)],
    ];
    let copies = echo::copies(&pieces, 1, 0.2, &|| false).unwrap();
    assert_eq!(copies.len(), 1);
    let bounds = BBox::of(&copies);
    assert!((bounds.min.x - 9.8).abs() < 1e-8);
    assert!((bounds.max.x - 20.2).abs() < 1e-8);
}
#[test]
fn echo_bridges_silhouette_steps_and_trims_open_ends_instead_of_fanning() {
    // Dos tramos de un mismo arco que se solapan 0,8 mm con un escalón lateral de 0,16 mm,
    // como los que deja la silueta de una malla facetada.
    let left = vec![p(0., 0.), p(10., 0.), p(20.8, 0.)];
    let right = vec![p(20., 0.16), p(30., 0.16), p(40., 0.16)];
    let copies = echo::copies(&[left.clone(), right.clone()], 3, 0.8, &|| false).unwrap();
    assert_eq!(copies.len(), 3, "un solo arco: tres ecos continuos");
    for (k, copy) in copies.iter().enumerate() {
        let cut = 0.8 * (k + 1) as f64;
        // Cada eco se acorta en los extremos libres en lugar de abrirse en abanico.
        assert!((copy[0].x - cut).abs() < 0.05, "{k}: {:?}", copy[0]);
        assert!((copy.last().unwrap().x - (40. - cut)).abs() < 0.05);
        assert!(copy.windows(2).all(|w| w[1].x >= w[0].x - 1e-9), "sin retrocesos");
    }
    // Dos líneas de sombreado vecinas no se unen: el salto entre ellas es transversal.
    let hatch = vec![vec![p(0., 0.), p(10., 0.)], vec![p(0., 0.6), p(10., 0.6)]];
    assert_eq!(echo::copies(&hatch, 1, 0.8, &|| false).unwrap().len(), 2);
}
#[test]
fn smooth_jitter_is_seeded_bounded_and_preserves_closed_seams() {
    let line = square(20., 80.);
    let settings = FinishSettings {
        jitter_mm: 0.3,
        ..Default::default()
    };
    let a = variation::lines(&line, &settings, 27, &|| false).unwrap();
    let b = variation::lines(&line, &settings, 27, &|| false).unwrap();
    assert_eq!(a, b);
    assert_ne!(
        a,
        variation::lines(&line, &settings, 28, &|| false).unwrap()
    );
    assert!(a[0][0].dist(*a[0].last().unwrap()) < 1e-8);
    assert!(a[0]
        .iter()
        .all(|q| crate::geom::dist_to_lines(*q, std::slice::from_ref(&line)) <= 0.3 + 1e-8));
    assert!(a[0].len() > line.len());
}
#[test]
fn seeded_shortening_and_small_gaps_are_reproducible_and_cancel() {
    let line = vec![p(0., 0.), p(100., 0.)];
    let settings = FinishSettings {
        irregularity: 0.5,
        gap_probability: 0.5,
        ..Default::default()
    };
    let out = variation::lines(&line, &settings, 123, &|| false).unwrap();
    assert!(out.len() > 1);
    assert!(length(&out) < 100.);
    assert_eq!(
        out,
        variation::lines(&line, &settings, 123, &|| false).unwrap()
    );
    assert!(variation::lines(&line, &settings, 123, &|| true).is_err());
}
#[test]
fn every_artistic_style_is_vectorial_clipped_deterministic_and_recipe_compatible() {
    let scene = Settings::default();
    for style in ArtisticStyle::ALL {
        let mut recipe = Recipe::new(Primitive::Torus.mesh(), scene.clone());
        recipe.passes = style.passes(&scene);
        recipe.inks = crate::doc::default_pens();
        let restored = Recipe::from_json(&recipe.to_json().unwrap()).unwrap();
        assert_eq!(restored, recipe);
        let mut engine = Engine::default();
        let drawing = engine.draw_recipe(&recipe, &|| false).unwrap();
        assert!(drawing.parts.iter().any(|p| !p.lines.is_empty()));
        let paper = &recipe.paper;
        assert!(drawing
            .parts
            .iter()
            .flat_map(|p| &p.lines)
            .flatten()
            .all(|p| p.x.is_finite()
                && p.y.is_finite()
                && p.x >= paper.margin_mm - 1e-7
                && p.x <= paper.width_mm - paper.margin_mm + 1e-7
                && p.y >= paper.margin_mm - 1e-7
                && p.y <= paper.height_mm - paper.margin_mm + 1e-7));
        let doc = drawing.document(paper.clone(), recipe.inks.clone(), []);
        let svg = crate::svg_out::export(&doc, &doc.layers());
        assert!(svg.contains("<path") && !svg.contains("<image"));
        let again = engine.draw_recipe(&recipe, &|| false).unwrap();
        for (a, b) in drawing.parts.iter().zip(&again.parts) {
            assert_eq!(a.lines, b.lines);
        }
    }
}
#[test]
fn finish_cache_ignores_names_ink_assignment_and_order_and_does_not_repeat_geometry() {
    let source = Arc::new(Primitive::Torus.mesh());
    let scene = Settings::default();
    let paper = crate::Paper::default();
    let mut passes = ArtisticStyle::Hand.passes(&scene);
    let mut engine = Engine::default();
    let first = engine
        .draw_passes(source.clone(), &scene, &passes, &paper, &|| false)
        .unwrap();
    let renders = engine.renders;
    let builds = engine.decorations.builds;
    passes[0].name = "Renombrada".into();
    passes[0].pen = 7;
    passes.reverse();
    let second = engine
        .draw_passes(source.clone(), &scene, &passes, &paper, &|| false)
        .unwrap();
    assert_eq!(engine.renders, renders);
    assert_eq!(engine.decorations.builds, builds);
    assert_eq!(first.parts[0].lines, second.parts[1].lines);
    passes[1].finish.seed += 1;
    let third = engine
        .draw_passes(source, &scene, &passes, &paper, &|| false)
        .unwrap();
    assert_eq!(engine.renders, renders);
    assert_eq!(engine.decorations.builds, builds + 1);
    assert_ne!(second.parts[1].lines, third.parts[1].lines);
    assert_eq!(second.parts[0].lines, third.parts[0].lines);
}
#[test]
fn colored_echo_keeps_its_own_ink_without_changing_the_order_of_the_gradient() {
    let source = Arc::new(Primitive::Sphere.mesh());
    let scene = Settings::default();
    let mut pass = ArtisticStyle::Echo.passes(&scene).remove(0);
    pass.pen = 5;
    pass.finish.echo_count = 1;
    pass.reading = Some(ReadingMap {
        measure: Measure::Height,
        encoding: Encoding::Dither,
        ramp: Ramp::Sequential,
        pens: vec![2, 1],
        ..Default::default()
    });
    let drawing = Engine::default()
        .draw_passes(source, &scene, &[pass], &crate::Paper::default(), &|| false)
        .unwrap();
    let part = &drawing.parts[0];
    assert!(part
        .families
        .iter()
        .any(|f| f.slot == usize::MAX && !f.lines.is_empty()));
    let doc = drawing.document(crate::Paper::default(), crate::doc::default_pens(), []);
    assert!(doc.items.iter().any(|i| i.stroke == Some(5)));
    assert!(doc
        .items
        .iter()
        .any(|i| i.stroke == Some(2) || i.stroke == Some(1)));
}
#[test]
fn finishing_preserves_the_legend_and_old_v2_defaults_and_validates_limits() {
    let mut recipe = Recipe::new(Primitive::Torus.mesh(), Settings::default());
    recipe.passes = ReadingStyle::Thermal.passes(&recipe.settings);
    recipe.inks = crate::doc::default_pens();
    recipe.passes[0].reading.as_mut().unwrap().legend = true;
    let mut engine = Engine::default();
    let original = engine.draw_recipe(&recipe, &|| false).unwrap();
    let area = reading_content_paper(&recipe.paper, recipe.passes.iter());
    let is_legend = |line: &Polyline| {
        line.iter()
            .all(|p| p.y > area.height_mm - area.margin_mm + 0.5)
    };
    recipe.passes[0].finish.jitter_mm = 0.15;
    let finished = engine.draw_recipe(&recipe, &|| false).unwrap();
    let a = original.parts[0]
        .lines
        .iter()
        .filter(|l| is_legend(l))
        .collect::<Vec<_>>();
    let b = finished.parts[0]
        .lines
        .iter()
        .filter(|l| is_legend(l))
        .collect::<Vec<_>>();
    assert!(!a.is_empty());
    assert_eq!(a, b);
    let base = Recipe::new(Primitive::Cube.mesh(), Settings::default());
    let mut json = serde_json::to_value(&base).unwrap();
    for pass in json["passes"].as_array_mut().unwrap() {
        pass.as_object_mut().unwrap().remove("finish");
    }
    json["settings"].as_object_mut().unwrap().remove("halo_mm");
    assert_eq!(Recipe::from_json(&json.to_string()).unwrap(), base);
    let mut p = base.passes[0].clone();
    p.finish.echo_count = 1;
    assert!(p.validate().is_err());
    p.finish = FinishSettings {
        halo_mm: f64::NAN,
        ..Default::default()
    };
    assert!(p.validate().is_err());
}

#[test]
fn an_echo_wholly_outside_the_paper_is_not_mistaken_for_a_legend() {
    let paper = crate::Paper {
        width_mm: 100.,
        height_mm: 100.,
        margin_mm: 20.,
        ..Default::default()
    };
    let mut pass = ArtisticStyle::Echo.passes(&Settings::default()).remove(0);
    pass.finish.echo_count = 1;
    pass.finish.echo_mm = 3.;
    let mut part = DrawingPart {
        pen: 0,
        name: "Contorno abierto".into(),
        lines: vec![vec![p(25., 79.), p(40., 79.)]],
        families: vec![],
        ramp: None,
    };
    Cache::default()
        .apply(&mut part, &pass, 1, &paper, &|| false)
        .unwrap();
    assert_eq!(part.lines, vec![vec![p(25., 79.), p(40., 79.)]]);
}
