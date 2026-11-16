# Volume · Phase B

16 of November of 2026. Implementation of § 9 of `plan-volumen-rediseno.md`, maintaining the algorithms and styles of Phase A. It doesn't incorporate tonal masks, new hatching patterns or perspective.

## Interface

- Paper as initial view, with 3D box of 92 × 72 pt in a reserved space below: do not cover the paper. Double-click the box to alternate the split view. It preserves the orbit that already existed.
- Scene of 176 pt and Passes of 220 pt. Below 1,100 pt, Scene becomes an accessible icon bar and Passes is opened as a floating sheet. Scene can also be folded manually.
- Chips for all seven styles, with names in Spanish and vector miniatures of the current model as the mouse passes. Miniatures are generated at lower resolution on a cancellable worker. Save style is visible at the end; own styles are retained in `estilos-volumen.json`, separate from photographic styles.
- List of activated passes, ink sample, name, string to sort, add, duplicate and delete. You can also go up and down from Stocks. The selection opens its parameters and attenuates the rest of the strokes on the paper; viewing them all removes that highlight. Exports retain their full trajectories and their physical widths.
- Essential parameters visible and a single Advanced per pass. Model › Advanced is the only folding second level of Scene; there are no third levels.
- No extensive descriptions of modes or code names. The space and the objects placed are called volume. The texts are in `locales/app.yml`, with English translation.
- Calculation and import indicated with a fine, indeterminate bar on the paper. Errors with recalculation action. Foot with strokes, inks, estimated minutes and calculation time; secondary export and Placing on canvas as single accent button in Volume. The global exporter is also secondary in this space.

## State, persistence and calculation

`volumetric_ui.rs` retains the status, imports, worker and shares. The interface is distributed in `volumetric_scene_panel.rs`, `volumetric_passes_panel.rs`, `volumetric_styles.rs`, `volumetric_preview.rs` and `volumetric_stats.rs`. The old controls and the three fixed papers (`role_active`, `valid_pens`) are removed.

Each pass has a stable identity in the app. Geometric requests are sorted by identity and exclude name, ink, order and activation. The view applies this data later, using the corresponding geometric response; it does not confuse previous results with a newly changed stack. The deactivated passes are also kept for activation without further calculation. Changing parameters or camera uses core caches and discarding outdated answers.

Deshacer and Re-Make retain the stack and their identities. The global bar, menu and ⌘Z/⇧⌘Z act on the stack in Volume, and on the active field when editing text. They don't accidentally undo the Design document.

Save, open, export and place using the actual passes, including custom combinations and more than three inks. The v1 recipes continue to migrate through the core. Styles retain their palette and reuse or add to it when applied.

The time estimate uses `output::prepare_cancellable` with the actual ink, speeds and settings of the output, in a separate, cancellable worker. It doesn't block the interface or enter the geometric key.

By combining shadow and other generators, a stable frame is shared, chosen without depending on the order of passes. The projection uses that same camera; historical styles keep their SVGs. A specific test covers shadow and edge alignment and rearrangement without new calculation.

## Verification

- `cargo test`: 314 passed tests, 9 ignored, no failures; includes the eleven SVG references of Phase A, identical byte to byte.
- Custom v2 recipe tests in the app, six mesh inks, metadata without geometrical invalidation, stable identity, undo, style storage and estimation with varying speeds and outdated responses.
- Captures of the renderer from egui to 980 × 620 and 1,440 × 900: light and dark themes, error, calculation, import, selected pass and floating sheet. Action checks in both languages.
- Review of the application installed in `/Applications/Vectorio.app`: selection and editing, octave ink, adding Waves, sort by drag, undo, exporting physical SVG and placing traces. The evidence document was returned blank.
- Unimpaired design detector and format/space verification of the modified files.

Hanging: lighting and drawing manipulation on Phase C paper; D-tone masks and improvements, effects and continuity of subsequent phases. The emphasis requested in § 9 is resolved in B.
