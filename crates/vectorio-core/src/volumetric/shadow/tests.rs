use super::*;
fn mesh(vertices: Vec<[f64; 3]>, triangles: Vec<[usize; 3]>) -> Mesh {
    Mesh {
        name: "prueba".into(),
        vertices,
        triangles,
        objects: vec![],
        face_objects: vec![],
    }
}
fn area(polys: &[Polyline]) -> f64 {
    polys
        .iter()
        .map(|p| crate::geom::signed_area(p).abs())
        .sum()
}
#[test]
fn analytic_visibility_removes_whole_and_partial_occlusion() {
    let m = mesh(
        vec![
            [0., 0., 0.],
            [2., 0., 0.],
            [0., 2., 0.],
            [0., 0., 1.],
            [2., 0., 1.],
            [0., 2., 1.],
        ],
        vec![[0, 1, 2], [3, 4, 5]],
    );
    let projection = project::Projection::new(&m, &|q| (p(q.x, q.y), -q.z));
    assert!(projection.visible(0, &|| false).unwrap().is_empty());
    assert_eq!(projection.visible(1, &|| false).unwrap().len(), 1);
    let mut m = m;
    m.vertices[4] = [1., 0., 1.];
    m.vertices[5] = [0., 1., 1.];
    let proj = project::Projection::new(&m, &|q| (p(q.x, q.y), -q.z));
    let visible = proj.visible(0, &|| false).unwrap();
    let a: f64 = visible
        .iter()
        .map(|p| crate::geom::signed_area(&p.iter().map(|v| v.point).collect::<Vec<_>>()).abs())
        .sum();
    assert!((a - 1.5).abs() < 1e-9);
}
#[test]
fn a_lit_face_can_be_shadowed_by_another_component() {
    let m = mesh(
        vec![
            [-2., -2., 0.],
            [2., -2., 0.],
            [2., 2., 0.],
            [-2., 2., 0.],
            [-0.5, -0.5, 1.],
            [0.5, -0.5, 1.],
            [0.5, 0.5, 1.],
            [-0.5, 0.5, 1.],
        ],
        vec![[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]],
    );
    let mut cache = Cache::default();
    let config = ShadowSettings {
        azimuth: 0.,
        elevation: 85.,
        threshold: 0.,
        ..Default::default()
    };
    let field = cache.field(Arc::new(m), &config, &|| false).unwrap();
    assert!(!field[0].dark);
    assert!(!field[0].cast.is_empty());
    assert!(field[2].cast.is_empty());
    let polygons: Vec<_> = field[0]
        .cast
        .iter()
        .map(|p| p.iter().map(|q| crate::p(q.x, q.y)).collect())
        .collect();
    assert!(crate::geom::inside_rings(crate::p(0.1, -0.1), &polygons));
}
#[test]
fn one_shadow_ink_covers_object_and_ground_and_exports_physical_paths() {
    let source = Arc::new(Primitive::Cube.mesh());
    let mut e = Engine::default();
    let mut s = Settings {
        recipe: RecipeKind::FlatShadow,
        ..Default::default()
    };
    let paper = crate::Paper::default();
    let drawing = e.draw(source.clone(), &s, &paper, &|| false).unwrap();
    let shadow = drawing.shadow.as_ref().unwrap();
    assert!(!shadow.dark.is_empty() && !shadow.floor.is_empty());
    assert!(!drawing.parts[0].lines.is_empty());
    assert!(drawing.parts[1].lines.is_empty() && drawing.parts[2].lines.is_empty());
    let doc = drawing.document(paper.clone(), crate::doc::default_pens(), [0, 2, 1]);
    let layers = doc.layers();
    assert_eq!(layers.len(), 1);
    let svg = crate::svg_out::export(&doc, &layers);
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
    s.shadow.ground = false;
    let without = e.draw(source, &s, &paper, &|| false).unwrap();
    assert!(without.shadow.as_ref().unwrap().floor.is_empty());
    assert!(!without.shadow.as_ref().unwrap().dark.is_empty());
}
#[test]
fn normals_are_consistent_even_if_closed_mesh_winding_is_reversed() {
    let original = Primitive::Cube.mesh();
    let expected = normals(&original, &|| false).unwrap();
    let mut reversed = original.clone();
    for (i, t) in reversed.triangles.iter_mut().enumerate() {
        if i % 3 == 0 {
            t.swap(0, 1);
        }
    }
    let actual = normals(&reversed, &|| false).unwrap();
    assert!(actual
        .iter()
        .zip(expected)
        .all(|(a, b)| (*a - b).length() < 1e-9));
}
#[test]
fn light_changes_shadow_and_camera_keeps_the_light_field_cached() {
    let m = Arc::new(Primitive::Torus.mesh());
    let mut e = Engine::default();
    let mut s = Settings {
        recipe: RecipeKind::FlatShadow,
        ..Default::default()
    };
    s.shadow.facets = 300;
    s.shadow.hatch_spacing_mm = 0.7;
    let paper = crate::Paper::default();
    let a = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    s.yaw = 95.;
    let b = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    assert!(Arc::ptr_eq(
        &a.shadow.as_ref().unwrap().faces,
        &b.shadow.as_ref().unwrap().faces
    ));
    s.shadow.azimuth += 100.;
    let c = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    assert_ne!(
        b.shadow.as_ref().unwrap().ground_world,
        c.shadow.as_ref().unwrap().ground_world
    );
    assert!(!Arc::ptr_eq(
        &b.shadow.as_ref().unwrap().faces,
        &c.shadow.as_ref().unwrap().faces
    ));
    let recipe = Recipe::new(m.as_ref().clone(), s.clone());
    assert!(Recipe::from_json(&recipe.to_json().unwrap()).unwrap() == recipe);
    assert!(e.draw(m, &s, &paper, &|| true).is_err());
    assert!(area(&c.shadow.as_ref().unwrap().dark) > 0.);
}

#[test]
fn faceting_changes_the_interpretation_and_keeps_the_original_and_output_stable() {
    let source = Arc::new(Primitive::Sphere.mesh());
    let original = source.as_ref().clone();
    let mut e = Engine::default();
    let mut s = Settings {
        recipe: RecipeKind::FlatShadow,
        ..Default::default()
    };
    s.shadow.facets = 800;
    let paper = crate::Paper::default();
    let a = e.draw(source.clone(), &s, &paper, &|| false).unwrap();
    let repeat = e.draw(source.clone(), &s, &paper, &|| false).unwrap();
    assert_eq!(a.parts[0].lines, repeat.parts[0].lines);
    s.shadow.facets = 80;
    let b = e.draw(source.clone(), &s, &paper, &|| false).unwrap();
    assert!(b.mesh.triangles.len() < a.mesh.triangles.len());
    assert_ne!(a.parts[0].lines, b.parts[0].lines);
    assert_eq!(b.display_mesh.triangles, b.mesh.triangles);
    assert_eq!(
        b.shadow.as_ref().unwrap().faces.len(),
        b.mesh.triangles.len()
    );
    assert_eq!(*source, original);
    s.recipe = RecipeKind::Atlas;
    let atlas = e.draw(source, &s, &paper, &|| false).unwrap();
    assert!(atlas.shadow.is_none());
    assert_eq!(
        *atlas.mesh,
        original.transformed(s.model_scale, s.model_rotation)
    );
    assert!(!atlas.curves.is_empty());
}
