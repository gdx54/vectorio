# Task: Create panel, Phase 4 (seamless geometry and White village art direction)

You 're a programming agent working on **Vector**, a Rust desktop application for drawing with plotter (core on `crates/vectorio-core`, interface egui on `crates/vectorio-app`). The 3 phase left facts and reviews:

- the intersection edges of the engine;
- the common style in `generate/style.rs`;
- the kit in `generate/kit/`;
- the scene " White village " in `generate/village.rs`;
- The miniatures in the gallery.

Technically, it works: tests pass, times are good, and encounters between volumes already have their line. But the White village **still doesn 't look like a White village**. This phase has two parts:

1. correct a few deficiencies in the review;
2. The artistic pass that turns the scene into something you want to plot.

No new scenes are added.

## The rules

The same as the 3 phase:

- Lee `AGENTS.md`, `crates/vectorio-core/src/scene/README.md` and `docs/crear/fases/entrega-fase3.md` before we start.
- The interface is in Spanish, with its English translation in `locales/app.yml`. The separation between objects/layers and inks is maintained.
- **Original work.** SVGeometric (`jarvisar/geometric`) is not licensed. Do not open, copy or translate its code or paste its scenes, its pieces or its palette.
- Identifiers in English; comments and documentation in Spanish.
- **Do not run `cargo fmt`** No new dependencies unless they are essential.
- If `cargo` is missing: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Do not commit unless the user asks for it. **Do not close or restart the Vector that is open.**
- Documents stored with the 2 and 3 phases must continue to open and regenerate equally. If you change the look of the White village, version its recipe the same way you did with City blocks.
- **Readability.** The new 3 phase code is very tight: lines of more than 200 characters and several sentences per line. In the files you touch (`style.rs`, `kit/`, `village.rs` and `intersections.rs`), split the long functions and use a sentence per line. Don't change the format of the rest of the project.

---

## 0 step · Corrections from the revision of the 3 phase

1. **The frame cuts the drawing.** I have measured it with 150 seeds in three distinct areas, and close to 17 % of the outputs of White village (76 from 450) reach the edge of the area less than 0.05 mm. The margin is 1 mm, so those drawings come out **cut** against the edge. In City blocks it doesn't happen to anyone.

   The cause: `fit_scene` measures the first pass and reconstructs with another scale, but the geometry depends on the scale. `Style::quantum` and `snap`, `Scene::visible`, the passage of the strata and the number of turns segments change, and the second pass comes out larger than the measurement. Fix it like this:
   - Everything that depends on the scale is calculated **once**, with the scale of the first pass, and passed fixed to the second. For example, a `Style` with the quant already solved;
   - After the second pass, check that the box fits into the area with its margin. If it doesn't fit, make a third pass by reducing the scale, never cutting;
   - The first pass is built on loose paper, so that its measurement is not cut out already.

   Test: with 500 seeds and three areas (square, A4 vertical and flat), no outlet remains less than half the margin of the edge.
   > **Made.** `fit_scene` builds the measurement passes on a loose, iterative paper (up to four times) until the output fits with half a margin; the final scene is built with the already adjusted scale. In 900 exits (150 seeds, three areas, White village in model and City blocks) the nearest one is 0.59 mm from the edge.
2. **The alignment is two loose lines.** `kit::eave` draws two segments to 0.3 m from the façade, with nothing to support them. In the sample they stand out from the silhouette of the house like sticks. Replace it with a real flight: the skirt (or a thin slab) protrudes from the walls, with a visible thickness of at least 1.5 mm on the leaf. The joint with the wall is powered by the engine.
   > **Made.** The skirt flies over the street façade (0.4 m or 1.6 mm on the leaf, whichever is larger) with a white vertical corner of at least 1.6 mm; the sides and rear remain beamed because they touch the neighbors and the wall of the terrace. The houses in the back row and the nave of the church do not fly: they have in front, very close, another volume. `kit::eave` is now a thin slab that cannot be drawn if its singing does not reach 1.5 mm.
3. **The fireplace can stay afloat.** According to the code, its base is at `top + rise·0,15` And his outer face to `min.x + q`. On a two- or four-water roof, the skirt is at that height at `x = min.x + q` only if `w ≤ 13,3·q`. With q = 0.5 m, in houses more than a few 6.7 m wide the chimney would stay above the roof on its lower side. Now it doesn't happen because the houses are narrow, but it will happen with the 2 pass proportions. Calculate the base from the height of the skirt at the lowest point of the chimney footprint, minus a clear frame, and place it near the comb, not next to the handle. Try a house from 9 m.
   > **Made.** The base comes out of the skirt height at the bottom of its footprint, minus a frame, and the fireplace only appears if its bottom keeps 1.6 mm on paper with the hastil.
4. **Dark Holes** `opening(..., dark: true)` only adds an upside. Either it's filled with dark hatching, like the 3 phase plan asked, or it's renamed. I prefer filler: it's what makes a window read on a white wall (step 2).
   > **Made.** `OpeningKind::Dark` (and `Twin`) fills the gap with a tight vertical hatching (0.6 times separation) cut to its frame; the `openings_clear_wall_hatching_on_every_side_and_dark_ones_get_filled` test checks it on the four facades.
5. **Version of the recipe.** The hidden parameter `scene_version`, with `show: "__internal__" = "never"`, is a flaw. Pass the version to `GeneratedGroup`'s own field (`#[serde(default)]`), or create a `ParamSpec` variant that is never displayed. Maintain the migration of City blocks recipes from the 2 phase.
   > **Made.** `ParamSpec::Version { current }` (`SCENE_VERSION`) is never shown or sorted and, if missing, takes the current version. City blocks goes through the 3 and preserves migration to 2; White village starts in the 1 with the dense people (the one from the 3 phase did not get saved in the repository, so it cannot be reproduced).
6. **`Style::lathe` depends on the internal order of the faces of `Scene::lathe`:** first the n facets of each section and the caps at the end, with the same `circle_segments`. That `lathe` returns that structure explicitly (facetes per meridian and covers apart), or leaves at least one test to fail if the order changes.
   > **Made.** `Scene::lathe_parts` returns a `Lathe` with faces per stretch and meridian and covers separately; `Style::lathe` no longer counts faces, and a test checks the height and angle of each facet.
7. **Tests of the kit.** The plan called for at least one geometric test per piece and there are four for twelve pieces. Complete the remaining ones, especially the staircase, the terrace, the terrain, the railing and the bell tower.
   > **Made.** `every_kit_piece_draws_repeatably_and_cleanly_on_its_own` covers the missing pieces: each one draws, is deterministic, gives finite coordinates and, alone, leaves no construction notices.

## Passage 1 · Seamless

It's the most visible defect in the sample. Each piece is made with several solid stickers, and each stick leaves a line in the middle of what should be **a single surface**:

- **Azotea with apron:** are four boxes over the house. There is a horizontal line along the entire wall, at the base of the railing, and vertical joints at the corners of the railing. In the village you see it in almost every house.
- **Woven to a water:** the upper vertical face of the roof continues the wall, but a line separates them.
- **Hastiales:** the triangle of the two-water roof is also separated from the wall by a line.
- **Staircase:** Each ladder is a column, and the side of the staircase emerges as a row of pillars with vertical joints, not as a single steps.

The joints between **different buildings** (closed houses) must be seen. The ones from **the same piece**, no.

**What to do:**

- **Motor: coplanar faces of the same solid.** Between two adjacent faces, coplanar and with the same outer normal of the same solid, the edge is not drawn. So you can break a non-convex face into convex pieces without noticing. The hatching of a broken face is still a single continuous family of lines: prove it with a test.
- **Motor: non-convex profile prisms.** Add `Scene::prism(profile: &[V3], vector: V3)`, which accepts a simple non-convex flat profile, such as the profile of a staircase or roofed house. It breaks the lids down into convex pieces, builds the solid with `solid_oriented` and returns the outer faces, grouped by logical face, so that they can be woven together.
- **Kit: Each piece is a single solid whenever possible.**
  - House with two water roofs: pentagonal profile prism, wall and hastile in a single solid.
  - House with a water roof: special profile prism.
  - House with four water roofs: a single solid convex.
  - Pre-tiled roof: a solid non-convex, with outer walls of full height, the crown of the pre-tiled roof, the inner walls and the sunken roof floor.
  - Staircase: prism of the staggered profile.
- **Sewing Test.** For each piece of the kit, no visible edge separates two coplanar faces with the same normal. The joints between townhouses continue to appear.

> **That's it.**
> - **Motor.** `build_solid` does not draw the edge between two faces of the same solid with the same normal (neither as edge nor as line). `Scene::hatch_faces` hatching a broken face into loose pieces between them: each line crosses the pieces of a single stroke, and one test compares the hatching of an entire face to that of the same face in two triangles.
> - **Prisms and polyhedra.** `Scene::polyhedron(faces)` receives flat and simple logical faces (non-convex included), joins vertices, directs the turn by adjacency and by volume, divides each face into convex pieces (ears and Hertel – Mehlhorn) and builds with `solid_oriented`; returns the pieces grouped by logical face, or nothing if the shell is not closed. `Scene::prism(profile, vector)` rests on it.
> - **Kit.** The house (plan, with apron, to one water, to two and to four water, with or without flight), the loose apron, the staircase, the tap (ring or sections in succession, with or without door), the bench and the porticoes of the pergola are one solid each. `Style::shade_groups` hatching together the coplanar faces of a solid.
> - **Test.** `no_kit_piece_draws_a_seam_between_coplanar_faces_of_its_own` travels through 20 parts at three scales and six seeds with the `Scene::seams` detector (only in testing) and checks that two townhouses continue to show their four-deck joint.

## Step 2 · Art direction of the White village

Compared to the current contact sheet, here's what's missing and what I'm asking:

- **The cal cannot be seen.** With the standard values, the two visible façades of almost all houses come out of wedges, so the village is grey and not white. It's just the opposite of the idea of style.
  - The angle of the sun must be **relative to the view**: with any  « Vista », the serial sun fully illuminates one of the two orientations of the visible façade, which is left blank.
  - The other orientation remains in the middle tone, with a more open hatching than now: some 2.2 times the base separation.
  - The dark is reserved for cast shadows, hollows and faces that really look away from the sun.
  - Test: with the serial values and in each view, at least the 40 % of the visible area of the wall is left without hatching.
- **Proportions of house, not tower.** Today houses have about 3 m facade, 2.6 m background and up to 5 m height plus the railing, and are read as towers.
  - It asks for facades from 4 to 9 m, from 1 to 2 plants from about 2.8 m and backgrounds from 5 to 7 m. If necessary, make the terraces deeper.
  - **houses are lined up along** street, with high jumps of at least one quintet. Every once in a while, a yard or a floor breaks the line.
- **Holes to be read.** The windows are small and deep above the lime: dark hatching filled hole (step 0.4). The door, in the top view down and into the street. The rhythm of the gaps is regular within each house and distinct between houses. The visible hastils and medians may carry some isolated gap.
- **Church with hierarchy.** The nave is clearly longer than wide (from 1: 2 to 1: 3) and the bell tower goes in a corner of the feet, not in the middle of the nave. Prove that your encounter with the skirt or hastil is well drawn.
- **Chimneys with moderation** Less and only on sloping roofs. In the 5 seed almost every roof has one.
- **Strata with character.** Today they are almost straight parallel stripes at a constant distance, and they look like a strip, not a cut terrain. It's the show's visual stamp, so it's worth watching out for:
  - irregular and deterministic layer thickness;
  - low frequency undulation more visible, from a 25 to a 35 % of the pass;
  - Occasionally, a layer that coins and disappears (lens);
  - the strata are only on the **outer faces of the cut block**, those on the edge of the model;
  - The inner walls between terraces are containment walls: they carry a coping (coronation line) and, at most, very open threads, never strata.
- **Streets.** The street is the target between rows. It's enough with a curb or a ladder where the level changes. The ladder axis really connects the streets of successive terraces.
- **Vegetation.** Cypresses go in pairs or in a row next to the church or on the edge of the model. The olive trees cluster in the sun's orchards, not loose one by one.

Make sure you keep reading all this with one ink: the difference between white, medium and dark has to work colorlessly.

> **That's it.**
> - **Visible whitewash.** The serial sun is calculated from the view: the facade of the visible street remains in Lit (0.74) and the other face visible in Mid (0.35) at 35 ° height;  « Turn of the sun »  (− 90 to 90 °) turns it from there, without upgrading the recipe version. In  « Engraving », the middle tone uses 2.2 times the separation (`Style::gap`). The coverage test (`Scene::lit_wall_fraction`, in tests only) gives between the 47 and the 65 % hatching-free wall in the four views, full bleed and in model.
> - **Houses.** Façades from 4 to 9 m that fill the hollow row (a floor of more than 9.5 m is split in two), backgrounds from 5 to 7 m, a top view and, with probability 0.55 per top view, a second of 2.8 m; if a house is equal to its neighbor, go up a quantity. The terraces are deeper (street of 2.4 m plus 7 m of houses). One 40 % of the sunsets with sufficient background is house with patio (closed, with curtain and door) and the rest garden upholstered with grooves and clustered olive trees.
> - **Holes.** The door goes into the lower top view and towards the street; the windows follow a step and a width typical of each house (equal within it, distinct between neighbors) and, in those of two floors, there is sometimes an isolated hole in the median, which is only seen where the neighbor is lower.
> - **Church.** `kit::church` with `Church { width, length, height, tower }`: nave from 1: 2.2 to 1: 2.8, cover, oculus and high windows in bays of about 3 m, and the bell tower embedded in a corner of the feet, which advances over the façade and enters the nave just to keep 1.6 mm on the leaf. `Church::ahead` says how far he goes, so that banks, trees and people in the square don't touch him. The `the_bell_tower_stands_at_a_front_corner_and_its_meeting_with_the_nave_is_drawn` test runs three scales, four views and two sides.
> - **Chimneys.** Only on sloping roofs and less likely; rooftops have casement, pergola, tent or reservoir.
> - **Strata.** `kit::Strata` is common throughout the terrain: thickness from 0.7 to 1.6 steps, low frequency shared rippling plus another own (up to a 30 % of the pass) and lenses in a 15 % of the layers. `kit::terrace` receives a `Cut` with the outer faces and height from which the front wall is seen; the inner walls carry coping and, in the final drawing, very open threads.
> - **Streets and stairs.** The street is the target of 2.4 m in front of each row; the change of level is the singing of the terrace. The central staircase rises from the street below and reaches the one above; the alleys carry their own stairs.
> - **Vegetation.** Pair of cypresses on the side of the church without bell tower, row of cypresses only on the edge of the core, olive trees grouped in orchards and olive groves; the rows of cypresses in the fields are rare.
> - **Checked.** 135 Core tests; on release, 500 seeds per generator without warnings (worst White village delays 296 ms), frames to 0.54 mm from the edge at least and White village serial with median of 84 ms in preview and 86 ms at the end, running the tests one at a time. Samples at `target/phase4/paso2/`.

## Step 3 · Checks and deliveries

- **Tests:**
  - the steps 0 and 1;
  - no cuts with 500 seeds in three areas;
  - Whitewash coverage ≥ 40 % with the serial values;
  - determinism;
  - 500 seeds without NaN, below 80 m and with worse weather below 1 s in release.
- **Performance:** the White village series, below 100 ms in Preview and 300 ms in Final.
- **Before delivery:**
  1. `cargo test --workspace`
  2. `cargo clippy --workspace --all-targets` without any new announcements.
  3. Run `kit_piezas`, `hoja_contactos` and `muestra_escena`.
  4. `python3 scripts/install-macos.py`.
  5. Visual verification on `/Applications/Vectorio.app`, only if there is no open vector.
- **Comparison:** stores in `target/phase4/` the 3 phase contact sheet and the new one, with the same 12 seeds, for comparison before and after.
- **Delivery:** writes `docs/crear/fases/entrega-fase4.md` with:
  - what is done at each step and the new API;
  - the times;
  - Your sincere evaluation of seed to seed of the new leaf: which plots and which not, and why;
  - Which doesn't convince you.

> **Made.** Framed to 500 seeds in three areas, 500 seeds per generator without notices or NaN and below 80 m, lime coverage and determinism in green; times, comparison and valuation in [create-phase4-delivery.md](entrega-fase4.md).

## Outside this phase

Castle, stadium and other scenes; shadows on other objects; figures and text. We'll get back to them when the White village is up.
