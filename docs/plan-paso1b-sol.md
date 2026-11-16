# Encargo para Sol · «Ciudad en fiesta», corrección 1

Archivo: `crates/vectorio-core/src/generate/festival.rs` (lo escribiste tú).
Las normas de `docs/plan-paso1-sol.md` siguen valiendo: español, sin `cargo fmt`,
sin dependencias, sin commit, traducciones en `app.yml`, `--test-threads=1`.
Va por hitos que dejan las pruebas en verde; para cuando no te quede margen.

## Qué está mal (revisión de las 12 semillas)

1. **La ciudad es un recuadro.** Hay 3×3 manzanas en el centro y el resto de
   la hoja son calles vacías que se pierden hacia los bordes. Debe ser **ciudad
   hasta el borde, a sangre**: el papel corta edificios, no asfalto.
2. **El anillo elevado no tiene sentido.** Es un bucle de juguete con coches
   dando vueltas y no se conecta con ninguna calle. Se sustituye por una
   **carrera urbana** que va *por las calles de la ciudad*.
3. **Está demasiado lejos.** A unos 1,5 mm por metro no salen ventanas,
   escaparates, toldos ni balcones (sus umbrales de visibilidad necesitan unos
   2 mm/m), y la gente sale diminuta. Por eso los edificios parecen cajas vacías.

## Hito 1 · Más cerca y ciudad hasta el borde

- **Escala fija por papel**, no por encuadre de 3×3:
  `scale = (min(ancho, alto) / 70.0).clamp(2.4, 4.0)` mm/m, con la cámara
  centrada en el centro de la plaza (`world_center`). En A4 sale unos 3 mm/m.
  Quita el `camera.fit` sobre la caja de 3×3. `fit_scene` puede quedarse si
  sigue haciendo falta para el recorte.
- **Rejilla infinita.** Las celdas van de `PITCH` en `PITCH` y la plaza es la
  celda (0, 0). Calcula con `extent()` (esquinas del papel proyectadas al suelo,
  como ya hace) el rango de celdas que tocan la hoja, más una de margen. Rellena
  **todas** esas celdas con manzanas. Las calles se generan para todo ese rango y
  siempre salen de la hoja por los dos extremos.
- **Núcleo y periferia.** Las 8 celdas alrededor de la plaza son el núcleo:
  allí van los hitos (`landmarks`), las colas y las azoteas con gente. En la
  periferia las manzanas siguen llevando ventanas, escaparates y toldos, pero
  menos gente (sin colas y sin azoteas). La periferia no puede quedar más vacía
  que el núcleo.
- **Presupuesto.** Mira `scene.load()` antes de cada manzana de periferia. Por
  encima del 80 % de `SEGMENT_BUDGET` o de `FACE_BUDGET`, esa manzana va sin
  `window_grid` (solo volumen y escaparate). No subas los topes.
- **Pruebas** en `festival.rs`:
  - la escala está entre 2,4 y 4,0 en A4 y en el área de la hoja de contactos;
  - ningún punto del suelo visible queda fuera de una celda con contenido: toma
    una rejilla de 20×20 puntos del papel, pásalos al suelo con
    `camera.ground(p, 0.0)` y comprueba que cada uno cae en una celda poblada o
    en una calle;
  - las de antes siguen pasando: deterministas, sin NaN, sin `warning` en las
    semillas 0 a 11 y `within_budget`.

## Hito 2 · Carrera urbana en lugar del anillo

Borra `circuit()` y `supports()`. El parámetro `circuit` pasa a llamarse
«Carrera urbana»; el `id` se queda y la versión de la receta sigue en 1.

- **Trazado.** El rectángulo de las cuatro calles que rodean el núcleo de 3×3.
  Es un bucle cerrado **sobre las calles existentes**, con las mismas `Band`.
  En esas calles:
  - marcas `Marking::Kerbs` en lugar de `Dashed`;
  - sin pasos de cebra ni tráfico normal.
- **Vallas** a los dos lados de la calzada de carrera: una línea a ±3,6 m del
  eje con postes cada 2 m (líneas cortas verticales de 1 m). En los cruces con
  calles normales la valla corta la calle: allí la calle normal termina en la
  valla, con una cola de gente mirando.
- **Coches de carrera** en un solo sentido: un `convoy` de `Vehicle::Car` por
  cada lado del rectángulo, con el rumbo de la vuelta. Que no ocupen las
  esquinas: deja 6 m libres en cada cruce.
- **Público** a lo largo de las vallas por fuera (`crowd.queue` paralelo a la
  valla, a 0,8 m) y **una grada** en un lado. Reutiliza `stand_base` y
  `crowd.stands`, mirando a la pista.
- **Meta**: una línea transversal a cuadros (dos filas de cuadrados con
  `closed`) y un pórtico de dos postes con un travesaño y `kit::sign(..., "META", ...)`.
- **Pruebas**:
  - el trazado es cerrado y cada tramo cae sobre el eje de una calle de la
    rejilla (x o y en la lista de calles);
  - no hay vehículos a menos de 6 m de un cruce de la carrera;
  - con la carrera activa siguen pasando las pruebas de costuras, `warning` y
    presupuesto.

## Hito 3 · Hoja de contactos, nota e instalación

- `cargo run -p vectorio-core --example hoja_contactos --release -- target/phase5/contactos.svg`.
- Para verla, recorta cada lámina de la ciudad en su propio SVG (todo el
  `<g transform="translate(...)">…</g>` completo, ojo con el cierre) y
  haz la miniatura con `qlmanage -t -s 1200`. `qlmanage` con la hoja entera se
  cuelga: no lo intentes.
- Comprueba a ojo:
  - ninguna lámina tiene asfalto vacío en los bordes;
  - se ven ventanas y escaparates;
  - la carrera se lee como calles cortadas con vallas.
- `cargo test --workspace -- --test-threads=1`, `cargo clippy --workspace --all-targets`
  (el código nuevo, sin avisos) y `python3 scripts/install-macos.py`.
- En `docs/plan-crear-fase5.md`, añade una línea a la nota del Paso 1 con
  «corrección 1: ciudad a sangre y carrera urbana».

## Fuera de este encargo

Puerto o paseo marítimo como otro final de la ciudad, avenida diagonal, noria,
parques y globos. Queda anotado como siguiente paso.
