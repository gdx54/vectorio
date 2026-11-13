
## Uso mínimo

```rust
use vectorio_core::geom::{p, BBox};
use vectorio_core::scene::{Camera, PartId, Scene, V3};

let paper = BBox { min: p(0.0, 0.0), max: p(210.0, 297.0) };
let mut camera = Camera::default();
camera.fit([V3::new(0.0, 0.0, 0.0), V3::new(4.0, 4.0, 3.0)], paper, 12.0);
let mut scene = Scene::new(camera, paper);
scene.box3(V3::new(0.0, 0.0, 0.0), V3::new(4.0, 4.0, 3.0));
let output = scene.render();
assert_eq!(output.parts[0].lines.len(), 9);
assert!(output.length_m(PartId(0)) > 0.0);
```

## API pública

Firmas de los métodos de instancia; los constructores y `Camera::fit` reciben los
argumentos indicados. Los tipos no dependen del documento ni de egui.

```text
V3 { x: f64, y: f64, z: f64 }
V3::new(x: f64, y: f64, z: f64) -> V3
dot(other: V3) -> f64; cross(other: V3) -> V3
length() -> f64; normalized() -> V3
Operadores V3 + V3, V3 - V3 y V3 * f64; Default = vector nulo.

Camera { yaw_deg: f64, elev_deg: f64, scale: f64, world_center: V3, paper_center: P }
Camera::default() -> Camera
project(point: V3) -> (P, f64)
facing(normal: V3) -> bool
ground(point: P, z: f64) -> V3
fit(&mut self, world: [V3; 2], paper: BBox, margin: f64)

PartId(pub u16)
Tone::{Lit, Mid, Dark}
ToneThresholds { lit: f64, dark: f64 }  // Default: 0,65 y 0,15

Scene::new(camera: Camera, paper: BBox) -> Scene
camera() -> &Camera; paper() -> BBox
part(&mut self, name: &str) -> PartId
set_part(&mut self, id: PartId); part_names() -> &[String]
set_sun(&mut self, sun: Option<V3>)
set_tone_thresholds(&mut self, thresholds: ToneThresholds)
tone(normal: V3) -> Tone
set_quiet(&mut self, quiet: bool)
paper_mm(meters: f64) -> f64
visible(meters: f64, minimum_mm: f64) -> bool
circle_segments(radius_m: f64) -> usize
line(&mut self, points: &[V3])
closed(&mut self, points: &[V3])
whole_line(&mut self, points: &[V3])
occluder(&mut self, points: &[V3])
solid(&mut self, vertices: &[V3], faces: &[Vec<usize>], smoothing: &[u16], quiet: bool) -> Vec<Vec<V3>>
box3(&mut self, min: V3, max: V3) -> Vec<Vec<V3>>
rotated_box(&mut self, min: V3, max: V3, yaw_deg: f64) -> Vec<Vec<V3>>
extrude(&mut self, polygon: &[V3], vector: V3) -> Vec<Vec<V3>>
lathe(&mut self, center: V3, profile: &[(f64, f64)], smooth: bool) -> Vec<Vec<V3>>
hatch_surface(&mut self, faces: &[Vec<V3>], angle_deg: f64, spacing_mm: f64)
solid_oriented(&mut self, vertices: &[V3], faces: &[Vec<usize>], smoothing: &[u16], quiet: bool) -> Vec<Vec<V3>>
hatch_face(&mut self, points: &[V3], direction: V3, spacing_mm: f64)
begin_shadows(&mut self, ground_z: f64, world_clip: Option<[f64; 4]>, hatch_angle_deg: f64)
hatch_shadows(&mut self, spacing_mm: f64)
render() -> SceneOutput

SceneOutput { parts: Vec<ScenePart> }
ScenePart { id: PartId, name: String, lines: Vec<Polyline> }
SceneOutput::length_m(&self, id: PartId) -> f64
```

## Decisiones y límites

- Cámara fija dentro de cada escena para mantener coherentes proyecciones y tramas.
  Sus ángulos se eligen antes de crearla. Elevación estrictamente entre 0° y 90°;
  yaw libre. Mundo en metros, papel en mm; la profundidad crece hacia el fondo.
- `solid` y los atajos devuelven sus caras orientadas hacia fuera para facilitar
  `tone` y `hatch_face`. La parte activa se captura al añadir cada línea.
  La parte inicial es «contorno»; los nombres se registran una sola vez.
- Los sólidos son convexos, cerrados y no degenerados. La API comprueba índices,
  caras degeneradas y dos caras por arista; la planitud y convexidad son
  precondiciones del generador. El perfil de revolución admite cuellos y cambios de convexidad,
  con alturas crecientes y radios nulos solo en las puntas. No se triangulan caras
  cóncavas ni se hacen operaciones booleanas entre sólidos.
- `set_quiet` afecta a los atajos siguientes; `solid` recibe su propia opción.
  Las superficies curvas usan un mismo grupo de suavizado lateral; las tapas
  tienen grupo cero. Se conservan sus bordes y las generatrices de silueta.
- Se compara profundidad en el plano de cada cara y se recortan intervalos con
  Cyrus–Beck. La rejilla de 4 mm usa sellos por consulta y DDA por segmento. No hay rasterizado,
  dependencia nueva ni aleatoriedad en el renderizado.
- Tolerancia de profundidad: **0,002 m**. Encogimiento de caras: **1e-9 mm**.
  La primera protege caras coplanarias; el segundo excluye aristas de sus propios
  oclusores. Las pruebas verifican las nueve aristas del cubo, caras pegadas,
  tramas propias, el cruce lineal de profundidad y cortes con error < **1e-6 mm**.
  Los huecos entre oclusores pegados quedan por debajo del mínimo de trazo.
- Se unen los tramos visibles de una misma polilínea a través de sus vértices.
  El mínimo de **0,05 mm** se aplica al trazo completo, después de unirlo.
  Las pasadas colineales repetidas se retiran dentro de cada parte, conservando
  su primera aparición (tolerancia de colinealidad: **1e-8 mm**). No se deduplica
  entre partes ni se optimiza su recorrido. Las partes se pueden separar en tintas.
- El sol apunta **hacia** la fuente de luz y debe estar sobre el horizonte. Cada
  grupo captura el sol al abrirse; abrir otro descarta el pendiente sin dibujar. Sin sol no se abre grupo. Las sombras se recortan en el mundo y se fusionan por intervalos
  en la hoja; la trama tiene fase global para alinear grupos de igual ángulo y
  separación. El ángulo se mide en grados de papel, positivo hacia abajo.
- Si un sólido cruza el plano de suelo, solo arroja sombra su porción superior.
  Los objetos por debajo del plano no arrojan sombra. `occluder` por sí solo no
  arroja sombra. Sin sol, `tone` devuelve `Mid`.
- Quedan para las siguientes fases el panel Crear, los generadores y la conexión
  con el documento, tal como pide el plan.

## Comprobación y ejemplo

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo test -p vectorio-core --release scene::tests::benchmark_5000_faces_20000_segments -- --ignored --nocapture
cargo run -p vectorio-core --example muestra_escena --release -- muestra_escena.svg --dimetric
```

El benchmark mide únicamente `render`, incluida la rejilla y la eliminación de
pasadas repetidas. Usa 5.000 caras repartidas por A4 y 20.000 segmentos de 7–11 mm,
con candidatos visibles y ocultos. Ejecuta siete repeticiones e informa mediana,
mínimo y máximo; exige mediana inferior a 100 ms. No pretende acotar el peor caso
de miles de caras superpuestas en una misma celda.

El ejemplo acepta una ruta de salida. `--dimetric` o `VECTORIO_DIMETRIC=1` añade
`*_dimetrica.svg`; ambos SVG conservan mm reales y grupos independientes por parte.

Las entradas degeneradas se descartan sin pánico: caras no planas o cóncavas,
cajas vacías, perfiles imposibles y valores no finitos. Una escena con cámara o
área inválida permanece vacía. `Camera::project` y `ground` devuelven el origen
ante entradas inválidas. La dirección de `hatch_face` se proyecta al plano.
`hatch_surface` une facetas contiguas con profundidad continua y ángulo de papel.
`solid` omite oclusores traseros de volúmenes convexos; `solid_oriented` respeta
el giro exterior y conserva todos los oclusores para volúmenes no convexos.
Las caras con anchura proyectada inferior a 1e-6 mm no ocultan.
Como protección de recursos, hay hasta 8.192 segmentos por círculo y 200.000
líneas por trama; las áreas enormes adaptan el tamaño de celda (hasta 1024 por eje).
La prueba `benchmark_long_diagonals_and_shaded_boxes` complementa la de segmentos
cortos con 500 cajas tramadas, sombras y 2.000 diagonales largas.
