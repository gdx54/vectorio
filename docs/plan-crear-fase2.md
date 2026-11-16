# Task: Create panel, Phase 2 (engine, generator, and panel corrections)

You 're a programming agent working on **Vector**, a Rust desktop application for drawing with plotter (core on `crates/vectorio-core`, interface egui on `crates/vectorio-app`). In the 1 phase the `crates/vectorio-core/src/scene/` scene engine was made: orthographic camera, solid, exact removal of hidden lines, hatching patterns, tones and shadows. It has been reviewed: the concealment is correct, it is well-tested and it is fast (some 15 more expensive with 5,000). This phase has two parts:

1. **Step 0:** correct what came out in the engine review.
2. **Steps 1  –  5:** assemble the generator infrastructure, its site in the document and the **mode Create** from the app, with two simple generators that prove everything works.

Then I'll review your work with another agent, so leave the code clear, proven and well explained.

## The rules

- Lee `AGENTS.md` and `crates/vectorio-core/src/scene/README.md` before we start.
- The interface is in **Spanish**, with its English translation in `crates/vectorio-app/locales/app.yml` (use `tr!` / `ts!` like the rest of the app). The separation between **objects/layers** and **inks** is maintained.
- **Original work.** The idea is inspired by SVGeometric (`jarvisar/geometric`), which **does not license**. Do not open, copy or translate its code or paste its designs.
- English IDs, like the rest of the code. Comments and documentation in Spanish, in the tone of the existing code.
- **Do not run `cargo fmt`**: the project uses dense lines on purpose (up to a few 150 characters).
- No new dependencies unless they are essential; if you add one, justify it in the summary.
- If `cargo` is not available: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Stay on the `feat/crear-motor-escena` branch. Do not commit unless the user asks for it.
- **Do not close or restart the Vector that is open.** The open version continues until the next start.
- Old files: a document saved before this phase has to be opened the same as before (new fields with `#[serde(default)]` and not written when they are empty).

---

## Step 0 · Engine corrections (`crates/vectorio-core/src/scene/`)

Do it in this order. Each with its own proof.

### 0.1 Continuous hatching patterns on curved surfaces (most important)
Today, to weave a cylinder, a cone or a dome you have to call `hatch_face` facet by facet. The result is hundreds of loose rays, each with its own direction, instead of continuous lines (in the cadaver: 938 strokes in the dark hatching). It should be added:

- `hatch_surface(faces: &[Vec<V3>], angle_deg: f64, spacing_mm: f64)`: hatching **several faces as a single** region, with a fixed angle on the sheet (paper degrees, the same criterion as `begin_shadows`) and global phase, like shadows.
  - For every line of hatching, cut against every face that looks at the camera.
  - The faces of a front-facing convex surface do not overlap on the leaf, so just sort the intervals and **join the sequences into a single** polyline.
  - Each vertex carries the depth of the plane of the face in which it is; on the shared edges it is continuous.
  - Faces that don't look at the camera are ignored.
- Test: a soft cylinder hatching to 0° gives exactly **one stroke per hatching line**, silhouette to silhouette, without slices on the edges between facets.
- Update `examples/muestra_escena.rs`:
  - In revolutionary solids, group the facets by tone and hatching each group with `hatch_surface`;
  - in boxes, keep `hatch_face`;
  - Adjust the frame so that the carcass occupies the leaf (now it stays above, with half a blank page).

### 0.2 No input generated can bring down the app
The generators will produce geometry with random parameters. A `assert!` that jumps with degenerate data would close Vector. The rule is: **the public methods of `Scene` and `Camera` do not panic in release because of the** data. Degenerated data is discarded without drawing anything:

- boxes of zero or negative size;
- Degenerated or unflattened faces;
- impossible rotation profiles;
- tangent extrusions;
- NaN or infinite values.

In addition:

- A non-tangent hatching direction is projected onto the face plane.
- Asking for shadows without the sun just doesn't produce shadows.
- Open a group of shadows with another open hatching the previous one or discard it (document which one).
- Leave `debug_assert!` where I help detect programming errors in tests.
- Test: a degenerate input suite that does not panic and does not produce NaN at the output.

### 0.3 Performance with long lines
The grid queries (in `Grid::hidden` and in `remove_overlaps`) run through all **box cells in the** segment. For a long diagonal line, like a hatching shadow that crosses the leaf, that's thousands of cells and lots of candidate faces that don't touch the segment. The current measurement uses only short segments (from 7 to 11 mm), so it does not detect it.

- It only travels through the cells that cross the segment (DDA or covered cell route).
- For repeated passes, you can index instead by quantified direction and displacement. Choose and justify.
- Add a second realistic measurement: 500 boxes in A4 with hatching by tone and shadows, plus 2,000 long diagonal lines. Objective: `render` below 100 ms in release. Indicate the two measurements in the summary.

### 0.4 Almost singing faces
`Face::new` discards faces with a projected area less than `1e-10` mm2. It's a tiny absolute threshold: an almost angular face passes through the filter and produces a huge, unstable depth gradient. Use a relative criterion, e.g. a projected width (longer edge area) less than `1e-6` mm. Test: a face almost singing doesn't produce false slices.

### 0.5 Non-convex revolutionary solids
`solid` directs the faces with the centroid, which forces the rotor profiles to be convex. Profiles with neck (jars, bulbous domes, bell towers) are necessary for the scenes.

- `lathe` must orient its **faces by** construction (outside the shaft, top and bottom caps) and accept non-convex profiles with increasing heights.
- `solid` needs to be able to respect the spin given to it (e.g. a parameter or a `solid_oriented` variant).
- Occultation is already common; the edge rule applies as well.
- Test: a vase with `(1.0, 0) → (0.5, 1) → (1.0, 2)` profile is drawn without false edges and with the neck hiding what it should.

### 0.6 Rear faces of convex solids
In a solid **Convex and closed**A face that doesn't look at the camera never hides anything that doesn't already hide its front faces. Don't add them as occlusions: it's half the faces for big scenes. This **not** applies to non-convex solids (0.5) or to `occluder`. Test: the result is identical with and without optimization in a convex solid scene.

### 0.7 Cleaning
Remove from the `README.md` of the module the section  « Validation of the 13 - 11 - 2026... »  with time of this equipment. It's delivery information, not documentation, and it's in your resume. Update the README with the new API.

---

## Step 1 · Generator infrastructure (core)

New module `crates/vectorio-core/src/generate/` (register it as `lib.rs`). It doesn 't depend on egui.

- **Declarative parameters.** `ParamSpec` with the following variants:
  - `Section { label }`: a headline, with no value.
  - `Range { id, label, min, max, step, default, random }`: `random` indicates whether to enter  « Aleatorio »  and optionally a narrower range.
  - `Choice { id, label, options: [(valor, etiqueta)], default, random }`.
  - `Toggle { id, label, default, random: probabilidad }`.
  - An optional field to display or hide control according to other values (`show`) and an optional aid (`hint`).

  The tags are text keys in Spanish that the app translates.
- **Values.** `Params` as an ordered map `id → ParamValue` (number, text or boolean), serializable. As I read it:
  - the missing keys take the default value;
  - The unknowns are ignored.
  - the numbers are cut to their range.

  That's how the version changes survive. Includes typed methods (`f64(id)`, `int(id)`, `choice(id)`, `flag(id)`).
- **Treatment `Generator`:**
  - data: `id()` (stable, never changing), `name()`, `category()`, `description()`, `params() -> Vec<ParamSpec>`;
  - `randomize(&self, rng, &mut Params, locks)`: has a default implementation that uses the `random` information of each `ParamSpec` and respects blocked parameters;
  - `generate(&self, &Params, &GenContext) -> Result<GenOutput, String>`.
- **`GenContext`:** the drawing area in mm (rectangle of paper minus margins), the seed, the thickness of the reference pen and a level of detail (`Preview` / `Final`), so that the preview can be lighter.
- **`GenOutput`:** list of `{ name, lines: Vec<Polyline> }` parts in sheet mm, plus an optional notice. Scenes get it directly from `SceneOutput`; 2D generators build it by hand. Add an assistant to calculate the line meters in part.
- **Register:** `generate::all() -> &'static [&'static dyn Generator]` and `generate::find(id)`.
- **Determinism:** with the same seed and parameters, identical output. Use only `geom::Rng` (or your own deterministic noise if needed), never system randomness.

## Step 2 · Groups generated in the document (core)

The model to be imitated is the **live photo**: `crates/vectorio-core/src/photo.rs`, `Document::photos`, `Item::group`, `Document::photo` and `Document::group_items` in `doc.rs`. A photo keeps its image and its settings, and its inks are objects that point to it with `group`.

- `Document::generated: Vec<GeneratedGroup>` with `{ id, name, generator: String, params: Params, seed: u64, area: BBox }`. Use the `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
- **One part = an object** of the document:
  - `Geometry::Path { closed: false }`, with `group = Some(id)` and name `«<grupo> · <parte>»`;
  - contour with the pen assigned and unfilled.
  - So each part moves to another ink with the tools that already exist.
- **Initial assignment of inks:** each part goes to a pen of the document. The generator may suggest an order (e.g. contour → ink 1, shadows → last); otherwise, they are distributed in order, spinning if there are more parts than inks.
- **Regenerate** (change parameters or seed of an existing group):
  - replace the lines of each object **with the part name**;
  - retains its pen, visibility, lock, `Placement` and position in the layers;
  - new parts create new objects;
  - The missing parts remove your object, but remember your pen in the group in case they come back.
- The lines are stored at local coordinates so that moving or climbing the group and then regenerating maintains the position. Explain the criterion in the code.
- Tests:
  - an old file is opened the same way;
  - back and forth of serialization;
  - regenerate preserves inks and positions;
  - A part that disappears and comes back gets its ink.

## Step 3 · Mode  « Create »  in the app

Starting points:
- `ViewMode` in `crates/vectorio-app/src/app.rs`;
- the bar mode selector (search for `ViewMode::Output, tr!("Salida")`);
- the shortcuts and the menu in `commands.rs` (`⌘1` Design, `⌘2` Output);
- `output.rs::show`, as an example in its own panel mode;
- `theme.rs` and `docs/plan-liquid-glass.md` for the current Liquid Glass style.

It should look like this:

- **Third mode  « Create »**, with this order in the selector and menu: Design · Create · Output. Shortcuts: `⌘1` Design, `⌘2` Create, `⌘3` Output (updates existing shortcuts and texts, and `LEEME.md`).
- **Side panel**, with the following elements:
  - **Gallery** generators by category, with name and description.
  - **Controls built from `params()`**: slider with numeric field, folding, box and folding headers. They respect `show`, show `hint` as an aid and have a **locked** by parameter.
  - **Seed**: numeric field and button **Random** (new parameters, except blocked, and new seed). A keyboard shortcut for new seed, only when the focus isn't on a text field.
  - **Metrics**: partial and total line meters, with visual notice above 40 m and stronger above 80 m (« the paper can saturate »), and calculation time.
  - **** buttons:** Put on the **canvas (create the group with a undo step and go back to Design with the selected group) or, if an existing group is being edited,** Update group **and** Cancel **.
- **Preview** in the center zone: the sheet with its color and the parts drawn with the ink color that would touch them. Reuse the existing paint on the canvas if you can.
- **Second plane calculation**, in a thread with version counter:
  - an outdated result is discarded;
  - a change of control releases the calculation without blocking the interface;
  - The preview uses `Preview` and the `Final` placement.
- **`generate` within `std::panic::catch_unwind`** as the last network. An error or panic is displayed on the panel as a message and never closes the app.
- **In Layers/inspector**, a generated group is displayed as a group row, similar to that of a photo (`group_row` in `inspector.rs`), with the action **« Edit in Create... »**, which opens the Create mode with its parameters and its seed.
- **Remember** the latest generator and its parameters between sessions, in the app preferences.

## Step 4 · Two demonstration generators

They are to test the infrastructure, not the ultimate art direction, but they should look clean and chart well in A4 with a pen of 0.3  –  0.5 mm, below half a second.

1. **" City blocks "** (scene category): an isometric grid of blocks on a base, using the engine.
   - Parameters: rows, columns, minimum and maximum height, probability of gap (place), street width, camera rotation (two or three views), elevation (isometric/dimetric), sun (angle), hatching separation.
   - Each block is a box with hatching per tone (`hatch_face`) and some cylindrical towers with a conical roof (`lathe` and `hatch_surface`), with shadows on the floor.
   - Parts: contour, roofs, clear hatching, medium hatching, dark hatching, shade.
   - The camera fits the area with `Camera::fit`. Use `Scene::visible` to not draw minor details of 1 mm.
2. **« Spirograph »** (Curves category), 2D generator without scene engine.
   - Hypotrocoid and epitrocoid with entire radii (close the curve with the mcd), pencil distance, number of layers with gap and part per layer.
   - It must come out in a single continuous stroke per layer.

## Step 5 · Tests and checks

- Core tests:
  - `Params`: default values, range cuts, unknown keys and `show`;
  - determinism of each generator;
  - **500 random seeds** per generator with `randomize`: no failure, none produces NaN, all remain within the area and limit of 80 m, and the largest takes less than 1 s in release (maximum report).
- Proof of the 2 pass document.
- Before delivery:
  1. `cargo test --workspace`
  2. `cargo clippy --workspace --all-targets` without any new announcements.
  3. `cargo run -p vectorio-core --example muestra_escena --release -- muestra_escena.svg --dimetric`: check that curved surfaces have continuous hatching.
  4. `python3 scripts/install-macos.py`.
  5. Visual check on `/Applications/Vectorio.app`. Only if **not** there is an open vector: open it, test Create → City blocks → Random multiple times → Place → move one part to another ink → Edit in Create → Update → Deshacer, and Output with the group. If one is already open, do not close it: indicate in the summary what you could not check.
- Update `LEEME.md` with Create mode and the new shortcuts.

## Outside this phase

The piece kit (walls, windows, roofs, trees, figures...), the scenes with artistic direction (town, castle, stadium...) and more 2D generators. They 'll arrive at the 3 phase.

## What to deliver

A brief summary in Spanish with:

- what is done at each step and the files touched;
- the new public API (firms);
- the decisions where this document leaves room, and why;
- the two engine measurements and the worst time of the generators;
- captures or description of the visual verification, or what could not be verified;
- The pending or the unconvincing.
