# Interface consistency · Create and Volume

16 of November of 2026. Adjustments requested after Phase F.

- **Spaces**: the Design/Create/Volume/Output selector uses a sliding knob with a stable identity, even when changing the bar tools. It preserves its four segments also to 980 px; the title and shape selector fit the available space.
- **Export**: keeps the same blue capsule in all spaces. Entering Volume no longer makes it a secondary button.
- **Volume Inks**: the menu allows you to choose a family and directly assign any document ink to it, with colour sampling and vertical displacement for long pallets. Those nested deployable items that could close the menu before applying a choice are removed. Each pass separately recalls the active family.
- **Styles**: the horizontal strip of effects disappears. The selector shared with Create offers search by name or category, vertical list and thumbnail as the mouse passes. Volume groups its 29 styles into Lines and Surfaces, Tone and Texture, Reading maps and Fine Art, in addition to My Styles. Creating uses the same component for its generators.

## Checks and inspections

`cargo test`: **398 Tests passed, 9 ignored, no** failures. The historical SVGs remain identical. The new tests verify the animation between spaces, the constant appearance of Export, the compact selector in both languages, the search and selection with the pointer and the assignment of the document's eight colors in each of the three families, in Spanish and English.

Revised footage to 980 × 620 and 1,440 × 900, with light and dark themes, including filtered catalog, open palette and Create. They regenerate with `VECTORIO_QA_COHERENCE=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture` and the ignored test `render_create_panels`. Interface detector and `git diff --check` with no findings.

Installation by `python3 scripts/install-macos.py` on `/Applications/Vectorio.app` without closing or restarting open windows.

## Suspended

The predefined color sets, the optical mix and the isolines have subsequently been delivered in [G] Expansion (volumen-ampliacion-g.md). Games can be retouched by family and kept within a style.
