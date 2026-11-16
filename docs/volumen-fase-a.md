# Volume · Phase A

16 November 2026. Scope: passes in the core, without an interface redesign or new artistic effects.

## Contract

`Pass` stores a name, enabled state, arbitrary ink, visibility, and a parameterized `PassKind`. Supported kinds are outline, edges, sections, field, waves, mesh, hatching, and ground. `Settings::passes()` translates the seven previous modes into stacks while preserving their output names, empty families, order, and historical ink distribution. Atlas does not gain an outline that did not previously exist; X-ray does not gain dashed lines.

`Engine::draw_passes` accepts an editable, repeatable, ordered stack. `Engine::draw` preserves the previous entry point for the app and examples; `Engine::draw_recipe` uses the stored stack. `passes/` adapts the existing algorithms in `pipeline.rs`, `art/`, and `shadow/`, retaining their tolerances, occlusion, simplification, and physical clipping. Fully dismantling those modules is left for later work; this phase wraps them.

Historical Flat shadow hatching retains the union of object and ground in one pass: separating them would change joins and SVG ordering. The independent ground pass reuses existing masks and the hatching phase. Visibility exposes the existing options: visible or complete. Tonal masks and dashed hidden lines remain for Phase D.

The output cache uses source identity, program parameters, and paper. Name, enabled state, ink, and order are excluded from its key. Geometric contexts are shared per facet count and cache sections, fields, levels, edges, and light by parameter; changing occlusion reuses the 3D geometry. Caches retain active programs and can prune history to avoid accumulating earlier edits indefinitely. Projection is recalculated when visibility or composition changes.

`DrawingPart.pen` identifies an ink without restricting it to three roles. `Drawing::items` and `document` accept assignments of arbitrary length; an empty assignment uses the passes' inks. Mesh families support up to 256 inks as a resource limit, distributed by height, depth, or object. Historical orientation classification still distinguishes three axes; no new color algorithm is introduced. The historical Waves and mesh style keeps two mesh families; custom stacks can have more.

## Persistence

The v2 recipe stores `passes` as the authoritative stack. `layer_pens` is removed from the new format; `set_layer_pens` adapts old assignments when saving. `Recipe::from_json` migrates v1 by translating `RecipeKind` and its three assignments, validates parameters and ink references, and preserves the original mesh. Future versions are rejected.

The app retains its historical selector and controls. Only assignment reading and writing are adapted to the new type. Custom-stack display and editing in the interface remain for Phase B; in this phase, custom stacks use the core API (`draw_passes` / `draw_recipe`).

## Evidence

`tests/volumetric_phase_a.rs` compares complete SVGs byte for byte against eleven files captured before the refactor: three from `volumetric`, five from `volumetric_arte`, and three from `sombra_plana`. Each comparison checks the historical entry point, explicit stack, and saved/reopened v2 recipe.

`tests/fixtures/volumetric/recipe-v1.json` is the real recipe emitted by the pre-refactor example, containing the torus, parameters, and inks. The test migrates it and compares its SVG with `atlas.svg`. Other tests cover all seven migrations, invalid versions and inks, a mixed stack with six mesh inks plus a seventh wave ink, per-pass caching, visibility changes, empty stacks, cancellation, and the independent ground pass.

At this phase, B–G remain pending: interface, direct manipulation, masks and tonal improvements, artistic effects, reading maps, and continuity between 3D objects and the document.
