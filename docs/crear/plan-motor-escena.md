# Task: isometric scene engine (Create panel, Phase 1)

You're a programming agent working on **Vector**, a desktop application in Rust for drawing with plotter. The core is in `crates/vectorio-core` (without interface) and the interface egui in `crates/vectorio-app`. Your job on this assignment is **just the core**: an engine that converts a simple 3D scene into 2D lines ready for the plotter, with the hidden lines exactly removed. Then I'll review your work with another agent, so leave the code clear, proven and well explained.

## Context: What is it for?

We're going to add a third mode to the app, **Create**, along with Design and Output. It will have procedural drawing generators: isometric scenes (a village, a castle, a stadium...) and other 2D generators. Each scene is planned in meters, constructed with simple 3D pieces (boxes, prisms, roofs, cylinders, cones) and drawn with a orthographic camera. This assignment is the basis of all this: the scene engine. **No** still do the panel, the generators or the integration with the document; they'll come in later phases.

## The rules

- Read `AGENTS.md` before we start.
- **Original work.** The idea is inspired by the SVGeometric project (`jarvisar/geometric`), which **does not have a** license. You don't open, copy or translate their code. Implement on the basis of the description of this document.
- Identifiers in English, such as the rest of the core (`Document`, `Item`, `fill`, `order`...). Comments and documentation in **Spanish**, in the tone of the existing code (short phrases explaining why).
- Use the guys that already exist, `vectorio_core::geom::{P, p, Polyline, BBox, Rng}`. Everything that comes out of the engine is in **millimeters over the blade, with and down**, like the rest of the core.
- **No new dependencies.** The algorithm described below does not need any polygon-to-polygon operations library.
- Do not touch `crates/vectorio-app` or other core modules except to record the new module in `crates/vectorio-core/src/lib.rs`.
- **Do not run `cargo fmt`**: the project uses dense lines on purpose (up to a few 150 characters) and the formatting would contaminate the entire diff.
- If `cargo` is not available: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- He works on a new branch, `feat/crear-motor-escena`, created from the current branch. Do not commit unless the user asks for it. If you ask, the message in Spanish and ending with the co-authoring line that tells you your surroundings.
- Do not close or restart the Vector that is open.

## What to Build

A new `crates/vectorio-core/src/scene/` module (with `mod.rs` and the files you see reasonable, e.g. `camera.rs`, `solids.rs`, `hidden.rs`, `hatch.rs`, `shadow.rs`). Register `pub mod scene;` to `lib.rs`.

### 1. Basic types

- `V3 { x, y, z }` in **meters**, z upwards, with minimum vector operations (sum, subtract, scale, scalar and vector product, normalize).
- `Camera`: orthographic, defined by
  - `yaw_deg`: rotate around the vertical,
  - `elev_deg`: slope downwards from the horizon (35,264 ° is true isometric; 30 ° and 26.57 ° give usual dimetrics),
  - `scale`: millimeters of paper per metre,
  - a center of the scene in meters and the center of the sheet in mm.

  Methods:
  - `project(V3) -> (P, f64)`: point on the leaf (mm, and below) and depth in metres (greater = further).
  - `facing(normal) -> bool`: if a face with that normal exterior looks at the camera.
  - `ground(P, z) -> V3`: the point at the height `z` that you see in that position of the leaf. It'll use the generators to cut the world off from what you see.
  - `fit(bbox_mundo, área_hoja, margen)`: Choose scale and center so that a box of the world fits into a rectangle of the sheet.
- Parts: each line belongs to a **part** with a name (e.g.  « contour »,  « carpet »,  « shadow »,  « hatching clear »). Use a `PartId(u16)` and a table of names in the scene: `scene.part("tejados") -> PartId` returns the same id for the same name. There is an active part (`scene.set_part(id)`) that applies to everything added afterwards. This is what will later become inks: the user can move each part to another ink, so don't mix parts.

### 2. The scene

`Scene` keeps the camera, the sheet area (to cut), the faces they hide and the lines they draw. Minimum API (names are indicative; choose the best and document them):

- `line(&[V3])`: an open polyline drawn where the tape does not move.
- `closed(&[V3])`: closed polyline.
- `whole_line(&[V3])`: a small detail that is drawn **whole or nothing**. If any part is covered or leaves the leaf, it is discarded, because a cutting detail is ugly.
- `occluder(&[V3])`: plane and convex polygon that only hides, without drawing anything.
- `solid(vertices, caras, grupos_suavizado)`: solid convex closed from faces given as indexes (accepts any direction of rotation; orient them outward with the centre). Every face is hidden. The edges are drawn like this:
  - an edge is drawn only if **at least one** of its two faces looks at the camera;
  - If the two faces share a softening group greater than 0 (facets of a curved surface), the edge is drawn only if it is **silhouette**, i.e. one face looks at the camera and the other does not;
  - Each edge emits **only once**, even if it is shared by two faces. Duplicate edges are the most typical mistake: the plotter would draw them twice.
  - `quiet` option to hide the solid without drawing edges.
- Shortcuts constructed on `solid`:
  - `box3(min, max)` (box aligned with the axles) and a vertical rotating variant;
  - `extrude(polígono_convexo, vector)`: prisms, walls, double water roofs;
  - `lathe(centro, perfil: &[(radio, altura)], suave: bool)`: solid with vertical revolution (cylinders, cones, domes, towers). A radio 0 is a vertex. The number of segments depends on the projected radius, so that the string error does not exceed a few 0.04 mm on the paper. `suave = true` uses softening groups so that only the silhouette is seen; with `false` all facets are seen (useful for trees or faceted style umbrellas). With good segments, the polygonal silhouette is sufficient at this stage.
- **Detail according to the** scale, so the generators do not draw what you won 't see:
  - `paper_mm(metros) -> mm`;
  - `visible(metros, mínimo_mm) -> bool`, to skip details too small for the pen;
  - `circle_segments(radio_m) -> usize`.

### 3. Accurate removal of hidden lines

This is the heart of the engine. Do not use z-buffer or rasterized: in the plotter leave lines cut into pieces. The method:

1. **Faces.** Each face is projected onto the leaf. Keep it:
   - its convex polygon on the screen, as semi-planes;
   - its box on the leaf;
   - its minimum depth;
   - Its **plane of depth over the** screen: `profundidad = a·x + b·y + c`, taken from three non-aligned vertices.

   The singing faces (nearly zero projected area) hide nothing and are discarded.
2. **Spatial Index.** A uniform grid over the sheet (cells of about 4 mm) with the list of faces touching each cell. When consulting a segment, go through the cells of your box and avoid checking the same face twice (a stamp per query instead of a `HashSet`).
3. **For each segment** A →B of each line (with its depth at the ends, which varies linearly along the segment):
   - discard faces whose minimum depth is not less than the maximum of the segment less the tolerance;
   - cuts the segment against the face polygon with **Cyrus-Beck**, giving a `[t0, t1]` interval;
   - Within that interval, just stay with the part where the face is **closer to** than the segment (the difference in depth is linear in `t`, so it's another slice);
   - accumulates the hidden intervals, arranges them and releases the gaps remaining between them;
   - connects the consecutive visible stretches across the vertices of the polyline, so that an uninterrupted line emerges as **a single stroke**.
4. **Tolerance.** Two are missing, explained in the code:
   - a depth (of the order of 2 mm in metres): a face must be at least this close to conceal;
   - A tiny shrinkage of the polygon of the face on the screen, so that a face never hides its own edges or those of its neighbor.

   The evidence must demonstrate that legitimate edges are not lost and that no false gaps appear.
5. **Cut the** sheet, discard less than 0.05 mm and apply the `whole_line` rule.

### 4. Hatching patterns on faces and light

- `hatch_face(&[V3] cara, dirección_mundo, separación_mm)`: parallel lines on a flat and convex face, **separated as indicated on paper** whatever the angle of the face with respect to the camera. The simplest thing: project the face to the leaf, hatching the 2D polygon at the angle that gives the projection of `dirección_mundo` and retrieving the depth of each point with the depth plane of the face. Hatching lines go through the elimination of occult like any other, so one face covered by another loses its hatching. They only make faces that look at the camera.
- Light: `Scene` has an optional sun (direction). A `tone(normal) -> Tone` assistant classifies each face into `Lit`, `Mid` or `Dark` according to the light, with configurable thresholds. So a generator can assign the part and separation of the hatching according to the tone: clear top, middle side, dark side.

### 5. Cast shadows

- `begin_shadows(z_suelo, recorte_mundo: Option<[x0, y0, x1, y1]>, ángulo_trama)`: From here, each solid that is added projects its shadow over the horizontal plane `z_suelo` following the sun. For a solid convex, the shadow is the **convex wrapping** from their projected peaks.
- `hatch_shadows(separación_mm)`: fill all the shadows in the group with parallel lines on the floor. **Shadows that overlap merge** so as not to draw twice: go through the hatching lines and, in each one, combine the intervals each shadow provides before emitting. Since they are convex polygons, no general operation between polygons is required. The resulting lines are in the plane of the ground and pass through the elimination of occult, so that the objects in front cover them. The lines of different groups with the same angle should be aligned, so that neighbouring shadows are woven together as one.

### 6. Get out.

`scene.render() -> SceneOutput`, with the visible polylines **by**, in mm. Within each part, it retains the order in which the lines were added. Do not optimize the route: that already does `vectorio_core::optimize` (`order`, `join`) later in the pipeline. It includes in `SceneOutput` a method for the total line length per part, in metres. It serves to warn of drawings that saturate the paper: the target is from 1 to 40 m per sheet, and above 80 m the paper suffers.

## Compulsory testing

Unit tests on the module itself (`#[cfg(test)]`), as in the rest of the core:

1. **Cube in isometric:** exactly 9 edges visible (external hexagon and inner 3), without duplicates, with the expected total length.
2. **Box behind box:** the capped edge is cut at the exact point (compared to the hand-calculated value, with a tolerance of 1e- 6 mm).
3. **No self-hidden:** The edges of a solid and the hatching patterns of its faces do not disappear due to its own faces or the neighboring coplanar faces (e.g., two sticky boxes forming a wall).
4. **Singing face:** is not hiding anything.
5. **`whole_line`:** is discarded entirely if something partially covers it and is retained if it is free.
6. **Soft-cylinder silhouette:** The two generators of the silhouette and the top lid are visible, but no edge between facets.
7. **Overlapping Shadows:** two boxes whose shadows are pressed produce hatching lines without overlapping or duplicate sections.
8. **No duplicate stroke:** a test function that detects overlapping linear segments at the output, applied to a mixed scene.
9. **Determinism:** the same scene produces exactly the same output twice.

**performance measurement** (test `#[ignore]` or within the example): a synthetic scene of some expensive 5,000 and 20,000 segments must be resolved in less than 100 ms at `--release`. Indicate the measured time in your summary.

## Visual example

Create `crates/vectorio-core/examples/muestra_escena.rs`, following the style of the other examples (`muestra_trazos.rs`: use comment in the header and exit route as an argument):

```
cargo run -p vectorio-core --example muestra_escena --release -- muestra_escena.svg
```

You must draw on A4 a **test carcass**, not an artistic scene:

- two or three boxes of different heights, one partially covering the other;
- a house with a double roof (box and prism);
- a cylindrical tower with a conical roof;
- a dome;
- a section of wall with fences (small row boxes);
- a pair of `whole_line` details (windows), one free and one covered;
- hatching patterns per tone on the faces;
- Cast shadows on the floor.

Each part goes into an SVG group with a different color. If an environment variable or an argument indicates it, it also generates a dimetric version, to check that nothing depends on the exact isometric. Print time and length by console.

## Checks before delivery

1. `cargo test --workspace`
2. `cargo clippy --workspace --all-targets` without any new announcements.
3. Generate `muestra_escena.svg` and look at it: clean edges, nothing visible that should be hidden, no legitimate edge lost, hatching patterns and no double shadows.
4. `python3 scripts/install-macos.py` (required by `AGENTS.md` after code changes, although this phase does not change the interface).

## What to deliver

A brief summary in Spanish with:

- the files created and the final public API (signs);
- the decisions taken where this document leaves room, and why;
- the tolerance values chosen and how they are validated by the tests;
- the time measured in the performance test;
- What is outstanding or what is not convincing you.

That summary, together with the diff, will be reviewed.
