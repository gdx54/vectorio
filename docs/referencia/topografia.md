# Topography · Fast Marching in Rust

In **Photo → drawing → Style → Topography**, open a photo and choose where the wave is born with Origin X/Y or «Choose origin in the image». The curves are calibrated to the pen: with «Fit to the pen and the size» the fineness (pen widths along the longest side) comes from the first ink's width and the size on the paper. «Max. ink» is the share of paper the curves cover in pure black (below 100 % they close up without merging); «Spacing» is the largest gap between curves in pure white, in pen widths; «Lights» bends the tone curve. «Smooth photo» and the edge-preserving analysis in «Detail» set how much fine detail reaches the field. «Simplify» reduces points on the polylines.

The «Shard» option adds a fragment, placed with «Place the shard on the image», where the same time field is cut into ×2–×5 more curves and drawn with an ink that is as many times finer: a second layer, «Negro fino», with its own pen. A white gap of one and a half pen widths separates both sets.

The engine shall run on the existing vectoring thread. You don't need a browser, JavaScript, Python, skfmm, d3 or new Load dependencies to work. Each result is an object of strokes in an ink; it can be repainted, scaled, exported to SVG and planned from Output with the normal Vectorio tools. The indicated thickness applies to new inks; an existing ink retains the physical thickness of the document, which also uses the preview.

## Algorithm and origin

Darkness → ink coverage → spacing between curves → wave speed. With one curve per unit of time, the spacing between consecutive curves equals the local speed, so a coverage `c = white + (black − white)·(1 − L)^tone` gives `F = pen / c`. The tone is read from the shared photo analysis (guided filter and local contrast), resampled to a grid of three pixels per pen width (six or more with a shard) and smoothed by 0.7 pen widths, so noise finer than the spacing doesn't make the curves tremble. The solver uses second-order upwind differences where two frozen neighbours allow it, which removes the diamond shape of first-order fronts; the first-order path is kept and still matches the original JavaScript.

[adaptation FastMarchingTopoPlot](https://github.com/rolandblok/FastMarchingTopoPlot) by Roland Blok. Consulted the 14 of November of 2026:

- `worker.js`, blob `71fd1c43dbeb48cbf0bdcf9d7bc30f363f5e946b`: Gaussian
  Separability, speed by brightness and Fast Marching resolution of the equation
  eiconal `|∇T| F = 1` with priority line.
- `script.js`, blob `63c641ecc7d58d29f60db1a07692834ad130d6ab`: composition of the
  Alpha over white, gray Rec. 601, uniform levels in the time interval and
  Cuts in the outlet to opaque areas.
- License **Unlicense**, held at
  `third_party/FastMarchingTopoPlot/UNLICENSE`.

Implementation on `crates/vectorio-core/src/topo.rs`, with `BinaryHeap`, contiguous maps, Gaussian and parallel curve extraction with Rayon. Reuse the marching squares of `raster.rs` to avoid depending on d3. Numerical checks include a variable time map calculated with the original JavaScript and a constant field with known symmetry and speed.

The upwind causal condition is maintained: if the quadratic solution falls before the largest neighbour accepted, a single-axis update is used. This corrects the original `1/F < |Tx−Ty| <= sqrt(2)/F` limit case.

The curves that reach the edge remain open. The edges of the outer rectangle that d3 uses to close the super-level polygons are not added: thus the plotter does not review the same frame once per level.

## Transparency and projects

The alpha is preserved by reducing the image. The transparent color is composed over white to calculate the velocity, and the wave can pass through that background. If activated  « Leave transparent zones », the curves are cut to alpha >= 0.5. The cut is made after simplifying and also examining the interior of the segments, so that a long line does not jump through a transparent hole.

Photos with transparency are saved as PNG within `.vectorio`; opaque still use JPEG. The reader recognizes both by their content and also opens the previous documents. The parameters of Topography and the photo are kept in the group to regenerate it with  « Volver to separate colors... »  from Layers.

The initial values are 75 % ink in black, a spacing of 8 pen widths in white, tone 1, origin in the center and a 0.3 mm pen for new inks. The resolution is that of the importer. With many levels, low-speed values or a small drawing, it is convenient to check the actual tip footprint and the ink map in Output.

## Replicable sample

```sh
cargo run -p vectorio-core --release --example muestra_topografia -- foto.png salida.svg
```

The example writes a SVG of 200 mm on the upper side and a PNG view of its same strokes and thicknesses. Optional arguments are fineness, ink in black and spacing:

```sh
cargo run -p vectorio-core --release --example muestra_topografia -- foto.png salida.svg 500 0.75 8
```
