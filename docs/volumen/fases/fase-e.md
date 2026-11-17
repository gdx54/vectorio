# Volume · Phase E

16 of November of 2026. Finished vector art by pass.

## Use

The gallery incorporates **Waves with halo**, **Eco contour**, **Stroke by hand** and **Waves with gradient**. Choosing them replaces the stack and allows undoing.

The pass editor contains **Artistic finish**, at the same level as its other sections:

- **Halo by concealing (mm)**: opens up space in the later stroke next to a nearer silhouette. It keeps the front stroke and keeps the distance when crossing vertices. Available for contour, edges, field, waves and mesh with visible lines.
- **Contour echoes** and **Echo movement (mm)**: up to twelve copies; positive outward and negative inward displacement. A copy to 0.2 mm allows the contour to thicken. In closed contours the orientation of outer and hollow areas is respected; in open profiles the displacement is lateral. With gradient, echoes use the chosen base ink and the original retains its color distribution.
- **Handshake**, softness, irregular length and small gaps: soft noise in millimeters and reproducible variation by scene seed and a pass-specific seed. Changing inks, names or order retains the result.

In **Actions → Apply ink gradient**, choose height, depth or light. It reuses the reading maps of G with transition blurred by vector alternation; the map editor allows the inks to be adjusted, including a two- or three-ramp. It is applied to strokes on the visible surface. Specialized, custom-made views retain their specific controls.

The halos are not offered in displaced sections, 2D hatching patterns, floor or composite reading views. Echoes are artistic displacements: a large displacement over a concave silhouette can produce crosses.

## Core and persistence

`volumetric/finishing/` contains the finish, the echoes and the sown variation. The halo expands the occlusion analytical intervals by stroke length, also in closed curves. The rest of the finish works after the masks and the ink distribution, with a cut to the printable area. Legends retain their geometry and echoes outside the paper are eliminated.

The finishing cache reuses geometry by changing metadata and avoids repeating previous calculations by modifying tremor or echoes. Stroke/point and cancellation limits are maintained. The entire output is still vectoral.

The recipe is still **v2**: adds finishing settings with default values disabled. It loads previous v2 recipes without these fields and maintains v1 migration. Save styles, recipes and undo/re-do retains the settings.

## Checks and inspections

- `cargo test`: **379 exceeded, 9 ignored, no** failures. The eleven historical SVG references remain identical byte to byte, including the migration of an actual v1 recipe.
- Proofs of the depth and distance of the halo, continuity between vertices and closures, external/interior echoes and gaps, double contour to 0.2 mm, shallow and reproducible noise, gaps, independent seeds, caches, legends, physical clipping and finite export.
- Persistence testing, undo/rework, ink reassignment and access to all four styles in Spanish and English to 980 and 1,440 px.
- Revised footage of the four styles and narrow panel: `VECTORIO_QA_FINISH=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture`. `target/volumetric/finish-*.png` files.
- Interface detector with no finds and `git diff --check` with no errors.

Installation by `python3 scripts/install-macos.py` at `/Applications/Vectorio.app`. The open session is retained; the new version is used at the next start.

## Suspended

**F · Continuity**: Reeditable 3D object in the document,  « Open in Volume »  from Create, multiview sheet and perspective. Optical mixing extensions of G and new isolines scalars remain pending upon delivery; they are not part of E.
