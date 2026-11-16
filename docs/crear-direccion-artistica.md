# Create · Art direction and procedural architecture

14 of November of 2026.

## Results

The Create record goes from five to seven generators. **Megastructures** offers a stratified tower, inner courtyard and large portico. **Port and Citadel** offers Fishery, Stores and Citadel. Recipes retain seed, parameters, locks, objects and ink identity when stored in the document.

`generate/composition.rs` provides seeds per plot, rhythms grouped with pauses and a height wrapper relative to the camera. The port frees the square and the transverse street, groups houses and moorings and reserves the place of the towers. Megastructures alternate bands of unequal heights, enclosures, galleries, slabs and gaps. City blocks receives covers and a height hierarchy; the city better organizes the succession of neighbouring heights.

## Motor and materials

`generate/massing.rs` builds a cell mass, with filling and emptying. It extracts its skin in six directions and fuses coplanar cells from the same material into rectangles. The edges are calculated before the faces merge: T-joints and inner cells do not produce a grid of lines. The adjacent sections are joined before going through the analytical concealment of the existing engine. The shadow is the union of the shadows of the skin, preserving the voids of the building; it is not replaced by the envelope of the entire tower.

The shared style adds **Ink Intensity**, from 0 to 1.5. Increases the presence of covers, vegetation and shadows; adapts the main hatching to a minimum of 1.45 times the width of the pen. The dark skirts receive a second, more open family. Faces in the sun keep white paper. The horizontal covers also get color and the windows keep the hatching away from the wall.

The dense scenes reserve the roof to the warm paper of the people and the shade to the cold paper of the water, instead of returning both to the same hatching. Classic pen games continue to use black, blue, red and green, with optional yellow. The new simple scenes offer five serial inks. The user's choice and changes of inks are still preserved.

Technical reference studied: [SVGeometric, from jarvisar](https://github.com/jarvisar/geometric), especially the orthographic engine, grid mass and composition by bands and families of details. The implementation added here is Rust proper; it does not incorporate files or a literal translation of JavaScript code.

## Validation

- `cargo test --workspace --release`: 223 tests passed; eight tests of
  Manual checks ignored by default.
- Explicit drilling of 500 seeds by each of the seven generators:
  3,500 drawings, finite, within the area and below 80 m. Worst times
  in that sweep: Megastructures 35.76 ms and Port 13.66 ms.
- Additional cases of the six new compositions, the four views,
  Vertical and flat sheets and pens of 0.25 and 0.7 mm. It is verified
  the recovery of geometry, parts and inks after saving.
- Combined mass test: nine edges in a block without internal joints;
  Emptying a corner reveals additional geometry and is deterministic.
- Richness test: more ink length than zero intensity and colours
  Different for cover and shadow in series games.
- All examples compiled in release. Clippy ends with announcements
  existing outside the modules added.
- Nine sheets reviewed in `target/art-direction`; SVG and PNG with inks
  And real game widths. The example `muestra_receta` supports `palette=`
  and `pens=`. Captures of the egui itself for Megastructures in 980 and 1440 px
  in `target/phase5`, without touching the user's open session.

In the square sample of White village, seed 4, is changed from 22.37 m before the change to 30.62 m, with more presence of roofs and shadows. It's not a universal measurement: length and frame change with paper and parameters.

## Installation

`python3 scripts/install-macos.py` has been executed. The release compilation ended correctly, but the creation of the preparation folder in `/Applications` was blocked by the environment permissions (`Operation not permitted`). The previous application was not replaced and no windows were closed.

To complete the installation from Terminal on the Mac:

```sh
python3 /Users/raullorenzo/Documents/Vectorio/scripts/install-macos.py
```
