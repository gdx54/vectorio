# Volume · Phase G · Reading maps

16 of November of 2026. Phase G is executed after D, before E and F, in the order of the updated plan.

## Measurement, coding and ramp

A pass can retain its line generator and add a **Reading map** from **Actions**. There are also passes and reading styles in the gallery and in **Add pass › Add reading map**.

The available measures are height, chamber depth, geodesic distance, light with shadows between components, medium curvature, Gaussian curvature, slope, slope orientation, environmental occlusion, thickness and objects. They are calculated on the transformed work mesh and cached without assigning inks.

- Signed mean curvature: Laplace – Beltrami cotangent, baricentric and normal mass coherent; positive indicates convex and negative concave.
- Gaussian curvature: angular defect divided by area. It distinguishes convex regions and chairs; it does not correspond to the mean curvature sign.
- Hanging: angle of the normal with the vertical, between 0 ° and 90 °; orientation: timetable azimuth from +Y, with circular continuity in the passage 360 °/ 0 °.
- Environmental occlusion: between 8 and 128 hemispheric rays of cosine distribution, deterministic, against a BVH of triangles. Zero means open to the sky and one covered.
- Thickness: first intersection of a beam inward following normal; zero means it did not find another surface.
- Geodesic distance: approach by edge paths, from the upper end of each component.

The codes for this phase are **Accurate boundaries**, **Differential Transition** and **Stroke density**. The first interpolates the field over each segment and cuts it into the thresholds. The second allocates two adjacent inks by stroke index, using a reproducible orderly sequence. The third thins the line family with 16 coverage levels and a single ink; the base spacing still belongs to the generator.

The sequential ramp orders the inks selected from dark to light by OKLab L. The divergent retains the order chosen and reserves paper around zero, with symmetrical range. The category is only offered for objects: use your belonging by face, without interpolating identifiers. **Inks** opens the family selector; the ramp sample remains visible in the editor and in the list.

In Advanced are the weighted combination of two measurements, inversion, threshold, manual range and histogram equalization. In light blends with another measurement, the automatic range takes the values of the vertices; cut shadow regions can saturate the end of the range. You can set a manual range. The legend identifies the mixes by their two measures and weights.

## Vector legend

**Legend on paper** reserves a band of 26 mm per legend within the printable area. It contains actual ramp, values or categories, measurement/units, model, date, camera/light and graphic scale. The typography is unique stroke of the existing engine: the SVG carries no embedded text or raster images.

The date is set by activating the legend, in UTC, is editable and is saved in the recipe. It doesn't change when you reopen the drawing. The scale of development corresponds to its physical packaging; the stereoscopic pair has its own reduced scale. Horizons are an artistic deformation: their scale is interpreted horizontally.

Moving or climbing the composition keeps the legend in its stripe. The zoom uses the model's actual frame and keeps the point marked. Hiding one pass retains the reserve of its legend so as not to move the others; disabling the Legend option frees up space.

## Effects of reading

- **Relief holes**: direction of maximum slope, with increasing length and density in sloping areas.
- **Map of needles**: physical grid of small strokes oriented by the projected normal.
- **Horizontal lines**: depth profiles and upper/lower floating horizon envelopes, with analytical crossings. Adjacent sections are joined together to prevent unnecessary riots.
- **Crests and valleys**: edges of discrete ends of medium curvature, separated by sign and threshold. It's not an implementation of continuous main curvature ridges.
- **Anaglifo**: cameras separated between 0.5 ° and 8 °, with red/cian style and stereoscopic torque variant.
- **Development**: forest of isometrically unfolded faces, prioritizing coplanar joints and cutting out joints that would produce solape. Pack the islands at a common scale, store mountain/valley folds in different inks and add tabs and binding numbers.
- **Rotation trail**: between two and twelve cameras, with inks distributed according to the time of vision.
- **Technical Section**: cropped half-space, stripped lid with gaps, contour, discontinuous hidden lines and XYZ dimensions of the resulting envelope.

The gallery incorporates 14 reading styles. The Anaglifo-style red/cyan ramp reuses or adds these inks to the document; the inks of existing objects are preserved.

## Contracts and limits

`reading/fields.rs`, `rays.rs`, `surface.rs`, `encode.rs`, `effects.rs`, `unfold.rs`, `section.rs` and `legend.rs` separate measurements, geometric queries and physical output. `Pass.reading` is optional; the previous v2 recipes open without this field, and the v1 migration continues to work.

`DrawingPart` retains an editable portion by pass and exposes its ink families. Export, placement, thumbnails, selection and estimation use those families. Changing ink assignments does not require geometry; luminosity sorting is applied when materializing the output with the actual palette. Changing the number of families does change thresholds.

Line reading uses the field of the projected visible surface, as do D masks. In a pass with hidden lines or all lines, that field remains that of the visible surface. Views that change the projection (horizons, anaglyph, turn, development and section) do not support the tonal mask of the main chamber; the hidden interface and incompatible recipes are rejected. The own reading passes generate visible regions; the visible/hidden/everything selection is retained in the 3D line passes.

The magnitudes are expressed in units of the standard model (**or**), the OBJ units are not interpreted as millimetres. Strokes, scripts, separation, tabs and legends are all measured in millimeters of paper. Curvatures depend on the quality of the triangulation; on non-manifold borders or edges they are left to zero so as not to interpret the curvature of the edge as a surface curvature. Thickness in open mesh and AO with few samples are approximations.

The horizon orders profiles by average depth; it does not replace a general volume renderer with folds. The development supports parameters of up to 512 facets and an effective maximum of expensive 1,024; it requires a mesh manifold and can produce several islands. It doesn't optimize slicing or gluing like a specialized papercraft application. Reading supports up to 100,000 expensive and 200,000 working vertices, and output up to 250,000 strokes; excesses are rejected with a message. There's cancellation and hidden caches.

## Checks and inspections

- `cargo test`: **365 Tests passed, 9 ignored and no** failed.
- The eleven historical SVG references byte to byte are preserved, including the actual v1 recipe.
- Curvature and scale of a sphere, integral of Gauss – Bonnet, signs in a toroid, open plane, AO under a cover and thickness of a sphere.
- Accurate boundaries without loss of length, reproducible covers, orientation continuity 360 °/ 0 °, OKLab ramp and divergent zero as paper.
- All styles generate finite, cut and reproducible geometry, with v2 recipe and vector export. Caches, cancellations, custom pallets and ink references are tested when storing styles/deshacer.
- Development of a cube without artificial slice diagonals; matching triangles opposite shared folds; hole of the toroid in the section; separate stereoscopic cameras; floating horizon with exact crosses.
- Legend that retains the drawing and does not lose zoom anchorage; controls and output actions to 980 × 620 and 1,440 × 900 in Spanish and English.
- Captures: `VECTORIO_QA_READING=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture`; results from `target/volumetric/lecture-*.png`.
- `git diff --check`, Rust format and error-free interface detector.
- In the installation of Applications: heat map selection, four inks, legend and ⌘Z/⇧⌘Z. Escape closes the Volume menus first; a regression test is added to preserve the active space.

References to the formulas: [libigl, discrete geometric quantities](https://libigl.github.io/tutorial/#chapter-2-discrete-geometric-quantities-and-operators) and [OKLab, wording by its author](https://bottosson.github.io/posts/oklab/).) Operations are implemented in Rust; no other geometric library is incorporated.

## Suspended

E and F surrendered later. The optical mix, ink sets and isolines of the new fields are completed in [G] Expansion (volumen-ampliacion-g.md). Continuous main curvature fields and flow lines remain on the slope.
