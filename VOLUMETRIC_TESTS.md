# VOLUMETRIC · verification

## Commands

```sh
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cargo test --workspace
cargo test -p vectorio-core volumetric
cargo test -p vectorio-app volumetric_ui
cargo run -p vectorio-core --release --example volumetric -- target/volumetric
# With a real file exported by Blender or another modeler:
cargo run -p vectorio-core --release --example volumetric -- target/volumetric modelo.stl
python3 scripts/install-macos.py
```

The example generates ATLAS, STRATA and X-RAY as physical SVGs and JSON recipes without interface or plotter. The tests are local and do not call outside services.

## Automated cases

- Cube: edges without diagonals, several cameras, horizontal/vertical/oblique slides, slides matching coplanar faces and orthographic projection invariant to depth.
- Sphere: continuity/closure in three directions, density; visible length less than complete; result unchanged when all faces are reversed.
- Toroid: two horizontal rings, vertical sections with two components, transition to one component and retained hole.
- Objective: concave polygon, relative indexes, objects, degenerate and repeated faces; errors with NaN, absent/zero indexes. Open mesh, disconnected, duplicate vertices and non-manifold edges.
- STL: ASCII and binary disc fixtures, binary with header  « solid », face/size counter, truncation, NaN, triangle limit and several ASCII solids. Real asynchronous import and failure retaining the previous model.
- Seven primitive: validation and size limits.
- SVG: parse with usvg, physical units 420 × 297 mm, Inkscape layers, with no embedded images or NaN/inf; all trajectories within the margin even with zoom, displacement and spin; deterministic output in all three presets.
- Recipe: exact equality of f64, polylines and parameters after saving/reopening a curved mesh; serialization/deserialization, rejection of invalid versions or parameters and enclosed resources.
- Caches: orbit/composition retain the object's mesh and curves; transforming the model or changing the field invalidates if necessary. Change of ink doesn't change the geometric version.
- Asynchronous work: core cancellation, worker retaining only the most recent order and discarding obsolete answers.
- Interface:  « According to the camera »  uses the current angles without waiting for an earlier response; selector distinguishes imported models and allows returning to the previous primitive; errors contrast at least 4.5: 1 in clear and dark panel. Accessible shares to 980 × 620 and 1440 × 900, Spanish and English; complete regression of previous functions using the workspace test suite.

## Scope of evidence

The tolerances are geometric, not pixel comparisons. Triangles with inverted winding do not change the occlusion of sections; incoherent norms remain a limit of X-RAY silhouette classification. The tests do not guarantee repair of arbitrary meshes, absence of self-intersections on faulty models, perspective, GPU performance or accuracy of the distance field within each face.

Pre-existing benchmarks and catches marked `ignored` are not considered to have been executed by `cargo test --workspace`. The visual checks and measured delivery times are noted at the end of this document.

## Delivery checks (15 November 2026)

- Final suite: 66 app tests, 216 core and 1 doctest (283 in total); ignored 9 are not part of that total. It includes the two additional tests after independent review.
- Manual capture executed: `cargo test -p vectorio-app render_volumetric_workspace -- --ignored`. Results in `target/volumetric/workspace-980.png` and `workspace-1440.png`; additional catch of dark error and imported model to review corrections.
- Example release, toroid of 3,072 triangles, 48 levels / 96 curves: ATLAS 22.0 ms, STRATA 4.5 ms and X-RAY 3.7 ms on this machine, cached shared on the last two calls. It is a measurement of that example, not a promise for any imports.
- `/Applications/Vectorio.app` installation compiled in release and ad hoc signature verified by the installer. No user sessions were closed or movements were sent to the plotter.
- Check with CUA in the installation: open VOLUMETRIC, import `cube-binary.stl`, change the Z axis →X and density 48 →24; see the change; place the two parts in Design and get in Out a route of 5.3 m, two inks and about three minutes estimated. Physical execution is not done.
- Attempts were made to open Illustrator to check external compatibility, but the app failed to open and the call ran out of time. External compatibility remains unchecked; SVGs do pass parse and suite vector controls.

- SVG exported from the app dialog: `target/volumetric/stl-cube-ui.svg`, XML read after saving: 420 × 297 mm, 24 traced, 2 layers and 0 raster images. The name introduced by automation was initially concatenated to the suggested; the disc test artifact was renamed.
- The independent review gave `ship` in its pass of the three findings: error contrast, selector after import and current camera direction. The catch and testing support these corrections; it is not interpreted as future capability certification.


## Dragging and widening of models

- Tested drag using real events `egui::RawInput::dropped_files` for OBJ and STL (also capitalized) from Design, Create, Output and VOLUMETRIC. Access to Solo3D, loading of the mesh, closure of import and complete preservation of the document shall be checked. A second import replaces the previous one and does not receive its obsolete error.
- Mesh quality: subdivided sphere of more than 40,000 triangles; geometric reduction, closed contours, return to all triangles with Original, independent screen proxy and recipe that retains the full source. Validation and readability tested.
- Current limits: 512 MB per file/recipe, 5 million faces and 15 million input vertices (STL without indexing). The matching vertices are compacting. The notices of 16 MB/ 8,000 polygons are no longer the current limits.
- Release measurement with two local user STLs, read without modification: 1,499,605 triangles / 71.5 MiB → load/validation 1.19 s, ATLAS 2.03 s and 40,000 workloads; 209,872 triangles / 10 MiB → load 0.11 s, ATLAS 0.15 s and 39,998 workloads. SVG is generated with the three presets in `target/volumetric-large` and `target/volumetric-calpon`; no large recipes are duplicated for this verification.
- Captures of the large model and access to Solo3D: `target/volumetric/large-1440.png` and `drop-980.png`. Reproducible generation: `VECTORIO_QA_MODEL=/ruta/modelo.stl cargo test -p vectorio-app render_volumetric_workspace -- --ignored`. She doesn't touch work windows.
- Battery of this expansion: 68 application tests + 218 core + 1 doctest = 287 pass; nine ignored. Ignored manual capture does run separately.
- You haven't measured the end of five million faces on Original. That option retains complete geometry and can take substantially longer than interactive quality. The individual call for native simplification is not interrupted internally; its result is discarded if the request becomes obsolete.

- Review of this expansion: permanent notice of quality/reduction with shares where applicable, verified in 980 × 620 and 1440 × 900. The independent review gives `ship` his pass on that finding.


## FLAT SHADOW · lighting delivery

- Seven core-specific tests: total/partial occlusion cutoff; a light-oriented face that receives shadow from another component; own shadow and ground in a single SVG layer; consistent orientation of normal in closed components with mixed winding; cached light field while orbiting; light change and cancellation; reversible faceting and deterministic output retaining the original. The junction of intervals does not overlap the joints and removes shade from the ground covered by the object.
- Interface: The tests for 980 × 620 and 1440 × 900, in Spanish/English, include the new preset. Alternating masses/strokes and changing ink does not invalidate geometry; changing light does. Final suite: 69 app + 225 core + 1 doctest = 295 tests pass; nine ignored. The manual capture shall also be executed separately.
- Real captures of egui triangles without opening windows: `VECTORIO_QA_SHADOW=1 VECTORIO_QA_MODEL=/ruta/modelo.stl cargo test -p vectorio-app render_volumetric_workspace -- --ignored`. `target/volumetric/shadow-*.png` files: small dark, large light, error, imported STL, large STL, 3D view only, trajectories and second tone/contour. The last case opens  « Finished and strokes »  by pointer input over the observed control.
- Example: `cargo run -p vectorio-core --release --example sombra_plana -- target/volumetric [modelo.stl]`. It generates three SVGs: one ink, side light and two contour tones. In 3,072 toroid faces: 1,200 facets, 1,089 strokes, initial 16.5 ms. With local user STL of 1,499,605 expensive: load 0.83 s, initial interpretation 975.8 ms, 1,200 facets / 2,177 strokes; later light switch 19.7 ms and second tone/contour 20.2 ms. Measurements of this machine and these models, no general guarantee. The input files are not modified.
- Independent visual review of the seven states of the new mode gave `ship`, with no material findings. It is a revision of this extension; it does not replace a complete source audit or a physical plotter execution.
- `/Applications/Vectorio.app` updated by the installer, signature verified; the Mach-O code/data sections match the release compilation. The user's open session is kept intact, which keeps its version until the next start. Visual verification in this mode uses real headless captures of egui, it is not presented as a CUA route of the installed application.


## Three artistic languages · 16 November 2026

- WAVES DE SUPERFICIE, MESH CROMATIC and WAVES + MESH use the production core, parameters in recipes and ink per function. Six art tests: distances following the toroid and origin by disconnected component; winding independence; deterministic variation, change of origin and interference; each edge and diagonal of the cube exactly once with the four chromatic criteria; physical SVG, cut with turn/zoom/position, documents and exact reconstruction after opening recipes with new engine; concealment of the drawn mesh, number of families, cache and cancellation. The opening of ancient recipes without artistic parameters is also checked.
- Interface tests for both languages and 980 × 620 / 1440 × 900 include all three modes. Active roles of inks are verified and that changing the allocation of ink does not require geometry. Full final suite: 70 app + 231 core + 1 doctest = 302 tests pass, nine are ignored. Ignored capture is explicitly executed as well.
- Real footage of egui: `VECTORIO_QA_ART=1 VECTORIO_QA_MODEL=/ruta/modelo.stl cargo test -p vectorio-app render_volumetric_workspace -- --ignored`. Ten states in `target/volumetric`: three light modes to 1440, dark mesh/combination to 980, interference with advanced controls, ink assignments and each mode with large STL. The opening and displacement of the groups is done by real egui pointer and wheel events. User windows are not altered.
- Example release `volumetric_arte`: Local STL of 1,499,605 expensive, load 0.85 s; 986.4 ms waves with 2,700 expensive and 275 curves; mesh 4.1 ms / 4,049 edges / three layers; combined 8.7 ms / three layers; interference 6.7 ms; mesh of one ink 4.1 ms / one layer. The latest calls share the mesh and part of the cache; they're local measures, not guarantees for any file. SVG of 420 × 297 mm without raster images on `target/volumetric-arte`.
- The independent visual review of these ten states gave `ship`, without material corrections, within reach of this extension. He demonstrated controls and core and did not repeat the full test audit. Preserves the existing visual system; PRODUCT.md/DESIGN.md and a new visual world are not part of this scope.
- `/Applications/Vectorio.app` installation updated with the authorized installer, verified signature and 14 Mach-O sections of code/data matching the binary release. The open session retains its version until the next start. A CUA tour of this installed version or a physical plotter test is not claimed.
