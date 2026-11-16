# Create · Phase 3 delivery

Implementation in `feat/crear-motor-escena`, with no commit and no new dependencies. No code from SVGeometric was consulted. The application is kept open upon completion.

## Changes by step

- **0  —  revision and engine:** `doc.rs`, `generate/mod.rs`, `generate/blocks.rs`,
  `scene/{mod,hidden,solids,intersections,tests}.rs`, `scene/README.md` and
  `examples/muestra_escena.rs`. Stored without orphan recipes; steps from
  Safe invalid randomness; real output framing with reconstruction;
  Sun height and cone corrected in City blocks. Analytical intersections between
  the front faces of different solids, with spatial index, depth,
  continuity between facets, normal concealment and exclusion of contacts already
  They're drawn. The mortuary includes a fireplace and deliberately interpenetrated boxes.
- **0  —  interface:** `create.rs` and `inspector.rs`. Editing works in copy and
  Restores parameters and locks remembered when updating/canceling. The view
  valid prior remains during Final; errors are shown on the sheet.
- **1  —  style:** `generate/style.rs` White paper with illuminated faces,
  density for medium/dark; threads on walls, maximum slope on skirts,
  angle of paper on the ground and continuous horizontal on revolutionary surfaces.
  Standard parts and suggestions for inks. It has deterministic wavy strata.
- **2  —  kit** `generate/kit/{mod,buildings,landscape,tests}.rs` Four roofs,
  whole hollows, chimneys, chimneys, chimneys, chimneys, bell towers with dome/chapel,
  Stairs, walls / terraces, cypress, olive, bush and cut ground.
  `examples/kit_piezas.rs` shows all the parts in A4.
- **3 - White village:** `generate/village.rs`, registered at `generate/mod.rs`
  and tests on `generate/tests.rs`. Solid terraces bands, level streets,
  Axis of stairs, townhouses, squares/gardens, high church and vegetation.
  Four views, two elevations, roof weights and conditional controls.
- **4  —  Gallery:** miniatures of all generators in `create.rs`, with a
  self-employed and cached in memory per session. Don't share a queue with him
  the main calculation. New texts translated into `locales/app.yml`.
- **5  —  revision:** `examples/hoja_contactos.rs`, extended tests, this document
  and update to `LEEME.md`.

## New public API

The measures of the kit are meters; hatching angles and separations are degrees and mm of paper.

```text
Scene::set_intersections(&mut self, enabled: bool)
Scene::construction_issues(&self, minimum_mm: f64) -> Vec<String>
generate::fit_scene(area: BBox, camera: Camera, build: impl Fn(Camera) -> Scene) -> Scene

style::PARTS: [&str; 6]
style::normal(face: &[V3]) -> V3
style::part(scene: &mut Scene, name: &str)
Style { spacing_mm: f64, detail: Detail }
Surface::{Wall, Roof, Ground}
Style::new(ctx: &GenContext, spacing_mm: f64) -> Style
Style::prepare(self, scene: &mut Scene)
Style::quantum(self, scene: &Scene) -> f64
Style::snap(self, scene: &Scene, value: f64) -> f64
Style::shade(self, scene: &mut Scene, faces: &[Vec<V3>], surface: Surface, ink: &str)
Style::lathe(self, scene: &mut Scene, center: V3, profile: &[(f64,f64)], ink: &str) -> Vec<Vec<V3>>
Style::finish(self, scene: &Scene) -> GenOutput

kit::Roof::{Flat, Shed, Gable, Hip}
kit::TowerTop::{Dome, Spire}
kit::Plant::{Cypress, Olive, Shrub}
kit::v(x: f64, y: f64, z: f64) -> V3
kit::house(scene: &mut Scene, style: Style, min: V3, max: V3, roof: Roof, rng: &mut Rng)
kit::opening(scene: &mut Scene, style: Style, origin: V3, horizontal: V3, width: f64, height: f64, dark: bool)
kit::chimney(scene: &mut Scene, style: Style, min: V3, max: V3)
kit::parapet(scene: &mut Scene, style: Style, min: V3, max: V3)
kit::eave(scene: &mut Scene, min: V3, max: V3)
kit::bell_tower(scene: &mut Scene, style: Style, origin: V3, width: f64, height: f64, top: TowerTop)
kit::stairs(scene: &mut Scene, style: Style, min: V3, width: f64, run: f64, rise: f64)
kit::retaining_wall(scene: &mut Scene, style: Style, min: V3, max: V3)
kit::terrace(scene: &mut Scene, style: Style, min: V3, max: V3, rng: &mut Rng)
kit::terrain(scene: &mut Scene, style: Style, min: V3, max: V3, rng: &mut Rng)
kit::vegetation(scene: &mut Scene, style: Style, origin: V3, height: f64, plant: Plant, rng: &mut Rng)

generate::find("white_village") -> Option<&'static dyn Generator>
```

## Decisions

- Intersections are calculated in (x, y, depth), taking advantage of the planes
  already projected. They only compare faces that share cells and ranks. Each solid
  It has identity; loose faces and solid quiet ones do not produce encounters.
  An encounter belongs to the most recent part of the solid. The segments
  adjacent pairs are joined with tolerances of 1e-7 mm and 1e-8 m, and are added
  after the requested strokes. Separation between parties shall be respected.
- A contact is omitted only if its segment is covered by an edge that
  It's actually broadcasting. A smooth edge between facets does not prevent an intersection.
  Deduplication keeps the first passes inside each part.
- The tones of a lathe are determined by full meridians using the normal
  the nearest facet at half-height. The border is avoided on stairs without
  Altering the geometry. Of course it doesn't have hatching; medium/dark have different density.
- The family retains six parts: `contorno`, `detalle`, `trama`, `sombra`,
  `vegetación`, `terreno`. The stairs belong to the land, to share the
  terrace contacts without repeated passes. Contour/detail/hatching/terrain suggest
  The first ink, vegetation the second and shade the last.
- Heights and locks use a minimum unit of 0.5 m and 2.5 mm of paper. The Commission
  platforms occupy contiguous ranges; the last steps reach the next
  terrace, without penetrating it. The bell tower is clearly framed on the ship and is
  separates it from its outer plans. `construction_issues(1.5)` checks the faces
  Leaders in testing/cleaning; 500 seeds pass without warning.
- The gaps are omitted below the actual projected 1 mm and kept whole.
  The door reserves space in the lower top view. Preview omits those fine details.
- The serial front view looks from the lower slope (yaw 225°).
  The church is on the highest terrace and has a fuse of 10 m to preserve
  their hierarchy. The variation leaves sunsets and orchards; not every batch is filled.
- The new City blocks cone has a slope of some 60.5°, different from the
  Supplements of the two elevations. City blocks recipes stored in phase 2
  They receive `scene_version=2` when charged and maintain style, frame, sun and cone
  previously. The new recipes use the 3 revision. There's evidence of regeneration.

## Verification and timing

`cargo test --workspace`: **155 correct tests**, including the API example; four load tests were run separately on release. Clippy ends without notifications in the added code; previous notifications remain in other modules.

Last release measurement, sequential, on useful A4 of 180 × 267 mm:

|Load|Measurement|
|---|---:|
|Series white village, preview|8,162 maximum mean; 12,767 maximum|
|Serious White village, Final|6,518 maximum mean; 7,059 maximum|
|White village, 500 random seeds|14,474 maximum|
|City blocks, 500 seeds|7,799 maximum|
|Spirograph, 500 seeds|1,127 maximum|
|Engine: 5,000 front / 20,000 segments|12,042 more than median|
|Motor: 500 framed boxes, shadows and diagonal 2,000|12,514 more than median|
|The same load without intersections|12,242 ms; activated excess cost: 2.2 %|

1,500 random outputs are finite, remain in the area and do not reach 80 m. The 500 of White village also does not activate the diagnosis of coplanar slices or overlaps. The serial output occupies more than 90 % of a useful dimension and is below 40 m. The timing depends on the equipment, it's not a guarantee for an arbitrary scene. Preview and Final were measured consecutively; the different caches and hatching patterns explain that Preview is not always the fastest.

We inspected the kit, the two mortuary chambers and the 36 contacts. In `/Applications/Vectorio.app` the gallery was checked with miniatures, White village, Random multiple times, Place, change from `vegetación` to green, Edit, Random, Update and Output. The party retained the green (#009460). By re-creating, the remembered settings recovered the previously edited seed; the updated recipe retained his. The application is left open with the test group.

## Contact and evaluation sheet

Reproducible artefacts in `target/phase3/`:

- `hoja_contactos.svg`: 4 × 3 seeds per generator, 36 drawings in total. The 0
  uses the serial parameters; 1  –  11 uses random parameters with its seed.
- `contactos_pueblo.png`, `contactos_manzanas.png`, `contactos_curvas.png`: views
  of checking by generator. The curve thumbnail reduces sampling to
  quickly raster it; the full SVG retains all the points.
- `kit_piezas.svg`, `muestra_escena.svg`, `muestra_escena_dimetrica.svg` and their PNGs.

In White village I consider usable without touch 10 of the 12 contacts. The **0** features well streets, central void and church; **2 and 4** balance focus with houses and vegetation; **8** has a clear cover rhythm. **1 and 11** are looser: the rear view makes the terrain slice dominant and hides streets and facades. **6** is very churchish and has less urban fabric, although it reads clean. It's an evaluation of this leaf, not an aesthetic evaluation of 500 seeds. Automatic testing only accredits geometry and time.

City blocks remains a geometrical demonstration: scarce versions, such as the 6, function as a volume study but not as a village. The spirograph provides contrast and stroke continuity; some combinations of many cycles remain dense even below 80 m.

The cups and holes are deliberately simple; they do not yet have the organic or constructive character of a hand-drawn illustration. The style already has a consistent family, but those two pieces would admit a next artistic pass. Castle, stadium, shadows on objects, figures and text are left out.

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo test -p vectorio-core --release -- --ignored --nocapture --test-threads=1
cargo run -p vectorio-core --example kit_piezas --release -- target/phase3/kit_piezas.svg
cargo run -p vectorio-core --example hoja_contactos --release -- target/phase3/hoja_contactos.svg
cargo run -p vectorio-core --example muestra_escena --release -- target/phase3/muestra_escena.svg --dimetric
python3 scripts/install-macos.py
```
