# Volume · Phase G extension

16 November 2026. Ink sets, optical mixing, scalar isolines, and optional section dimensions.

## Use

In the map editor, **Ink set** applies Graphite, Four-color process, Primary colors, Thermal, or Earth and indigo. It reuses identical document inks and adds missing ones while preserving existing inks and objects. Individual families can then be changed through **Inks**. Changing an ink makes the selection custom. Saving a style preserves the selection and palette.

The gallery adds **Thermal optical mix**, **Curvature isolines**, and **Light isolines**, bringing the built-in styles to 32, organized in the existing searchable selector.

For a surface **Reading map** pass, **Encoding** offers:

- **Optical mixing**: converts the field into thermal, grayscale, or cool/warm colormap colors and separates them into actual pens. Generates crossed hatching over visible regions. **Screen scale** adjusts the physical pattern; pen width limits its detail. The editor swatch shows the colormap, while the legend reproduces its ink separation. Diverging distributions preserve uninked paper at zero.
- **Field isolines**: 2–96 levels of any available scalar measure. Supports range, inversion, measure combinations, equalization, and a single threshold. **Label values** adds vector labels while avoiding collisions between them. Constant fields produce no curves.

Both encodings generate their own geometry and are offered on surface maps. Specialized views such as anaglyph, unfolding, section, and horizons retain their own encodings. Object identifiers remain categories rather than interpolated scalar fields.

In **Technical section**, turning off **Show XYZ dimensions** removes the dimension annotation from paper and SVG. The section, hidden lines, and hatching remain. The switch is an essential control, is saved in recipes and styles, and participates in undo/redo. Earlier recipes keep dimensions enabled.

## Implementation and limits

`reading/optical.rs` reuses the optical-density separation from `mix.rs` through analytical color input. It creates no intermediate image and embeds no raster data. A 129-value table and a physical vector pattern produce reproducible coverage with different angles per family. Reproduction is approximate and depends on actual pen colors, widths, and behavior.

Changing a physical ink updates its separation and strokes. Meshes, fields, and visible regions are reused; names and pass order remain metadata. Optical mode preserves family order rather than sorting it by brightness for export. The legend uses the darkest ink for annotations.

`reading/scalar.rs` intersects the field with each visible region, interpolates in model space, and then projects, including perspective. It joins coincident endpoints without bridging branches and removes repeated segments. Light isolines interpolate the vertex field within illuminated regions and are clipped against analytical shadows; shadowed regions remain at zero. Curvature and isoline quality still depend on working-mesh triangulation.

Physical clipping, cancellation, and output limits are retained. New options have defaults compatible with earlier v2 recipes; v1 migration continues to work.

## Checks and inspection

- `cargo test`: **408 passed, 9 ignored, no failures**. All eleven historical SVG references remain byte-identical, including the real v1 recipe.
- Analytical separation equivalent to the reference color-photo engine, uninked paper at the diverging zero, and optical updates without rebuilding model geometry.
- Height isolines checked on a sloping plane under orthographic and perspective projection; every scalar measure produces finite coordinates.
- Reusable ink sets preserve previous objects; pointer selection and subsequent family editing are covered.
- Section dimensions enabled through the interface, output without annotations, retained hatching and hidden lines, persistence, and loading older recipes.
- Finite, clipped vector output; determinism, cancellation, and export without embedded text or images.
- Reviewed 980 × 620 and 1440 × 900 captures in light and dark themes: `VECTORIO_QA_EXTRA=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture`, generating `target/volumetric/extra-*.png`.
- Interface checks and `git diff --check` report no findings.

Installed with `python3 scripts/install-macos.py` at `/Applications/Vectorio.app`, without closing or restarting open windows.

## Remaining work

An independent palette library is available; see [Palette library and scene catalog](../paletas-y-catalogo-escenas.md). Suggestive contours, apparent ridges, principal-curvature strokes, and shared preparation are covered in [Contours, curvature strokes, and large models](contours-and-large-models.md). GLB/glTF remains future work.
