use super::*;
use crate::BBox;
use super::super::ParamValue;

fn context(seed: u64) -> GenContext {
    GenContext { area: BBox { min: p(5.0, 5.0), max: p(195.0, 195.0) }, seed, pen_mm: 0.35, detail: Detail::Final }
}

#[test]
fn worlds_are_deterministic_and_options_change_the_drawing() {
    let ctx = context(4);
    let mut prm = Params::default().normalized(&Domes.params());
    let sea = Domes.generate(&prm, &ctx).unwrap();
    assert_eq!(sea, Domes.generate(&prm, &ctx).unwrap());
    prm.set("world", ParamValue::Text("space".into()));
    let space = Domes.generate(&prm, &ctx).unwrap();
    assert_ne!(sea, space);
    assert_eq!(space, Domes.generate(&prm, &ctx).unwrap());
}

#[test]
fn worlds_views_and_protagonists_are_clean_finite_and_within_budget() {
    for seed in 0..12 {
        for (world, key, choices) in [
            ("underwater", "creature", ["whale", "squid", "saurian", "none"]),
            ("space", "craft", ["rocket", "station", "whale", "none"]),
        ] {
            let mut prm = Params::default().normalized(&Domes.params());
            prm.set("world", ParamValue::Text(world.into()));
            prm.set(key, ParamValue::Text(choices[seed as usize % choices.len()].into()));
            prm.set("yaw", ParamValue::Text(["225", "315", "45", "135"][seed as usize % 4].into()));
            prm.set("domes", ParamValue::Number(5.0 + (seed % 5) as f64));
            check_scene(&prm, &context(seed));
        }
    }
}

fn check_scene(prm: &Params, ctx: &GenContext) {
    let world = World { prm, ctx, space: prm.choice("world") == "space", style: Style::new(ctx, 1.1) };
    let scene = world.scene();
    assert!(scene.load().within_budget(), "{} {:?}", ctx.seed, scene.load());
    let issues = scene.construction_issues(1.5);
    assert!(issues.is_empty(), "{} {}: {issues:?}", prm.choice("world"), ctx.seed);
    let output = world.style.finish(&scene);
    assert!(output.total_m() > 0.5 && output.total_m() < 80.0);
    for point in output.parts.iter().flat_map(|part| &part.lines).flatten() {
        assert!(point.x.is_finite() && point.y.is_finite());
        assert!(ctx.area.contains(*point, 1e-7), "{point:?}");
    }
}

#[test]
fn every_dome_is_connected_and_first_six_interiors_are_distinct() {
    for count in 5..=9 {
        let mut prm = Params::default().normalized(&Domes.params());
        prm.set("domes", ParamValue::Number(count as f64));
        let ctx = context(count as u64);
        let world = World { prm: &prm, ctx: &ctx, space: false, style: Style::new(&ctx, 1.1) };
        let pods = world.pods(&world.camera());
        let edges = links(&pods);
        assert_eq!(pods.len(), count);
        assert_eq!(edges.len(), count - 1);
        let mut connected = vec![false; count];
        connected[0] = true;
        for (a, b) in edges {
            assert!(connected[a] && !connected[b]);
            connected[b] = true;
        }
        assert!(connected.iter().all(|c| *c));
        let mut themes: Vec<_> = pods.iter().take(6).map(|pod| pod.habitat).collect();
        themes.sort();
        themes.dedup();
        assert_eq!(themes.len(), count.min(6));
    }
}

#[test]
fn rectangular_paper_and_thick_pen_remain_valid() {
    for (width, height) in [(110.0, 190.0), (190.0, 110.0)] {
        let mut ctx = context(15);
        ctx.area.max = p(width, height);
        ctx.pen_mm = 0.7;
        let prm = Params::default().normalized(&Domes.params());
        check_scene(&prm, &ctx);
    }
}

#[test]
#[ignore = "500 semillas en ambos niveles; medir en release con --ignored --nocapture"]
fn randomized_500_seeds_in_preview_and_final() {
    use std::{collections::BTreeSet, time::{Duration, Instant}};
    let mut worst = [Duration::ZERO; 2];
    for seed in 0..500 {
        let mut prm = Params::default();
        Domes.randomize(&mut Rng::new(seed), &mut prm, &BTreeSet::new());
        for (i, detail) in [Detail::Preview, Detail::Final].into_iter().enumerate() {
            let mut ctx = context(seed);
            ctx.detail = detail;
            let start = Instant::now();
            let output = Domes.generate(&prm, &ctx).unwrap();
            worst[i] = worst[i].max(start.elapsed());
            assert!(output.warning.is_none(), "{seed} {detail:?}: {:?}", output.warning);
            for point in output.parts.iter().flat_map(|part| &part.lines).flatten() {
                assert!(point.x.is_finite() && point.y.is_finite() && ctx.area.contains(*point, 1e-7));
            }
        }
    }
    eprintln!("Cúpulas, 500 semillas: Preview {:?}, Final {:?}", worst[0], worst[1]);
    assert!(worst[0].as_secs_f64() < 0.25 && worst[1].as_secs_f64() < 1.5);
}
