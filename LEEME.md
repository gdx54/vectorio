# Vectorio

A Rust desktop application for making beautiful things with a pen plotter (EasyDraw V4 A3 / EBB controller). It is Linear's creative sister: handwritten text, multicolor drawings with fills, photos converted into drawings, and an accurate preview of how they will look on paper.

## Features

- **Volume (⌘4)**: primitives and OBJ/STL models, composable passes with independent inks, real sections, waves, mesh, lighting, and shadows. Includes curvature, slope, ambient occlusion, and thickness maps; optical mixing, isolines, halos, echo outlines, and reproducible jitter. Offers orthographic or perspective orbit cameras, multiview sheets, vector legends, and 35 categorized styles, including suggestive contours, apparent ridges, and principal-curvature strokes. Recipes are saved with 3D objects so they can be reopened from Design. Create scenes can be opened in Volume. Technical section coordinates can be hidden with “Show XYZ dimensions.” The original model is preserved; OBJ/STL files up to 512 MB and 5 million triangles are supported, with interactive, detailed, or original quality. See the [Volume plan](docs/volumen/plan-rediseno.md) and [latest extension](docs/volumen/contours-and-large-models.md).
- **Handwritten text boxes**, with 11 single-line typefaces:
  - Eight single-line fonts from Evil Mad Scientist and Hershey: Capitol (connected cursive), Allure, Society, Hershey Script, Pepita, Casual, Neato, and Architect.
  - Three original fonts: Vectorio handwritten, neat, and technical.
  - Press T, drag a box, and type or paste a complete text. The right, bottom, and corner handles change its width and height without scaling the letters; the text reflows.
  - Inner margins, line spacing, paragraph spacing, first-line indentation, and left, centered, right, or justified alignment. Automatic hyphenation uses an included Spanish dictionary.
  - The box preserves all text and paginates it automatically according to its height. Previous/Next moves through pages; Page break (⌘Enter) inserts a break at the cursor. Export pages generates one SVG per sheet and repeats the other objects in the design.
  - Fill page fits the box to the printable area; A4 sheet prepares portrait A4 paper and its text box. Older text objects can be converted into boxes in the inspector.
  - Adjustable jitter (0–2), slant, line spacing, alignment, and wrapping. Join letters writes each word without lifting the pen.
  - Another variation changes the handwriting without changing the text. Accents, ñ, ¿, and ¡ are supported.
- **Layers**: a flat list with one object per row. The top object is in front.
  - Drag one or more objects to change their order without changing their inks. Bring forward/Send backward and their shortcuts apply to the whole selection.
  - Double-click a name to rename it; Enter confirms and Esc cancels.
  - Each row shows the outline color as a ring and the fill color as a solid circle. A click opens the integrated color editor with RGB, hexadecimal, and a preview. X, Cancel, or Esc discards the change; Done confirms it. Actions stay visible in a fixed footer.
  - Editing a color from a row affects only that object's outline or fill. Other inks and objects are preserved.
- **Pen palette**: independent of layers, with a name, color, and physical width for each ink.
  - Each ink is a card showing its color, a sample at the actual stroke width, its name, hexadecimal value, and width in millimeters.
  - Click to draw with it; double-click its color to edit it or its name to rename it. Change its width in the inspector.
  - Drag a card and the others move aside to show its destination. Drop it onto a layer or canvas object to assign the outline ink; hold ⌥ to assign the fill ink.
  - The palette does not group objects or determine the drawing's visual order. The inspector shows the actual hexadecimal color and allows direct editing; shared inks are chosen with Choose from palette.
  - Automatic color names update when recolored; names entered by the user are preserved.
- **Fills**: hatching, crosshatching, continuous zigzag without lifting the pen, solid, concentric, stippling, and waves. Spacing, angle, and inset are adjustable.
- **Shapes and generators**: rectangle, ellipse, polygon, star, heart, spiral, spirograph, Lissajous, and polar rose.
- **Freehand**: strokes are smoothed automatically. Closed strokes can be filled.
- **Photo → drawing** (I or drag a photo): a single sheet with a live Drawing preview showing actual fills and hatching at each pen's width.
  - Navigable preview: pinch to zoom around the pointer; use two fingers or drag to pan, or use Zoom in/Zoom out and Fit (also double-click). Original, Drawing, and Overlay share the same framing; zoom does not change the size on paper.
  - Colors: k-means turns each color into a pen with a fill. No outline by default; all hatching shares an angle, adjustable globally or separately per color.
  - Mixing: overlays hatching or dots from several pens (CMYK, RYB, or photo colors) to mix colors optically, producing more vivid colors than a flat fill. Black is preferred for shadows. Imported layers are identified as fill patterns, with their ink shown in the fill swatch. Import preserves the preview's strokes and widths.
  - Lines: stroke centerlines.
  - Portrait: a direction field computed from the smoothed RGB structure tensor makes streamlines follow image edges and shapes. Shadows receive extra passes, optionally crossed, while highlights preserve the paper. Supports black, RYB (yellow, red, blue, and black), CMYK, photo colors, or selected pens. Density, flow, shape following, angle in unstructured areas, passes, length, and highlight reserve are adjustable. Produces open strokes per ink; the photo and its settings are retained for regeneration from Layers. A starting point: RYB, density 240, three passes, and a 0.35 mm pen, with a sharp, high-contrast photo. Density is relative to image size: when greatly reducing the drawing on paper, reduce density too so the physical stroke width does not close the highlights.
  - Silhouettes: closed shapes.
  - Engraving: modulated waves.
  - Topography: a native Rust adaptation of FastMarchingTopoPlot. A wave travels through the image at a brightness-guided speed; its arrival times generate contour lines that are closer together in shadows. Curve count, gamma, minimum speed, smoothing, and origin are adjustable; click the image to set the origin. PNG alpha is preserved to omit transparent regions, and the photo and parameters are saved for regeneration. Output consists of strokes per ink, compatible with SVG and Output. Details and attribution are in `docs/referencia/topografia.md`.
  - After adding a drawing, it is selected as a whole so it can be moved or deleted in one action.
- **Multiple selection**: drag in empty space for a selection box; ⇧ or ⌘ click adds or removes objects. Move, duplicate, delete, and edit them together.
- **Native macOS menus** with all commands and a keyboard shortcut sheet (⌘/).
- **Create (⌘2)**: generators with previews and background calculation.
  - White village builds a terraced hillside with streets, stepped alleys, lamps, people, a fountain square, trees, benches, and a long-nave church with a bell tower at a corner of its entrance end and two cypresses. Each terrace has a street lined with attached houses, one or two stories high with stepped heights, balconies, plinths, street doors, and overhanging roofs. Occasional courtyard houses or walled olive gardens break the rows. Rooftops have pergolas, washing lines, stair enclosures, and water tanks; surrounding land has furrows, olive groves, and cypresses.
  - In Poster, sun-facing façades remain white and sides receive cool shadows; roofs, shadows, plants, and ground each have their own ink. In Engraving, every face receives light hatching. The default sun depends on the view: street façades stay white and the other visible face has a midtone with open hatching. Sun rotation turns the light relative to that default.
  - Irregular, wavy strata, occasionally with lenses, appear only on the model's outer terrain faces; capped retaining walls separate terraces. Terrain chooses terraces or flat land, Streets sets their count, and Shadow density adjusts cast-shadow line spacing. Full bleed fills the sheet with terrain; Surrounding countryside pulls the camera back to include gardens and olive groves; Model cuts the terrain into a block with a paper margin.
  - City blocks combines blocks and towers; Spirograph draws continuous curves per layer. New scenes share a whitewashed style with hatching per surface. Each house, staircase, wall, or bench is a single piece, without seams between walls, gables, parapets, or steps; attached houses retain their boundary. Windows and doors displace wall hatching and may remain white or be filled with Dark windows.
  - The gallery shows thumbnails calculated in the background. Adjust parameters, lock the ones to preserve, and use Random (N when not typing). Each generator includes scene-matched ink sets such as Whitewash and indigo, Earth and olive, India ink, Sunset, and Vectorio. Choose the number of pens: with fewer pens, parts are combined into available inks, merging vegetation before hatching and hatching before ground. Document pens distributes the existing inks.
  - Place adds a group with one object per part and adds missing pens from the set, reusing matching colors. Each part keeps its own ink; updating with the same set preserves inks you have changed. Edit in Create in Layers updates the recipe while preserving inks, position, scale, visibility, and locking. The last generator, parameters, and locks are remembered across sessions. Editing a group works on a copy and preserves remembered settings when updating or canceling.
  - Megastructures builds a continuous mass with uneven strata, galleries, louvers, voids, roofs, and machinery: Stratified tower, Inner courtyard, or Grand portico. Harbor and citadel adds a fishing waterfront, warehouses with a crane, or a citadel with a square, docks, sailboats, lighthouse, and awning-covered façades. Shared Ink intensity strengthens roofs and shadows while respecting pen width; dense scenes separate warm roofs from cool shadows.
  - In Festival city, Show signs removes shop, stall, venue, and finish-line text while preserving architecture and decorations. The two tall Parade characters are parade giants. Street life adds small bicycle racks to central sidewalks; median tree crowns leave crossings clear and the park becomes lighter during the fair. The Ballpoint set, available in every generator, uses black, blue, red, and green with four pens; a fifth adds yellow for the terrain. Outlines and text are black, hatching and shadows blue, accents red, and vegetation green.
  - Domes connects five to nine small worlds: village, garden, court, market, vegetable garden, and laboratory, with people inside. World switches between Underwater (fish, coral, divers, a boat, and a whale, squid, or marine reptile) and Space (planets, a galaxy, astronauts, and a rocket, station, or stellar whale). Main subjects can be hidden; people, background, and viewing angle are adjustable. Glass reveals interiors while hiding the background; walkways reach dome edges. Includes Abyss, Orbit, and Ballpoint ink sets.
- **Output (⌘3)**: a workspace for preparing EasyDraw output; no driver or connection yet. Uses the same canvas and glass panels as Design, with a right-hand inspector for summary, settings, and export, view controls above the paper, and simulation in the bottom panel.
  - Total ink map, using Linear's criterion and scale: gray (one pass), yellow (two), orange (three), red (4–5), carmine (6–9), and purple (10 or more). Counts each pen's physical footprint plus adjustable bleed; remaining inside a cell does not multiply passes. Hotspots retain their maximum when zooming out.
  - Estimated moisture maps before and after planning show the maximum number of still-wet passes per region. Waiting does not reduce total deposited ink.
  - Drying protection: adjustable drying time and wet-pass limit. The planner seeks other strokes within the same ink and waits with the pen lifted when needed. Splits long strokes or repeated overlaps without simplifying their geometry; preserves palette ink order.
  - Time-based simulation includes drawing, travel, pen lifts, drying, and pen-change pauses. Two parallel timelines for actions and active ink share a high-contrast playhead, click/drag navigation, a pause list, and time/travel comparison against the unprotected route. Supports zoom, pinch, and panning on paper.
  - JSON plan with waits and route SVG. SVG preserves strokes and their order but does not execute pauses; these remain in the JSON plan for a future driver. See `docs/referencia/formato-salida.md`. Every export reports its saved location or failure reason. Output settings are saved in the `.vectorio` project.
  - The route is simulated over the selected paper color. Changing settings briefly waits for changes to settle, cancels the previous calculation, and preserves the simulation position. Only strokes crossing regions above the wet-pass limit are split.
  - The model is conservative: it considers a footprint wet until the end of each segment dries. Default values of 20 s drying, three wet passes, and 0.3 mm bleed are starting points to adjust with real paper and pen tests. The pen-change pause represents a planned minimum; changing pens remains manual. Background analysis warns about strokes outside the paper.
- **Paper preview**: white, ivory, kraft, gray, or black.
- **Files**: JSON `.vectorio` projects, SVG import, and SVG export in millimeters with an Inkscape layer per pen or a separate SVG per pen.

## Performance (M5)

- Drawing is rendered on the GPU through wgpu/Metal.
- Fills, vectorization (thresholding, EDT, contours, thinning, and k-means), and route optimization are distributed across CPU cores with rayon.
- Calculations run on background threads. While dragging an object, its last computed geometry is transformed immediately.

## Interface (macOS 27 styling)

- A consistent toolbar integrated with the window controls.
- Floating glass panels with readable contents.
- Concentric corner radii: 18 pt panels and 8 pt controls.
- SF Pro typography.
- Settings (⌘, or Vectorio → Settings): Light, Dark, or System appearance, also available under View → Appearance; interface scale, starter ideas, paper color, plotter name, and defaults for new documents (paper, speeds, and drying). Use this document copies values from the current project. Settings apply immediately and are saved in `~/Library/Application Support/Vectorio/preferencias.json`.
- Language: System, Spanish, or English. Panels, menus, notices, paper sizes, styles, fonts, and default pen names in new documents are translated; text already saved in projects remains unchanged. Internal core error messages remain in Spanish. Strings live in `crates/vectorio-app/locales/app.yml`, keyed by Spanish source text; a test fails if any translation is missing.

## Keyboard shortcuts

The full list is available in the app with ⌘/.

| Key | Action |
|---|---|
| V · P · T · S | Select · Freehand · Text box · Shapes |
| T + drag · ⌘Enter | Draw a text box · Page break in the editor |
| I · ⇧⌘I | Photo → drawing |
| ⌘Z / ⇧⌘Z | Undo / Redo |
| ⌘D · ⌫ | Duplicate · Delete (deletes a pen when the pen list has focus) |
| ⌘W · ⌘Q | Close window (the app stays open; Dock restores the document) · Quit |
| ⌘A · drag in empty space · ⇧/⌘ click | Select all · Selection box · Add or remove |
| ⇥ · ⇧⇥ | Next / previous object |
| Esc | Cancel, close, deselect |
| ⌘] · ⌘[ · ⌘L · ⌘R | Forward · Backward · Lock · Another variation |
| ⌘, | Settings |
| ⌘S · ⇧⌘S · ⌘O · ⌘E · ⇧⌘E · ⌘N | Save · Save as · Open · Export SVG · One SVG per pen · New |
| Arrow keys (⇧) | Move 1 mm (10 mm) |
| ⌘1 · ⌘2 · ⌘3 · ⌘4 | Design · Create · Output · Volume |
| N (in Create, outside text fields) | Randomize parameters and seed, respecting locks |
| ⌘= · ⌘− · Pinch · ⌘ + wheel | Zoom |
| Space + drag | Pan (in Output, Space plays the simulation) |
| ⌘0 · ⇧⌘0 | Fit to window · Actual size |
| ⇧ while moving / rotating | Lock axis / use 15° steps |

## Build

```bash
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cargo test --workspace --locked
cargo run --release -p vectorio-app
```

## Install on macOS

```bash
python3 scripts/install-macos.py
```

Installs or updates `/Applications/Vectorio.app` with the Relief icon. The installed app runs its binary directly: it does not read the source project or rebuild at startup. If compilation fails, the installer preserves the previous version. An already open window keeps its current version until the next launch.

Development and test profiles omit debug symbols and incremental compilation to limit disk usage. `target/` caches can be rebuilt and are not published to GitHub. `cargo clean` removes them, including examples and screenshots stored there; save any output you want to keep first.

## Structure

- `crates/vectorio-core`: tested application logic: geometry, fonts, fills, shapes, vectorization and optical mixing (`mix`), optimization, accumulation maps and drying plans (`output`), SVG, documents and generated groups (`generate`), and the scene engine (`scene`).
- `crates/vectorio-app`: the eframe/egui interface: `app`, `canvas`, `commands`, `create` (generators), `inspector`, `vectorize_ui`, `output` (output preparation), `prefs` and `prefs_ui` (settings), `colorpanel` (integrated color editor), and `theme`.
- `crates/vectorio-core/fonts`: single-line SVG fonts.

## Next steps

- EBB serial driver for sending directly to EasyDraw, with pen-change pauses.
- Named groups.
- Layer-aware fill clipping so lower objects are not drawn beneath upper objects.
- A glyph editor.
- PNG preview export.
- Cache route drawing in Output for photos with very large stroke counts.

## Credits

- Topography: Roland Blok's [FastMarchingTopoPlot](https://github.com/rolandblok/FastMarchingTopoPlot) algorithm, adapted to Rust. Its Unlicense is retained at `third_party/FastMarchingTopoPlot/UNLICENSE`.
- EMS fonts (Allure, Capitol, Casual Hand, Neato, Pepita, Society, Tech) by Evil Mad Scientist Laboratories, and Hershey Script derived from Dr. A. V. Hershey's fonts. SIL Open Font License 1.1: `crates/vectorio-core/fonts/OFL.txt`. See [svg-fonts](https://gitlab.com/oskay/svg-fonts) and [Hershey Text](https://www.evilmadscientist.com/2011/hershey-text-an-inkscape-extension-for-engraving-fonts/).
