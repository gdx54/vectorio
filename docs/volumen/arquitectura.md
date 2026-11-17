# VOLUMETRIC · audit and architecture

## Based observed (15 November 2026)

Workspace Rust, pure core `vectorio-core` and application eframe/ egui 0.29 on wgpu/Metal. `Document` stores editable objects separate from `Pen`; their polylines and physical coordinates are measured in mm, Y below. `Document::layers` gathers by ink, binds and orders tours. `svg_out` exports physical vector SVG with Inkscape layers; `output` prepares routes and waits. You don't need a new controller.

`scene` already implements solid procedural, orthographic camera (world in meters, Z above), hatching patterns, shadows, spatial grid and analytical occlusion of segments against projected convex faces. Prior to this delivery there was no importer of meshes or general sections. `topo` interprets photographs using Fast Marching; it is not topography of 3D surfaces. `generate`'s architectural generators work, with their own recipes and versioned workers. They don't change their algorithms.

Before editing: Initial branch `feat/crear-motor-escena`, numerous local changes and new files; preserved. `codex/volumetric` is created on that same state, with no stash, reset or commits. The full initial test suite passes (application, core and doctest; manual tests/benchmarks ignored). There is no language migration or change in LINEAR.

## Decision

Modular library within `vectorio-core::volumetric`, usable without interface. Independent adapter in `vectorio-app::volumetric_ui`, accessible as fourth space. `scene::V3`, `Camera`, `Scene`, `Polyline`, `Item`, `Document`, inks and the existing output are reused. The interface input and module can be removed without changing the drawings of other engines. The simplification of large meshes uses `meshopt` 0.6.2 (native meshoptimizer, without external service). `serde_json/float_roundtrip` is activated to preserve exactly f64 coordinates when opening projects and recipes.

Contracts: validated triangulated model → standard transformation → scalar field/planes → 3D polylines with → level camera orthographic → occlusion and mm cutting → vector parts by function → objects of the document → layers by existing ink and output. Geometry does not contain colors. The recipes retain the original model and parameters, without depending on the OBJ route.

## References investigated

Real implementations of [hidden-line.js](https://github.com/neurofuzzy/three-plotter-renderer/blob/develop/src/hidden-line.js) and [optimize.js](https://github.com/neurofuzzy/three-plotter-renderer/blob/develop/src/optimize.js) from three-plotter-renderer) are inspected. The first pairs positions, classifies profiles/angles, divides crosses and contains ray casting and GPU depth alternatives. Its screen tolerances and partial dot classification are not transferred to the physical SVG. The second eliminates duplicates, joins colineals and delegates ordering; some passes compare pairs. Vectorio already has its own grid and optimization. The latter are reused and Marching Triangles is implemented in Rust, without copying JavaScript code or adding Three.js.

[License MIT](https://github.com/neurofuzzy/three-plotter-renderer/blob/develop/LICENSE), copyright 2020 Geoff Gaudreault: if in the future code or a substantial portion is copied, copyright and permission must be retained. This delivery is a technical consultation, does not incorporate reference code. The investigation of `perspective-hatch.js` could not be completed due to a lack of access; it is not presented as verified. Freestyle/Grease Pencil and vpype are benchmarks for future phases, not dependencies or implemented capabilities.

## Modules and contracts implemented

- `mesh.rs`: original indexed mesh, object names and facial belonging; validation and diagnosis of boundaries/non-manifold. Centered transformation at extension greater than two units; XYZ scale and XYZ rotations. Cube/prisma, sphere, cylinder, cone, plane, toroid and composition of three volumes.
- `import.rs`: OBJ UTF- 8, positive/negative indexes and `v/vt/vn` faces. Norms are extrapolated from geometry; materials and textures are ignored. Ear clipping of simple polygons planes, up to 256 vertices per face. No instructions are executed and no external OBJ resources are opened.
- `stl.rs`: Little-endian binary STL and strict ASCII; size detection and binary counter, even with  « solid »  header. Normals and attribute colors are ignored, without being converted into inks. Several ASCII solids retain their membership. Incomplete formats, invalid counters and excess triangles are rejected before running or reserving geometry.
- `contours.rs`: scalar values by vertex and Marching Triangles. Directional height, camera depth orthographic and distance to the centre (linear interpolation). Curves with identifier/level value. Coplanar regions retain their boundaries, not their diagonals. End welding with real distance `1e-7` at normalized coordinates, consulting neighbouring cells. Grade two graphs form cycles; forks end strokes, they never close artificially.
- `pipeline.rs`: transformed and curved model cache; camera and composition; adaptation to `Scene` and the document. ATLAS separates primary/secondary, STRATA displaces complete slices and X-RAY filters diagonal through adjacency and normal. 3D curves are preserved before projecting; final simplification in mm. The inks are assigned later.
- `volumetric_ui.rs`, `volumetric_controls.rs`, `volumetric_preview.rs`: status/worker, context controls and two views respectively. They don't store section algorithms or import. Own navigation with ⌘4; in narrow windows the space selector is deployable. It does not change ⌘1–3.

The camera keeps azimuth/elevation and uses the existing ortonormal base, without 2D distortions. A spherical frame leaves space for drawing turns; scale, position and composition turn are applied before the output cut. Top view/elevation/profile are approximated to 0.01 ° from the ends: the existing `Camera` contract excludes 0 ° and 90 ° and is retained so as not to alter the generators. There's no prospect at this stage.

The depth field is calculated geometrically in camera coordinates, without rasterization. In orthographic projection their isolines are equivalent to planes perpendicular to the camera; they are kept as a field independent of the planes defined in the object. The distance to the center is a linear approximation by vertices, not the exact intersection with spheres: it can be constant on meshes of equidistant vertices. It is not claimed that there is curvature or scalar illumination.

## Visibility and paper

All triangles used as occlusors are recorded, including those with normal inversions. The existing engine cuts intervals of each segment by the projected contour of each face and its plane of depth; it does not decide visibility only by normal or by an average point. The spatial grid reduces candidates and works between different components. The steady direction of the vertices in slices/clusters is maintained so as not to depend on the result of the winding. No raster is included in the SVG.

The standard mesh expands internally × 1000 and the camera scale is divided by that factor when reusing `Scene`. Thus its architectural tolerance of 0,002 m here equals to `0,000002` standard units, without modifying the tolerance of other engines. There are tangent cases, faces almost aligned with the camera and extremely thin surfaces that still depend on numerical tolerances. STRATA deliberately displays complete sections and disables the occlusion of the original solid, which no longer coincides with the displaced strata.

Cut to the rectangle of the paper minus margins before placing/exporting; coordinates are not saturated against the edge (that would create false lines). It is simplified by RDP, by default 0,025 mm. The SVG retains the width of the pen assigned. The 2D view uses 0.5 pt ≈ 0,1764 mm; it does not promise that that is the physical width of the tool. `Document::layers` retains its route link/order by ink.

## Resources, cancellation and caches

OBJ and STL: 512 MB, 15 million input vertices, 5 million triangles; JSON recipe: 512 MB. Up to 160 slices and two million section segments; interactive shares offer 4  –  128 slices. A model exceeding the limit is rejected with a resource limit notice; no random faces are discarded or the graphic thread is blocked. The reading uses `take(límite + 1)` before parsing.

A persistent worker retains the intermediate results; imports are read on another thread and are never repeated when changing ink. Atomic versions cancel out outdated calculations, with consultation on triangles, levels, face preparation, strokes and elimination of duplicates. The construction of the grid and an individual call for simplification are not interruptible but are limited by budgets. Only one result of the current version enables placement or export; the previous one is kept attenuated while it is calculated.

Changing camera preserves slices of the object; invalidates slices of the depth field. Changing composition preserves the mesh and sections. Changing ink/revealed/dual view does not require geometric calculation. Changing the model or its invalid transformation of derived mesh and curves. The solid 3D is a deep-painted preview, not a z-buffer mesh editing viewer; the SVG uses analytical occlusion.

## Persistence and compatibility

The `.json` versioned recipes include mesh, parameters, paper and geometrically separate inks. When the inks are opened, they are reused or added without repainting previous objects. Placer adds editable trajectory objects to `Document`, compatible with save, undo, SVG, and output. The `.vectorio` project retains those layouts; the 3D source is saved using **Save recipe...**, not as a new variant of `Geometry`. This decision avoids migrating the document and touching its multiple consumers during the MVP. The 3D edition linked from a layer is pending.


## Global drag and large meshes

An OBJ or STL released in Design, Create, Output or VOLUMETRIC opens the module in **Only 3D** and loads the model in second plane. Documents and recipes from other spaces are kept. A new drag replaces the outstanding import, invalidating its response and canceling block read/parse/validation. With several models in one gesture the last one is opened; other types of gesture file do not cover the model. Modal dialogues retain their existing blockage.

`mesh.rs` compresses the matching vertices when validating, which prevents keeping three copies per triangle of the STLs. Welding and diagnostic maps use hashing; it does not rely on its iteration order to construct geometry. `reduce.rs` applies simplification by collapse of edges with meshoptimizer, aiming and preserving the selected vertices of the original. Triangles are not removed by random sampling. The original clean mesh remains in memory and in the recipe.

Three reproducible qualities: **Interactive**, objective 40,000 triangles/relative error 0,001; **Detailed**, objective 160,000 /error 0,0001; **Original**, all the triangles. The target can be exceeded when topology or error prevents further reduction. The quality chosen applies to the sections, occlusion and SVG: it is expressly reported in Model and also together with Export (outside the accordion), with quality, original/work number and scope on SVG. A simplification is not presented as original geometry. The solid-state viewfinder has its own reference mesh of some expensive 20,000, independent of the calculation detail. The calculation and the recipe retain the chosen quality.

The original validation and reduction shall be cached by model/quality. Changing camera, color, scale of paper or number of slices doesn't reimport or simplify the model. The individual native call to meshoptimizer is uninterruptible; cancellation is checked before and after and an outdated response is never published.

References: [API and meshopt-rs code](https://github.com/gwihlidal/meshopt-rs), [simplification of meshoptimizer](https://github.com/zeux/meshoptimizer). meshopt-rs offers MIT/Apache- 2.0; is used MIT. The packaged native code is licensed under MIT. The corresponding notices shall be included in `packaging/macos/VOLUMETRIC_NOTICES.txt` and copied to the installed bundle.


## FLAT SHADOW: lighting and ink regions

`volumetric/shadow` adds an alternative route to the sections. `ShadowSettings`, included by default in old recipes, retains directional light (direction and height from 5 ° to 85 °), threshold, objective facets (64  –  8,000), floor, ground separation, second tone, contour and hatching in mm. The artistic mesh is obtained by collapse of edges of the working mesh with maximum relative error 0.04; the target can be exceeded by topological constraints. The real number is displayed next to Export and the original is kept. The 3D view uses this same mesh, to match facets and shadows.

`shadow/project.rs` projects triangles and builds a 2D BVH. The depth on each face is symmetrical; cutting against the three semiplanes of the oclusor and its plane of depth produces the exact region it covers. The convex subtraction calculates camera visibility. The same procedure from the light calculates regions of shadow cast on illuminated faces, also between separate components. The shadow is not decided solely by normal nor by a central point, nor is a deep raster map used.

The normals are oriented consistently by adjacency; in closed components the exterior is chosen by signed volume. Open components retain the initial side of their faces and do not automatically become solids. Non-manifold or self-intersecting meshes remain unrepaired. The artistic threshold adds shade to the geometric classification by orientation. The second optional tone corresponds to a cosine band of 0.28 above the threshold.

`shadow/mod.rs` calculates the shadows of the ground on a horizontal plane below the entire model. In the final view, the parts of the ground covered by the complete silhouette of the object are removed. The frame includes object and shadow. `shadow/mask.rs` joins the intervals of all shadows, subtracts the hidden floor and separates the intermediate tone. The hatching has a global phase in mm: two adjacent facets or its own shadow and that of the ground form the same segment when touched. They don't repeat their meetings. It cuts to the physical margin; the shadow part uses a single ink by default. The optional contour reuses the visibility of `Scene` segments and omits the internal edges.

The paper view draws the masses with convex polygons, without triangulation edges.  « See plotter strokes »  shows the effective trajectories; export or place always uses those trajectories, never a raster fill. The default separation of 0.22 mm can be adjusted to the width of the pen. Shadow, tone and contour assignments are functions independent of the document inks.

The light field is stored by mesh identity, light, threshold and second tone. Changing camera, paper position or hatching retains that field; changing ink or alternating masses/strokes does not throw geometric work. The versioned/cancellable worker is retained. Defensive budgets: 512 fragments per face in cuts, 250,000 regions or strokes; excess returns explicit error, without silently suppressing geometry. 3D view still uses depth order as a reference; paper and SVG use analytical cutting.


## WAVES OF SUPERFICIE, CHROMATIC MESH and WAVES + MESH

The `volumetric/art` module shares import, simplification, camera orthographic, cutting, `Scene`, documents and inks. New `RecipeKind` presets use `ArtSettings` with default values for old recipes: 64  –  20,000 target triangles (initial 2,700), 4  –  160 levels, pattern, angular origin, deformation, frequency, chromatic criterion, one/three mesh families and optional contour. The artistic reduction uses the work mesh with relative error 0.04; the target may be exceeded by topology/error. The actual number and its scope shall be reported on SVG together with the shares. The full source is unchanged.

`art/field.rs` forms the graph of unique edges, with real lengths. Each connected component receives a seed at the extreme vertex according to azimuth/elevation. Dijkstra calculates the distance along the triangulated surface; it is an approximation by edges, not an exact geodesic or curvature field. Interference subtracts the distances from the opposite ends. The deformation adds spatial harmonics with phases derived from the seed by stable hash; it alters the scalar field and never displaces points outside the surface. `isolines` assembles the curves over triangles and retains separate components and holes. A level can produce several curves.

`art/edges.rs` emits each pair of shared vertices only once, including triangulation diagonals. The orientation uses the absolute value of adjacent normals, without depending on the winding. Height/depth classify the midpoint of each edge into bands; depth is recalculated as it orbits. Objects uses imported belonging and repeats the palette between objects if there are more than inks. A shared edge is allocated to a single family, retaining the physical budget of a pass.

`art/mod.rs` keeps the graph, field, curves and edges cached for mesh identity and relevant parameters. Changing camera, composition or color doesn't recalculate distances or curves; camera depth does reclassify colors. Changing density recalculates levels, changing seed/deformation/origin recalculates the field and changing mesh target invalidates its derivatives. Cancellation is consulted in graphs, search, levels, face preparation, edges and rendering; versioned discarding of the worker is maintained.

In combination, part 0 is waves and parts 1 / 2 are the mesh. In mesh, the three parts are chromatic families; in waves they are primary, secondary, and silhouette. These papers are translated into document inks after the calculation. The interface allows you to choose active families and hides unused allocations. The recipes retain `ArtSettings`, seed, inks and source, with default value compatibility. The output remains editable traces in Design and Output trajectories, not new linked 3D objects.

The 3D view adds curves/edges to the full reference; the 2D view and SVG remove hidden parts with the mesh itself drawn. Using the detailed source as an enclosure would produce false simplified string concealment: it's not done. All three modes retain physical margins, rotation, zoom, and position, without generating embedded raster images. External dependencies are not added.

## Updated · Phase G (16 November 2026)

The A-D phases replace the exclusive modes with a stack of passes and preserve the scene/paper/pass interface. Phase G adds `volumetric/reading/` for measurements, ramps and reading effects, and `volumetric_reading.rs` for its editor. `DrawingPart` holds one part per pass, with ink families that the document gathers per pen. The recipe continues in 2 version with new optional fields and retains migration since v1. The current details, checks and limitations are described in [fases/fase-g.md](fases/fase-g.md).
