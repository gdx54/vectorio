# View-dependent contours and large models

17 November 2026.

## Drawing form with lines

Volume adds **Suggestive contours** and **Apparent ridges** to the searchable style gallery and **Add pass** menu. Each style combines a feature pass and an independent silhouette pass. Either can be edited, hidden, recolored, reordered, or saved in a custom style. A feature pass uses one ink; palette selection takes the first ink of the chosen palette.

Both effects respond to camera orbit and perspective. **Normal smoothing** reduces triangulation noise without moving vertices. More working facets give finer detail. Higher thresholds suppress weaker lines. Suggestive contours also have **Minimum angle** to avoid unstable lines in regions facing the viewer directly. Existing tonal masks, finishing, selection highlighting, undo, vector legends, and SVG export remain available.

The implementation fits a symmetric normal-derivative tensor to coherent, area-weighted vertex normals using regularized least squares. Adjacency and the intrinsic fit are cached independently of the camera.

- Suggestive contours follow zero radial curvature with a positive radial derivative. The radial direction is the tangential component of the direction toward the eye. Differentiation includes its spatial variation. The threshold is scaled by squared model extent.
- Apparent ridges follow local maxima of the largest singular value of the normal derivative composed with inverse projection from a local viewing plane. The leading direction is lifted to the surface for differentiation. Signs are aligned per triangle and minima are rejected. Strength is scaled by model extent.

Triangle interpolation locates crossings and trims threshold endpoints. Analytical occlusion and paper clipping produce vector segments, joined without bridging branches. Perspective distances and the near plane use the analytical scene's world-unit conversion. **Ridges and valleys** remains the earlier discrete mesh-edge effect.

Open/non-manifold boundary neighborhoods, ambiguous directions, and singular grazing projections are excluded. Rejected projection samples do not enter curvature gradients as zeros. On coarse, irregular, or noisy meshes, lines may be short or angular; smoothing and more facets improve the result. A sphere has no suggestive contours, and a plane has neither feature. The separate silhouette pass remains useful.

Definitions follow [DeCarlo et al., Suggestive Contours for Conveying Shape (2003)](https://gfx.cs.princeton.edu/gfx/proj/sugcon/) and [Judd, Durand and Adelson, Apparent Ridges for Line Drawing (2007)](https://people.csail.mit.edu/tjudd/apparentridges.html). No external implementation code was copied and no raster intermediate is introduced.

## Curvature-following strokes

**Curvature strokes** adds a third form-based style, bringing the gallery to 35 built-in styles. It combines a principal-curvature flow pass with a separate silhouette. Choose **Maximum** or **Minimum** curvature direction and adjust physical **Stroke spacing**, **Stroke length**, and **Normal smoothing**. Duplicate the pass and switch its direction to combine both families, assigning an independent ink to each pass. Recipes and saved styles preserve every control; existing v2 recipes keep their previous behavior.

The eigenvectors of the fitted symmetric tensor define an unoriented line field. Directions are sign-aligned before interpolation. Bidirectional integration uses midpoint directions inside a triangle and explicitly crosses manifold neighbor edges, recording each crossing so segments remain on the surface. It stops at boundaries, sharp transitions, ambiguous/flat regions, abrupt direction changes, closed loops, or the length budget. It does not jump between nearby disconnected surfaces.

Deterministic area-weighted seeds and a spatial occupancy index distribute strokes in model space. Nearby disconnected components have independent spacing. The millimeter controls use the camera's framing scale; perspective foreshortens surface spacing, so it is not a uniform screen-space grid. Cached 3D strokes stay attached to the model during orbit: only projection and analytical visibility are repeated. Zoom, direction, spacing, length, smoothing, or mesh edits invalidate the relevant result. Limits are 60,000 seed candidates and 250,000 retained 3D points; excessive output requests a larger spacing.

The geometric motivation follows [Girshick et al., Line Direction Matters: An Argument for the Use of Principal Directions in 3D Line Drawings](https://web.cs.ucdavis.edu/~ma/SIGGRAPH02/course23/notes/papers/Girshick2000.pdf). This implementation reuses Vectorio's tensor fit and vector visibility engine, without textures or external implementation code.

## Large-model preparation

The engine validates and prepares a source once, sharing the result across passes with different facet counts. Each existing quality level has its own cached working mesh. Orbit and ink edits reuse this preparation; replacing the source invalidates it. The original source and quality targets remain unchanged: Interactive aims for 40,000 triangles, Detailed for 160,000, and Original retains the source before pass-specific reduction. Topology/error constraints may retain more faces than a target.

Reading passes skip the colored triangle drawing that was immediately discarded. Specialized line effects skip unused visible-surface subdivision. The ray hierarchy is built only for ambient occlusion or thickness. It uses median partitioning and precomputed centers; traversal avoids a heap allocation per ray. Cancellation never stores a partial hierarchy or reduction.

Pass-specific facet reductions are now cached before model transformation and shared with multiview child engines using immutable mesh references. Rotating or rescaling the model no longer repeats simplification. The cache holds at most 16 facet configurations, separated by quality and target count; replacing the source clears parent and child caches. No additional quality reduction is introduced.

The benchmark imports the spider STL with **1,499,605 triangles and 749,798 vertices**, at Interactive quality, with an 1,800-facet silhouette and a 3,000-facet curvature map:

```sh
cargo run -p vectorio-core --example volumetric_performance --release --locked --offline -- /path/to/model.stl
```

| Stage | Before | After |
| --- | ---: | ---: |
| First stack | 2,996 ms | 1,073 ms |
| Orbit by 15 degrees | 51 ms | 48 ms |
| Ink-only edit | <1 ms | <1 ms |

The baseline is one local release run; after is the median of three. Individual first-stack results ranged from 1,035 to 1,918 ms, including a run during compilation. These are local measurements, not guarantees. Import is measured separately and excluded from stack timings. Before and after produce exactly **715 strokes / 3,709 points** initially and **794 strokes / 4,056 points** after orbit. New feature-style spider captures separately take about one second at 6,000 facets.

These changes remove repeated preparation and unused work. Full-resolution Original quality, dense hatching, and complex visibility can still be expensive. Out-of-core import and further visibility acceleration remain potential follow-ups.

A second benchmark uses the spider source in **Original** quality and a four-view orthographic sheet. The passes still explicitly request 1,800 and 3,000 facets; Original preserves the source before those pass reductions.

```sh
cargo run -p vectorio-core --example volumetric_performance --release --locked --offline -- /path/to/model.stl original-four
```

| Four-view generation/export | Before shared facet preparation | After |
| --- | ---: | ---: |
| First sheet | 5,157 ms | 1,492 ms |
| Rotate model by 15 degrees | 2,628 ms | 186 ms |
| Ink-only edit | 2 ms | 2 ms |

Before is one release run; after is the median of three (1,485–1,507 ms initially, 184–188 ms after rotation). Import is separate. SVG byte fingerprints match: `af8be23961dfc3c0` initially, `ddfa17727ccb6da3` after rotation, `e3a7b1ab1ec259d4` after recoloring. Initial output has 1,887 strokes / 7,781 points; rotated output has 1,864 strokes / 7,609 points. The helper's orbit stage is a no-op for fixed orthographic views and is not presented as a rotation speedup.

## Verification

Tests cover sphere tensor sign and inverse scale, planes, a smooth ridge, rejection of minima and false boundary maxima, view dependence, scale invariance, threshold trimming, cancellation, analytical occlusion, perspective, clipping, and recipe round trips. Orbit reuses the intrinsic fit. Preparation is shared across facet counts, orbit, inks, and quality changes; replacing a model invalidates it. Median ray intersections match exhaustive triangle tests, including skip lists and distance limits. Lazy ray construction retries after cancellation.

Flow tests check orthogonal directions and umbilic rejection, both analytic cylinder families, segment midpoints on mesh triangles, maximum length, deterministic density, independent close components, cancellation, recipe persistence, perspective clipping, and stroke reuse during orbit/ink edits. Multiview tests assert shared mesh allocations, reuse after rotation, source/quality invalidation, bounded caches, and retry after cancellation.

All eleven historical SVG references, including the real v1 migration fixture, remain byte-identical. Recipes remain version 2; old recipes keep their behavior.

Offscreen egui captures cover both effects at 980 × 620 in dark mode and 1440 × 900 in light mode, using the torus and imported spider:

```sh
VECTORIO_QA_FEATURES=1 VECTORIO_QA_MODEL=/path/to/model.stl \
  cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture
```

Captures are in `target/volumetric/features-*.png`. Installation preserves open application windows; the updated version runs on the next launch.

For curvature strokes, use `VECTORIO_QA_FLOW=1` instead of `VECTORIO_QA_FEATURES=1`. It captures both torus families and the spider, including narrow dark and wide light layouts. Full workspace checks: **439 passed, 10 ignored**; all eleven historical SVG references remain byte-identical.

## Remaining extensions

GLB/glTF import, out-of-core loading, and further acceleration for full-resolution Original visibility remain future work. See also [Palette library and scene catalog](../paletas-y-catalogo-escenas.md).
