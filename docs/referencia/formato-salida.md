# Output plan · `vectorio-output-v1`

JSON file that generates **Output → Save plan with wait...**. It describes the entire route, with drying pauses, for a future driver to run it without recalculating anything.

- Units: millimeters and seconds.
- `(0, 0)` originates in the upper left corner of the paper; the Y grows downwards (as in SVG).
- The compatible changes (new fields) keep `format`. A change that breaks the reading up the version to `v2`. A `vectorio-core` test fails if the scheme changes without touching the version.

## Fields of first level

|Field|Content|
|---|---|
|`format`|`"vectorio-output-v1"`|
|`units`|`"mm, s"`|
|`generator`|Program and version generated, e.g. `"Vectorio 0.1.0"`|
|`origin`, `y_axis`|`"top-left"`, `"down"`|
|`paper`|`name`, `width_mm`, `height_mm`, `margin_mm`|
|`pens`|Inks of the document: `name`, `color` `[r, g, b]`, `width_mm`, `custom_name`. The `pen` indices point to here|
|`motion`|`draw_speed`, `travel_speed` (mm/s), `lift_time` (s), `optimize`, `join_tolerance`|
|`settings`|Used drying settings: `drying`, `reorder`, `dry_seconds`, `max_wet_passes`, `soak_mm`, `cell_mm`, `chunk_mm`, `pen_change_seconds`|
|`strokes`|Strokes already ordered and oriented: `pen` and `path` (list of points `{x, y}`)|
|`steps`|Actions in time order (see below)|
|`stats`|Total of the plan: `draw_mm`, `travel_mm`, `draw_seconds`, `travel_seconds`, `lift_seconds`, `wait_seconds`, `change_seconds`, `waits`, `pen_changes`, `strokes`, `total_seconds`, `outside_points`|
|`baseline_stats`|The same total of drawing without drying protection, for comparison|

## Actions in `steps`

Each carries `action`, `start_s` and `end_s`. The actions are consecutive: the end of one is the beginning of the next.

|`action`|Extra fields|Meaning|
|---|---|---|
|`draw`|`stroke`|Draw `strokes[stroke]` with the pen down, from its first to its last point|
|`travel`|`from`, `to`|Movement with the pen up (includes up and down time). When each ink is finished it goes back to `(0, 0)`|
|`wait`|`at`|Wait with the pen climbing over the `at` until the zone that's going to step on the next stroke dries.|
|`change_pen`|`pen`|Pause to manually change the `pen` pen. `end_s - start_s` is the expected minimum|

## What the plan doesn't do

- It does not carry machine orders (EBB or other); only the route and times.
- Waits are not included in the SVG of the route, only here.
- Strokes off the paper are preserved and counted in `stats.outside_points`.
