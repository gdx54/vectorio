# VOLUMETRIC · exploration of artistic languages

Study of the 15 of November of 2026. **Initial study and already integrated modes**: FLAT SHADOW, SUPERFICIE WAVES, CROMATIC MESH and WAVES + MESH. HTML comparison retains the initial prototype; the application now has its own controls, recipes and export. The user is interested in the three directions (lines on volume, mesh of various inks and combination) and adds lighting with projected shadows.

## Addresses

- **Waves and interferences.** Distances propagating along the edges of the surface (Dijkstra, geodesic approximation), converted to isolines. A coherent spatial field alters the path without removing the curves of the triangles. Control source, density, deformation and seed. Evolution: tangent fields/curvature and integration of flow lines for more directional engraving; do not confuse the waves demonstrated with that still pending technique.
- **Chromatic mesh.** Draw the edges of the working mesh, including triangulation diagonals. Artistic complexity independent of the import limit. In the prototype, inks are separated by orientation; height, depth and coherent regions are also proposed. The original model remains independent of the simplified artistic mesh. In order for its edges not to disappear like inner strings, the occlusion must use the drawn mesh itself.
- **Lines + mesh.** Superponer waves and structure with different roles for inks. Control the balance, complexity and visibility of each family. The trajectories and assignments of ink must remain separate.
- **Light and shadow.** Directional light with azimuth and height; receiver plane and position relative to the object; shadow as contour or hatching. The prototype calculates shadows over the ground with three azimuts and elevation of 50 °, reusing geometric projection and joining intervals of `scene::cast_shadow` / `hatch_shadows`. It allows the shadow to be isolated or accompanied by the volume. The object's own shadows then require a proof of visibility with respect to light, not just classification as normal. Light can also govern the density of an engraving on the surface.

## Comparative test

Same local STL used to test large models, reduced by the existing engine to its preview mesh (~20,000 expensive). Common camera orthographic. Curves, artistic meshes of three densities and shadows are calculated; comparison allows changing representation, complexity, number of inks and direction of light. You don't change the app or the source of the model.

Reproducible artifacts in the view directory of this chat: `volumetric-art-study.rs`, `volumetric-art-data.json` and `volumetric-lenguajes.html`. The prototype links the actual Rust core, reuses import, sections and occlusion and is not a secondary production engine. The prototype's geodesic distances have since been integrated into the core, with caching, controls, cancellation and testing.

Tests: finite geometry, three different shadows, real vector lines and comparison fragment less than 1 MB. The variants are steering tests, not an Original mesh performance certification or a full implementation of curvature engraving.

## Technical basis and suggested order

Orientation separation and projected shading can be integrated using the current infrastructure. Then add geodesic fields, flow lines and mixing families with contextual controls; preserve seed, origin and light in the recipes. Avoid accumulating too many effects buttons that cannot be combined.

Primary references: [Real-Time Hatching, Praun et al.](https://gfx.cs.princeton.edu/proj/hatching/) for orientation by curvature fields and density-tone ratio; [Apparent Ridges for Line Drawings, Judd et al.](https://people.csail.mit.edu/tjudd/apparentridges.html) for lines in shape depending on point of view. They are references for evolution; the study does not incorporate their implementations nor presents hatching patterns rasterized as vector SVG.

Interactive verification: the five modes change the visible geometry; low/high mesh density produces different paths; one/three inks generate one/three chromatic families; changing the azimuth alters the shade. Volume occultation and clear/dark vision are also observed. The final fragment occupies some 341 kB.


## First integration: FLAT SHADOW

Implemented following the request to join its own shadow and projected into a single ink, with a faceted/cel shading appearance. White paper for light, one ink for all shades, intermediate tone and optional contour. The directional light retains direction/height; the threshold changes the amount of shade and a simplified artistic mesh controls the faceting. The original is preserved.

The mode checks for occlusion with respect to light, in addition to orientation: one component can cast shadows on another, even on faces oriented towards the focus. The paper view shows flat masses;  « See plotter strokes »  allows you to inspect the actual output. SVG and canvas receive parallel hatching vector patterns joined, without drawing the divisions of each triangle. The proper density depends on the pen and is adjusted in  « Finished and strokes ». There's no degradation or raster texture.

The first integration was subsequently completed with the three modes described below. Example shadows: `cargo run -p vectorio-core --release --example sombra_plana -- target/volumetric modelo.stl`.


## Integration of the three languages · 16 November 2026

- **WAVES OF SUPERFICIE**: distance by the edge graph from a chosen end in each component, converted to isolines over the triangles. Geodesic approach, no shortcuts to the inside of the solid. Direction of origin, levels, spatial deformation, frequency and reproducible seed.  « Interferences »  uses the difference of distances from two opposite ends, generating bands of equal phase difference. Optional and main/secondary contours in independent inks.
- **CROMATIC MESH**: all the edges of the artistic triangulation, including the coplanar diagonals, once. Adjustable complexity while retaining the original. One, two or three ink families, distributed by orientation of faces, height, depth of chamber or source objects. A file with a single object produces a family with that last rule; the other rules allow it to be divided by geometry.
- **WAVES + MESH**: waves in one ink, mesh in one or two additional ones. The complexity of the mesh and wavelength levels allow the two families to be balanced. The occlusion corresponds to the artistic mesh itself, avoiding hiding its edges with the more detailed geometry of the original.

The three modes are Rust production geometry: paper view and export with physical clipping and analytical occlusion, placement in Design/Output and recipes with parameters and full source. The 3D view is a reference of the surface with full curves and edges; the paper shows the visible output. The engraving by curvature flow lines continues to be a future direction, different from the waves implemented.

Example of reproducible SVG: `cargo run -p vectorio-core --release --example volumetric_arte -- target/volumetric-arte modelo.stl`. It generates waves, mesh, combined, interference and mesh from an ink. It doesn't send any moves to the plotter.
