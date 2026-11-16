# Task for Sol · Festival city, correction 1

File: `crates/vectorio-core/src/generate/festival.rs` (you wrote it). The rules of `docs/plan-paso1-sol.md` remain valid: Spanish, without `cargo fmt`, without dependencies, without commit, translations into `app.yml`, `--test-threads=1`. Go down landmarks that leave the evidence green, until there's no room for you.

## What's wrong (review of 12 seeds)

1. **The city is a box.** There is 3 × 3 City blocks in the center and the rest of
   The leaves are empty streets that are lost to the edges. It must be ** city
   to the edge, full bleed **: paper cuts buildings, not asphalt.
2. **The elevated ring doesn't make sense.** It's a toy loop with cars.
   It's going around and it's not connecting to any street. It is replaced by a
   **urban race** going *through the streets of the city*.
3. **It's too far away.** To some 1.5 mm per metre there are no windows,
   windows, curtains or balconies (their visibility thresholds need some
   2 mm/m), and people come out tiny. That's why the buildings look like empty boxes.

## Hito 1 · Closer and city to the edge

- **Fixed scale by paper**, not by frame of 3 × 3:
  `scale = (min(ancho, alto) / 70.0).clamp(2.4, 4.0)` mm/m, with the camera
  centered in the centre of the square (`world_center`). In A4 comes out some 3 mm/m.
  Take the `camera.fit` off the box of 3 × 3. `fit_scene` may stay if
  Still missing the cut.
- **Infinite grid.** The cells go from `PITCH` to `PITCH` and the square is the
  the cell (0, 0). Calculate with `extent()`
  as it already does) the range of cells touching the leaf, plus one margin. Rhinestone
  **all** those cells with City blocks. The streets are generated for all that range and
  They always come out of the leaf at both ends.
- **Core and periphery.** The Commission 8 The cells around the square are the core:
  There go the milestones (`landmarks`), queues and rooftops with people. In the
  The City blockss continue to carry windows, windows and canvases, but
  Less people (no tails and no roofs). The periphery can't be more empty.
  than the core.
- **Budget.** Look at `scene.load()` before every peripheral block. By
  above the 80 % of `SEGMENT_BUDGET` or `FACE_BUDGET`, that block goes without
  `window_grid` (only volume and window). Don't go up the tops.
- **Tests** in `festival.rs`:
  - the scale is between 2.4 and 4.0 in A4 and in the contact sheet area;
  - No point of visible soil is left outside a contained cell:
    a grid of 20 × 20 points of paper, pass them to the ground with
    `camera.ground(p, 0.0)` and check that everyone falls into a populated cell or
    On a street;
  - The previous ones are still going on: determinists, no NaN, no `warning` in the
    Seeds 0 to 11 and `within_budget`.

## Hito 2 · Urban career instead of ring

Delete `circuit()` and `supports()`. The `circuit` parameter is called  « Urban Carriage »; the `id` remains and the recipe version continues at 1.

- **Traced.** The rectangle of the four streets surrounding the core of 3 × 3.
  It 's a closed loop **over the existing streets**, with the same `Band`.
  On those streets:
  - markings `Marking::Kerbs` instead of `Dashed`;
  - No zebra footsteps or normal traffic.
- **Fences** on both sides of the racetrack: a line ± 3.6 m from the
  axis with poles each 2 m (vertical short lines of 1 m). At the crossroads with
  normal streets the fence cuts the street: there the normal street ends at the
  fence, with a queue of people watching.
- **Racing cars** in one direction: a `convoy` of `Vehicle::Car` by
  each side of the rectangle, with the direction of rotation. That they do not occupy the
  Corners: leave 6 m free at each crossing.
- **Public** along the fences outside (`crowd.queue` parallel to the
  fence, to 0.8 m) and **a grade of** on one side. Reuse `stand_base` and
  `crowd.stands`, looking at the runway.
- **Target**: a square cross-sectional line (two square rows with
  `closed`) and a two-masted portico with a crossbow and `kit::sign(..., "META", ...)`.
- **Testing of**:
  - The layout is closed and each section falls on the axis of a street in the
    grid (x o y in the list of streets);
  - no vehicles less than 6 m from a race crossing;
  - With the active race they continue to pass the sewing tests, `warning` and
    The budget.

## Hito 3 · Contact sheet, note and installation

- `cargo run -p vectorio-core --example hoja_contactos --release -- target/phase5/contactos.svg`.
- To see it, he cuts every sheet of the city into his own SVG (all the
  `<g transform="translate(...)">…</g>` complete, eye closed) and
  Make the miniature with `qlmanage -t -s 1200`. `qlmanage` with whole leaf
  Don't try it.
- Check with your eye:
  - no sheet has empty asphalt on the edges;
  - You can see windows and windows.
  - The race reads like fenced streets.
- `cargo test --workspace -- --test-threads=1`, `cargo clippy --workspace --all-targets`
  (the new code, no warnings) and `python3 scripts/install-macos.py`.
- In `docs/plan-crear-fase5.md`, add a line to the 1 Pass note with
  "Correction 1: city full bleed and city race".

## Outside of this assignment

Port or promenade as another end of the city, diagonal avenue, noria, parks and balloons. It's noted as the next step.
