# Crear · entrega de la fase 3

Implementación en `feat/crear-motor-escena`, sin commit y sin dependencias nuevas.
No se consultó código de SVGeometric. Se conserva la aplicación abierta al terminar.

## Cambios por paso

- **0 — revisión y motor:** `doc.rs`, `generate/mod.rs`, `generate/blocks.rs`,
  `scene/{mod,hidden,solids,intersections,tests}.rs`, `scene/README.md` y
  `examples/muestra_escena.rs`. Guardado sin recetas huérfanas; pasos de
  aleatoriedad inválidos seguros; encuadre por salida real con reconstrucción;
  altura de sol y cono corregido en Manzanas. Intersecciones analíticas entre
  caras delanteras de sólidos distintos, con índice espacial, profundidad,
  continuidad entre facetas, ocultación normal y exclusión de contactos ya
  dibujados. El bodegón incluye una chimenea y cajas interpenetradas deliberadamente.
- **0 — interfaz:** `create.rs` e `inspector.rs`. Editar trabaja en copia y
  restaura parámetros y candados recordados al actualizar/cancelar. La vista
  previa válida permanece durante Final; los errores se muestran sobre la hoja.
- **1 — estilo:** `generate/style.rs`. Blanco del papel en caras iluminadas,
  densidad para medio/oscuro; hiladas en paredes, máxima pendiente en faldones,
  ángulo de papel en suelo y horizontales continuas en superficies de revolución.
  Partes estándar y sugerencias de tintas. Estratos ondulados deterministas.
- **2 — kit:** `generate/kit/{mod,buildings,landscape,tests}.rs`. Cuatro tejados,
  huecos enteros, chimeneas, pretiles, aleros, campanario con cúpula/chapitel,
  escaleras, muros/bancales, ciprés, olivo, arbusto y terreno cortado.
  `examples/kit_piezas.rs` muestra todas las piezas en A4.
- **3 — Pueblo blanco:** `generate/village.rs`, registro en `generate/mod.rs`
  y pruebas en `generate/tests.rs`. Bandas de bancales macizos, calles de nivel,
  eje de escaleras, casas adosadas, plazas/huertos, iglesia alta y vegetación.
  Cuatro vistas, dos elevaciones, pesos de tejados y controles condicionales.
- **4 — galería:** miniaturas de todos los generadores en `create.rs`, con un
  trabajador propio y caché en memoria por sesión. No comparten cola con el
  cálculo principal. Nuevos textos traducidos en `locales/app.yml`.
- **5 — revisión:** `examples/hoja_contactos.rs`, pruebas ampliadas, este documento
  y actualización de `LEEME.md`.

## API pública nueva

Las medidas del kit son metros; ángulos de trama y separaciones son grados y mm de papel.

```text
Scene::set_intersections(&mut self, enabled: bool)
Scene::construction_issues(&self, minimum_mm: f64) -> Vec<String>
generate::fit_scene(area: BBox, camera: Camera, build: impl Fn(Camera) -> Scene) -> Scene

style::PARTS: [&str; 6]
style::normal(face: &[V3]) -> V3
style::part(scene: &mut Scene, name: &str)
Style { spacing_mm: f64, detail: Detail }
Surface::{Wall, Roof, Ground}
Style::new(ctx: &GenContext, spacing_mm: f64) -> Style
Style::prepare(self, scene: &mut Scene)
Style::quantum(self, scene: &Scene) -> f64
Style::snap(self, scene: &Scene, value: f64) -> f64
Style::shade(self, scene: &mut Scene, faces: &[Vec<V3>], surface: Surface, ink: &str)
Style::lathe(self, scene: &mut Scene, center: V3, profile: &[(f64,f64)], ink: &str) -> Vec<Vec<V3>>
Style::finish(self, scene: &Scene) -> GenOutput

kit::Roof::{Flat, Shed, Gable, Hip}
kit::TowerTop::{Dome, Spire}
kit::Plant::{Cypress, Olive, Shrub}
kit::v(x: f64, y: f64, z: f64) -> V3
kit::house(scene: &mut Scene, style: Style, min: V3, max: V3, roof: Roof, rng: &mut Rng)
kit::opening(scene: &mut Scene, style: Style, origin: V3, horizontal: V3, width: f64, height: f64, dark: bool)
kit::chimney(scene: &mut Scene, style: Style, min: V3, max: V3)
kit::parapet(scene: &mut Scene, style: Style, min: V3, max: V3)
kit::eave(scene: &mut Scene, min: V3, max: V3)
kit::bell_tower(scene: &mut Scene, style: Style, origin: V3, width: f64, height: f64, top: TowerTop)
kit::stairs(scene: &mut Scene, style: Style, min: V3, width: f64, run: f64, rise: f64)
kit::retaining_wall(scene: &mut Scene, style: Style, min: V3, max: V3)
kit::terrace(scene: &mut Scene, style: Style, min: V3, max: V3, rng: &mut Rng)
kit::terrain(scene: &mut Scene, style: Style, min: V3, max: V3, rng: &mut Rng)
kit::vegetation(scene: &mut Scene, style: Style, origin: V3, height: f64, plant: Plant, rng: &mut Rng)

generate::find("white_village") -> Option<&'static dyn Generator>
```

## Decisiones

- Las intersecciones se calculan en (x, y, profundidad), aprovechando los planos
  ya proyectados. Solo se comparan caras que comparten celdas y rangos. Cada sólido
  tiene identidad; las caras sueltas y los sólidos quiet no producen encuentros.
  Un encuentro pertenece a la parte del sólido más reciente. Los segmentos
  contiguos de una pareja se unen con tolerancias de 1e-7 mm y 1e-8 m, y se añaden
  después de los trazos solicitados. Se respeta la separación entre partes.
- Un contacto solo se omite si su segmento está cubierto por una arista que
  realmente se emite. Un borde suave entre facetas no impide una intersección.
  La deduplicación conserva las primeras pasadas dentro de cada parte.
- Los tonos de un torno se deciden por meridianos completos usando la normal de
  la faceta más próxima a media altura. Se evita la frontera en escalera sin
  alterar la geometría. Claro no tiene trama; medio/oscuro tienen distinta densidad.
- La familia conserva seis partes: `contorno`, `detalle`, `trama`, `sombra`,
  `vegetación`, `terreno`. Las escaleras pertenecen al terreno, para compartir los
  contactos del bancal sin pasadas repetidas. Contorno/detalle/trama/terreno sugieren
  la primera tinta, vegetación la segunda y sombra la última.
- Alturas y retranqueos usan una unidad mínima de 0,5 m y 2,5 mm de papel. Las
  plataformas ocupan franjas contiguas; los últimos peldaños llegan al siguiente
  bancal, sin penetrarlo. El campanario se encastra claramente en la nave y se
  separa de sus planos exteriores. `construction_issues(1.5)` comprueba las caras
  delanteras en pruebas/depuración; las 500 semillas pasan sin avisos.
- Los huecos se omiten por debajo de 1 mm real proyectado y se conservan enteros.
  La puerta reserva espacio en la planta baja. Preview omite esos detalles finos.
- La vista frontal de serie mira desde la parte baja de la ladera (yaw 225°).
  La iglesia está en el bancal más alto y tiene un fuste de 10 m para conservar
  su jerarquía. La variación deja solares y huertos; no se llena cada lote.
- El cono nuevo de Manzanas tiene pendiente de unos 60,5°, distinta de los
  complementos de las dos elevaciones. Las recetas de Manzanas guardadas en fase 2
  reciben `scene_version=2` al cargarse y mantienen estilo, encuadre, sol y cono
  anteriores. Las recetas nuevas usan la revisión 3. Hay prueba de regeneración.

## Verificación y tiempos

`cargo test --workspace`: **155 pruebas correctas**, incluyendo el ejemplo de
API; cuatro pruebas de carga se ejecutaron aparte en release. Clippy termina
sin avisos en el código añadido; permanecen avisos previos en otros módulos.

Última medición release, secuencial, sobre A4 útil de 180 × 267 mm:

| Carga | Medición |
|---|---:|
| Pueblo blanco de serie, Preview | 8,162 ms de mediana; 12,767 ms máximo |
| Pueblo blanco de serie, Final | 6,518 ms de mediana; 7,059 ms máximo |
| Pueblo blanco, 500 semillas aleatorias | 14,474 ms máximo |
| Manzanas, 500 semillas | 7,799 ms máximo |
| Espirógrafo, 500 semillas | 1,127 ms máximo |
| Motor: 5.000 caras / 20.000 segmentos | 12,042 ms de mediana |
| Motor: 500 cajas tramadas, sombras y 2.000 diagonales | 12,514 ms de mediana |
| Misma carga sin intersecciones | 12,242 ms; sobrecoste activadas: 2,2 % |

Las 1.500 salidas aleatorias son finitas, permanecen en el área y no alcanzan
80 m. Las 500 de Pueblo blanco tampoco activan el diagnóstico de rebanadas o
solapes coplanares. La salida de serie ocupa más del 90 % de una dimensión útil
y queda por debajo de 40 m. Los tiempos dependen del equipo, no son garantías
para una escena arbitraria. Preview y Final se midieron consecutivamente; las
cachés y las tramas distintas explican que Preview no sea siempre el más rápido.

Se inspeccionaron el kit, las dos cámaras del bodegón y los 36 contactos. En
`/Applications/Vectorio.app` se comprobó la galería con miniaturas, Pueblo blanco,
Aleatorio varias veces, Colocar, cambio de `vegetación` a verde, Editar, Aleatorio,
Actualizar y Salida. La parte conservó el verde (#009460). Al volver a Crear,
los ajustes recordados recuperaron la semilla anterior a editar; la receta
actualizada conservó la suya. La aplicación se deja abierta con el grupo de prueba.

## Hoja de contactos y valoración

Artefactos reproducibles en `target/phase3/`:

- `hoja_contactos.svg`: 4 × 3 semillas por generador, 36 dibujos en total. El 0
  usa los parámetros de serie; 1–11 usan parámetros aleatorizados con su semilla.
- `contactos_pueblo.png`, `contactos_manzanas.png`, `contactos_curvas.png`: vistas
  de revisión por generador. La miniatura de curvas reduce el muestreo para
  rasterizarla con rapidez; el SVG completo conserva todos los puntos.
- `kit_piezas.svg`, `muestra_escena.svg`, `muestra_escena_dimetrica.svg` y sus PNG.

En Pueblo blanco considero aprovechables sin retoque 10 de los 12 contactos.
El **0** presenta bien calles, vacío central e iglesia; **2 y 4** equilibran el
foco con casas y vegetación; **8** tiene un ritmo de cubiertas claro. **1 y 11**
son más flojos: la vista trasera vuelve dominante el corte del terreno y oculta
calles y fachadas. **6** es muy de iglesia y tiene menos tejido urbano, aunque
se lee limpio. Es una valoración de esta hoja, no una evaluación estética de
las 500 semillas. Las pruebas automáticas solo acreditan geometría y tiempos.

Manzanas sigue siendo una demostración geométrica: las versiones escasas, como
la 6, funcionan como estudio de volúmenes pero no como pueblo. El espirógrafo
aporta contraste y continuidad de trazo; algunas combinaciones de muchos ciclos
quedan densas incluso por debajo de 80 m.

Las copas y los huecos son deliberadamente simples; aún no tienen el carácter
orgánico o constructivo de una ilustración dibujada a mano. El estilo ya tiene
una familia coherente, pero esas dos piezas admitirían una siguiente pasada
artística. Quedan fuera castillo, estadio, sombras sobre objetos, figuras y texto.

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
cargo test -p vectorio-core --release -- --ignored --nocapture --test-threads=1
cargo run -p vectorio-core --example kit_piezas --release -- target/phase3/kit_piezas.svg
cargo run -p vectorio-core --example hoja_contactos --release -- target/phase3/hoja_contactos.svg
cargo run -p vectorio-core --example muestra_escena --release -- target/phase3/muestra_escena.svg --dimetric
python3 scripts/install-macos.py
```
