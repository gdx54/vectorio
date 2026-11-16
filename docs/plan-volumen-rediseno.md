# Volume · Redesign plan (interface, organization, and artistic language)

16 November 2026. Review of `codex/volumetric` and a phased implementation plan. Written so another agent (Codex or Sonnet) can execute it step by step without rediscovering the context.

## 1. Diagnosis

The engine is well built: robust import, staged caching, a cancelable worker, analytical occlusion, physical clipping, and recipes. The problem is **the mental model** it presents.

**Seven mutually exclusive modes that represent different kinds of things.** `RecipeKind` (`crates/vectorio-core/src/volumetric/mod.rs`) mixes three concepts in one selector:

- *Line generators*: sections (ATLAS/STRATA), edges (X-RAY), waves, and mesh.
- *A tonal treatment*: FLAT SHADOW.
- *A hardwired combination*: WAVES + MESH (`Hybrid`).

Consequently, the artistically interesting combinations cannot be requested: silhouette + waves only in shadow + fine mesh in the light. Each new combination would require another `RecipeKind`. `Hybrid` is the symptom.

**Flat `Settings`.** One struct contains parameters for every mode: slices, fields, shadows, art, STRATA, X-RAY, and so on. The interface must hide and show controls according to the mode, and recipes store unused parameters. Inks are limited to three fixed roles (`layer_pens: [usize; 3]`, `Drawing::items(pens: [usize; 3])`).

**An interface organized around the technical pipeline rather than intent.** A 326 px panel contains collapsible Model → Transform → Camera → Topography → Composition and inks → Recipe → Light and shadow → Artistic surface sections. Uppercase English code names (ATLAS, STRATA, X-RAY) conflict with the Spanish interface requirement. Camera and light are controlled only by sliders; nothing is manipulated in the preview. Statistics compete with action buttons.

**No continuity with the document.** Place on canvas creates fixed paths; the 3D object cannot be edited again from Design. Volume (⌘4) is isolated from Create (⌘2), which also has a 3D scene engine (`scene`).

**Limited tonal range.** A plotter creates tone through line density and direction. Currently there is only binary shadow plus an intermediate tone made from straight parallel hatching. There is no form-following hatching, layered crosshatching, dashed hidden lines, halos, or perspective.

## 2. Central idea: a scene and a stack of passes

Replace “choose a mode” with **“a scene and a stack of drawing passes,”** like layers.

- **Scene** (shared): model, transformation, camera, light, paper, and composition.
- **Passes** (ordered list): each generates lines and has its own ink, visibility, and mask.

| Pass | What it draws | Origin |
|---|---|---|
| Outline | Silhouette and outer profile; optional echo with n outward offsets | X-RAY |
| Edges | Sharp edges selected by dihedral angle | X-RAY |
| Sections | Plane intersections: direction, count, range, and offset per stratum | ATLAS, STRATA |
| Field curves | Height, depth, or distance isolines | ATLAS |
| Waves | Geodesic waves / interference | WAVES |
| Mesh | Edges of the artistic mesh | COLOR MESH |
| Hatching | Hatching by light level; see §4 | FLAT SHADOW |
| Cast shadow | Outline or hatching of the shadow on the ground | FLAT SHADOW |

Common properties of every pass:

- **Ink**: any document ink; the three-ink limit ends.
- **Visibility**: visible · hidden (optionally dashed) · all.
- **Tonal mask**: entire surface · light only · shadow only · light band [a, b]. This makes combinations artistically useful: waves only in shadow, mesh only in light.
- **Ink distribution** (optional): by orientation, height, depth, or object; currently available only for Mesh.
- Enabled/disabled state, name, and order.

The seven existing modes become **styles**, or saved stacks, with Spanish interface names:

- *Contour lines* (formerly ATLAS): Field curves + Outline.
- *Strata* (STRATA): offset Sections + Outline.
- *X-ray* (X-RAY): visible Edges + dashed hidden Edges in another ink.
- *Flat shadow*: Hatching (two levels) + Cast shadow + Outline.
- *Waves*, *Color mesh*, and *Waves and mesh*: direct equivalents.
- New styles (§4): *Engraving*, *Crosshatch etching*, *Technical drawing*, and *Shadow topography*.

## 3. Proposed interface

```
┌ Toolbar ──────────────────────────────────────────────────────────────────────────┐
│ Styles: [thumbnail][thumbnail][thumbnail]…                      [Save style]       │
├ Scene (left, collapsible) ┬──────── Large paper ─────────┬ Passes (right) ─────────┤
│ Model  ▸ cube ▾ Import    │                             │ 👁 ● Outline            │
│ Camera ▸ top · elevation  │        paper preview        │ 👁 ● Waves              │
│          iso · persp.    │       (actual strokes)      │ 👁 ● Hatching           │
│ Light  ▸ sun sphere      │                    ┌─────┐  │ [+ Add]                 │
│ Paper  ▸ A4 · margins    │                    │ 3D  │  │ ── Waves ──             │
│                          │                    └─────┘  │ parameters for the      │
│                          │                             │ selected pass           │
├──────────────────────────┴─────────────────────────────┴─────────────────────────┤
│ 12,430 strokes · 3 inks · ~18 min plotting                      [Export] [Place]   │
└──────────────────────────────────────────────────────────────────────────────────┘
```

Principles:

1. **Paper takes center stage.** The 3D view becomes an inset, with an optional split view.
2. **Direct manipulation.** Dragging in the 3D inset orbits the camera; a **sun gizmo**, represented by a small sphere, sets the light. Dragging on paper moves the drawing; the wheel scales it. Sliders remain available for fine adjustment.
3. **A Layers-like pass panel.** Eye, ink swatch, name, and drag-to-reorder handle. Only the selected pass's parameters appear below; no mode-dependent hiding and showing. Clicking a pass highlights its strokes on paper.
4. **Styles with thumbnails.** A top-bar gallery whose thumbnails are generated by the worker using the current model at low resolution. Choosing a style replaces the stack and can be undone.
5. **A clean footer.** One line for stroke count, inks, and estimated time, reusing `output`; quality warnings appear only when necessary, with actions on the right.
6. Spanish names throughout; the workspace is called **Volume** (⌘4).
7. Reuse `theme::glass_panel`, `accent_button`, and the existing Liquid Glass appearance.

## 4. Artistic improvements, ordered by impact and cost

1. **Hatching by light level** (high impact, low cost). Current lighting has binary shadow plus an intermediate tone. Compute `N·L`, including existing light occlusion, and divide it into 2–4 bands. Each band adds a hatching family at its own angle (0°, 45°, 90°, 135°), creating traditional etching-style crosshatching. Reuse `shadow/mask.rs`, including intervals and the global phase in millimeters.
2. **Form-following hatching** (high impact, medium cost). Object sections or isolines **clipped by the tonal mask** create strokes that describe the volume only where shadow exists; spacing decreases in darker bands. Combine `contours` with the mask, without introducing curvature fields yet. Style: *Engraving*.
3. **Dashed hidden lines in their own ink** (medium/low cost). Hidden intervals are already computed during clipping. Retain them rather than discard them, and apply a dash pattern in millimeters.
4. **Halos** (high visual impact, medium cost). Interrupt a stroke where it passes behind a closer silhouette. This technical illustration technique conveys depth without tone.
5. **Outline echo and thickness** (low cost). Offset the silhouette N times outward or inward, or add a second pass offset by 0.2 mm to thicken it.
6. **Perspective** (medium/high cost). A camera with focal distance and a near plane, already fitted to the sheet. Adapt `scene::Camera` without changing Create generators.
7. **A seed with a real effect** (low cost). The seed currently does little. Use it for deterministic hand jitter through smooth noise displacement in millimeters, irregular stroke lengths, and small hatching gaps: a hand-drawn appearance with reproducibility.
8. **Multiview sheets** (medium cost). Repeat N cameras on one sheet: a complete rotation or top + front + side. Reuse composition and clipping.
9. **Ink gradient by field** (low cost). Use 2–3 inks according to height, depth, or light, softening boundaries by alternating strokes.
10. Later: curvature fields and flow lines (Praun), suggestive contours / apparent ridges (Judd), and GLB/glTF.

## 5. Code organization

Core (`crates/vectorio-core/src/volumetric/`):

- `scene_settings.rs`: `SceneSettings { model, camera, light, composition, detail }`.
- `pass.rs`: `Pass { name, enabled, pen: usize, visibility, mask, coloring, kind: PassKind }`. `PassKind` owns its parameters as enum data, replacing flat settings.
- `passes/`: one file per type: `outline.rs`, `edges.rs`, `sections.rs`, `field.rs`, `waves.rs`, `mesh.rs`, `hatch.rs`, and `ground.rs`. Each provides `fn lines(ctx: &PassCtx, p: &Params) -> Vec<Line3 or Polyline>`. `PassCtx` gives access to shared working-mesh, artistic-mesh, graph, and light-field caches.
- `engine.rs`: orchestrates passes, caches by mesh identity and pass-parameter hash, then applies mask → occlusion → clipping → simplification. `Drawing { parts: Vec<Part> }` has a part per pass or color family.
- `style.rs`: built-in styles, including the seven current modes and new combinations, represented as stacks of passes.
- **Version 2** recipes; `Recipe::from_json` migrates v1 → v2 by translating `RecipeKind` into a stack. Keep a test with a real v1 recipe.
- `pipeline.rs`, `art/`, and `shadow/` are progressively moved into `passes/` while retaining their algorithms and tests.

App (`crates/vectorio-app/src/`):

- `volumetric_ui.rs`: state, worker, and actions only.
- `volumetric_scene_panel.rs` (left), `volumetric_passes_panel.rs` (right), `volumetric_styles.rs` (gallery), and `volumetric_preview.rs` (paper and 3D inset).
- Remove `volumetric_art_controls.rs` and the three-role `role_active`/`valid_pens` logic.

Document:

- A new `Item` or 3D-object metadata retains the recipe alongside its paths. Double-clicking in Design opens Volume with that recipe; Update replaces the paths. Include automatic project recovery.
- Create (⌘2): an Open in Volume action converts the generated scene into a mesh.

## 6. Phases

Each phase leaves a working app, with tests and `python3 scripts/install-macos.py`.

**Phase A · Passes in the core without changing the interface.** Introduce `Pass`, `PassKind`, `engine.rs`, and `passes/` as wrappers around existing algorithms. Translate all seven `RecipeKind` values into stacks that produce identical SVGs, demonstrated by equivalence tests using `volumetric` and `volumetric_arte`. Recipe v2 plus migration. Remove the three-ink limit. *Recommended model: Opus, high reasoning for type and cache design.*

**Phase B · New interface.** Three zones, a Layers-like pass panel, a style gallery with Spanish names, a clean footer, prominent paper, and a 3D inset. No new effects yet. *Recommended model: Sonnet or Codex, medium reasoning; visual review in the installed app.*

**Phase C · Direct manipulation.** Orbit in the inset, sun gizmo, move/scale on paper, and highlight the selected pass. *Recommended model: Sonnet or Codex, medium reasoning.*

**Phase D · Tone.** Tonal masks per pass, layered crosshatching, form-following hatching, and dashed hidden lines. Styles: *Engraving*, *Crosshatch etching*, *Technical drawing*, and *Shadow topography*. *Recommended model: Opus, high reasoning for masks and clipping; Sonnet for the remainder.*

**Phase E · Artistic finish.** Halos, outline echo, seeded jitter, and ink gradients. *Recommended model: Sonnet, medium reasoning; high for halos.*

**Phase F · Continuity.** Re-editable 3D objects in the document, Open in Volume from Create, multiview sheets, and perspective. *Recommended model: Opus for the document model and perspective; Sonnet for the remainder.*

## 7. Acceptance criteria

- No existing style changes its SVG in Phase A, verified by equivalence tests.
- Any combination of passes is possible without changing code.
- Changing a pass's ink, visibility, or order does not recalculate geometry.
- Changing one pass's parameters recalculates only that pass.
- All interface text is in Spanish and included in `locales/app.yml`.
- Output is always vector-based with physical clipping; no embedded raster images.

## 8. Reading maps: color as information

### What is wrong today

`art/edges.rs` chooses an ink for each **whole edge** using coarse rules: the dominant axis of the absolute normal (X/Y/Z), three fixed height/depth bands, or the object index modulo the number of inks. This produces abrupt changes unrelated to real boundaries, with no ordering between inks and no legend. It looks random because it does not encode a readable quantity.

### Model: measure → encoding → ramp

Separate three decisions, applicable to **any pass** (§2):

1. **Measure**, a scalar field per vertex or face, computed in the core and cached per mesh:
   - Height, camera depth, and geodesic distance already exist.
   - **Light**: `N·L` with light occlusion; `shadow` already provides part of this.
   - **Mean curvature**, distinguishing convex and concave regions using discrete Laplace–Beltrami, and **Gaussian curvature** from angle defect. Reveals ridges, valleys, and saddles.
   - **Slope**, the angle from vertical, and **aspect**, the compass orientation of the slope, as on geological exposure maps.
   - **Ambient occlusion**: how much sky each point sees, sampled over a hemisphere against occluders. Corners and cavities are dark; this measure makes volume especially legible.
   - **Thickness**, measured by an inward ray: thin and thick regions, useful for 3D-printed parts.
   - Combinations: `a·campo1 + b·campo2`, thresholds, inversion, and histogram equalization.
2. **Encoding**, or how a value is drawn with lines:
   - **Ink ramps with exact boundaries**: split strokes where the field crosses a threshold, interpolating along the segment as `contours` already does, instead of coloring a whole edge.
   - **Soft transitions**: ordered alternation of neighboring inks by line index, proportional to the value. No hard bands.
   - **Density**: the value controls hatching spacing, creating a heat map with one ink.
   - **Optical mixing**: convert the value into a colormap color, then use `mix.rs`, already used for photos, to separate it into actual pen coverages. Overlay crossed hatching clipped to projected faces.
   - **Field isolines**: contour lines of curvature, occlusion, or light, resembling a thermal drawing, with optional value labels.
3. **Ramp**: sequential, automatically sorting selected inks by OKLab lightness; diverging, with concave and convex curvature in different inks and paper at zero; categorical only for objects. Thermal-style ramps using 4–5 pens from the user's library.

### Technical legend on paper

An optional margin legend shows the actual ink ramp and values, measure name and units, graphic scale, camera and light azimuth/elevation, model, and date. Use the existing single-line lettering engine. This turns the sheet into a technical document.

### New 3D → 2D effects, represented as passes

- **Relief hachures**, in the Lehmann / historical cartography style: short strokes along the steepest slope, longer and denser on steeper areas. Well suited to technical drawing and plotters.
- **Needle map**: a regular paper grid with a small stroke in each cell, oriented by the projected normal or a field gradient; length represents magnitude.
- **Horizon lines**, or stacked profiles: horizontal screen sweeps displaced by depth, with floating-horizon visibility. Depth per face is affine, and `shadow/project.rs` already has a 2D BVH.
- **Ridges and valleys**: extreme-curvature lines in two inks for convex and concave regions.
- **Anaglyph**: two cameras separated by 2–4°, one red and one cyan, producing depth with glasses. Alternative: a side-by-side stereo pair.
- **Unfolding**: flatten the artistic mesh, using two inks for mountain and valley folds and adding glue tabs. It can be cut out and assembled, turning 3D back into 3D.
- **Rotation trail**: overlay several rotations, progressing through an ink ramp over time.
- **Technical section**: a plane cut with standardized hatching on the cut face, bounding-volume dimensions, and dashed hidden lines.

### Phase G · Reading maps, before E

Measures: light, curvatures, slope, and ambient occlusion. Encodings: ramps with exact boundaries, dithering, and density. Ramps sorted by lightness, plus legends. Then effects: relief hachures, horizon lines, and anaglyph first, for high impact at low-to-medium cost; unfolding and technical sections last. *Recommended model: Opus, high reasoning for curvature, ambient occlusion, and exact splitting; Sonnet or Codex for legends, ramps, and anaglyph.*

## 9. Interface visual hierarchy: Phase B details

This replaces and refines §3. Each zone answers one question. The eye follows style → paper → passes → output.

**Five zones, in order of importance:**

1. **Paper, centered and given the most space.** This is the result; panels never cover it. A small 3D inset at the bottom right, approximately 92×72 pt, expands to split view on double-click. The legend (§8) appears on paper because it is plotted.
2. **Passes, on the right, 180–220 pt.** The most frequently used area. A Layers-like list at the top: eye, ink/ramp swatch, name, drag handle, and `+` to add. Below, only the selected pass's parameters, arranged as follows: essentials always visible (measure/type, density or levels, inks, mask); a collapsed Advanced section (precision, seed, simplification, ranges); no collapsible sections nested inside other collapsible sections, replacing the current maximum of three levels.
3. **Style, in the top bar beside the workspace selector.** Chips with Spanish names and thumbnails on hover. Save style at the end. Choosing a style replaces the stack and can be undone.
4. **Scene, on the left, 150–180 pt, collapsible as a whole.** Set once and usually leave alone: Model (name, triangle count, import, quality); Camera (Top/Front/Iso/Perspective chips plus orbit in the inset); Light (sphere gizmo with a draggable sun); Paper (a one-line summary expanding to size, margins, and position). Model transformation lives under Model › Advanced.
5. **Footer, one line.** Left: strokes, inks, estimated time, and calculation time in secondary text. Show warnings only when necessary, using one line, warning color, and an action. Right: secondary Export SVG and Place on canvas, the screen's only accent button.

**Rules:**

- One accent button per screen: Place on canvas.
- Three text sizes: small secondary zone title, normal label, and muted value/help. No all-caps labels or English code names in the Spanish interface.
- One-line help below a control only when needed, such as “Drag in the 3D view to orbit.” Remove long mode descriptions; thumbnails explain styles.
- Show calculation state as a thin progress bar above the paper, replacing panel text.
- While editing a selected pass, highlight its strokes on paper and dim the rest.
- In narrow windows, Scene collapses to icons and Passes becomes a floating sheet over the paper.
- Reuse `theme::glass_panel`, `accent_button`, and `theme.rs` tokens; strings belong in `locales/app.yml`.
