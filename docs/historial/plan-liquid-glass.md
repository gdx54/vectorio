# Plan · Consistent Liquid Glass material (clear and dark)

Plan to implement it in phases (Sonnet). Each phase compiles, passes `cargo test` and `cargo clippy --all-targets -- -D warnings` and installs with `python3 scripts/install-macos.py`. Preserves Spanish as the source language, `tr!/ts!/tm!` in any new text and object/layers  ↔  inks separation. It doesn't change any behavior, just the look.

## 1. Diagnostic (catches of 12 - 11 - 2026)

What's on `theme.rs` today is a panel filled with **vertical degraded** (`glass_top` → `glass`), a bright **just above** (`rim_top` → `rim_bottom`), a stroke of 1 px and a shadow. It's not a single material; each piece is painted in its own way:

|Piece by piece|How does it look today?|This is a problem.|
|---|---|---|
|Upper bar (`app.rs::toolbar`)|`TopBottomPanel` opaque `pal.bar` and separating line|It 's a classic macOS 14 bar. In Liquid Glass the bar is transparent over the contents and each control group floats in its own glass capsule.|
|Side panels and state panels|`glass_shape`|In **dark**, the degraded 66 →34 over a canvas of 30 reads like a misty grey box with a bezel above, more 2010 style than glass. In **clear**, the almost white glass over the 232 canvas remains like a flat white card, and the white glow over white is not seen.|
|Canvas|Smooth color, and `canvas::fit` separates the paper from the panels|There's nothing behind the glass. Since egui does not blur, the translucency contributes nothing and only makes it dirty.|
|Segmented (`segmented`, `choice`, `text_segment`)|opaque background `pal.control` and rectangular selection r = 8|In the dark it's gray on gray and the selection doesn't stand out. They're not capsules.|
|Accent button (`accent_button`)|Blue rectangle plane|In macOS 26 is a glass capsule stained with brightness.|
|Ink cards (`inspector::paint_pen_card`)|`pal.card` 64.64.70 almost opaque in the dark|Heavy blocks competing with the color band.|
|Fields and deployable|`pal.control` opaque rectangles and the ▼ of egui|They're opaque on the glass, and the triangle isn't Mac's.|
|Sheets (Styles, Shortcuts, Color, Photo) and notices|`glass_frame` = `pal.bar` opaque|Each with its own margin and radius.|
|Radios|18 panel, inner margin 14, control 8, segmented 10, card 12|They are not concentric: 18 − 14 = 4 ≠ 8.|
|Secondary text|152 (dark) or 110 (clear) opaque grey to 10.5  –  11.5 pt|It lacks contrast in the dark, especially in cards and ideas to start with.|

In short, the light almost arrives because the shadow separates the panel from the background. The dark fails because in the dark the shadow cannot be seen, and the only thing that defines the panel is a degraded gray with a brightness above.

## 2. Principles of the material

egui 0.29 can't read or blur what's underneath, so the glass is simulated with four layers, **always the same** for everything that floats:

1. **Shadow**: double.
   - One of contact, short and sharp, to define the edge.
   - Another environment, wide and soft, to take it off the bottom.
   - In the dark, the contact becomes a black outer stroke, because a black shadow over black is not visible.
2. **Body**: a nearly uniform **and translucent** dye, with a very slight vertical degradation (≤ 4 % luminosity), not the current of ~ 30 units.
3. **Specular edge**: a ring of 1 px by **all** the perimeter, whose intensity depends on the orientation of the normal.
   - Maximal up on the left, a second maximal weaker down on the right and almost nothing on the sides.
   - That double light is what makes it look like glass and not plastic.
4. **Interior line of depth**: 1 px right inside the ring, dark and very faint, only in the lower half. It's thick without bezel.

The controls inside the glass use **alpha-filled** (dark white, light black), not opaque gray. That's how they work on any glass and canvas.

## 3. Stages

### Phase 1 · A single material in `theme.rs`

1. Replace `glass_shape` with an API of materials:
   ``` rust
   Subject: Panel, Capsule, Sheet, Popup, Tinted(Color32)
   In the case of the manufacturer, the manufacturer shall provide the manufacturer with the following information:
   Pub fn glass_panel_with(ui, pal, radius, margin, m: Material, add) -> InnerResponse<R> // the current plus the material
   ```
   - `Panel`: side panels, state and output panels.
   - `Capsule`: the bar groups (radio = high / 2).
   - `Sheet`: moulded leaves, more opaque.
   - `Popup`: menus and drop-downs.
   - `Tinted`: the accent button.
2. **Specular edge**: reuse `rounded_outline` (up to 12 sections by corner so that the ring does not look faceted).
   - For each vertex, calculate the external normal `n` and define `k = 0.5 + 0.5·dot(n, (-0.6,-0.8))`. The light is coming from the top left.
   - Ring intensity: `rim_hi·k^3 + rim_lo·(1-k)^3`. The second term is shining down to the right.
   - Band of 1.0 px (not 1.25), with alpha per vertex as already done.
3. **Inner line**: second ring between `shrink(1.0)` and `shrink(2.0)` with `pal.inner_shade`, multiplied by `smoothstep(0.4, 1.0, y_rel)`.
4. **Double Shadow**: contact `Shadow{offset:(0,1), blur:3}` with `shadow_contact`, and environmental `Shadow{offset:(0,10), blur:32}` with `shadow`.
5. **new palette**. Replace `glass_top`, `rim_top`, `rim_bottom` and `card` with the fields in this table. They're starting values, adjusted for catches.

   |Field|Of course it is.|It 's dark.|
   |---|---|---|
   |`canvas` (top → below, very soft degraded)|238,238,242 → 230,230,235|22,22,25 → 16,16,18|
   |`glass` (panel)|255,255,255 α 0,74|44,44,50 α 0,72|
   |`glass_sheet`|250,250,252 α 0,94|38,38,42 α 0,94|
   |`glass_tint_top` (degraded from the body, mixed above)|white α 0.10|white α 0.04|
   |`rim_hi` / `rim_lo`|white α 1.0 / white α 0.55|white α 0.34 / white α 0.14|
   |`inner_shade`|black α 0.05|black α 0.35|
   |`outline` (external stroke)|black α 0.07|black α 0.60|
   |`shadow` / `shadow_contact`|black α 0.12 / 0.08|black α 0.45 / 0.50|
   |`fill_control` (fields, segmented without selection)|black α 0.05|white α 0.08|
   |`fill_hover`|black α 0.08|white α 0.12|
   |`fill_card` (ink card)|white α 0.70|white α 0.07|
   |`knob` (selected segment)|opaque white, with contact shade|white α 0.20, with `rim_hi` ring|
   |`text`|29,29,31|245,245,247|
   |`secondary`|96,96,104|174,174,182|
   |`tertiary` (leads, ideas... ")|132,132,140|128,128,136|
   |`separator`|black α 0.08|white α 0.10|
   |`accent`|0,122,255|10,132,255|

   - The dark lowers the canvas to 16  –  22 and raises the glass to 44  –  50: the panel remains **above the background**, not at the same level.
   - The clear lowers the canvas a little so that the white glass contrasts.
   - It applies canvas degradation with a `Mesh` of 4 vertices on `canvas::show` and `output::show`, instead of `fill(pal.canvas)`. Leave `pal.canvas` as the middle color for the uses you already have (deactivated control attenuation, `vectorize_ui.rs:602`).
6. **Concentric radios**:
   - `PANEL_RADIUS = 22`, `PANEL_PAD = 12` (replacing `PANEL_MARGIN + 4`) and `CONTROL_RADIUS = PANEL_RADIUS - PANEL_PAD = 10`.
   - The ink cards use `CONTROL_RADIUS + 2`.
   - The capsules use high / 2.
   - The sheets use `SHEET_RADIUS = 26` with `SHEET_PAD = 20`.
   - Put everything in `theme.rs` and remove the loose numbers: `22.0` from the state, `20.0` from the notice and `Margin::same(22.0)` and `18.0` from the sheets.
7. **Tests** in `theme.rs`:
   - WCAG contrast: composite `text`, `secondary` and `tertiary` over `glass` over the `canvas` (alpha over opaque).
     - `text` ≥ 7: 1.
     - `secondary` ≥ 4.5: 1.
     - `tertiary` ≥ 3: 1.
     - Check clear and dark.
   - The ring is symmetrical to the diagonal and the maximum falls up to the left.

### Phase 2 · Transparent capsule bar (`app.rs`)

1. Remove the opaque `TopBottomPanel`.
   - Canvas (`CentralPanel`) occupies the entire window.
   - The bar turns into a `egui::Area` with `Order::Middle` and high `TOOLBAR_H`.
   - Keep the window drag and double click on the empty space: now the `interact` goes over the rectangle of the **bar before the** of the controls, as today.
2. **Upper edge effect**: a degradation of the canvas colour from α 0.85 to 0 in the early `TOOLBAR_H + 12` px.
   - It's painted on the layer of canvas, above the paper and under the bar.
   - So the title and buttons are read even if the paper passes underneath.
3. Each group of the bar is painted with `Material::Capsule`, high 34 and radio 17: the tools, Photo/Open, Deshacer/Re-Make and Design/Output.
   - Rewrite `segmented` with the glass capsule bottom, `fill_control` inside and padding 3.
   - Rewrite `paint_segment_bg` with a **capsule-shaped** (`knob`): contact shadow in light and ring mirror in dark.
   - Animate the knob between segments with `ctx.animate_value_with_time(id, x, 0.18)`. It 's optional but very macOS 26.
   - The text of the selected segment goes to `text` and semibold. The rest, in `secondary`.
4. The title  « Vector / Untitled »  is left without capsule, in `text` and `secondary`, as in Finder.
5. `canvas::fit`: the paper fits under the bar (add `TOOLBAR_H` to the upper margin), because now the canvas starts at y = 0.
   - Check `canvas.rs:121` and the coordinates that subtract the bar.
   - See also `help_sheet`, the warning anchor and `floating_panels` (`top`).
6. **Output**: the same. `output::show` must not leave an opaque strip.

### Phase 3 · Controls and contents on the glass

1. **`visuals()`**:
   - `widgets.inactive` = `fill_control`, `hovered` = `fill_hover` and `active` = `accent` α 0.18.
   - `rounding` = `CONTROL_RADIUS` in general; small buttons (`small_button`,  « Ajust ») in capsule.
   - `window_fill` / `menu` use the color of the material `Popup`.
   - `menu_rounding` = 12 and `popup_shadow` = environmental shadow.
   - `extreme_bg_color` (text fields) = `fill_control` also, not pure white or black.
2. **`accent_button`**: switch to capsule with `Material::Tinted(accent)`.
   - Body with accent, upper half mixed a 10 % with white and the ring specular on top.
   - Semibold white text.
   - When the mouse passes, it lights up a 6 %. When pulsed, it obscures a 8 %.
   - It is also used in the `colorpanel.rs:129` made, which today carries its own `Button`.
3. **Displayable**: a `icon` own for `ComboBox` with an up/down chevron (two  « V »  of 4 px in `secondary`) instead of ▼.
   - Make it `theme::combo_icon` function and apply it to all `ComboBox` with `.icon(theme::combo_icon)`.
4. **Ink cards** (`paint_pen_card`):
   - Back `fill_card` with the same specular ring, in a fine version, and without `glass_stroke`.
   - The color band stays, but the brightness (`sheen`) passes to a white degraded α 0.22 to 0 instead of a rectangle.
   - The hex code and thickness are in `tertiary` (11 pt). The name, in `text` 12.5 semibold.
   - In `Ghost` (tracking) use `Material::Popup`, not the color of the opaque card.
5. **`section()`**: 11 pt semibold on `secondary`, with + 0.3 of interlettering if egui allows it. If not, leave it like this.
   - Help texts under the sections go to `tertiary`.
   - Check `inspector.rs`, `output.rs` and `prefs_ui.rs`: any `pal.secondary` that is a lead and not a label goes to `tertiary`.
6. **`swatch`** (paper): the selection ring in `accent` to 2 px with 2 px separation, as now. The edge stroke goes to `outline`.
7. **`close_button`**: capsule `fill_control`.

### Stage 4 · Sheets, menus and notices

1. `glass_frame` disappears and all the sheets use `Material::Sheet` with `SHEET_RADIUS` and `SHEET_PAD`.
   - It affects Settings (`prefs_ui.rs:55`), Shortcuts (`app.rs:837`), Color (`colorpanel.rs:61`) and Photo (`vectorize_ui.rs:313`).
   - For this purpose `glass_panel_with` accepts a `Frame` / margin, or `glass_sheet(ui, pal, add)` is added.
   - Today they all make `.fill(pal.bar.to_opaque())`: that opacity is now given by the α 0.94 of the material.
2. The leaf veil passes to black α 0.25 in light and α 0.45 in dark, plus a soft radial degradation if it's cheap. If not, without degradation.
3. **Warning (`toast`)**: `Material::Capsule`, centered under the bar.
4. The menus of egui (Export, deployable) take `window_fill`, `window_stroke` = `outline` and the shadow of `visuals()`. They can't wear the mesh ring; with the stroke and the shadow that's enough.
5. The circles of `vectorize_ui.rs:517/523` (`pal.bar`) pass to `Material::Capsule` circular or to `glass_sheet`.

### Phase 5 · What's behind the glass (optional, after validating 1  –  4)

egui does not blur. But we do know what's under each panel: the paper, which is a known colored rectangle.

1. **Paper  « seen through  »**: if the rectangle of the on-screen paper cuts a panel or bar, paint inside the shape of the glass and **before** of the body:
   - a `Shadow` with `blur` ≈ 24 and `spread` 0 of the rectangle of the paper, cut to the panel;
   - with the color of the paper and α 0.55.

   So you see a clear spot of diffuse edges, which is the blurring of the paper. `glass_panel_with` paints it if it receives `backdrop: Option<(Rect, Color32)>`, which `app.rs` calculates from `self.view`.
2. **Adjustment of the paper**: `canvas::fit` ceases to reserve the entire width of the panels. It puts the paper between them, but allows it, when zooming or scrolling, to pass underneath. It already does; it only changes that now it will be seen.
3. Measure the cost with a big project. If a frame takes > 1 longer, turn it off during drag.

### Phase 6 · Visual inspection

Capture in clear and dark without touching the user's preferences or windows:

```bash
H=$(mktemp -d); mkdir -p "$H/Library/Application Support/Vectorio"
echo '{"appearance":"oscuro"}' > "$H/Library/Application Support/Vectorio/preferencias.json"
HOME="$H" /Applications/Vectorio.app/Contents/MacOS/vectorio &   # separate instance; close it with kill <pid> when finished
```

- Use `screencapture -x -l <windowid>` with the window ID of that bid. This project has already used a small Swift with `CGWindowListCopyWindowInfo`.
- **I never** You shut down other instances of Vectorio.

Checklist, in both light and dark:

- [x] The panels separate from the canvas without looking at the shadow.
- [x] The edge shines up to the left and a little down to the right, never like a bezel.
- [x] The bar has no stripe or line. The groups are capsules and the segmented knot is distinguished at first glance.
- [x] Export is a bright blue capsule.
- [x] Ink cards weigh less than their color band.
- [x] Track texts are read in the dark.
- [x] Settings, Shortcuts, Color and Photo share radio, margin and material.
- [x] The output mode has no opaque bands.
- [x] Check at the interface scale 0.85 and 1.25 (General Adjusters →).

Results of the review (12 - 11 - 2026): spikes in the shadow of the small capsules, the hollow knot of the sliders, the almost invisible round boxes in the clear, the gray of an invisible pass on the ink map and the adjustment of the paper when starting with another scale were corrected.

## 4. Out of range

- Real defocus with its own wgpu pass (`egui_wgpu::CallbackTrait` on a copy of the framebuffer). It's possible, but it's expensive and fragile in 0.29 frame. It would be a separate plan if the 5 phase isn't enough.
- `NSVisualEffectView` native behind the window: it would only blur the desk, not the canvas.
- Changes in layout or flow, and changes in icons (except for deployable chevron).

## 5. Suggested cast

|Stage|The risk|Model / reasoning|
|---|---|---|
|1 Material and pallet|Medium: it is the basis of everything|Sonnet, high reasoning|
|2 Transparent bar|Medium: coordinates and drag|Sonnet, high|
|3 Controls|Down there.|Sonnet, half|
|4 Sheets|Down there.|Sonnet, half|
|5 Fund through|Mean-high|Sonnet, alto, or opus if the 1  –  4 do not convince|
|6 Captures and fine-tuning|—|Opus for the Final Judgment|
