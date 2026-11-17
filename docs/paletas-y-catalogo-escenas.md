# Palette library and unified scene catalog

17 November 2026.

## Reusable palettes

Open **Edit → Palettes…** from any workspace, **Design → Inks → Palettes…**, or **Volume → Reading map → Palettes…**. Palettes are independent of drawing styles and project files.

The library contains the five existing built-in ink sets and a separate **My palettes** list. Create a palette, duplicate a preset, or use **Save current inks…** to capture the document inks or the selected pass families. Saved palettes retain the name, ink order, RGB colors, pen names, and physical widths. **Edit** includes renaming, changing colors and widths, adding or removing inks, and moving inks up or down. Changes are applied to the library only when **Save palette** succeeds. Closing an unfinished editor offers to keep editing or discard it.

**Add to document** reuses inks with identical color and width and appends missing inks. It preserves existing objects, ink indices, and ink names. Editing or deleting a library entry never changes drawings that already use it. Deletion requires an explicit confirmation inside the library.

In Volume, saved palettes also appear in **Ink set**. **Apply to pass** assigns the palette to the originating pass, preserves its geometry settings, updates optical separation, and supports Volume undo/redo. Specialized effects retain their minimum family counts. The current editor supports up to eight families; density uses only the first palette ink and does not add unused inks to the document.

The library supports up to 128 user palettes, with 1–64 inks each. It is stored in `~/Library/Application Support/Vectorio/paletas.json`, using a versioned format and atomic replacement. Invalid or unsupported files are preserved. Stale edits are rejected if another session has changed the file; reload the library before retrying. Tests use isolated temporary files, never the user's library.

## One scene selector

Create now has a single searchable catalog for both the generator and its primary variant, using the same component as Volume. Entries are grouped as Architecture, Cities, Harbors, Worlds, and Curves.

There are 16 choices: White village, Blocks, three Megastructure compositions, four Festival city events, three Harbor types, two Dome worlds, and two Spirograph curves. Labels identify the generator and variant together, such as **Domes · Underwater**. The corresponding World, Harbor, Composition, Event, or Curve dropdown is removed from the parameter panel. Other parameters, including camera, inks, terrain, and district, remain editable.

Changing a variant preserves the seed and unrelated parameters. When editing a placed scene, choosing a variant of that same generator keeps the group being edited. Existing project and preference formats are unchanged: the catalog writes the same generator and parameter identifiers as the previous controls.

Each variant has its own thumbnail cache key. Thumbnails are generated on demand by a separate worker when previewed, rather than calculating the full catalog when Create opens. The shared selector focuses search when opened and scrolls to the current entry.

## Validation

Persistence tests cover save, rename, duplicate-name rejection, deletion, physical ink reuse, invalid files and widths, unsupported versions, and stale writes. Pointer tests exercise the palette save/apply flow in Spanish and English. Volume tests cover family requirements, optical inks, and undo/redo. Catalog tests cover all generator variants, old default recipes, preserved settings, and distinct cached thumbnails.

`cargo test --workspace --locked --offline`: **418 passed, 10 ignored, no failures**. All eleven historical SVG references remain byte-identical. The macOS installation was updated with `python3 scripts/install-macos.py`, without closing or restarting open windows.

Visual checks use the existing egui renderer at 980 × 620 and 1440 × 900 without closing or restarting user windows. Generate them with:

```sh
cargo test -p vectorio-app render_palette_library -- --ignored --nocapture
VECTORIO_QA_SCENE_CATALOG=1 cargo test -p vectorio-app render_create_panels -- --ignored --nocapture
```

## Next application work

Suggestive contours, apparent ridges, principal-curvature flow strokes, and shared source/facet preparation are implemented in subsequent extensions; see [Contours, curvature strokes, and large models](volumen/contours-and-large-models.md). GLB/glTF import and further acceleration for full-resolution Original visibility remain future tasks.
