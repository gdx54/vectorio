use super::*;
use crate::{geom, svg_out, Document, Paper};
fn settings(kind: RecipeKind) -> Settings {
    Settings {
        recipe: kind,
        art: ArtSettings {
            facets: 650,
            lines: 32,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn length(d: &Drawing) -> f64 {
    d.parts
        .iter()
        .flat_map(|p| &p.lines)
        .map(|l| geom::length(l))
        .sum()
}
#[test]
fn graph_distance_follows_the_surface_and_seeds_every_component() {
    let m = Primitive::Torus.mesh().transformed([1.; 3], [0.; 3]);
    let graph = field::Graph::new(&m, &|| false).unwrap();
    let s = ArtSettings {
        deformation: 0.,
        origin_azimuth: 0.,
        origin_elevation: 0.,
        ..Default::default()
    };
    let values = graph.values(&s, 1, &|| false).unwrap();
    let seed = values.iter().position(|d| *d == 0.).unwrap();
    let (far, d) = values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    assert!(*d > (m.points()[seed] - m.points()[far]).length() * 1.15);
    let disconnected = Primitive::Assembly.mesh();
    let graph = field::Graph::new(&disconnected, &|| false).unwrap();
    let values = graph.values(&s, 1, &|| false).unwrap();
    assert!(values.iter().all(|v| v.is_finite()));
    assert!(graph.components.len() > 1);
    assert_eq!(
        values.iter().filter(|v| **v == 0.).count(),
        graph.components.len()
    );
    let mut inverted = m.clone();
    for t in &mut inverted.triangles {
        t.swap(0, 1);
    }
    let opposite = field::Graph::new(&inverted, &|| false)
        .unwrap()
        .values(&s, 1, &|| false)
        .unwrap();
    assert_eq!(values.len(), disconnected.vertices.len());
    assert_eq!(opposite, graph_values(&m, &s, 1));
}
fn graph_values(m: &Mesh, s: &ArtSettings, seed: u64) -> Vec<f64> {
    field::Graph::new(m, &|| false)
        .unwrap()
        .values(s, seed, &|| false)
        .unwrap()
}
#[test]
fn origins_interference_and_seeded_warp_change_the_field_deterministically() {
    let m = Primitive::Sphere.mesh();
    let mut s = ArtSettings::default();
    let a = graph_values(&m, &s, 1);
    assert_eq!(a, graph_values(&m, &s, 1));
    assert_ne!(a, graph_values(&m, &s, 2));
    s.origin_azimuth += 100.;
    assert_ne!(a, graph_values(&m, &s, 1));
    s.pattern = WavePattern::Interference;
    let interference = graph_values(&m, &s, 1);
    assert!(interference.iter().any(|v| *v < 0.) && interference.iter().any(|v| *v > 0.));
    s.deformation = 0.;
    assert_eq!(graph_values(&m, &s, 1), graph_values(&m, &s, 100));
}
#[test]
fn triangle_mesh_includes_each_diagonal_once_and_all_color_rules_work() {
    let m = Primitive::Cube.mesh();
    let cam = Camera::default();
    let mut s = ArtSettings::default();
    let edges = edges::strokes(&m, &s, &cam, false, &|| false).unwrap();
    assert_eq!(edges.len(), 18);
    let keys: std::collections::BTreeSet<_> =
        edges.iter().map(|e| format!("{:?}", e.points)).collect();
    assert_eq!(keys.len(), 18);
    assert_eq!(
        edges
            .iter()
            .map(|e| e.role)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    for color in [
        MeshColor::Orientation,
        MeshColor::Height,
        MeshColor::Depth,
        MeshColor::Objects,
    ] {
        s.coloring = color;
        for count in 1..=3 {
            s.mesh_inks = count;
            let strokes = edges::strokes(&m, &s, &cam, false, &|| false).unwrap();
            assert_eq!(strokes.len(), 18);
            assert!(strokes.iter().all(|e| e.role < count));
            let hybrid = edges::strokes(&m, &s, &cam, true, &|| false).unwrap();
            assert!(hybrid.iter().all(|e| e.role >= 1 && e.role <= count));
        }
    }
    s.coloring = MeshColor::Objects;
    s.mesh_inks = 3;
    let multi = Primitive::Assembly.mesh();
    let strokes = edges::strokes(&multi, &s, &cam, false, &|| false).unwrap();
    assert!(
        strokes
            .iter()
            .map(|e| e.role)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            >= 2
    );
}
#[test]
fn all_three_modes_export_real_physical_paths_and_roundtrip_without_losing_roles() {
    let m = Arc::new(Primitive::Torus.mesh());
    let original = m.as_ref().clone();
    let paper = Paper::default();
    let mut e = Engine::default();
    for kind in [
        RecipeKind::SurfaceWaves,
        RecipeKind::TriMesh,
        RecipeKind::Hybrid,
    ] {
        let mut s = settings(kind);
        s.zoom = 2.;
        s.position_mm = [80., -30.];
        s.paper_rotation = 32.;
        let d = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
        assert_eq!(d.parts.len(), 3);
        assert!(length(&d) > 0.);
        assert!(d.art.is_some() && d.shadow.is_none());
        if kind == RecipeKind::TriMesh {
            assert!(d.curves.is_empty());
        } else {
            assert!(!d.curves.is_empty());
        }
        if kind == RecipeKind::Hybrid {
            assert!(d.parts.iter().all(|p| !p.lines.is_empty()));
        }
        let doc = d.document(paper.clone(), crate::doc::default_pens(), [0, 2, 1]);
        let restored = Document::from_json(&doc.to_json()).unwrap();
        assert_eq!(doc.items, restored.items);
        let layers = doc.layers();
        let svg = svg_out::export(&doc, &layers);
        assert!(usvg::Tree::from_str(&svg, &usvg::Options::default()).is_ok());
        assert!(!svg.contains("<image") && !svg.contains("NaN"));
        for q in layers.iter().flat_map(|l| &l.paths).flatten() {
            assert!(
                q.x >= paper.margin_mm - 1e-7
                    && q.y >= paper.margin_mm - 1e-7
                    && q.x <= paper.width_mm - paper.margin_mm + 1e-7
                    && q.y <= paper.height_mm - paper.margin_mm + 1e-7
            );
        }
        let recipe = Recipe::new(m.as_ref().clone(), s.clone());
        let reopened = Recipe::from_json(&recipe.to_json().unwrap()).unwrap();
        assert_eq!(reopened, recipe);
        let rebuilt = Engine::default()
            .draw(Arc::new(reopened.mesh), &reopened.settings, &paper, &|| {
                false
            })
            .unwrap();
        assert_eq!(
            d.parts.iter().map(|p| &p.lines).collect::<Vec<_>>(),
            rebuilt.parts.iter().map(|p| &p.lines).collect::<Vec<_>>()
        );
        let repeat = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
        assert_eq!(
            d.parts.iter().map(|p| &p.lines).collect::<Vec<_>>(),
            repeat.parts.iter().map(|p| &p.lines).collect::<Vec<_>>()
        );
    }
    assert_eq!(*m, original);
}
#[test]
fn mesh_occlusion_uses_the_drawn_mesh_and_color_count_changes_only_roles() {
    let m = Arc::new(Primitive::Sphere.mesh());
    let paper = Paper::default();
    let mut e = Engine::default();
    let mut s = settings(RecipeKind::TriMesh);
    s.art.facets = 90;
    s.visible_only = false;
    let all = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    assert!(all.mesh.triangles.len() < m.triangles.len());
    s.visible_only = true;
    let visible = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    assert!(length(&visible) > length(&all) * 0.2 && length(&visible) < length(&all) * 0.8);
    s.art.mesh_inks = 1;
    let mono = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    assert!(Arc::ptr_eq(&visible.mesh, &mono.mesh));
    assert!(mono.parts[1].lines.is_empty() && mono.parts[2].lines.is_empty());
    assert!((length(&mono) - length(&visible)).abs() < 0.1);
    s.recipe = RecipeKind::Hybrid;
    let combined = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    s.yaw += 40.;
    s.art.mesh_inks = 2;
    let orbit = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    assert!(Arc::ptr_eq(&combined.curves, &orbit.curves));
    s.art.facets = 300;
    let detailed = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    assert!(detailed.mesh.triangles.len() > orbit.mesh.triangles.len());
    assert!(!Arc::ptr_eq(&detailed.curves, &orbit.curves));
    assert!(e.draw(m, &s, &paper, &|| true).is_err());
}
#[test]
fn depth_colors_track_the_camera_and_old_recipes_default_the_new_settings() {
    let m = Arc::new(Primitive::Torus.mesh());
    let mut e = Engine::default();
    let mut s = settings(RecipeKind::TriMesh);
    s.art.coloring = MeshColor::Depth;
    let a = e.draw(m.clone(), &s, &Paper::default(), &|| false).unwrap();
    s.yaw += 100.;
    let b = e.draw(m.clone(), &s, &Paper::default(), &|| false).unwrap();
    assert!(!Arc::ptr_eq(
        &a.art.as_ref().unwrap().edges,
        &b.art.as_ref().unwrap().edges
    ));
    assert_ne!(
        *a.art.as_ref().unwrap().edges,
        *b.art.as_ref().unwrap().edges
    );
    let mut old =
        serde_json::to_value(Recipe::new(m.as_ref().clone(), Settings::default())).unwrap();
    old["settings"].as_object_mut().unwrap().remove("art");
    assert_eq!(
        Recipe::from_json(&old.to_string()).unwrap().settings.art,
        ArtSettings::default()
    );
    let mut bad = s;
    bad.art.facets = usize::MAX;
    assert!(bad.validate().is_err());
    bad.art.facets = 300;
    bad.art.frequency = f64::NAN;
    assert!(bad.validate().is_err());
}
#[test]
fn open_net_is_draped_on_the_fine_surface_and_old_files_draw_every_facet() {
    let m = Primitive::Sphere.mesh().transformed([1.; 3], [0.; 3]);
    let s = ArtSettings {
        mesh_facets: 200,
        ..Default::default()
    };
    let camera = Camera::default();
    let full = edges::strokes(&m, &s, &camera, false, &|| false).unwrap();
    let net = crate::volumetric::reduce::reduced(&m, 200, 0.04, &|| false).unwrap();
    let mut open = edges::strokes(&net, &s, &camera, false, &|| false).unwrap();
    assert!(open.len() * 3 < full.len(), "{} {}", open.len(), full.len());
    edges::drape(&mut open, &m, &|| false).unwrap();
    let [lo, hi] = m.bounds();
    let (center, radius) = ((lo + hi) * 0.5, (hi.x - lo.x) * 0.5);
    let mut inner = 0;
    for stroke in &open {
        assert!(stroke.points.len() >= 2);
        for q in &stroke.points[1..stroke.points.len() - 1] {
            inner += 1;
            let d = (*q - center).length();
            assert!((d - radius).abs() < radius * 0.01, "{d} {radius}");
        }
    }
    assert!(inner > open.len());
    // Los archivos sin el campo conservan la malla entera de antes.
    let mut json = serde_json::to_value(ArtSettings::default()).unwrap();
    json.as_object_mut().unwrap().remove("mesh_facets");
    let old: ArtSettings = serde_json::from_value(json).unwrap();
    assert_eq!(old.mesh_facets, 20_000);
    assert_eq!(ArtSettings::default().mesh_facets, DEFAULT_NET);
}
