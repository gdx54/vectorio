# Encargo para Sol · Fase 5, Paso 1: «Ciudad en fiesta» (versión acotada)

Presupuesto corto (≈8 % semanal). El plan va por **hitos que dejan el proyecto
en verde**: si el presupuesto se acaba a medias, lo hecho sirve y está probado.
Para en cuanto completes un hito y no te quede margen para el siguiente.

## 0. Antes de empezar (leer solo esto)

- `AGENTS.md` (normas del proyecto) y la sección 3 de `docs/plan-crear-fase5.md`.
- `crates/vectorio-core/examples/kit_denso.rs`: **es la chuleta**. Enseña cómo
  se llama cada pieza del kit (multitud, cintas, vehículos, fachadas, torre,
  azotea, rótulos) y cómo se encuadra una escena con `fit_scene`.
- `crates/vectorio-core/src/generate/village.rs`, **solo** las líneas 431–475
  (`style`, `scene`) y 610–668 (`impl Generator`). Es el modelo que debes copiar.
- `crates/vectorio-core/src/generate/kit/mod.rs` (las líneas `pub use`): la API
  disponible.

No leas enteros `village.rs`, `scene/*.rs` ni `facade.rs`: usa `grep -n` para
encontrar una firma. No toques el motor (`scene/`) ni el kit salvo que haya un
fallo real, y en ese caso arréglalo y pruébalo allí.

## Normas que no se negocian

- Interfaz en español; cada texto nuevo de la receta (nombre, descripción,
  etiquetas, opciones, pistas, secciones) lleva su traducción en
  `crates/vectorio-app/locales/app.yml`. La prueba
  `every_key_used_in_the_code_is_translated_and_none_is_left_over` lo exige.
- Identificadores en inglés; comentarios y documentación en español.
  Archivos nuevos: una sentencia por línea y funciones cortas.
- **No ejecutes `cargo fmt`.** No añadas dependencias. No hagas commit.
- `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"` si no encuentras `cargo`.
- No cierres ni reinicies Vectorio si está abierto.
- **Regla de origen:** nada de marcas, logotipos ni rótulos reales. Rótulos
  inventados y genéricos en español (PAN, LIBROS, MERCADO, CAFÉ, FERIA…).
- Pruebas con tiempos: `--test-threads=1`.

## Hito 1 · Generador mínimo registrado (lo imprescindible)

Archivo nuevo `crates/vectorio-core/src/generate/festival.rs`, con
`pub struct Festival;` e `impl Generator`:

- `id` `"festival_city"`, `name` «Ciudad en fiesta», `category` «Escenas»,
  descripción de una línea.
- `inks()` devuelve `&inks::DENSE` (ya existe: línea, gente, vegetación, agua,
  trama, rótulos, terreno).
- `params()` reducido, con `ParamSpec::Version{current:1}` al final:
  - `people` «Gente» 0–1 (por defecto 0,6);
  - `landmarks` «Hitos» 1–3 (por defecto 2);
  - `circuit` «Circuito» (sí o no, por defecto sí);
  - `yaw` «Vista» y `sun` «Giro del sol», copiados del pueblo.
- `generate`: igual que el pueblo (`normalized`, `scene`, `finish`, y en
  depuración `construction_issues(1.5)` va a `warning`).
- Registro en `generate/mod.rs`: `mod festival;`, un `static FESTIVAL` y
  añadirlo a `ALL` (el array pasa de 3 a 4 elementos).

Contenido del hito 1 (escena sencilla pero ya densa):

- Rejilla de 3×3 manzanas de unos 24 m con calles de 8 m. Cada calle es una
  `Band` con `ribbon(..., Marking::Dashed)` y un `zebra` en cada cruce. El paso
  de cebra va **fuera** de una estación de la banda (`+0.3` m); si no, el
  detector de costuras protesta.
- Cada manzana: 1–3 `house(..., Roof::Flat, ...)` de 3–6 plantas, con
  `window_grid` y `shopfront` en las paredes que dan a la cámara (patrón del
  panel 6 de `kit_denso`) y `roof_terrace` en alguna azotea.
- Gente: `Crowd::new(style)` **una sola** para toda la hoja (su tope de 3.000
  vale para la lámina). Cruces con `crossing`, colas con `queue` delante de las
  tiendas y `field` en una plaza que sustituye una manzana.
- Tráfico: `convoy` con coches, motos y autobuses en dos carriles de dos o tres
  calles.
- Cámara: `Camera::fit` sobre la caja de la ciudad y `fit_scene`, como en
  `kit_denso`. A sangre: las calles exteriores pasan de la hoja.

Pruebas, en `festival.rs` dentro de `#[cfg(test)] mod tests`:
- determinista con la misma semilla;
- sin NaN y todos los puntos dentro del área;
- `warning` vacío en las semillas 0 a 11;
- `Scene::load().within_budget()` en las semillas 0 a 11.

Comprobación del hito: `cargo test -p vectorio-core --lib -- festival` y
`cargo test -p vectorio-app` (traducciones). Las pruebas genéricas de
`generate/tests.rs` iteran por `all()` y cubren ya al generador nuevo.

## Hito 2 · Hitos y circuito

- `landmarks` decide cuántos de estos se ponen, sorteados sin repetir, cada uno
  en lugar de una manzana:
  - **torre de cristal**, con `glass_tower`;
  - **recinto de escenario**: una caja baja como escenario, dos `stands` llenos
    de gente mirando al escenario y un `field` delante;
  - **depósito circular**, con `scene.lathe` y un rótulo `sign`.
- `circuit`: una cinta elevada (`ribbon(..., 0.8, Marking::Kerbs)`) que rodea dos
  manzanas y sube y baja (patrón del panel 4 de `kit_denso`), con un `convoy` de
  coches. No debe cortar a los edificios: elévala por encima de su altura o
  rodéalos por fuera.
- Pruebas: las mismas del hito 1, para 1, 2 y 3 hitos y con y sin circuito.

## Hito 3 · Ejemplo, instalación y nota

- `examples/hoja_contactos.rs` recorre `all()`: ya incluye la ciudad con sus 12
  semillas. Ejecútalo con
  `cargo run -p vectorio-core --example hoja_contactos --release -- target/phase5/contactos.svg`.
  Mira la hoja con `qlmanage -t -s 1600 -o target/phase5 target/phase5/contactos.svg`
  y abre el PNG.
- Corrige solo lo evidente: algo flotando, solapes o una hoja vacía.
- `cargo test --workspace -- --test-threads=1` y
  `cargo clippy --workspace --all-targets`. El código nuevo no debe dar ningún
  aviso; los avisos antiguos de otros archivos no se tocan.
- `python3 scripts/install-macos.py`.
- En `docs/plan-crear-fase5.md`, bajo el punto 1 de «Pasos», una nota
  «*Hecho (versión acotada).*» con qué entra y qué queda fuera.

## Fuera de este encargo (no lo hagas)

Noria, faro, playa o puerto, parque con tres especies de árboles, globos y
banderas, avenida diagonal, la valoración semilla a semilla en detalle y el
parámetro de distrito dominante. Déjalo listado como pendiente en la nota.

## Trampas conocidas

- Yaw 45: las paredes visibles son +x y +y, y +x va a la izquierda en el papel.
  Usa `wall.faces_camera(scene)`, no lo deduzcas a mano.
- Dos sólidos coplanarios que se solapan dan aviso en `construction_issues`.
  Deja huecos (`kit::street::apart`) o funde las piezas.
- Con mucha gente se llega antes al tope de segmentos (40.000) que al de caras.
  Si `within_budget` falla, baja la densidad de `field`, no subas el tope.
- La prueba de 500 semillas (ignorada) exige menos de 80 m de trazo y menos de
  1 s por semilla.
