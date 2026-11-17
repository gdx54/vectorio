# Create · Phase 2 delivery

## Implementation

- **Step 0  —  engine:** `scene/{mod,camera,solids,hidden,hatch,shadow,tests}.rs` and
  Your `README.md`. Hatching surface patterns with depth vertices per facet;
  Degenerate input rejection, plane-projected directions and shadows
  Optional; DDA route with overhead coverage; relative filter for faces
  The manufacturer shall ensure that the manufacturer is able to provide the manufacturer with the following information:
  removal of rear occlusers only in convex solids. The Example
  `examples/muestra_escena.rs` groups the turns by tone and frames the strokes
  real in a second pass, maintaining the physical separation of hatching.
- **Steps 1 and 4  —  Generators:** `generate/{mod,blocks,spiro,tests}.rs`, registered
  In `lib.rs`. Declaration schemes, sound serial values, conditions
  of visibility, blocks, seed and context, recording and two generators.
  City blocks delivers six parts; Spirograph, one closed curve per layer.
- **Step 2  —  document:** `generate/document.rs` and `doc.rs`. Persistent recipes,
  a part for unfilled open object, regeneration by party identity,
  Memory of missing inks, independent duplication and re-painting of inks
  erase or reorder the pallet. Ancient documents still omit the field
  `generated` when it's empty.
- **Step 3  —  application:** `create.rs`, `app.rs`, `commands.rs`, `inspector.rs`,
  `main.rs`, `prefs.rs`, `i18n.rs` and `locales/app.yml`. Selector and menu Design ·
  Creating · Output; ⌘1/⌘2/⌘3. Gallery, controls, locks, seed of 64 bits,
  Random/N, metrics, ink preview, placement/updating and
  Cancellation, editing from Layers, preferences and English translation.
  `LEEME.md` collects the mode and shortcuts.
- **Step 5  —  validation:** engine tests, migration of parameters, groups,
  identity/transformations/inks, undo/re-do, outdated results,
  panic capture, translations and layout in 980 × 620 in both languages.
  The two engine measurements and the 500 seeds per generator are tests.
  Default ignored which were executed expressly on release.

## Added public API

```text
Scene::hatch_surface(&mut self, faces: &[Vec<V3>], angle_deg: f64, spacing_mm: f64)
Scene::solid_oriented(&mut self, vertices: &[V3], faces: &[Vec<usize>], smoothing: &[u16], quiet: bool) -> Vec<Vec<V3>>
V3::finite(self) -> bool

Params(pub BTreeMap<String, ParamValue>)
ParamValue::{Number(f64), Text(String), Bool(bool)}
Params::{f64(&str)->f64, int(&str)->usize, choice(&str)->&str, flag(&str)->bool}
Params::set(&mut self, id: &str, value: ParamValue)
Params::normalized(&self, specs: &[ParamSpec]) -> Params
ParamSpec::{Section, Range, Choice, Toggle}
ParamSpec::{id()->Option<&'static str>, label()->&'static str, hint()->Option<&'static str>, shown(&Params)->bool}
Show { id: &'static str, equals: &'static str }
Detail::{Preview, Final}
GenContext { area: BBox, seed: u64, pen_mm: f64, detail: Detail }
GenContext::valid(&self) -> bool
GenPart { name: String, lines: Vec<Polyline>, pen_order: Option<usize> }
GenOutput { parts: Vec<GenPart>, warning: Option<String> }
GenOutput::{lengths_m(&self)->Vec<f64>, total_m(&self)->f64}
From<SceneOutput> for GenOutput
Generator: Sync {
  id(), name(), category(), description() -> &'static str
  params() -> Vec<ParamSpec>
  randomize(&self, rng: &mut Rng, params: &mut Params, locks: &BTreeSet<String>)
  generate(&self, params: &Params, context: &GenContext) -> Result<GenOutput, String>
}
generate::all() -> &'static [&'static dyn Generator]
generate::find(id: &str) -> Option<&'static dyn Generator>
generate::randomize_specs(specs: &[ParamSpec], rng: &mut Rng, params: &mut Params, locks: &BTreeSet<String>)

GeneratedGroup::new(name: String, generator: String, params: Params, seed: u64, area: BBox) -> GeneratedGroup
Document::generated: Vec<GeneratedGroup>
Document::generated_group(&self, id: u64) -> Option<&GeneratedGroup>
Document::put_generated(&mut self, group: GeneratedGroup, output: GenOutput, replacing: Option<u64>) -> Vec<u64>
```

Variants of `ParamSpec` are described by their fields in `generate/mod.rs`. The new `GeneratedGroup` fields include `parts` (object name →), `inks` (optional ink name →) and reference `placement` tables.

## Criteria selected

- The explicit orientation of `solid_oriented` retains all the occlusors. The
  The convex variant continues to orient centerwise and keeps only front faces.
- Open another group of shadows **discards** slope; does not draw passes
  unexpected. Without the sun, you can't open a group. Faces not flat or concave are
  They're discarding it. Rotary profiles can change from convexity, retaining
  Strictly increasing heights and zero radii only at extremes.
- DDA is used both to consult occlusors and to index/consult passes
  It keeps seals for consultation and covers borders and corners.
  Avoid quantifying directions and mistakenly separating nearly equal lines.
- The interface input is an entire text of 64 bits, avoiding losses
  of precision by converting large seeds to a floating numerical control.
- `Params::normalized` applies the scheme when used: fill in missing, remove
  Unknown and hygienic types/ranges. `random: None` excludes a parameter; in
  ranges, `Some((min,max))` allows for a narrower band. `show.equals` admits
  text, numbers or booleans and the prefix `!` for inequality.
- The heights of City blocks are interpreted as the ends of an interval, including
  if they are introduced in reverse order. The spirograph closes with the mcd; radios
  Equals in a hypotrocoid are communicated as an error because they would produce a
  The point. Randomly avoiding that case by respecting the locks.
- The recipe paper is his original area. Local lines are calculated
  Subtracting `area.min`; each object retains its `Placement`. The new objects
  They inherit the transformation of the group's benchmark. The existing ones maintain
  identifier, name, ink, visibility, blocking, transformation and order.
- A single worker receives versioned orders, discards the obsolete tail and
  check the version after the calculation. Preview uses less detail; final
  is calculated before the document is amended. Panic becomes
  mistakes. The document changes only upon receipt of the current Final, in one step
  Cancellation of outstanding results.
- No dependencies were added or SVGeometric code consulted.

## Measurements and visual inspection

Release measurements of this delivery, with tests executed sequentially:

|Load|The time|
|---|---:|
|5,000 expensive / 20,000 short segments|12,933 ms of median (7 renders)|
|500 framed boxes + shadows + 2,000 long diagonal boxes|12,601 ms of median (5 renders)|
|City blocks, 500 random seeds|3,084 maximum|
|Spirograph, 500 random seeds|1,120 maximum|

The 1,000 samples generated ended correctly, with finite coordinates, within area A4 and less than 80 m per sheet. Time depends on the team.

`/Applications/Vectorio.app` was opened after verifying that there was no open Vector. Tested Create → City blocks → Random multiple times → Put → select shade → assign red from the → palette Edit in Create → Random → Update. The shadow kept the red. Deshacer recovered the previous geometry in one step. ⌘3 calculated and showed the group's path in Salida; ⌘2 allowed to check the spirograph as well. The two SVGs of the carcass were inspected with continuous hatching in cylinder, cone and dome.

The capture detected that the Create panel was invading the top bar. The space reserve was corrected and a header and foot position test was added for both languages. This last correction, the exact seed field and part labels are installed for the following start: the test window is kept open, without restarting. That final native-looking review is pending at the next start; the corrected arrangement is verified with the rendering of egui in tests.

Out of range: the 3 stage art direction parts kit and generators. The protection of engine resources limits extreme resolutions; no maximum time is guaranteed for arbitrary or overlapping geometry.
