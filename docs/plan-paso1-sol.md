# Task for Sol · Phase 5, Step 1: Festival city (reduced scope)

Short budget (≈ 8 % weekly). The plan goes for **milestones that leave the project green**: if the budget ends in half, what has been done is useful and proven. Stop as soon as you complete one milestone and there's no room for the next one.

## 0. Before we start (just read this)

- `AGENTS.md` (project standards) and the 3 section of `docs/plan-crear-fase5.md`.
- `crates/vectorio-core/examples/kit_denso.rs`: **is the** chuleta. It teaches how
  It's called each piece of the kit (multitude, tapes, vehicles, facades, tower,
  roof, labels) and how to frame a scene with `fit_scene`.
- `crates/vectorio-core/src/generate/village.rs`, **only** the lines 431  –  475
  (`style`, `scene`) and 610  –  668 (`impl Generator`). It's the model you have to copy.
- `crates/vectorio-core/src/generate/kit/mod.rs` (lines `pub use`): the API
  Available.

Do not read the whole `village.rs`, `scene/*.rs` or `facade.rs`: use `grep -n` to find a signature. Do not touch the engine (`scene/`) or the kit unless there is a real failure, and in that case fix it and test it there.

## Non-negotiable rules

- Interface in Spanish; each new text of the recipe (name, description,
  It has been translated into more than a dozen languages.
  `crates/vectorio-app/locales/app.yml`. The proof
  `every_key_used_in_the_code_is_translated_and_none_is_left_over` is demanding it.
- Identifiers in English; comments and documentation in Spanish.
  New files: one sentence per line and short functions.
- **Do not run `cargo fmt`** Do not add dependencies. Don't commit.
- `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"` if you can 't find `cargo`.
- Do not close or restart Vectorio if it is open.
- **Rule of origin:** no actual marks, logos or labels. Labels and labels
  invented and generic in Spanish (PAN, LIBROS, MERCADO, CAFÉ, FERIA...).
- Time trials with `--test-threads=1`.

## Hito 1 · Minimum registered generator (required)

New file `crates/vectorio-core/src/generate/festival.rs`, with `pub struct Festival;` and `impl Generator`:

- `id` `"festival_city"`, `name`  « City at the party », `category`  « Scenes »,
  description of a line.
- `inks()` returns `&inks::DENSE` (already exists: line, people, vegetation, water,
  It's all about hatching, labels, land.
- `params()` reduced, with `ParamSpec::Version{current:1}` at the end:
  - `people`  « People »  0  –  1 (by default 0.6);
  - `landmarks`  « Landmarks »  1  –  3 (by default 2);
  - `circuit`  « Circuit »  (yes or no, by default yes);
  - `yaw`  « Vista »  and `sun`  « Turn of the Sun », copied from the village.
- `generate`: like the people (`normalized`, `scene`, `finish`, and in
  the clearance `construction_issues(1.5)` goes to `warning`).
- Register in `generate/mod.rs`: `mod festival;`, a `static FESTIVAL` and
  add it to `ALL` (the array goes from 3 to 4 elements).

Content of the 1 milestone (simple but already dense scene):

- 3 × 3 City blocks grid of some 24 m with streets of 8 m. Every street is one.
  `Band` with `ribbon(..., Marking::Dashed)` and one `zebra` at each crossing. The Passage
  **outside** of a band station (`+0.3` m); otherwise the
  The sewing detector is protesting.
- Each block: 1  –  3 `house(..., Roof::Flat, ...)` of 3  –  6 plants, with
  `window_grid` and `shopfront` on the walls facing the chamber
  6 panel of `kit_denso`) and `roof_terrace` on some roof.
- People: `Crowd::new(style)` **one single** for the whole leaf (your 3,000 top)
  It 's for the sheet. Crosses with `crossing`, queues with `queue` in front of the
  shops and `field` in a square that replaces a city block.
- Traffic: `convoy` with cars, motorcycles and buses on two or three lanes
  The streets.
- Camera: `Camera::fit` over the city box and `fit_scene`, as in
  `kit_denso`. full bleed: outside streets pass through the leaf.

Tests, in `festival.rs` within `#[cfg(test)] mod tests`:
- determinist with the same seed;
- without NaN and all points within the area;
- `warning` empty in the seeds 0 to 11;
- `Scene::load().within_budget()` in the seeds 0 to 11.

Check of the milestone: `cargo test -p vectorio-core --lib -- festival` and `cargo test -p vectorio-app` (translations) Generic testing of `generate/tests.rs` iterates by `all()` and already covers the new generator.

## Hito 2 · Hito and circuit

- `landmarks` decides how many of these are placed, drawn without repeating, each
  Instead of a city block:
  - **glass tower**, with `glass_tower`;
  - **stage enclosure**: one low box as stage, two `stands` full
    of people staring at the stage and a `field` in front;
  - **circular deposit**, with `scene.lathe` and a `sign` label.
- `circuit`: an elevated ribbon (`ribbon(..., 0.8, Marking::Kerbs)`) surrounding two
  City blocks and up and down (pattern of 4 panel of `kit_denso`), with a `convoy` of
  The cars. You must not cut buildings: elevation above their height or
  Round them out.
- Tests: the same as the 1 milestone, for 1, 2 and 3 milestones and with and without circuit.

## Hito 3 · Example, installation and note

- `examples/hoja_contactos.rs` travels through `all()`: already includes the city with its 12
  Seeds. Execute him with
  `cargo run -p vectorio-core --example hoja_contactos --release -- target/phase5/contactos.svg`.
  Look at the sheet with `qlmanage -t -s 1600 -o target/phase5 target/phase5/contactos.svg`
  and open the PNG.
- It only corrects the obvious: something floating, overlaps or an empty leaf.
- `cargo test --workspace -- --test-threads=1` and
  `cargo clippy --workspace --all-targets`. The new code must not give any
  Notice; old notices from other files are not touched.
- `python3 scripts/install-macos.py`.
- In `docs/plan-crear-fase5.md`, under the point 1 of  « Steps », a note
   "  *Made (shortened version).  "*  "  What goes in and what goes out.

## Outside of this assignment (don't do it)

Noria, lighthouse, beach or port, park with three species of trees, balloons and flags, diagonal avenue, detailed seed to seed valuation and dominant district parameter. Leave it listed as pending on the note.

## Famous Traps

- Yaw 45: the visible walls are +x and +y, and +x goes to the left on the paper.
  Use `wall.faces_camera(scene)`, don't deduct it by hand.
- Two coplanar solids overlapping give warning on `construction_issues`.
  Leave gaps (`kit::street::apart`) or melt the pieces.
- With a lot of people you get to the top of the segment (40,000) sooner than face-to-face.
  If `within_budget` fails, lower the density of `field`, do not climb the top.
- The 500 seeds test (ignored) requires less than 80 m of stroke and less than
  1s by seed.
