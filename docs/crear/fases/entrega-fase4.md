# Create · Phase 4 delivery

I order in [plan-create-phase4.md](plan-fase4.md); each step carries there its note  «  Done »  with the detail. Here's the summary, the new API, the timing and a sincere evaluation of the contact sheet.

Warning: `village.rs` and much of the kit remain unconfirmed on git (the 3 phase recipe was also not saved). The recipe version of the White village is still in 1.

## I did it.

- **Step 0 · Phase corrections 3.**
  - The framing of the model iterates until the exit fits by half a margin.
  - The roof flies over the facade of the street and the fireplace rests on the skirt.
  - Dark holes are filled with hatching.
  - The stage version goes on a `ParamSpec::Version`.
  - `Scene::lathe_parts` and the tests on each piece of the kit.
- **Step 1 · Seamless.**
  - A solid doesn't draw edges between its coplanar faces.
  - `Scene::polyhedron` and `Scene::prism` build parts from a single solid: houses, stairs, carpets, benches and pergolas.
  - `Scene::hatch_faces` hatching a broken face with a single stroke.
  - The `Scene::seams` detector (tested only) runs through 20 parts.
- **Step 2 · Art direction**
  - Sol relative to sight: the street façade becomes white and the other face visible in mid-tone, with hatching to 2.2 times.
  - Homes from 4 to 9 m in rows of townhouses with high jumps; houses with courtyards and upholstered gardens.
  - Door to the street and rhythm of holes in the house.
  - Church 1: 2.2  –  1: 2.8 with bell tower in a corner of the feet.
  - Chimneys only on sloping roofs.
  - Common, irregular, undulating and lensed strata, only on the outer faces; containment walls with coping.
  - Cypresses in pairs or in rows at the edges; clustered olives.
- **step 3 · Checks.**
  - The framing test passes to 500 seeds in three areas (square, vertical and tempered) for White village in model and City blocks.
  - `cargo test --workspace`, clippy, examples and installation were executed.

## New API

```text
// generate::kit
Church { width, length, height, tower: Option<(bool /*left*/, TowerTop)> }
Church::ahead(&self, scene: &Scene, style: Style) -> f64     // how far the tower projects forward
church(scene, style, front: V3, plan: Church, rng)
Cut { exterior: [bool; 4] /*front, right, back, left*/, wall_from: Option<f64> }
Cut::block() -> Cut
Strata::new(scene, style, bottom, top, min: V3, max: V3, rng) -> Strata
terrace(scene, style, min, max, cut: Cut, strata: &Strata)
pick(rng, &[T]) -> T                                          // now public

// scene (step 1)
Scene::polyhedron(&mut self, faces: &[Vec<V3>]) -> Vec<Vec<Vec<V3>>>
Scene::prism(&mut self, profile: &[V3], vector: V3) -> Vec<Vec<Vec<V3>>>
Scene::hatch_faces(&mut self, pieces: &[Vec<V3>], direction: V3, spacing_mm: f64)
Scene::lathe_parts(...) -> Lathe
// tests only: Scene::seams, Scene::lit_wall_fraction(part, step_mm)

// White village: "Sun rotation" parameter (sun, −90…90°, relative to the default light)
```

## The times

Release on this Mac. The tests were run one at a time (`--test-threads=1`).

|Measure|Results|The limit|
|---|---|---|
|Standard White village, preview (median of 7)|85 MS|100 MS|
|Serial White village, final drawing (medium of 7)|86 MS|300 MS|
|500 seeds, worse White village|287 MS|1 S.|
|500 seeds, worse City blocks / Spirograph|12 Ms / 1 MS|1 S.|
|Frame, 500 seeds × 3 areas, minimum distance to the edge|0.54 mm|≥ half the margin|
|Lime cover, four views, full bleed and model|47–65 %|≥ 40 %|
|Contacts sheet, White village by seed|19–58 MS|—|

## Comparison

In `target/phase4/`:
- `contactos_fase3.svg`: copy of the 3 phase sheet, generated before the start of this phase;
- `contactos_fase4.svg`: the new leaf;
- `*_pueblo.svg(.png)`: only the row of White village;
- `paso2/`: samples in A4, in model and full bleed.

Both leaves use the same 12 seeds. The 3 phase framed by series in model and the 4 full bleed, so the composition comparison is not direct. What compares well is the tone, the proportion of the houses and the organization.

**Previously:**
- Almost all the faces were plotted, so the people were grey.
- 3 m facade houses that looked like towers;
- Churches in the midst of the crowd;
- Strata like a uniform strip.

**Now:**
- The facades of the street are empty.
- lines along clear streets;
- square with church, bell tower and cypresses;
- A farmhouse that gets dismantled in the orchards.

## Valuation from seed to seed (new leaf, area of 190 × 190 mm, full bleed)

|Seed|Would you plough it?|Why? Why?|
|---|---|---|
|0 (seriously)|Yes, I did.|Good balance: square and church upstairs, two clear rows, orchards on the edges. A few rooftop houses are still somewhat cubic.|
|1|I doubt it.|The church with the needle and the street read well, but on the left are tall blocks of roof that look like towers.|
|2|I 'm not.|Too many houses, tight, and the church is lost between them; with the fields, the leaf is very loaded (28 m).|
|3|Yes, I did.|Scattered village with many cobbled gardens; it reads well. The roof cubes are a bit plentiful.|
|4|I doubt it.|The square is fine, but it's dominated by brick roofs and the whole thing reads like blocks.|
|5|Yes, I did.|Several streets with steps, a good variety of decks and open square.|
|6|Yes, the best.|Long streets of top-view houses with roofs, church with needles and cypress trees: that's just the idea.|
|7|I doubt it.|It's only partially visible in the miniature; correct rows, but without a clear focus.|
|8|I doubt it.|The village stands on a narrow diagonal and the square is read little; too much olive loose around.|
|9|Yes, I did.|Dense but tidy, with space in the center and a good mix of roofs.|
|10|I doubt it.|The church is fine, but the roof houses on the right form a mass of boxes.|
|11|Without judging|I haven't reviewed it in detail.|

In summary: it would plough 0, 3, 5, 6 and 9; with adjustments, 1, 4, 7, 8 and 10; it would discard the 2.

## What doesn't convince me

- **Precyl roofs still look like** boxes when they are majority (seeds 1, 4 and 10). Although they have house proportions, the high railing and lack of roofing make them abstract volumes. I'd lower their odds or I'd always put something on them.
- **The field weighs too much full bleed in small areas.** In 190 × 190 mm the grooves and olives occupy more than half of the leaf and rise the meters (22 - 30 m versus 8 - 21 m before). The village should take up more leaf or the field should clear with the distance.
- **There are too many rows of** cypresses scattered through the fields still in some seeds (3 and 5). I've reduced their frequency, but they're still common in the open field.
- **The preview is barely faster than the final drawing** (85 versus 86 ms). Comply, but without margin, and in parallel with other tests it came to pass 100 ms. Probably what weighs are the framing passes and not the hatching; I haven't measured it in parts.
- **Time tests depend on the load.** In parallel, `benchmark_long_diagonals_and_shaded_boxes` And the village one failed once. You'd have to mark them to run them alone or measure them with more tolerance.
- **The contact sheets of 12 MB and 21 MB** take minutes to raster with Quick Look; to check the row of the White village separately is appropriate.
- **I didn't do a visual check on the app installed**, because Vectorio was open with user work; I just checked the SVG samples.

## Subsequent adjustment

After the evaluation, the weight of the field and the roofs was reduced:
- 0.3 (formerly 0.5), two 0.4 and four 0.15 roofs;
- one third of the olive groves is left blank and the rest is more open (step 4.2 m);
- result in release: White village serial to 38 ms in preview and 39 ms at the end, worst case of 500 seeds 129 ms, frames to 0.53 mm from the edge, and 12 seeds from the leaf pass to 19 - 29 m (formerly 22 - 30 m).
