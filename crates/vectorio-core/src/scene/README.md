# Scene engine

## Minimum use

```rust
use vectorio_core::geom::{p, BBox};
use vectorio_core::scene::{Camera, PartId, Scene, V3};

let paper = BBox { min: p(0.0, 0.0), max: p(210.0, 297.0) };
let mut camera = Camera::default();
camera.fit([V3::new(0.0, 0.0, 0.0), V3::new(4.0, 4.0, 3.0)], paper, 12.0);
let mut scene = Scene::new(camera, paper);
scene.box3(V3::new(0.0, 0.0, 0.0), V3::new(4.0, 4.0, 3.0));
let output = scene.render();
assert_eq!(output.parts[0].lines.len(), 9);
assert!(output.length_m(PartId(0)) > 0.0);
```

## Public API

Signatures of the methods of instance; the builders and `Camera::fit` receive the indicated arguments. The types do not depend on the document or egui.

```text
V3 { x: f64, y: f64, z: f64 }
V3::new(x: f64, y: f64, z: f64) -> V3
dot(other: V3) -> f64; cross(other: V3) -> V3
length() -> f64; normalized() -> V3
Operators V3 + V3, V3 - V3 and V3 * f64; Default = zero vector.

Camera { yaw_deg: f64, elev_deg: f64, scale: f64, world_center: V3, paper_center: P }
Camera::default() -> Camera
project(point: V3) -> (P, f64)
facing(normal: V3) -> bool
ground(point: P, z: f64) -> V3
fit(&mut self, world: [V3; 2], paper: BBox, margin: f64)
axes() -> (V3, V3, V3)  // right and down on the paper, and into the background

PartId(pub u16)
Tone::{Lit, Mid, Dark}
ToneThresholds { lit: f64, dark: f64 }  // Default: 0.65 and 0.15

Scene::new(camera: Camera, paper: BBox) -> Scene
camera() -> &Camera; paper() -> BBox
part(&mut self, name: &str) -> PartId
set_part(&mut self, id: PartId); part_names() -> &[String]
set_sun(&mut self, sun: Option<V3>)
set_tone_thresholds(&mut self, thresholds: ToneThresholds)
tone(normal: V3) -> Tone
set_quiet(&mut self, quiet: bool)
set_intersections(&mut self, enabled: bool)
construction_issues(&self, minimum_mm: f64) -> Vec<String>
paper_mm(meters: f64) -> f64
visible(meters: f64, minimum_mm: f64) -> bool
circle_segments(radius_m: f64) -> usize
line(&mut self, points: &[V3])
closed(&mut self, points: &[V3])
whole_line(&mut self, points: &[V3])
occluder(&mut self, points: &[V3])
hole(&mut self, points: &[V3])
hatch_hole(&mut self, points: &[V3], direction: V3, spacing_mm: f64)
solid(&mut self, vertices: &[V3], faces: &[Vec<usize>], smoothing: &[u16], quiet: bool) -> Vec<Vec<V3>>
box3(&mut self, min: V3, max: V3) -> Vec<Vec<V3>>
rotated_box(&mut self, min: V3, max: V3, yaw_deg: f64) -> Vec<Vec<V3>>
extrude(&mut self, polygon: &[V3], vector: V3) -> Vec<Vec<V3>>
lathe(&mut self, center: V3, profile: &[(f64, f64)], smooth: bool) -> Vec<Vec<V3>>
hatch_surface(&mut self, faces: &[Vec<V3>], angle_deg: f64, spacing_mm: f64)
solid_oriented(&mut self, vertices: &[V3], faces: &[Vec<usize>], smoothing: &[u16], quiet: bool) -> Vec<Vec<V3>>
hatch_face(&mut self, points: &[V3], direction: V3, spacing_mm: f64)
hatch_faces(&mut self, pieces: &[Vec<V3>], direction: V3, spacing_mm: f64)
polyhedron(&mut self, faces: &[Vec<V3>]) -> Vec<Vec<Vec<V3>>>
prism(&mut self, profile: &[V3], vector: V3) -> Vec<Vec<Vec<V3>>>
lathe_parts(&mut self, center: V3, profile: &[(f64, f64)], smooth: bool) -> Lathe
begin_shadows(&mut self, ground_z: f64, world_clip: Option<[f64; 4]>, hatch_angle_deg: f64)
hatch_shadows(&mut self, spacing_mm: f64)
load() -> Load; mark() -> Mark
clip_to_sphere(&mut self, mark: Mark, center: V3, radius: f64)
render() -> SceneOutput

Load { faces: usize, segments: usize }; Load::within_budget() -> bool
FACE_BUDGET = 8_000; SEGMENT_BUDGET = 40_000  // upper limit for a dense sheet

SceneOutput { parts: Vec<ScenePart> }
ScenePart { id: PartId, name: String, lines: Vec<Polyline> }
SceneOutput::length_m(&self, id: PartId) -> f64
```

## Decisions and limits

- Fixed camera inside each scene to maintain consistent projections and hatching patterns.
  Its angles are chosen before it's created. Elevation strictly between 0 ° and 90 °;
  Free yaw. World in meters, paper in mm; depth grows to the bottom.
- `solid` and the shortcuts turn their faces outward to facilitate
  `tone` and `hatch_face`. The active part is captured by adding each line.
  The initial part is  « contour »; names are recorded only once.
- Solids are convex, closed, and not degenerate. The API checks for indexes,
  Degenerated faces and two faces per edge; flatness and convexity are
  the preconditions of the generator. The revolution profile admits necks and convexity changes,
  with increasing heights and zero radii only at the tips. No operations are performed.
  Boolean between solids.
- `polyhedron` and `prism` support non-convex (flat and simple) logical faces:
  They break into convex pieces and return the logically grouped pieces. The
  edge between two faces of the same solid with the same normal is not drawn, so
  the partitions leave no seam; that of two distinct solids (two houses)
  - Yes, they are. `hatch_faces` hatching the pieces of one face as one.
- `lathe_parts` returns a `Lathe` with `segments`, facets in
  `bands[tramo][meridiano]` and the covers apart.
- `set_quiet` affects the following shortcuts; `solid` is given its own option.
  Curved surfaces use the same lateral softening group; covers
  They have group zero. Its edges and silhouette generators are preserved.
- It compares depth in the plane of each face and cuts intervals with
  This is Cyrus Beck. The grid of 4 mm uses seals per query and DDA per segment. It's not rasterized,
  new dependence or randomness in rendering.
- Deep tolerance: **0,002 m**. Facial composition: **1e- 9 mm**.
  The first protects coplanar faces; the second excludes edges of its own
  the occlusors. The tests check the nine edges of the cube, sticky faces,
  own hatching patterns, depth line crossing and slices with error < **1e- 6 mm**.
  The gaps between glued shutters are below the stroke minimum.
- The visible sections of the same polyline are joined by their vertices.
  The minimum **0.05 mm** shall be applied to the complete stroke after joining it.
  Repeated colineal passes are withdrawn within each part, preserving
  the first appearance (colinearity tolerance: **1e- 8 mm**). It is not deduplicated
  between the parties, nor is their route optimized. The parts can be separated into inks.
- The sun points **towards the** light source and must be above the horizon. Each
  group captures the sun when it opens; opening another discards the slope without drawing. Without the sun, you can't open a group. Shadows are cut in the world and fused at intervals.
  in the leaf; the hatching has a global phase to align groups at equal angles and
  separation. The angle is measured in degrees of paper, positive down.
- If a solid crosses the floor plane, it only shades its upper portion.
  Objects below the plane do not cast shadow. `occluder` by itself not
  It casts a shadow. Without the sun, `tone` returns `Mid`.
- A `hole` (window, door) is not an occlusor: it erases non-atomic strokes
  which falls within its polygon and at its depth (± **0.01 m**), i.e. the
  hatching and the shadows of his wall. What's in front or behind is still visible, so
  That a hole in the back façade doesn't pierce other buildings. They are preserved
  the strokes of `whole_line` (frames) and the fill of `hatch_hole`.
- The `generate` module provides generators, style and kit. Interface and persistence
  They live outside the engine; this layer is still not dependent on egui or the document.

## Budget and cuts

`load` counts the faces and stroke segments carrying the scene; a dense sheet must not pass from `FACE_BUDGET` faces or `SEGMENT_BUDGET` segments. With a lot of people, you get to the top of segments sooner than you do face-to-face.

`mark` keeps track of where the scene ends at that moment and `clip_to_sphere` discards what is added after it is outside a sphere: faces, gaps and shadows in its center, and strokes cut right at the edge. It's used by the glass dome of the kit so that the inside doesn't look like the outside of the glass.

## Verification and example

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo test -p vectorio-core --release scene::tests::benchmark_5000_faces_20000_segments -- --ignored --nocapture
cargo run -p vectorio-core --example muestra_escena --release -- muestra_escena.svg --dimetric
```

The benchmark measures only `render`, including grid and repeat pass removal. Use 5,000 faces distributed by A4 and 20,000 segments of 7  –  11 mm, with visible and hidden candidates. Run seven repetitions and report median, minimum and maximum; require median below 100 ms. He doesn't intend to cover the worst case of thousands of faces overlapping in one cell.

The example accepts an exit route. `--dimetric` or `VECTORIO_DIMETRIC=1` adds `*_dimetrica.svg`; both SVGs retain real mm and independent groups per side.

Degenerate entries are discarded without panic: non-flat or concave faces, empty boxes, impossible profiles and infinite values. A scene with an invalid camera or area remains empty. `Camera::project` and `ground` return the origin for invalid entries. The direction of `hatch_face` is projected to the plane. `hatch_surface` joins contiguous facets with continuous depth and paper angle. `solid` omits rear convex-volume shutters; `solid_oriented` respects the outer turn and retains all shutters for non-convex volumes. Faces with a projected width of less than 1e-6 mm do not hide. As a resource protection, there are up to 8,192 segments per circle and 200,000 lines per hatching; the huge areas adapt cell size (up to 1024 per axis). The `benchmark_long_diagonals_and_shaded_boxes` test complements the short segment test with 500 truncated boxes, shadows and long diagonal 2.000. Only in tests, `lit_wall_fraction(part, step_mm)` samples the sheet and returns the visible vertical wall fraction of that part left in Lit, that is, without hatching.

## Meeting between volumes

Each valid solid receives an identifier, even if it is `quiet`. Loose faces don't belong to a solid. By default, `render` searches for encounters between front faces of distinct solids with common sheet boxes and depth. The same grid avoids comparing couples who don't share cells.

The straight `depth_a(x,y) = depth_b(x,y)` is calculated on the sheet and cut against both polygons. Parallel/coplanar planes, sections below 0.05 mm and encounters that are already covered by an emitted edge of those faces are excluded. The latter is verified on really drawable edges; the smooth joints between facets do not prevent drawing a legitimate intersection.

The adjacent segments of each pair of solids form polylines, with end joints at 1e-7 mm and depth at 1e-8 m. They go through the concealment and removal of the usual repeat passes. They are added after the requested strokes, on the active part when building the **most recent** solid. The different parts retain their identity even if they are assigned the same ink.

`set_intersections(false)` switches off the encounter output and excludes added solids from future encounters while it is off. `quiet` solids only hide and never participate in the calculation of encounters.

`construction_issues` is an explicit diagnostic of the kit, outside of the render: look for faces of normal size with a common footprint, coplanar or separated less than the minimum indicated on paper. Opposite normals in contact are legal. The test uses the front faces that can intervene in the drawing; it is not intended to validate arbitrary volume booleans or replace the design of the pieces. White village applies it in purification tests and compilations.
