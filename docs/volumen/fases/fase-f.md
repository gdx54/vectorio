# Volume · Phase F

16 of November of 2026. Continuity between Create, Volume and Design, multiview sheet and perspective.

## Use

- **Put on the canvas** keeps a 3D object next to its strokes, with model, recipe, passes and inks. In Design, double-click your strokes or choose **Edit in Volume...** in the inspector or in your group menu in Layers.
- **Upgrade 3D object** replaces the strokes in their position of the stack and retains the transformation applied in Design. Reopening does not alter the document. The inks retouched in Design are incorporated into the recipe; blocked objects must be unlocked before updating. Placing and updating have their own step of undo/re-do.
- In **Create → Open in Volume**, the six 3D scenes become a real mesh, including the rear faces. Preparation takes place in a worker. The 2D spirograph generator doesn't offer this action. It imports the geometry constructed by the scene, without attempting to reconstruct volume from its 2D drawing; details that are only lines and the hatching patterns of Create do not become surfaces.
- In **Scene → Paper → Views on the** sheet, choose a view, top view/elevation/profile/isometric or a full turn from 2 to 12 views. The separation is expressed in millimeters. Each cell has a camera, cut, pass and ink distribution; the output remains a single vector sheet. The four technical views use orthographic projection.
- In **Scene → Camera → Perspective**, adjust the focal length and near plane. Distances are relative to the size of the model. The unique view and the turn support perspective; the previous recipes remain orthographic.

## Document and recovery

`Document.volumes` keeps recipes and links between passes, ink families and objects. Duplicate creates a separate group; reorder or delete inks remakes recipes. Save eliminates metadata from groups without strokes, and upload valid recipes, identifiers, and transformations. The project is still compatible with previous documents without 3D objects.

The application saves atomic automatic copies in second plane, in `~/Library/Application Support/Vectorio/Recuperacion`, separated from the user file and with one file per session. When restarting, offer to recover an interrupted session, discard your copy or leave it for later. It does not offer copies belonging to processes that are still open. Save or exit confirmed clears the copy of that session. The tests use a temporary folder and do not alter the user's copies.

## Projection and composition

`volumetric/camera.rs` defines Volume's own camera and its adaptation to the shared analytical engine. The perspective uses projective depth, cuts polygons and segments against the near plane before dividing and corrects the interpolation weights of the reading maps. The conversion of engine units also retains focal length and close plane. Create's generators keep their orthographic camera and their previous outputs.

`volumetric/continuity.rs` composes the views with camera caches and preserves the ink families. The capture of the full Create surfaces is activated only during conversion; normal renders do not store that additional geometry. The camera changes, composition and perspective are stored in the v2 recipe and in the volume history.

## Checks and inspections

- `cargo test`: **393 exceeded, 9 ignored, no** failures. The eleven historical SVG references remain identical byte to byte, including the actual v1 recipe.
- Persistence of the 3D object, updating in the same position, design transformation, inks, duplication, blocking, removal and undo/re-do.
- Automatic recovery with full 3D recipe and rejecting invalid projects without deleting them.
- Conversion of the six 3D generators; proof of a cube with the twelve sides triangulated and without open edges, keeping the usual rendering identical.
- Leakage points, projection/reverse, projective depth, occlusion and close plane; the seven historical algorithms with perspective produce finite and cut strokes.
- Technical and turntable sheets, color, physical clipping, determinism, persistence and cancellation.
- Revised seizures of 1,440 × 900 and 980 × 620: `VECTORIO_QA_CONTINUITY=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture`. `target/volumetric/continuity-*.png` files. Corrected the width of the floating blade of the complete turn.
- Interface detector with no finds and `git diff --check` with no errors.

Installation with `python3 scripts/install-macos.py` on `/Applications/Vectorio.app`, keeping the session open. The new version is used at the next start.

## Suspended

The A – F phases of the plan are being implemented. G extensions over optical mixture and isolines scales are completed in [G] Expansion (volumen-ampliacion-g.md). Curvature fields for flow lines, suggestive contours and GLB/glTF remain later extensions of the plan.
