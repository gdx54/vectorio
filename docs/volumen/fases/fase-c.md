# Volume · Phase C

16 of November of 2026. Direct manipulation of camera, light and composition; the interface of Phase B and the existing pass core are maintained.

## Use

- Drag into the 3D frame orbiting the camera. Double click retains the alternation with the split view. The camera block prevents the spin, and the arrows allow the angles to be adjusted when the view is in focus.
- Light incorporates a sphere with a pulling sun: the angle around the center determines the azimuth and the distance to the center determines the elevation. Near the center, the sun is high; near the edge, low. The elevation is maintained between 5 ° and 85 °. The arrows adjust the light and the numerical controls are still available.
- Dragging on the paper displaces the drawing in millimeters. The wheel and pin climb around the cursor, retaining the point being pointed. The composition scale is between 0.05 and 3; it does not change the model's XYZ scale.
- The gestures do not act on the paper when using the floating sheet of Passes or when moving the pointer over the panels. The views display accessible labels, keyboard focus and short aids in Spanish and English.
- The highlight of the selected pass, already made in B, is also retained while the drawing is updated. It does not modify the exported SVG.

## Status and response

`volumetric_interaction.rs` focuses on the conversion of display points to millimeters, anchored zoom, interim view composition, orbit and light control.

As the worker completes the exact projection, the last drawing is transformed immediately to accompany the displacement and scale. The coefficients of that transformation are calculated once per frame. The output still awaits a valid response from the worker and maintains the final physical clipping; the provisional view is not exported.

The shared light updates the hatching, floor and shadow contour passes, including the off. The v2 recipe keeps the camera, light, scale and position values unchanged.

The history includes the scene settings in addition to the stack. A drag is a single step to undo even if it is kept pulsed and still; the wheel brings together successive changes. Deshacer and Refacer are not recorded as a new edition. ⌘Z/⇧⌘Z act on the scene when the focus is on the paper, 3D view or the sun; the text fields retain their own undoing. Choosing or importing a new model discards the history of the previous scene.

## Checks and inspections

- `cargo test`: 325 tests passed, 9 ignored and no failure.
- Physical dragging of the paper, undoing/re-doing a gesture with pause, orbit with and without block, absence of accumulated movement while keeping the cursor still, wheel and local pinch to the paper.
- Zoom angle even when reaching its limits, conversion of light coordinates and boundaries over the horizon, drag and keyboard of the sphere, light propagation to the passes and recovery from recipe v2.
- Drag light inside the sliding panel and consecutive arrow pulses without losing focus. In the installed app, click orientation and repeated keyboard adjustment were checked; the drag is covered by egui event testing.
- The floating sheet consumes its gestures before the paper; a language-changing scenario takes language blockage to avoid interference between tests.
- Captures of egui in sizes 980 × 620 and 1,440 × 900, including panel and sheet light, displaced and scaled drawing, light and dark themes.
- The 11 SVG references from the previous phases continue to match byte to byte. You don't change the geometrical algorithms of the core.

Hanging: Phase D, with tonal masks, hatching by levels, hatching that follows the shape and a discontinuous hidden line. Other artistic effects and continuity with the document remain in their later stages.
