# Task for Sol · Festival city, correction 4 (final)

From here on, you're still with the city. This document summarizes what state it is in and what is missing, so you don't have to reconstruct it by reading everything.

The rules of `docs/plan-paso1-sol.md` shall continue to apply:
- Spanish at the interface, and each new recipe text with its translation into
  `crates/vectorio-app/locales/app.yml`;
- without `cargo fmt`, no new dependencies and no commits;
- `--test-threads=1` in the tests;
- `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"` if you do not find `cargo`;
- Do not close or restart Vectorio if it is open;
- Invented and generic labels in Spanish, without marks.

It goes for milestones that leave the tests green: if the margin runs out, by the time you finish one.

## 0. State of origin

Rama `feat/crear-motor-escena`, with many changes **unconfirmed** (neither confirm nor discard). All tests go through and clippy doesn 't notice anything on `generate/` or `scene/`. The app installed on `/Applications/Vectorio.app` carries everything listed here.

### Map of archives

|The file|What's the matter?|
|---|---|
|`generate/festival.rs`|Recipe, `City` (camera, streets, City blocks, landmarks), parameters and tests|
|`generate/festival/events.rs`|Events in the square: verbena, market, fair, horseback riding|
|`generate/festival/stories.rs`| « Living on the street »: work with crane, balloon, pigeons, tenders, scaffolding|
|`generate/festival/coast.rs`|Coast: beach or port on one of the front sides|
|`generate/festival/avenue.rs`|diagonal avenue, wedge-shaped buildings and park|
|`generate/village.rs`|White village; shares with the city figures, pigeons, vendor and vegetation|
|`generate/kit/`|Common parts (`landscape.rs` has `vegetation` with `Plant::{Cypress, Olive, Shrub, Palm, Pine}`)|

### Current city parameters

`people`, `landmarks`, `event`, `stories`, `circuit` (« Urban Road »), `coast` (« Costa »: none/beach/port), `avenue` (« Avenida diagonal », only without race), `yaw`, `sun`, `shadow`. The recipe version is still on 1.

### What has been done (corrections 1 to 3)

You 'll find the details in the 1 passage from `docs/plan-crear-fase5.md`.

- **Base:** city full bleed, urban race, shadows projected, the four
  events and street life.
- **Cost:**
  - beach with palm walk, umbrellas, lifeguard, volleyball and boats;
  - Port with pier, long, spike with lighthouse and pants.
- **diagonal avenue:**
  - passes in front of the square, or behind if there is a coastline;
  - the middle one with trees and lamps, and four lanes;
  - buildings in wedges with a shafted forehead;
  - A park with a fountain, cypresses on the edge, pine trees at the bottom and olive trees in front.
- **Crossing with the village:** slender pine trees and palm trees at the entrance to the square.

## Hito 1 · Finishing the avenue

Look at `avenida_{225,315,45,135}.svg` and `av_costa.svg` before touching anything; the order to generate them is in  « How to look at a sheet ».

1. **It's a hole in the middle.** At the central junction, there are a few missing. 30 m of median,
   Because that's where two streets from the grid come in. `free_runs` avoids the streets with
   `in_street`. It closes the stretch between the two median streets and trees, and
   Take a zebra step with people crossing the avenue on every street you come in.
2. **You can hardly see it on the coast.** The avenue is at the bottom and just looks out the window.
   the upper edge. The camera adds up the movements `COAST_SHIFT` and
   `AVENUE_SHIFT` to `camera()` from `festival.rs`. Look for a frame in which
   You can see the sea and the middle at the same time, even if it's on the side.
3. **The bowls can't be read.** The wedges on the side of the square are facing and
   They look like boxes. Give the bow something to distinguish it: a cylindrical turret.
   lower (`scene.lathe`) or a wider shed with window.
4. **Tests.** already exists `festival_coasts_and_avenue_are_clean_in_every_view`.
   It adds one that verifies that, with the avenue active, there is a stroke of
   vegetation near its axis in the four views.

## Hito 2 · Dominant district

It's the pending parameter of the plan. Add `district` (« District »), a choice between  « Commercial »,  « Residential »  and  « Leisure », with a random draw:

- **Commercial:** what it is now, with windows on all facades.
- **Residential:** lower heights and pairs, more balconies and tenders, few
  windows and some yard with trees in the middle of the block.
- **Leisure:** terraces with tables on sidewalks, more rooftops with people, signs and
  a more likely stage-type milestone.

It changes the proportions, not the geometry of the grid. The core around the square remains the same in all three districts.

Evidence: the sweeping of seeds and views already exists. Add the district to the `check_seeds` loop and check on all three options that there are no announcements and that you don't exceed the budget.

## Hito 3 · Globes and flags

- **Vertical banners** hanging from the street lights and the street lights
  Core streets, each 2 or 3 lighthouse.
- **Flags** on masts on some core roofs and on the lighthouse
  the port.
- Loose **balloon clusters** attached to the posts and rails of the
  square. The balloon seller already exists in `kit::life`; reuse his drawing
  of the globe.

They're strokes and they don't cover anything: don't turn them into shutters, or the seam detector will protest. Ink  « line », or  « labels »  if it looks better.

## I did. 4 · Review sheet by sheet

It generates the seeds 0 to 11 in the 225 view and four more in each of the other views, with random parameters. Note in `docs/plan-crear-fase5.md`, in one line per sheet, what fails:
- something floating or cut;
- empty asphalt;
- the covered square;
- People on a building;
- text on top of each other.

Set aside what's obvious and write down what doesn't take time.

## Hito 5 · Closure

- `cargo test --workspace --release -- --test-threads=1` and
  `cargo clippy --workspace --release --all-targets`. The new code should not
  give no warning.
- Contacts sheet:
  `cargo run -p vectorio-core --example hoja_contactos --release -- target/phase5/contactos.svg`.
  **No** pass `qlmanage` to the entire sheet, because it hangs (weight 23 MB).
  Cut each sheet in its own SVG, as in the previous order.
- `python3 scripts/install-macos.py`. It takes a few minutes. Throw it in a second.
  It's a plan.
- In `docs/plan-crear-fase5.md`, a note  « Correction 4  »  below the previous ones.

## How to look at a sheet

```bash
cargo run -q -p vectorio-core --example muestra_receta --release -- festival_city target/phase5/x.svg 4 avenue=si circuit=no coast=port yaw=225
```

```bash
cd target/phase5 && qlmanage -t -s 1400 -o . x.svg
```

This leaves `x.svg.png`. Choice options are passed by their key (`coast=port`, `event=fair`).

## Known traps (learned in 3 correction)

- **Switches in `muestra_receta`:** are as `si` / `no`. `true` and `false`
  They're read as text and ignored, and the sheet comes out with the default values,
  With the race set and no avenue.
- **`scene.lathe`** does not draw anything if the profile heights do not increase
  strictly. For a ladder, climb 0.02 m instead of repeating the height.
- **The paper covers little.** On the default scale, the sheet only covers a few
  ± 35 m horizontally around the centre. Everything that is placed away from the
  Place has to be checked with `on_paper`.
- **The margin of `on_paper` It goes in millimeters and expands the area of the leaf.** One of them
  Negative margin shrinks it: in `avenue_span` I put `-40` and the avenue
  disappeared without making a mistake.
- **Streets and avenues:** the grid streets pass through `self.pieces(a, b)`,
  which cuts the land and the avenue, and by `self.street_band(a, b)`, which
  The curb ends up obliquely opposite the avenue. If you add another street, pass it.
  By two o'clock.
- **Diagonal coordinate:** `Avenue::offset(p)` gives the meters to the axis of the
  Avenue and `side(origin)` the side of a city block. `cut_lots` lists the cells
  And these don't carry a regular city block: they carry a wedge or a park.
- **Paint order:** the camera is calculated once in `City::new`, first the
  The coast and then the avenue. If an element depends on the view (what goes
  front or back), uses `scene.camera().axes().2` flattened with `flat`, as
  `park_trees`.
