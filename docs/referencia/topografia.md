# Topography · Fast Marching in Rust

In **Photo → drawing → Style → Topography**, open a photo, adjust the number of curves and choose where the wave is born with Origin X/Y or  « Choose origin in the image ». High gamma concentrates the curves in the medium dark tones. Low minimum speed squeezes the shadows.  « Swapping photo »  reduces the fine detail before solving the field.  « Simplifying »  reduces points on the polylines, without adding fillers.

The engine shall run on the existing vectoring thread. You don't need a browser, JavaScript, Python, skfmm, d3 or new Load dependencies to work. Each result is an object of strokes in an ink; it can be repainted, scaled, exported to SVG and planned from Output with the normal Vectorio tools. The indicated thickness applies to new inks; an existing ink retains the physical thickness of the document, which also uses the preview.

## Algorithm and origin

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

The initial values are 200 levels, gamma 1, additional speed 0.1 and origin in the center. The resolution is that of the importer. With many levels, low-speed values or a small drawing, it is convenient to check the actual tip footprint and the ink map in Output.

## Replicable sample

```sh
cargo run -p vectorio-core --release --example muestra_topografia -- foto.png salida.svg
```

The example writes a SVG of 200 mm on the upper side and a PNG view of its same strokes and thicknesses. Optional arguments are levels, gamma and speed:

```sh
cargo run -p vectorio-core --release --example muestra_topografia -- foto.png salida.svg 200 1.0 0.1
```
