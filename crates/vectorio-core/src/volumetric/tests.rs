use super::*;
use std::sync::Arc;
fn section(m: &Mesh, n: V3, levels: &[f64]) -> Vec<Contour> {
    let v = field_values(m, Field::Height, n, &Camera::default());
    isolines(m, &v, levels, &|| false).unwrap()
}
#[test]
fn cube_cuts_horizontal_vertical_oblique_and_coplanar() {
    let m = Primitive::Cube.mesh();
    for n in [
        V3::new(0., 0., 1.),
        V3::new(1., 0., 0.),
        V3::new(1., 2., 3.).normalized(),
    ] {
        let curves = section(&m, n, &[0.]);
        assert_eq!(curves.len(), 1);
        let c = &curves[0].points;
        assert_eq!(c.first(), c.last());
        assert!(c.iter().all(|p| p.dot(n).abs() < 1e-8));
    }
    let c = section(&m, V3::new(0., 0., 1.), &[-1., 1.]);
    assert_eq!(c.len(), 2);
    assert!(c.iter().all(|c| c.points.len() == 5));
}
#[test]
fn sphere_continuity_orientations_and_density() {
    let m = Primitive::Sphere.mesh();
    for n in [
        V3::new(0., 0., 1.),
        V3::new(0., 1., 0.),
        V3::new(1., 2., 3.).normalized(),
    ] {
        let c = section(&m, n, &[-0.6, -0.2, 0.2, 0.6]);
        assert_eq!(c.len(), 4);
        for c in c {
            assert_eq!(c.points.first(), c.points.last());
            assert!(c.points.iter().all(|p| (p.dot(n) - c.value).abs() < 1e-8));
        }
    }
    assert_eq!(
        section(
            &m,
            V3::new(0., 0., 1.),
            &[-0.8, -0.6, -0.4, -0.2, 0.2, 0.4, 0.6, 0.8]
        )
        .len(),
        8
    );
}
#[test]
fn torus_has_holes_and_changes_connectivity() {
    let m = Primitive::Torus.mesh();
    let n = V3::new(0., 0., 1.);
    let c = section(&m, n, &[0.]);
    assert_eq!(c.len(), 2);
    assert!(c.iter().all(|c| c.points.first() == c.points.last()));
    assert_eq!(section(&m, V3::new(1., 0., 0.), &[0.]).len(), 2);
    assert_eq!(section(&m, V3::new(1., 0., 0.), &[0.8]).len(), 1);
}
#[test]
fn obj_concave_negative_indices_objects_and_defects() {
    let (m, r) = parse_obj(
        "o ala\nv 0 0 0\nv 2 0 0\nv 2 2 0\nv 1 1 0\nv 0 2 0\nf -5 -4 -3 -2 -1\nf 1 1 2\n",
        "concavo",
    )
    .unwrap();
    assert_eq!(m.triangles.len(), 3);
    assert_eq!(r.removed, 1);
    assert_eq!(r.boundary_edges, 5);
    assert_eq!(m.objects[1], "ala");
    for text in ["v NaN 0 0\nf 1 1 1", "v 0 0 0\nf 1 2 3", "v 0 0 0\nf 0 0 0"] {
        assert!(parse_obj(text, "mal").is_err());
    }
    let mut mesh = Primitive::Cube.mesh();
    mesh.triangles.push([0, 0, 1]);
    mesh.triangles.push(mesh.triangles[0]);
    let (m, r) = mesh.validated().unwrap();
    assert_eq!(r.removed, 2);
    assert_eq!(m.triangles.len(), 12);
}
#[test]
fn open_disconnected_and_duplicate_vertices_are_not_forced_closed() {
    let (m, r) = parse_obj(
        "v -1 0 -1\nv 1 0 1\nv -1 0 1\nv 3 0 -1\nv 5 0 1\nv 3 0 1\nf 1 2 3\nf 4 5 6\n",
        "abierto",
    )
    .unwrap();
    assert_eq!(r.boundary_edges, 6);
    let c = section(&m, V3::new(0., 0., 1.), &[0.]);
    assert_eq!(c.len(), 2);
    assert!(c.iter().all(|c| c.points.first() != c.points.last()));
    let cube = Primitive::Cube.mesh();
    let mut duplicate = cube.clone();
    duplicate.vertices = cube
        .triangles
        .iter()
        .flatten()
        .map(|i| cube.vertices[*i])
        .collect();
    duplicate.triangles = (0..12).map(|i| [i * 3, i * 3 + 1, i * 3 + 2]).collect();
    assert_eq!(duplicate.clone().validated().unwrap().1.boundary_edges, 0);
    assert_eq!(section(&duplicate, V3::new(0., 0., 1.), &[0.]).len(), 1);
}
#[test]
fn edges_omit_triangulation_and_views_are_orthographic() {
    let cube = Primitive::Cube.mesh();
    assert_eq!(feature_edges(&cube, &Camera::default(), 25.).len(), 12);
    let camera = Camera::default();
    let a = V3::new(1., 2., 3.);
    let b = a + camera.axes().2 * 4.;
    assert!(camera.project(a).0.dist(camera.project(b).0) < 1e-9);
    for yaw in [0., 45., 90., 180.] {
        let c = Camera {
            yaw_deg: yaw,
            ..camera
        };
        assert!(feature_edges(&cube, &c, 25.).len() >= 12);
    }
}
#[test]
fn export_is_finite_physical_layered_deterministic_and_clipped() {
    let m = Arc::new(Primitive::Torus.mesh());
    let mut engine = Engine::default();
    let mut settings = Settings::default();
    let paper = crate::Paper::default();
    for kind in [RecipeKind::Atlas, RecipeKind::Strata, RecipeKind::XRay] {
        settings.recipe = kind;
        settings.zoom = 2.;
        settings.position_mm = [90., -40.];
        settings.paper_rotation = 23.;
        let a = engine
            .draw(m.clone(), &settings, &paper, &|| false)
            .unwrap();
        let doc = a.document(paper.clone(), crate::doc::default_pens(), [0, 2, 1]);
        let layers = doc.layers();
        let svg = crate::svg_out::export(&doc, &layers);
        assert!(!layers.is_empty());
        assert!(svg.contains("width=\"420mm\"") && svg.contains("height=\"297mm\""));
        assert!(svg.contains("inkscape:groupmode=\"layer\""));
        assert!(!svg.contains("<image") && !svg.contains("NaN") && !svg.contains("inf"));
        assert!(usvg::Tree::from_str(&svg, &usvg::Options::default()).is_ok());
        for p in layers.iter().flat_map(|l| &l.paths).flatten() {
            assert!(p.x.is_finite() && p.y.is_finite());
            assert!(
                p.x >= paper.margin_mm - 1e-7
                    && p.y >= paper.margin_mm - 1e-7
                    && p.x <= paper.width_mm - paper.margin_mm + 1e-7
                    && p.y <= paper.height_mm - paper.margin_mm + 1e-7
            );
        }
        let b = Engine::default()
            .draw(m.clone(), &settings, &paper, &|| false)
            .unwrap();
        assert_eq!(
            a.parts.iter().map(|p| &p.lines).collect::<Vec<_>>(),
            b.parts.iter().map(|p| &p.lines).collect::<Vec<_>>()
        );
    }
}
#[test]
fn occlusion_splits_back_sections_without_normal_culling() {
    let m = Arc::new(Primitive::Sphere.mesh());
    let mut e = Engine::default();
    let paper = crate::Paper::default();
    let mut s = Settings {
        cuts: 16,
        ..Default::default()
    };
    s.visible_only = false;
    let all = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    s.visible_only = true;
    let visible = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    let length = |d: &Drawing| {
        d.parts
            .iter()
            .flat_map(|p| &p.lines)
            .map(|l| crate::geom::length(l))
            .sum::<f64>()
    };
    assert!(length(&visible) > length(&all) * 0.3);
    assert!(length(&visible) < length(&all) * 0.8);
    let mut flipped = m.as_ref().clone();
    for t in &mut flipped.triangles {
        t.swap(0, 1);
    }
    let v = e.draw(Arc::new(flipped), &s, &paper, &|| false).unwrap();
    assert!(
        (length(&visible) - length(&v)).abs() < 1e-5,
        "visible {} flipped {}",
        length(&visible),
        length(&v)
    );
}
#[test]
fn recipe_roundtrip_cancellation_invalid_limits_and_camera_field() {
    let recipe = Recipe::new(Primitive::Cube.mesh(), Settings::default());
    let r = Recipe::from_json(&recipe.to_json().unwrap()).unwrap();
    assert_eq!(recipe, r);
    let mut e = Engine::default();
    let m = Arc::new(r.mesh);
    assert!(e
        .draw(m.clone(), &r.settings, &crate::Paper::default(), &|| true)
        .is_err());
    let mut s = r.settings;
    s.cuts = usize::MAX;
    assert!(e
        .draw(m.clone(), &s, &crate::Paper::default(), &|| false)
        .is_err());
    s = Settings {
        field: Field::CameraDepth,
        ..Default::default()
    };
    let a = e
        .draw(m.clone(), &s, &crate::Paper::default(), &|| false)
        .unwrap();
    s.yaw = 100.;
    let b = e.draw(m, &s, &crate::Paper::default(), &|| false).unwrap();
    assert_ne!(a.curves[0].points, b.curves[0].points);
}
#[test]
fn cache_reuses_curves_for_color_composition_and_camera_but_not_model_or_field() {
    let m = Arc::new(Primitive::Torus.mesh());
    let mut e = Engine::default();
    let mut s = Settings::default();
    let paper = crate::Paper::default();
    let a = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    s.yaw = 80.;
    s.zoom = 1.2;
    s.paper_rotation = 40.;
    let b = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    assert!(Arc::ptr_eq(&a.mesh, &b.mesh) && Arc::ptr_eq(&a.curves, &b.curves));
    s.model_rotation[1] = 30.;
    let c = e.draw(m.clone(), &s, &paper, &|| false).unwrap();
    assert!(!Arc::ptr_eq(&b.mesh, &c.mesh) && !Arc::ptr_eq(&b.curves, &c.curves));
    s.field = Field::CameraDepth;
    let d = e.draw(m, &s, &paper, &|| false).unwrap();
    assert!(!Arc::ptr_eq(&c.curves, &d.curves));
}
#[test]
fn all_primitives_validate_and_non_manifold_input_is_reported() {
    for primitive in [
        Primitive::Cube,
        Primitive::Sphere,
        Primitive::Cylinder,
        Primitive::Cone,
        Primitive::Plane,
        Primitive::Torus,
        Primitive::Assembly,
    ] {
        let m = primitive.mesh();
        assert!(m.vertices.len() < MAX_VERTICES && m.triangles.len() < MAX_FACES);
        assert!(m.validated().is_ok());
    }
    let mesh = Mesh {
        name: "ramificado".into(),
        vertices: vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [0., -1., 0.],
        ],
        triangles: vec![[0, 1, 2], [0, 1, 3], [0, 1, 4]],
        objects: vec![],
        face_objects: vec![],
    };
    assert_eq!(mesh.validated().unwrap().1.non_manifold_edges, 1);
}

fn binary_stl(m: &Mesh) -> Vec<u8> {
    let mut bytes = vec![0; 80];
    bytes[..12].copy_from_slice(b"solid binary");
    bytes.extend((m.triangles.len() as u32).to_le_bytes());
    for t in &m.triangles {
        for _ in 0..3 {
            bytes.extend(0f32.to_le_bytes());
        }
        for i in t {
            for v in m.vertices[*i] {
                bytes.extend((v as f32).to_le_bytes());
            }
        }
        bytes.extend(0u16.to_le_bytes());
    }
    bytes
}
fn ascii_stl(m: &Mesh) -> String {
    let mut s = String::from("solid prueba\n");
    for t in &m.triangles {
        s.push_str("facet normal 0 0 0\nouter loop\n");
        for i in t {
            let [x, y, z] = m.vertices[*i];
            s.push_str(&format!("vertex {x} {y} {z}\n"));
        }
        s.push_str("endloop\nendfacet\n");
    }
    s.push_str("endsolid prueba\n");
    s
}
#[test]
fn stl_binary_and_ascii_cube_have_closed_topology_and_real_sections() {
    let cube = Primitive::Cube.mesh();
    let bin = binary_stl(&cube);
    let ascii = ascii_stl(&cube);
    for bytes in [&bin[..], ascii.as_bytes()] {
        let (m, r) = parse_stl(bytes, "cubo").unwrap();
        assert_eq!(m.triangles.len(), 12);
        assert_eq!(r.boundary_edges, 0);
        assert_eq!(r.non_manifold_edges, 0);
        let c = section(&m, V3::new(0., 0., 1.), &[0.]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].points.first(), c[0].points.last());
        let drawing = Engine::default()
            .draw(
                Arc::new(m),
                &Settings::default(),
                &crate::Paper::default(),
                &|| false,
            )
            .unwrap();
        assert!(drawing.parts.iter().any(|p| !p.lines.is_empty()));
    }
}
#[test]
fn stl_truncation_counts_nonfinite_and_ascii_state_are_checked() {
    let cube = Primitive::Cube.mesh();
    let mut bin = binary_stl(&cube);
    bin.truncate(bin.len() - 1);
    assert!(parse_stl(&bin, "truncado").is_err());
    let mut bin = binary_stl(&cube);
    bin[96..100].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(parse_stl(&bin, "nan").is_err());
    let mut bytes = vec![0; 84];
    bytes[80..84].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(parse_stl(&bytes, "contador").is_err());
    for text in ["solid x\nfacet normal 0 0 0\nouter loop\nvertex 0 0 0\nendsolid x","solid x\nfacet normal 0 0 0\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet", "solid x\nendsolid x"]{assert!(parse_stl(text.as_bytes(),"mal").is_err());}
    let count = MAX_FACES + 1;
    let mut big = vec![0; 84 + 50 * count];
    big[80..84].copy_from_slice(&(count as u32).to_le_bytes());
    assert!(parse_stl(&big, "grande")
        .unwrap_err()
        .contains("5 millones"));
}
#[test]
fn stl_ascii_preserves_multiple_solids_and_reports_open_geometry() {
    let tri = Mesh {
        name: "cara".into(),
        vertices: vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
        triangles: vec![[0, 1, 2]],
        objects: vec![],
        face_objects: vec![],
    };
    let mut second = tri.clone();
    for p in &mut second.vertices {
        p[2] += 1.;
    }
    let text = ascii_stl(&tri) + &ascii_stl(&second);
    let (m, r) = parse_stl(text.as_bytes(), "dos").unwrap();
    assert_eq!(m.objects.len(), 2);
    assert_eq!(m.face_objects, vec![0, 1]);
    assert_eq!(r.boundary_edges, 6);
}

#[test]
fn curved_mesh_recipes_preserve_floating_point_coordinates_exactly() {
    let mut recipe = Recipe::new(
        Primitive::Torus.mesh(),
        Settings {
            yaw: 36.197431226719,
            elevation: 52.691271283617,
            ..Default::default()
        },
    );
    recipe.inks = vec![crate::Pen::new("Índigo", [53, 33, 108], 0.1764)];
    let restored = Recipe::from_json(&recipe.to_json().unwrap()).unwrap();
    assert!(
        recipe == restored,
        "la receta debe conservar exactamente los parámetros y coordenadas"
    );
    let a = Engine::default()
        .draw(
            Arc::new(recipe.mesh),
            &recipe.settings,
            &recipe.paper,
            &|| false,
        )
        .unwrap();
    let b = Engine::default()
        .draw(
            Arc::new(restored.mesh),
            &restored.settings,
            &restored.paper,
            &|| false,
        )
        .unwrap();
    assert!(a
        .parts
        .iter()
        .zip(&b.parts)
        .all(|(a, b)| a.lines == b.lines));
}

#[test]
fn disk_fixtures_obj_ascii_and_binary_stl_use_the_same_pipeline() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/volumetric");
    for file in ["cube.obj", "cube-ascii.stl", "cube-binary.stl"] {
        let (m, r) = load_mesh(&root.join(file)).unwrap();
        assert_eq!(m.triangles.len(), 12);
        assert_eq!(r.boundary_edges, 0);
        let c = section(&m, V3::new(0., 0., 1.), &[0.]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].points.first(), c[0].points.last());
    }
}

fn subdivided_sphere(rounds: usize) -> Mesh {
    let mut m = Primitive::Sphere.mesh();
    for _ in 0..rounds {
        let mut triangles = Vec::new();
        let mut edges = std::collections::HashMap::new();
        for t in &m.triangles {
            let mut midpoint = |a: usize, b: usize| {
                *edges.entry((a.min(b), a.max(b))).or_insert_with(|| {
                    let q = (v3(m.vertices[a]) + v3(m.vertices[b])).normalized();
                    let i = m.vertices.len();
                    m.vertices.push(arr(q));
                    i
                })
            };
            let a = midpoint(t[0], t[1]);
            let b = midpoint(t[1], t[2]);
            let c = midpoint(t[2], t[0]);
            triangles.extend([[t[0], a, c], [a, t[1], b], [c, b, t[2]], [a, b, c]]);
        }
        m.triangles = triangles;
    }
    m
}
#[test]
fn large_mesh_quality_is_reversible_and_original_geometry_is_preserved() {
    let source = Arc::new(subdivided_sphere(2).validated().unwrap().0);
    let before = source.as_ref().clone();
    assert!(source.triangles.len() > 40_000);
    let mut engine = Engine::default();
    let mut settings = Settings {
        cuts: 6,
        visible_only: false,
        ..Default::default()
    };
    let paper = crate::Paper::default();
    let interactive = engine
        .draw(source.clone(), &settings, &paper, &|| false)
        .unwrap();
    assert!(interactive.mesh.triangles.len() < source.triangles.len());
    assert!(interactive.display_mesh.triangles.len() <= interactive.mesh.triangles.len());
    assert!(interactive
        .curves
        .iter()
        .all(|c| c.points.first() == c.points.last()));
    settings.mesh_detail = MeshDetail::Original;
    let original = engine
        .draw(source.clone(), &settings, &paper, &|| false)
        .unwrap();
    assert_eq!(original.mesh.triangles.len(), source.triangles.len());
    assert!(original.display_mesh.triangles.len() < original.mesh.triangles.len());
    assert!(source.as_ref() == &before);
    let recipe = Recipe::new(source.as_ref().clone(), settings);
    let restored = Recipe::from_json(&recipe.to_json().unwrap()).unwrap();
    assert_eq!(restored.mesh.triangles.len(), before.triangles.len());
    assert_eq!(restored.settings.mesh_detail, MeshDetail::Original);
}
#[test]
fn cancellation_interrupts_large_validation_and_loading() {
    let m = subdivided_sphere(1);
    assert!(m.validated_cancellable(&|| true).is_err());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/volumetric/cube-binary.stl");
    assert!(load_mesh_cancellable(&root, &|| true).is_err());
}

#[test]
fn pass_cache_survives_metadata_reordering_and_only_recomputes_the_changed_program() {
    let source = Arc::new(Primitive::Torus.mesh());
    let settings = Settings::default();
    let paper = crate::Paper::default();
    let mut passes = settings.passes();
    passes.truncate(2);
    // Dar parámetros diferentes a las dos pasadas permite contar su trabajo por separado.
    if let PassKind::Field(p) = &mut passes[1].kind {
        p.cuts = 21;
    }
    let mut engine = Engine::default();
    let first = engine
        .draw_passes(source.clone(), &settings, &passes, &paper, &|| false)
        .unwrap();
    let count = engine.renders;
    assert_eq!(count, 2);
    passes[0].name = "Tinta de prueba".into();
    passes[0].pen = 8;
    passes.reverse();
    let reordered = engine
        .draw_passes(source.clone(), &settings, &passes, &paper, &|| false)
        .unwrap();
    assert_eq!(engine.renders, count);
    assert_eq!(first.parts[0].lines, reordered.parts[1].lines);
    assert_eq!(reordered.items([])[1].stroke, Some(8));
    passes[1].enabled = false;
    let disabled = engine
        .draw_passes(source.clone(), &settings, &passes, &paper, &|| false)
        .unwrap();
    assert_eq!(engine.renders, count);
    assert!(disabled.parts[1].lines.is_empty());
    passes[1].enabled = true;
    if let PassKind::Field(p) = &mut passes[1].kind {
        p.cuts += 1;
    }
    let edited = engine
        .draw_passes(source.clone(), &settings, &passes, &paper, &|| false)
        .unwrap();
    assert_eq!(engine.renders, count + 1);
    assert_eq!(edited.parts[0].lines, reordered.parts[0].lines);
    assert_ne!(edited.parts[1].lines, reordered.parts[1].lines);
    passes[0].visibility = Visibility::All;
    let all = engine
        .draw_passes(source, &settings, &passes, &paper, &|| false)
        .unwrap();
    // La oclusión cambia; el campo y las isolíneas tridimensionales se reutilizan.
    assert!(Arc::ptr_eq(&edited.curves, &all.curves));
    assert_ne!(edited.parts[0].lines, all.parts[0].lines);
}

#[test]
fn empty_stack_ground_pass_and_invalid_passes_are_handled() {
    let source = Arc::new(Primitive::Cube.mesh());
    let settings = Settings::default();
    let paper = crate::Paper::default();
    let mut engine = Engine::default();
    let empty = engine
        .draw_passes(source.clone(), &settings, &[], &paper, &|| false)
        .unwrap();
    assert!(empty.parts.is_empty() && empty.items([]).is_empty());
    let ground = Pass {
        name: "Suelo".into(),
        pen: 12,
        enabled: true,
        visibility: Visibility::Visible,
        mask: TonalMask::All,
        dash: DashPattern::default(),
            reading: None,
            finish: FinishSettings::default(),
        kind: PassKind::Ground(ShadowSettings::default()),
    };
    let drawn = engine
        .draw_passes(source.clone(), &settings, &[ground], &paper, &|| false)
        .unwrap();
    assert!(!drawn.parts[0].lines.is_empty());
    assert_eq!(drawn.items([])[0].stroke, Some(12));
    let mut passes = settings.passes();
    passes[0].kind = PassKind::Mesh(MeshParams {
        facets: 400,
        coloring: MeshColor::Height,
        inks: 8,
        group: 8,
    });
    assert!(engine
        .draw_passes(source.clone(), &settings, &passes, &paper, &|| false)
        .is_err());
    assert!(engine
        .draw_passes(source, &settings, &[], &paper, &|| true)
        .is_err());
}

#[test]
fn mixed_shadow_and_edges_share_a_frame_independent_of_pass_order() {
    let source = Arc::new(Primitive::Cube.mesh());
    let scene = Settings::default();
    let paper = crate::Paper::default();
    let shadow = ShadowSettings {
        outline: true,
        ..Default::default()
    };
    let outline = Pass {
        name: "Contorno".into(),
        enabled: true,
        pen: 1,
        visibility: Visibility::Visible,
        mask: TonalMask::All,
        dash: DashPattern::default(),
            reading: None,
            finish: FinishSettings::default(),
        kind: PassKind::Outline(OutlineParams::Shadow(shadow.clone())),
    };
    let edges = Pass {
        name: "Aristas".into(),
        enabled: true,
        pen: 2,
        visibility: Visibility::Visible,
        mask: TonalMask::All,
        dash: DashPattern::default(),
            reading: None,
            finish: FinishSettings::default(),
        kind: PassKind::Edges { angle: 25. },
    };
    let mut engine = Engine::default();
    let a = engine
        .draw_passes(
            source.clone(),
            &scene,
            &[outline.clone(), edges.clone()],
            &paper,
            &|| false,
        )
        .unwrap();
    let count = engine.renders;
    let b = engine
        .draw_passes(source, &scene, &[edges, outline], &paper, &|| false)
        .unwrap();
    assert_eq!(engine.renders, count);
    assert_eq!(a.camera, b.camera);
    let bounds = |lines: &[crate::Polyline]| crate::BBox::of(lines);
    assert_eq!(bounds(&a.parts[0].lines), bounds(&a.parts[1].lines));
    assert_eq!(a.parts[0].lines, b.parts[1].lines);
    assert_eq!(a.parts[1].lines, b.parts[0].lines);
}
