# Task: Create panel, Phase 3 (visual style, parts kit, and White village)

You 're a programming agent working on **Vector**, a Rust desktop application for drawing with plotter (core on `crates/vectorio-core`, interface egui on `crates/vectorio-app`). The 1 and 2 phases made these modules, which have been reviewed and are in operation:

- the `crates/vectorio-core/src/scene/` scene engine: precise concealment, hatching patterns, shadows and neck twists;
- the `crates/vectorio-core/src/generate/` generator infrastructure: declarative parameters, regenerative groups in the document and two test generators;
- the mode **Create** of the app (`crates/vectorio-app/src/create.rs`).

This phase gives the jump from  « functional »  to **« merits drawing  »**. It has three parts:

1. small revision corrections and **intersection edges between solids**;
2. a **own drawing style** and a **reusable** parts kit;
3. The first scene with art direction: **" White village "**.

Then I'll review your work with another agent, so leave the code clear, proven and well explained.

## The rules

The same as the 2 phase:

- Lee `AGENTS.md`, `crates/vectorio-core/src/scene/README.md` and `docs/crear-fase2-entrega.md` before we start.
- The interface is in Spanish, with its English translation in `locales/app.yml`. The separation between objects/layers and inks is maintained.
- **Original work.** SVGeometric (`jarvisar/geometric`) is not licensed. Do not open, copy or translate its code or paste its scenes, its pieces or its palette.
- Identifiers in English; comments and documentation in Spanish.
- **Do not run `cargo fmt`** No new dependencies unless they are essential.
- If `cargo` is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Do not commit unless the user asks for it. **Do not close or restart the Vector that is open.**
- Documents stored with the 2 phase must continue to open and regenerate the same.

---

## 0 step · Corrections from the revision of the 2 phase

1. **Orphan recipes.** `Document::to_json` already discards photos that have run out of ink. Do the same with the objectless `generated`. Proof: erasing all the parts of a group and saving it doesn't leave the recipe in the file.
2. **Edit should not step on the remembered** Now `CreateUi::edit` writes the group parameters in the generator preferences and deletes the locks. Separate the two:
   - while editing a group, work on a copy;
   - preferences are only updated when new groups are created;
   - When you cancel or update, you return the remembered parameters and locks.
3. **Preview:**
   - with an error, the sheet shows the error (or the last attenuated valid preview), not  « Preparing preview... »;
   - While calculating the End, the last preview is kept instead of leaving the sheet blank.
4. **`randomize_specs`:** a `step` less than or equal to 0, or not finite, cannot produce NaN; in that case it is not quantified.
5. **Frame of scenes.** City blocks occupies only about 63 % of the useful width because it frames a box with 3 m of margin. Add to `generate` a reusable helper with this pattern:
   - constructs the scene with an interim camera;
   - measuring the actual output box;
   - Adjusts the scale and center, and the **rebuilds the**, so that the hatching separation remains physical in mm.

   The carpenter already does it by hand. Use it on City blocks and on White village. Test: with the default values, the output occupies at least 90 % of the useful width or height.
6. **Light.** The sun of City blocks has the fixed height (`z = 1.5`): the roofs always come out clear and the walls never, so the range of tones is short. Add a **sun height parameter** (e.g. from 20° to 70°) and apply it to the new scene as well.
7. **Conical fabric of City blocks.** Its slope (54.7 °) matches the complement of the isometric elevation. The rear faces are cornered and the cone looks like a pawn with the entire ellipse. Use a slope that doesn't match any of the elevations offered and document why.
8. **Solids intersection edges** is the most important correction of this step.

   **What fails.** In the carcass, the lower case `(-2.7, 4.5, 0)–(-0.7, 6.5, 1.6)` is inserted into the upper `(-4, 3, 0)–(-2, 5, 2.8)`: overlap 0.7 × 0.5 m in top view, although the commentary says  "  overlap in projection ". The engine only draws the edges of each solid. The line where the bottom lid enters the top walls is not the edge of either, so it's not drawn. The hatching of one face ends up dry against that of the other, without contour, and it reads like a mistake.

   This is not just an example problem. The kit for this phase needs it in many places:
   - a fireplace running through the roof;
   - the bell tower coming out of the ship;
   - a half-buried house on the slope or in a terrace;
   - A cypress that sinks in the ground.

   **What to do:**
   - **Each solid has an identifier.** `solid`, `solid_oriented`, `box3`, `rotated_box`, `extrude` and `lathe` assign a new one to their faces. The loose faces of `occluder()` and the solids `quiet` do not generate intersections.
   - **Only front faces.** For each pair of **separate** front faces whose boxes overlap on the sheet and in depth, calculate the common segment. Use the grid that already exists so you don't compare everyone to everyone. With rear faces it is not necessary: a point on the rear face of a closed solid is always covered by that same solid.
   - **Calculation in 2D.** With orthographic camera, each `Face` is already a plane in (x, y, depth): it has `origin` and `gradient`. The intersection is the straight of the leaf where the two depths coincide. Cut it off with the semiplanes on both sides (Cyrus-Beck, as now) and assign its depth to each end. If you prefer to do it in coordinates of the world, explain why.
   - **Cases that are discarded:** parallel planes or coplanar, and shorter segments than `MIN_STROKE_MM`.
   - **One more stroke.** Each segment enters as a normal `Stroke`, subject to the same concealment. Thus, the part of the intersection that remains behind another object is cut off like any edge.
   - **Contacts.** One box supported on top of another, or the rails on the wall, give segments that match existing edges. The current deduplication should absorb them: check that no double strokes appear.
   - **Part of the line.** Goes to the contour part of the solid that was added **after**: the chimney, not the roof. Document the rule in `scene/README.md`. Add a `Scene::set_intersections(bool)` switch, activated by default, in case any part needs to turn it off.
   - **Faceted Surfaces.** In turns, facet-on-facet segments are joined in a continuous polyline with the existing strokes junction.

   **Tests:**
   - Two boxes interpenetrating in a known isometric view: exactly the visible intersection segments appear, with the analytical length.
   - A chimney that crosses a two-water roof: its contour over the skirt is a closed polyline on the visible part and there are no chimney strokes inside the roof.
   - A twisted cylinder running through a box: united polyline, with no gaps greater than the tolerance.
   - Contacts (box on box, belts): without duplicate segments.
   - No intersection segments with solid `quiet`, with coplanar faces or with the switch off.
   - The performance measurements of the 2 phase do not worsen by more than one 20 %.

   **Example.** In `muestra_escena.rs`, correct the commentary of the boxes: they interpenetrate on purpose, as a test case. Add a fireplace to the house that goes through the skirt.

## Step 1 · Own style (`generate/style.rs`)

The scenes share a **Vector style** defined in a single site, so that they are recognized as belonging to the same family and can be retouched together. The starting proposition is this: if something doesn't work on paper when you test it, adjust it and explain why:

- **It tones with density, never thickness.** Three levels:
  - Of course: without hatching, it's the target of the paper;
  - medium: simple hatching;
  - Dark: more dense or cross-hatching.

  In a White village, the lime is read precisely because the faces in the sun remain empty.
- **The hatching follows the surface:**
  - walls: parallel to the threads, i.e. to the horizontal edge;
  - sloping roofs: according to the line of maximum slope;
  - floor and floor: a fixed angle of paper;
  - revolution surfaces: horizontal on the sheet (`hatch_surface`).
- **Clean tone bands on curved surfaces.** Today the boundary between bands follows the facets and comes out on a ladder (see in the dome and cones). It classifies by complete vertical stripes of the lattice (all facets of the same meridian with the same tone, decided by the normal at half its height). Another equivalent option: smooth the border. It explains the choice.
- **Cast shadow** with its own part and a different hatching angle than the faces, so that it is distinguished even if it goes with the same ink.
- **Shape cut.** Every scene rests on a block of terrain cut with a knife. Its lateral faces bear the **strata** as horizontal level lines with slight deterministic ripples. It's the visual stamp of the series: the scene floats on the leaf like a model.
- **Standard parts** with fixed names, so that the user distributes inks equally in all scenes: `contorno`, `detalle` (windows, doors, tiles, fences), `trama` (medium and dark; or separated if the result asks), `sombra`, `vegetación`, `terreno` (cut block and strata). He suggests inks with `pen_order`: contour and detail with the first ink, vegetation with the second and shade with the last.
- **Level of detail.** Nothing below 1 mm on the sheet (`Scene::visible`). The preview may omit fine details.

## 2 step · Parts kit (`generate/kit/`)

Pure functions that draw on a `Scene` with the style of the 1 pass. They receive their position, their measurements and a `&mut Rng` when they have variation; they are deterministic and discard degenerate inputs without panicking. At least:

- **House** (prisma) with one- or two-water roof, four-water or pre-tiled plane.
- **Huecos:**
  - windows and doors such as lines `whole`: either seen whole or not drawn;
  - on shaded walls, the hollow may be filled with dark hatching;
  - The holes are distributed along the façade without any corners or other holes.
- **The chimney, the apron, the simple apron.**
- **Bell tower:** prismatic fuse, hollow bell body, dome end or chapel (0.5 phase 2 collared bell tower).
- **Staircase** between two levels.
- **Containment Wall and terrace:** staggered platforms that follow a slope.
- **Vegetation:** cypress (extended twig), olive or cup tree (irregular convex cup from a twig with a soft random profile), low shrub.
- **Ground cut** with strata (step 1).

**How the pieces come together.** The intersection edge of the 0.8 pass avoids the contourless slide, but a poorly solved volume collision remains unusual. The kit follows these rules:

- **Deliberate casting or clean contact.** One piece is cast into another only when reality does so, like the chimney on the roof, the tower on the ship or the house on the hillside, and with clear depth. Otherwise, the pieces touch face-to-face (contact) or separate.
- **Uncut.** Two parallel faces of distinct solids exactly match or remain at a distance of at least 1.5 mm on the sheet. In this way, strips of less than a millimeter do not come out between the top of one box and the top of another box that is almost as high. Apply it by counting heights and locks in the kit, not correcting it later.
- **No overlapping coplanar faces** of different solids with different hatching patterns, because they would cross in moiré. Two townhouses on the same façade line share plane but do not overlap: that's fine, and the vertical board is drawn.
- Add a debugging check (`debug_assert` or a warning in `GenOutput::warning` in test mode) that detects slices and overlapping coplanar faces in the kit scenes. Pass it to the 500 seeds.

Each piece has at least one geometric test:

- number of visible edges expected in a known view;
- a window covered by another house is not drawn in half;
- the part does not produce NaN with extreme measures;
- the minor details of 1 mm are omitted.

Write an example `examples/kit_piezas.rs` that draws a sample of all the pieces on a sheet A4.

## Step 3 · Scene " White village " (category of scenes)

A town of whitewashed houses that climbs up in terraces on a hillside, with its church, its cobblestone streets and its cypresses, all on the cut model. The idea is that of the white Andalusian peoples, but interpreted in the style of the 1 passage, not as a copy of any reference.

**Procedural composition with criterion:**

1. **Ladder:** from 3 to 6 terraces of variable height and background; slope is a parameter.
2. **Streets:** one or two main streets follow the contour lines and a staircase or slope connects them. The houses face the street.
3. **Houses:**
   - grouped into compact City blocks along the streets, varying in height (1 to 3 plants), width and roof type;
   - Shared mediums: neighbouring houses touch each other, without absurd gaps between them;
   - the attached boxes must not duplicate strokes; the engine already removes repeated passes.
4. **Church:**
   - a ship with its bell tower at its highest point or in a square;
   - is the focus of the composition: higher and more detailed;
   - It's optional.
5. **Vegetation:** cypresses next to the church and on the edges; olives on the empty terraces. Density is a parameter.
6. **Rhythm and air:** leaves gaps (places, orchards, courtyards). A saturated town reads worse on paper. Limit the total length by default to about 40 m on A4.

**Parameters:**
- size;
- banknotes;
- pending;
- density of houses;
- maximum number of plants;
- the proportion of each roof type;
- church (yes/no);
- vegetation;
- sight (four turns);
- the elevation (isometric/dimetric);
- Sun (height and azimuth);
- hatching separation;
- depth of the cut ground.

Use `show` when one control only makes sense with another (e.g. bell tower ones, church only).

**The quality:**
- with the default values and with at least 8 of each 10 random seed, the result must look good without retouching;
- Final layout below 300 ms on release; preview below 100 ms.

## Step 4 · Thumbnails in the gallery (app)

Each generator in the gallery displays a small thumbnail:

- its output with the default values, in `Preview`;
- calculated on a secondary basis with the same or a separate worker;
- stored in memory during the session.

As long as it doesn't exist, the queue is shown as it is now. You must not delay the main preview.

## Step 5 · Contact and verification sheet

- Example `examples/hoja_contactos.rs`: Draw on a large SVG a grid of 4 × 3 seeds from each generator, with the seed written under each. It's the primary tool of visual review: include it in your delivery and comment on which seeds you think are loose and why.
- Tests:
  - those of the 0 pass;
  - the kit;
  - determinism of White village;
  - **500 seeds** with `randomize`: no faults or NaN, within the area, below 80 m and worst weather below 1 s in release.
- Before delivery:
  1. `cargo test --workspace`
  2. `cargo clippy --workspace --all-targets` without any new announcements.
  3. Run the two new examples and `muestra_escena`, and check the SVGs. Look at the interpenetrating boxes in the mortuary, in the fireplace and in the houses above the terraces: every meeting between volumes must have its line.
  4. `python3 scripts/install-macos.py`.
  5. Visual verification in `/Applications/Vectorio.app`, only if there is no open Vector: Create → White village → Random multiple times → Put → change the ink of `vegetación` → Edit in Create → Update (the ink is preserved) → Output.
- Update `LEEME.md` if you change anything visible.

## Outside this phase

The castle, stadium and other scenes will reuse this kit in later stages. Shadows on other objects (not just on the floor), human figures and text in the scenes are also not coming in now.

## What to deliver

A brief summary in Spanish (you can go to `docs/crear-fase3-entrega.md`) with:

- what is done at each step and the files touched;
- the new public API (firms);
- style decisions and why;
- the times (default White village, worse than 500 seeds, preview);
- the contact sheet and your honest assessment of what works and what doesn't;
- The pending or the unconvincing.
