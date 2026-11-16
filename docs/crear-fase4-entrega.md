# Entrega: panel «Crear», fase 4

Encargo en [plan-crear-fase4.md](plan-crear-fase4.md); cada paso lleva allí su nota
«Hecho» con el detalle. Aquí va el resumen, la API nueva, los tiempos y una
valoración sincera de la hoja de contactos.

Aviso: `village.rs` y buena parte del kit siguen sin confirmar en git (la receta de
la fase 3 tampoco llegó a guardarse). La versión de la receta del Pueblo blanco
sigue en 1.

## Lo hecho

- **Paso 0 · Correcciones de la fase 3.**
  - El encuadre de la maqueta itera hasta que la salida cabe con medio margen.
  - El tejado vuela sobre la fachada de la calle y la chimenea se apoya en el faldón.
  - Los huecos oscuros se rellenan de trama.
  - La versión de escena va en un `ParamSpec::Version`.
  - `Scene::lathe_parts` y las pruebas de cada pieza del kit.
- **Paso 1 · Sin costuras.**
  - Un sólido no dibuja aristas entre caras coplanarias suyas.
  - `Scene::polyhedron` y `Scene::prism` construyen piezas de un solo sólido: casas, escaleras, tapias, bancos y pérgolas.
  - `Scene::hatch_faces` trama una cara partida de un solo trazo.
  - El detector `Scene::seams` (solo en pruebas) recorre 20 piezas.
- **Paso 2 · Dirección artística.**
  - Sol relativo a la vista: la fachada de la calle queda en blanco y la otra cara visible en tono medio, con trama a 2,2 veces.
  - Casas de 4 a 9 m en hileras adosadas con saltos de altura; casas con patio y huertos tapiados.
  - Puerta a la calle y ritmo de huecos por casa.
  - Iglesia 1:2,2–1:2,8 con campanario en una esquina de los pies.
  - Chimeneas solo en tejados inclinados.
  - Estratos comunes, irregulares, ondulados y con lentes, solo en las caras exteriores; muros de contención con albardilla.
  - Cipreses en parejas o en hilera en los bordes; olivos agrupados.
- **Paso 3 · Comprobaciones.**
  - La prueba de encuadre pasa a 500 semillas en tres áreas (cuadrada, vertical y apaisada) para Pueblo blanco en maqueta y Manzanas.
  - Se ejecutaron `cargo test --workspace`, clippy, los ejemplos y la instalación.

## API nueva

```text
// generate::kit
Church { width, length, height, tower: Option<(bool /*izquierda*/, TowerTop)> }
Church::ahead(&self, scene: &Scene, style: Style) -> f64     // cuánto avanza la torre
church(scene, style, front: V3, plan: Church, rng)
Cut { exterior: [bool; 4] /*delantera, derecha, trasera, izquierda*/, wall_from: Option<f64> }
Cut::block() -> Cut
Strata::new(scene, style, bottom, top, min: V3, max: V3, rng) -> Strata
terrace(scene, style, min, max, cut: Cut, strata: &Strata)
pick(rng, &[T]) -> T                                          // ahora público

// scene (paso 1)
Scene::polyhedron(&mut self, faces: &[Vec<V3>]) -> Vec<Vec<Vec<V3>>>
Scene::prism(&mut self, profile: &[V3], vector: V3) -> Vec<Vec<Vec<V3>>>
Scene::hatch_faces(&mut self, pieces: &[Vec<V3>], direction: V3, spacing_mm: f64)
Scene::lathe_parts(...) -> Lathe
// solo en pruebas: Scene::seams, Scene::lit_wall_fraction(part, step_mm)

// Pueblo blanco: parámetro «Giro del sol» (sun, −90…90°, relativo a la luz de serie)
```

## Tiempos

Release en este Mac. Las pruebas se ejecutaron de una en una (`--test-threads=1`).

| Medida | Resultado | Límite |
|---|---|---|
| Pueblo blanco de serie, vista previa (mediana de 7) | 85 ms | 100 ms |
| Pueblo blanco de serie, dibujo final (mediana de 7) | 86 ms | 300 ms |
| 500 semillas, peor Pueblo blanco | 287 ms | 1 s |
| 500 semillas, peor Manzanas / Espirógrafo | 12 ms / 1 ms | 1 s |
| Encuadre, 500 semillas × 3 áreas, distancia mínima al borde | 0,54 mm | ≥ medio margen |
| Cobertura de cal, cuatro vistas, a sangre y maqueta | 47–65 % | ≥ 40 % |
| Hoja de contactos, Pueblo blanco por semilla | 19–58 ms | — |

## Comparación

En `target/phase4/`:
- `contactos_fase3.svg`: copia de la hoja de la fase 3, generada antes de empezar esta fase;
- `contactos_fase4.svg`: la hoja nueva;
- `*_pueblo.svg(.png)`: solo la fila del Pueblo blanco;
- `paso2/`: muestras en A4, en maqueta y a sangre.

Las dos hojas usan las mismas 12 semillas. La fase 3 encuadraba de serie en maqueta y la 4 a sangre, así que la comparación de composición no es directa. Lo que sí se compara bien es el tono, la proporción de las casas y la organización.

**Antes:**
- casi todas las caras estaban tramadas, así que el pueblo se leía gris;
- casas de 3 m de fachada que parecían torres;
- iglesias en medio de la masa;
- estratos como un rayado uniforme.

**Ahora:**
- las fachadas de la calle quedan en blanco;
- hileras a lo largo de calles claras;
- plaza con iglesia, campanario y cipreses;
- un caserío que se deshace en huertas.

## Valoración semilla a semilla (hoja nueva, área de 190 × 190 mm, a sangre)

| Semilla | ¿La plotearía? | Por qué |
|---|---|---|
| 0 (serie) | Sí | Buen equilibrio: plaza e iglesia arriba, dos hileras claras, huertos en los bordes. Unas pocas casas de azotea se leen aún algo cúbicas. |
| 1 | Con dudas | La iglesia con aguja y la calle leen bien, pero a la izquierda asoman bloques altos de azotea que parecen torres. |
| 2 | No | Demasiadas casas, apretadas, y la iglesia se pierde entre ellas; con los campos, la hoja resulta muy cargada (28 m). |
| 3 | Sí | Pueblo disperso con muchos huertos tapiados; se lee bien. Los cubos de azotea abundan un poco. |
| 4 | Con dudas | La plaza está bien, pero predominan las azoteas con pretil y el conjunto se lee como bloques. |
| 5 | Sí | Varias calles con escalones, buena variedad de cubiertas y plaza abierta. |
| 6 | Sí, la mejor | Calles largas de casas de una planta con tejado, iglesia con aguja y cipreses: es justo la idea. |
| 7 | Con dudas | Solo se ve parcialmente en la miniatura; hileras correctas, pero sin un foco claro. |
| 8 | Con dudas | El pueblo queda en una diagonal estrecha y la plaza se lee poco; demasiado olivo suelto alrededor. |
| 9 | Sí | Densa pero ordenada, con plaza en el centro y una buena mezcla de tejados. |
| 10 | Con dudas | La iglesia está bien, pero las casas de azotea de la derecha forman una masa de cajas. |
| 11 | Sin juzgar | No la he revisado con detalle. |

En resumen: plotearía 0, 3, 5, 6 y 9; con ajustes, 1, 4, 7, 8 y 10; descartaría la 2.

## Lo que no me convence

- **Las azoteas con pretil siguen pareciendo cajas** cuando son mayoría (semillas 1, 4 y 10). Aunque tienen proporción de casa, el pretil alto y la falta de tejado las vuelven volúmenes abstractos. Bajaría su probabilidad o les pondría siempre algo encima.
- **El campo pesa demasiado a sangre en áreas pequeñas.** En 190 × 190 mm los surcos y olivos ocupan más de la mitad de la hoja y suben los metros (22–30 m frente a 8–21 m de antes). El pueblo debería ocupar más hoja o el campo debería aclararse con la distancia.
- **Hay demasiadas hileras de cipreses** dispersas por los campos todavía en algunas semillas (3 y 5). Reduje su frecuencia, pero siguen siendo frecuentes en el campo abierto.
- **La vista previa apenas es más rápida que el dibujo final** (85 frente a 86 ms). Cumple, pero sin margen, y en paralelo con otras pruebas llegó a pasar de 100 ms. Probablemente lo que pesa son las pasadas del encuadre y no la trama; no lo he medido por partes.
- **Las pruebas de tiempo dependen de la carga.** En paralelo, `benchmark_long_diagonals_and_shaded_boxes` y la del pueblo fallaron una vez. Habría que marcarlas para ejecutarlas solas o medir con más tolerancia.
- **Las hojas de contactos de 12 MB y 21 MB** tardan minutos en rasterizarse con Quick Look; para revisar conviene la fila del Pueblo blanco por separado.
- **No hice una comprobación visual en la app instalada**, porque Vectorio estaba abierta con trabajo del usuario; solo revisé las muestras SVG.

## Ajuste posterior

Tras la valoración se rebajó el peso del campo y de las azoteas:
- azoteas con pretil 0,3 (antes 0,5), dos aguas 0,4 y cuatro aguas 0,15;
- un tercio de los olivares se deja en blanco y el resto va más abierto (paso 4,2 m);
- resultado en release: Pueblo blanco de serie a 38 ms en vista previa y 39 ms en final, peor caso de 500 semillas 129 ms, encuadre a 0,53 mm del borde, y las 12 semillas de la hoja pasan a 19–29 m (antes 22–30 m).
