# Plan: Output polish, preferences, and languages

Plan to execute it in phases. Each phase leaves the app compiling, with tests in green and installed.

## Rules for all phases

- Read `AGENTS.md` before we start. The interface is still in Spanish. The separation between objects/layers and inks is maintained.
- **Do not run `cargo fmt`**: the project uses dense lines on purpose and the formatting would contaminate the entire diff.
- If `cargo` is not available: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- At the end of each phase:
  1. `cargo test --workspace`
  2. `cargo clippy --workspace --all-targets` (without new notices)
  3. `python3 scripts/install-macos.py`
- Do not close or restart the Vector that is open.
- Update `LEEME.md` when you change something visible.
- One commit per phase, only if the user asks for it. Complete the message with the co-authoring line indicating the session.
- **Step 0 (before phase A):** Output work is uncommitted on `feat/tipografias-y-menu`. Ask the user to confirm or authorize the commit before touching anything, so they can compare.

---

## Phase A · Adjustments to the exit plan

The basis is good and should be preserved: conservative model, intact safe routes, secondary calculation, proper paper layer and good tests. Changes are in order of priority. A1 – A5 are mandatory. A6 – A9 is only made if the measurements justify it.

### It's A1. A single check of the exit list
Today there are three different versions:
- `output::show` uses `rendered.rev == rev && !tracing() && !is_computing() && ready()`.
- `export_svg` and `export_per_pen` do not verify `is_computing()`.
-  « Plan with waits... »  in the bar only check `output_ui.ready()`.

Changes:
- Add `pub fn output_ready(&self) -> bool` to `VectorioApp` (`app.rs`) with full condition and usable in all those sites.
- `output::show` calculates `current` and `ready` with it.
- Proof: with true `is_computing()`, `output_ready()` is false.

### It's A2. Notification of the result of exports
Now `OutputUi::export_plan` and `export_svg` keep the error in `self.error`, which is only seen in the inspector. Exporting from the bar, the user knows neither success nor failure.
- Have them return `Result<Option<PathBuf>, String>`. `None` means the dialogue has been canceled.
- In `app.rs`, it displays a warning with `notify`:  « Plan saved to... »  or error.
- Keep the error on the inspector when the export is launched from the inspector.

### It's A3. The tour uses the color of the paper
`map()` always paints the paper `Color32::WHITE` and ignores `app.paper_tint`. With black paper and white ink, the simulation shows nothing.
- In `Map::Route`, paint the paper with `PAPER_TINTS[paper_tint]`. We 'll have to pass `paper_tint` to `map()`.
- The heat maps are still white, which is how you read them.
- The label on the bar doesn't change.

### It's A4. Remove the old Plotter mode and rename
`ViewMode::Plotter` is no longer on the canvas, so there's dead code:
- `canvas.rs`: the `ViewMode::Plotter` branch of `show`, `simulate` and `paint_plot` (section  « Plotter », to the 718 line).
- `app.rs`, `status_bar`: the `if self.mode == ViewMode::Plotter` block with Reproduce, speed and  « Deployment »  (towards the lines 789  –  803).
- `Sim.show_travel`, if you stop using it. `OutputUi` already has his.

Then rename `ViewMode::Plotter` → `ViewMode::Output` throughout the box. He's a renowned mechanic.

### It's A5. Recalculate without pulling
When you drag a slider, complete calculations are chained, and each result puts `sim.progress = 1.0`.
- **Preview Wait:** When the settings change, save the moment of change to `OutputUi`. Don't launch the work until 250 ms after the last change. He wants a repaint with `ctx.request_repaint_after`.
- **Cancellation:** is added `output::prepare_cancellable(..., cancel: &AtomicBool)`. `prepare` calls her with a flag that never activates. Check the flag on the `schedule` loop, every few dozen strokes, and on the `prepare` fingerprint loop. If activated, return an internal `Err` that the UI ignores. `OutputUi` keeps a `Arc<AtomicBool>` per job and activates it when it launches a new one.
- It retains the position of the simulation when a new plan arrives. Only put `progress = 1.0` when there was no plan.
- Core test: with the flag activated, `prepare_cancellable` immediately ends in error.

### It's A6. Fragment only where necessary (algorithm)
Now, if a single zone exceeds the limit, they slice **all** the strokes into `chunk_mm` sections. That adds ups and downs to areas that would never cross the line.

A cell with `total_heat ≤ max_wet_passes` can never get past it, so:
- In `prepare`, when `needs_protection` is true, only the strokes whose fingerprint touches any cell with `total_heat.counts[i] > max_wet_passes` are sliced. The others go whole.
- New test: a zone with 5 passes over the same line and a long line of 200 mm away from it. The long line is on a single stretch and `wet_after.max() <= cap`.
- Existing tests must continue, especially `many_crossings_keep_all_geometry…`.

### It's A7. Format of the JSON plan (thinking of the driver)
- Add to `OutputPlan`:
  - `generator: "Vectorio <versión>"`. The core doesn't know the version of the app: recycle it as a parameter of `prepare` or fill it in the app before serializing.
  - `origin: "top-left"` and `y_axis: "down"`.
  - The margin is already within `paper`.
- Create `docs/formato-salida.md` with the `vectorio-output-v1` scheme: units, shares (`change_pen`, `travel`, `wait`, `draw`) and what each field means.
- Schematic test: Serialise a small plan and check the list of first-level keys and those of each action. If someone changes the format without changing the version, they fail.

### A8. Heat map as texture (measured before)
`map()` clones the mesh for each frame (`egui::Shape::Mesh(mesh.clone())`). With one photo, you can have millions of vertices.
- Measure first: add to `examples/muestra_secado.rs` a case with one photo or many lines, and count the vertices of `heat_mesh`.
- If it passes through some 200 000 vertices, change to texture: `grid.pixels(2048)` → `egui::ColorImage` → `ctx.load_texture(..., TextureOptions::NEAREST)`, painted with `painter.image` over `paper_rect`. The colors of `HEAT_COLORS` do not depend on the theme, so the texture does not have to be regenerated when changing appearance.
- Adapt the `exports_and_simulation_stay_visible…` test: instead of `has_heat` with mesh, check that there is an image shape or rectangle with the texture of the map.

### A9. Core errors as type (prepare phase C)
- In `vectorio-core/src/output.rs`, change `Result<OutputPlan, String>` to `Result<OutputPlan, OutputError>`, with an enum of variants: `InvalidSettings`, `MissingPen`, `InvalidPenWidth`, `InvalidCoordinates`, `InvalidLength`, `Cancelled`.
- The Spanish text goes to the app (app `output.rs`) with a function `error_text(&OutputError) -> String`.

### Out of range for now
`draw_route` performance with huge photos (caching the shapes already drawn). Write it down in the next steps of `LEEME.md`.

---

## Phase B · Preferences

### B1, please. Model and file
- Create `crates/vectorio-app/src/paths.rs` with `pub fn support_dir() -> Option<PathBuf>`, which returns `~/Library/Application Support/Vectorio`. Use it on `styles.rs` and `theme::Appearance` instead of repeating the route.
- Creates `crates/vectorio-app/src/prefs.rs`:
  ``` rust
  # [derives(Clone, Debug, PartialEq, Serialize, Deserialize)]
  #[be (default)]
  public struct Preferences {
      Pub appearance: theme::Appearance,
      public language: Language, // System · Spanish · English (C phase activates it)
      Pub ui_scale: f32, // 0.8..= 1.5, ctx.set_zoom_factor
      pub paper_tint: usize, // today is lost at closing
      pub plotter_name: String, // "EasyDraw"; replaces the fixed text
      pub show_tips: bool, // advice from the document inspector
      public new_document: NewDocument, // paper, PlotSettings and OutputSettings by default
  }
  ```
- Use `Default` with the current values. `NewDocument::default()` is leaving `Document::default()`.
- File: `preferencias.json` in `support_dir()`. Atomic writing, just like today in `Appearance::save`: temporal and `rename`.
- **Migration:** if there is no `preferencias.json` but yes `apariencia.json`, take the appearance from there. Save the new file and leave the old file un erased.
- Move `Appearance::load/save` to `Preferences::load/save`. `Appearance` retains `label()` and `apply()`.
- In `VectorioApp`, replace `appearance` with `prefs: Preferences`. Add `fn set_prefs(&mut self, ctx, new)`, which applies changes (look, scale, ink on the paper) and saves. If it fails, notify `notify` as `set_appearance` does today.
- Tests:
  - Back and forth through JSON;
  - a blank JSON `{}` gives the default values;
  - The migration from `apariencia.json`. Use a time directory: make `load` / `save` receive the route in a probable internal function.

### B2, please. Adjustment window
- Create `crates/vectorio-app/src/prefs_ui.rs` with a `egui::Window` centered, without resizing, of some width 520 pt, with the style of `theme::glass_panel` and `theme::close_button`. Esc closes it.
- Pesta with `segmented` / `text_segment`, as in the bar:
  - **General of the Army:**
    - Aspect (System · Clear · Dark).
    - Language (System · Spanish · English). Until phase C, deactivated with the note  « Soon ».
    - Interface size, slider 80  –  150 %.
    - Showing advice.
  - **New document:**
    - Paper: size, width, height and margin. Reuse the document inspector's paper controls if they can be extracted to a function.
    - Color of the preview paper.
    - Speed: drawing, travel, up/down.
    -  « Use the buttons in this document »  and  « Reset ».
  - **Out of the way:**
    - Name of the plotter.
    - Initial drying values: weather, wet passes, steam, pauses when changing ink.
    - Note: they only apply to new documents; each project keeps its own.
- The changes apply at the moment, as in macOS: there is no  « Accept »  button. It's saved by dropping a slider or changing a value, not every frame.

### B3, please. Opening the Settings
- Add `Command::Preferences` with the abbreviation ⌘, to `COMMAND_KEYS` (`Key::Comma`).
- Native menu: in the menu  « Vector », after  « About Vector », add a separator and `item("Ajustes…", Some("CmdOrCtrl+Comma"), Command::Preferences)`.
- Add  «⌘  ⌘, »  to the shortcut sheet (`CHEATSHEET`).
- **Remove the menu  « Look »  from the top bar of** (`app.rs`, `ui.menu_button("Aspecto", …)`). The appearance is changed in Settings, and the Display submenu → Appearance is retained as a shortcut.

### B4. Use your preferences
- `new_document()` and `blank()` create the document with `prefs.new_document`: paper, plot and output.
- `paper_tint` is read from `prefs` and saved when changing, both from the menu and from Settings.
- `"EasyDraw"` in `output.rs` and in the bar (« Preparation · EasyDraw ») becomes `prefs.plotter_name`.
- `show_tips` hides the advice of `document_settings` from the inspector.
- `ui_scale` → `ctx.set_zoom_factor(prefs.ui_scale)` when starting and changing it.
- Updates to `LEEME.md`: section  " Adjustments (⌘,)  ". Remove the mention from the Aspect bar menu.

---

## Phase C · Languages: infrastructure and first screens

Objective: Spanish (by default and reserve) and English, ready to publish the code. In this phase, adjustments, native menus, the bar, the shortcut sheet and Output are fully translated. The rest is in phase D.

### C1 Library and archives
- Add to `vectorio-app`:
  - `rust-i18n = "3"`;
  - `sys-locale = "0.3"`, to detect the system language;
  - `serde_yaml` as a development dependency for key testing.
- In `main.rs`: `rust_i18n::i18n!("locales", fallback = "es");`.
- Create `crates/vectorio-app/locales/es.yml` and `locales/en.yml`.
- Use semantic keys by area: `menu.archivo.abrir`, `salida.guardar_plan`, `ajustes.general.idioma`... to interpolate: `t!("salida.secando", tiempo = …)`.
- `Language` (from `prefs.rs`) wins `fn resolve(self) -> &'static str`, which returns `"es"` or `"en"`. `Sistema` uses `sys_locale::get_locale()`: if it starts with `es`, `"es"`; otherwise, `"en"`.
- `set_prefs` calls `rust_i18n::set_locale(...)` when the language changes.

### C2. Numbers and units
- Create `crates/vectorio-app/src/fmt.rs` with `decimal(x, decimales)`: comma in Spanish, dot in English.
- Move there `duration`, `distance` and `pct` from `output.rs`, with units translated (« min »,  « s »,  « h »).
- It replaces the `.replace('.', ",")` in the app.

### C3. Native menu and shortcut sheet
- `CHEATSHEET` goes from `const` to `fn cheatsheet() -> Vec<(String, Vec<(String, String)>)>`.
- When you change the language, reconstruct the menu: save `menu = NativeMenu::install(ctx)` again. Check that `init_for_nsapp` replaces the previous menu and that the event manager is fine. If macOS has problems, alert with  « The menu will change language when you reopen Vectorio »  and do not reconstruct it.
- `AboutMetadata` (credits, comment) is also translated.

### C4. Displays of this phase
Translate all visible texts into:
- `prefs_ui.rs`;
- `commands.rs` (menu and sheet);
- the upper bar of `app.rs` and the `notify` notices in that file;
- `output.rs` (app) and `error_text` of A9;
- `theme.rs` (`Appearance::label`, tooltips).

In the `en.yml`, the terms of the domain are as follows:

|Spanish|English|
|---|---|
|ink / pen|pen|
|Get out.|Output|
|Dried|drying|
|wet passes|wet passes|
|Drying|bleed|
|Travel|travel|

### C5. Tests
- Parity test: load `es.yml` and `en.yml` with `serde_yaml` and check that they have the same keys.
- Use key test: go through `src/*.rs` and extract the `t!("…")` letters with a simple regular expression or `str::find`. Every key must exist in `es.yml`. `rust-i18n` returns its own key when it's missing, so it doesn't pass silently.
- Repeat the exit visibility test (`exports_and_simulation_stay_visible…`) in English. The texts are longer and should not be left out of the panel in 980 × 620.

---

## Phase D · Languages: the rest of the app

It's mechanical, but long: some 900 texts. Make a commit per file and pass the C5 tests on each.

1. `inspector.rs` (some 390 texts). Start with the `theme::section` headlines.
2. `vectorize_ui.rs`.
3. `colorpanel.rs`, `canvas.rs`, `styles.rs`. The names of predefined styles are translated; those saved by the user, not.
4. **Core tags** (`label()` in `fill.rs`, `font.rs`, `mix.rs`, `shapes.rs`, `vectorize.rs` and `trace.rs`): the core should not depend on `rust-i18n`.
   - Add to each enum `fn key(self) -> &'static str`, a stable identifier in ASCII.
   - In the app, translate with `t!(format!("core.trazo.{}", s.key()))`.
   - Leave `label()` in the core for sample and testing, or remove it if no one else is using it.
   - Pass `Err(String)` from `doc.rs`, `shared.rs` and `trace.rs` to enums, as in A9, or at least translate them into the app.
5. **It does not translate:**
   - the content of the documents already created (pen names, objects, paper);
   - the sample text  « Hola, Vectorio ».

   The default pens of a new document do come out in the active language: `default_pens()` receives the names from the app, or the app renames them after creating the document.
6. `LEEME.md`: mention the language in Settings. For open publication, propose to the user a `README.md` in English. You don't believe it without asking.

---

## Default decisions (change if the user says otherwise)

- The  « Apparent »  menu disappears from the bar. Display → Appearance is retained.
- Languages: Spanish and English.  « System »  Choose Spanish if the system is in Spanish; in any other case, English.
- Settings are a window within the app (`egui::Window`), not a separate native window.
- When you start, you still open a blank piece of paper. There is no preference to open the last project, because it would require reading Documents at the start.
