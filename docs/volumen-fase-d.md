# Volume · Phase D

16 of November of 2026. Vector tone, hatching by levels, hatching that follows the form and a discontinuous hidden line.

## Use

The gallery incorporates four styles:

- **Engraving**: sections in object space, with increasing density in three dark bands and contour.
- **crosshatch etching**: four cumulative hatching families, to 0 °, 45 °, 90 ° and 135 °, plus contour. The lighter areas stay like paper.
- **Technical drawing**: visible edges and discontinuous hidden edges in independent passes and inks.
- **Topography in shade**: height curves cut into the shade, more contour.

In **Add pass** are available **Level hatching** and **Form hatching**. You can choose from two to four levels. The first allows you to adjust separation in millimetres and angles per level in Advanced; the second allows you to choose slides and X/Y/Z direction. Their plans accompany the rotation and XYZ scale of the object, rather than being connected to the screen.

All passes have **Tonal mask**: All surface, only in light, only in shadow or light band, bounded between 0 and 1. The intensity is N·L per face, with coherent norms and occlusion of light: the shadows that another component casts on the object are zero. Light and shadow use the threshold of the scene. In shared borders, the darkest face commands to avoid a review of the same edge by both masks.

Masks describe the surface visible on the paper. Therefore, a mask over a pass with hidden lines or all lines cuts against that projected surface. Hatching patterns of regions and soil do not generate rear lines; the discontinuous occult selector is offered in 3D line passes.

For hidden line, select **Discontinuous Hidden** in Visibility and adjust **Guion (mm)** and **Hueco (mm)**. The pattern maintains its phase by passing through the vertices of each polyline. The intervals are retained during analytical occlusion; they are not approximated by subtracting 2D drawings.

## Core and persistence

- `pass.rs`: `TonalMask`, `DashPattern`, `ToneHatchParams`, `HatchMode`, `PassKind::ToneHatch` and `Visibility::Hidden`.
- `shadow/tone.rs`: visible and shadowy regions, spatial index for cutting segments, joining intervals, hatching tonal patterns and physical scripts. It reuses the analytical polygons and the global phase of the existing shadow; it does not introduce rasterization.
- `engine.rs`: cache of tonal results independent of names, inks and order; changing a mask reuses the pass geometry. The light and mask parameters invalidate the affected results. Caches and shredded fragmentation, with cancellation.
- The `scene` renderer optionally retains the hidden intervals. The default behavior of the Create generators does not change.
- **v2** recipe, compatible with previous v2 recipes without the new fields and with existing v1 migration. Masks, script patterns and hatching parameters are also stored in styles. Removing/re-making retains the changes.
- Controls in Spanish and English, within the existing contextual editor, without new levels of folds. The narrow sheet allows access to controls by vertical scrolling.

## Checks and inspections

- `cargo test`: **341 Tests passed, 9 ignored, no** failures.
- The eleven historical SVG references retain byte-to-byte matching, including migration from an actual v1 recipe.
- Light/shadow partition in waves, mesh and curves; exact boundaries and gaps; shadows between components; cumulative families by level; curved sections and transformation of their planes; complementary visibility/hiddenness and scripts in millimetres.
- Finite and paper-cut vector export, determinism, ink/name/order-free cache, validation of ranges and compatibility with previous v2.
- Style selection, mask editing, recipe and undoing; stocks and masks accessible to 980 × 620 and 1,440 × 900 in Spanish and English.
- Captures of the four styles in both sizes, light and dark themes: `VECTORIO_QA_TONE=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture`. Files in `target/volumetric/{grabado,aguafuerte,tecnico,topografia}-{980,1440}.png`.
- Interface detector with no finds and `git diff --check` with no errors.

The installation is updated with `python3 scripts/install-macos.py` on `/Applications/Vectorio.app`, without closing or restarting the open session. Captures are generated using the same egui code and core geometry; the new version will be available at the next start.

## Suspended

The tone of this phase is constant per face; it does not implement curvature fields or flow lines. According to the updated order of the plan, it follows **G · Reading maps** before **E · Artistic finish**; then **F · Continuity**. No halos, contour echo, seeded tremor, ink gradients, perspective or 3D objects are added to the document.
