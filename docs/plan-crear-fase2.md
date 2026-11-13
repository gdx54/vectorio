# Encargo: panel «Crear», fase 2 (correcciones del motor, generadores y panel)

Eres un agente de programación trabajando en **Vectorio**, una aplicación de escritorio en Rust para dibujar con plotter (núcleo en `crates/vectorio-core`, interfaz egui en `crates/vectorio-app`). En la fase 1 se hizo el motor de escenas `crates/vectorio-core/src/scene/`: cámara ortográfica, sólidos, eliminación exacta de líneas ocultas, tramas, tonos y sombras. Se ha revisado: la ocultación es correcta, está bien probada y es rápida (unos 15 ms con 5.000 caras). Esta fase tiene dos partes:

1. **Paso 0:** corregir lo que salió en la revisión del motor.
2. **Pasos 1–5:** montar la infraestructura de generadores, su sitio en el documento y el modo **Crear** de la app, con dos generadores sencillos que demuestren que todo funciona.

Después revisaré tu trabajo con otro agente, así que deja el código claro, probado y bien explicado.

## Reglas

- Lee `AGENTS.md` y `crates/vectorio-core/src/scene/README.md` antes de empezar.
- La interfaz va en **español**, con su traducción al inglés en `crates/vectorio-app/locales/app.yml` (usa `tr!`/`ts!` como el resto de la app). Se mantiene la separación entre **objetos/capas** y **tintas**.
- **Trabajo original.** La idea se inspira en SVGeometric (`jarvisar/geometric`), que **no tiene licencia**. No abras, copies ni traduzcas su código ni calques sus diseños.
- Identificadores en inglés, como el resto del código. Comentarios y documentación en español, en el tono del código existente.
- **No ejecutes `cargo fmt`**: el proyecto usa líneas densas a propósito (hasta unos 150 caracteres).
- Sin dependencias nuevas salvo que sean imprescindibles; si añades una, justifícala en el resumen.
- Si `cargo` no está disponible: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Sigue en la rama `feat/crear-motor-escena`. No hagas commit salvo que el usuario lo pida.
- **No cierres ni reinicies la Vectorio que esté abierta.** La versión abierta sigue hasta el siguiente inicio.
- Archivos antiguos: un documento guardado antes de esta fase tiene que abrirse igual que antes (campos nuevos con `#[serde(default)]` y sin escribirse cuando están vacíos).

---

## Paso 0 · Correcciones del motor (`crates/vectorio-core/src/scene/`)

Hazlas por este orden. Cada una con su prueba.

### 0.1 Tramas continuas sobre superficies curvas (lo más importante)
Hoy, para tramar un cilindro, un cono o una cúpula hay que llamar a `hatch_face` faceta por faceta. El resultado son cientos de rayitas sueltas, cada una con su propia dirección, en lugar de líneas continuas (en el bodegón: 938 trazos en la trama oscura). Hay que añadir:

- `hatch_surface(faces: &[Vec<V3>], angle_deg: f64, spacing_mm: f64)`: trama **varias caras como una sola región**, con un ángulo fijo en la hoja (grados de papel, el mismo criterio que `begin_shadows`) y fase global, como las sombras.
  - Por cada línea de trama, recorta contra cada cara que mire a la cámara.
  - Las caras de una superficie convexa vista de frente no se solapan en la hoja, así que basta con ordenar los intervalos y **unir los consecutivos en una sola polilínea**.
  - Cada vértice lleva la profundidad del plano de la cara en la que está; en las aristas compartidas es continua.
  - Las caras que no miran a la cámara se ignoran.
- Prueba: un cilindro suave tramado a 0° da exactamente **un trazo por línea de trama**, de silueta a silueta, sin cortes en las aristas entre facetas.
- Actualiza `examples/muestra_escena.rs`:
  - en los sólidos de revolución, agrupa las facetas por tono y trama cada grupo con `hatch_surface`;
  - en las cajas, mantén `hatch_face`;
  - ajusta el encuadre para que el bodegón ocupe la hoja (ahora queda arriba, con media página vacía).

### 0.2 Ninguna entrada generada puede tumbar la app
Los generadores producirán geometría con parámetros aleatorios. Un `assert!` que salte con datos degenerados cerraría Vectorio. La regla es: **los métodos públicos de `Scene` y `Camera` no entran en pánico en release por culpa de los datos**. Los datos degenerados se descartan sin dibujar nada:

- cajas de tamaño cero o negativo;
- caras degeneradas o no planas;
- perfiles de torno imposibles;
- extrusiones tangentes;
- valores NaN o infinitos.

Además:

- Una dirección de trama no tangente se proyecta sobre el plano de la cara.
- Pedir sombras sin sol simplemente no produce sombras.
- Abrir un grupo de sombras con otro abierto trama el anterior o lo descarta (documenta cuál).
- Deja `debug_assert!` donde ayude a detectar errores de programación en las pruebas.
- Prueba: una batería de entradas degeneradas que no entra en pánico y no produce NaN en la salida.

### 0.3 Rendimiento con líneas largas
Las consultas a la rejilla (en `Grid::hidden` y en `remove_overlaps`) recorren **todas las celdas de la caja del segmento**. Para una línea diagonal larga, como una trama de sombra que cruza la hoja, eso son miles de celdas y muchas caras candidatas que no tocan el segmento. La medición actual solo usa segmentos cortos (de 7 a 11 mm), así que no lo detecta.

- Recorre solo las celdas que atraviesa el segmento (DDA o recorrido de celdas cubiertas).
- Para las pasadas repetidas, puedes indexar en su lugar por dirección y desplazamiento cuantizados. Elige y justifica.
- Añade una segunda medición realista: 500 cajas en A4 con trama por tono y sombras, más 2.000 líneas diagonales largas. Objetivo: `render` por debajo de 100 ms en release. Indica las dos mediciones en el resumen.

### 0.4 Caras casi de canto
`Face::new` descarta las caras con área proyectada menor de `1e-10` mm². Es un umbral absoluto minúsculo: una cara casi de canto pasa el filtro y produce un gradiente de profundidad enorme e inestable. Usa un criterio relativo, por ejemplo una anchura proyectada (área entre arista más larga) menor de `1e-6` mm. Prueba: una cara casi de canto no produce cortes falsos.

### 0.5 Sólidos de revolución no convexos
`solid` orienta las caras con el centroide, lo que obliga a que los perfiles de torno sean convexos. Los perfiles con cuello (jarrones, cúpulas bulbosas, campanarios) son necesarios para las escenas.

- `lathe` debe orientar sus caras **por construcción** (hacia fuera del eje, tapas arriba y abajo) y aceptar perfiles no convexos con alturas crecientes.
- `solid` necesita poder respetar el giro que le dan (por ejemplo un parámetro o una variante `solid_oriented`).
- La ocultación ya es general; la regla de aristas también vale.
- Prueba: un jarrón con perfil `(1.0, 0) → (0.5, 1) → (1.0, 2)` se dibuja sin aristas falsas y con el cuello ocultando lo que debe.

### 0.6 Caras traseras de sólidos convexos
En un sólido **convexo y cerrado**, una cara que no mira a la cámara nunca oculta nada que no oculten ya sus caras delanteras. No las añadas como oclusores: es la mitad de caras para escenas grandes. Esto **no** vale para sólidos no convexos (0.5) ni para `occluder`. Prueba: el resultado es idéntico con y sin la optimización en una escena de sólidos convexos.

### 0.7 Limpieza
Quita del `README.md` del módulo la sección «Validación del 13-11-2026…» con tiempos de este equipo. Es información de la entrega, no documentación, y va en tu resumen. Actualiza el README con la API nueva.

---

## Paso 1 · Infraestructura de generadores (núcleo)

Módulo nuevo `crates/vectorio-core/src/generate/` (regístralo en `lib.rs`). No depende de egui.

- **Parámetros declarativos.** `ParamSpec` con estas variantes:
  - `Section { label }`: un encabezado, sin valor.
  - `Range { id, label, min, max, step, default, random }`: `random` indica si entra en «Aleatorio» y opcionalmente un rango más estrecho.
  - `Choice { id, label, options: [(valor, etiqueta)], default, random }`.
  - `Toggle { id, label, default, random: probabilidad }`.
  - Un campo opcional para mostrar u ocultar el control según otros valores (`show`) y una ayuda opcional (`hint`).

  Las etiquetas son claves de texto en español que la app traduce.
- **Valores.** `Params` como mapa ordenado `id → ParamValue` (número, texto o booleano), serializable. Al leerlo:
  - las claves que faltan toman el valor por defecto;
  - las desconocidas se ignoran;
  - los números se recortan a su rango.

  Así sobreviven los cambios de versión. Incluye métodos tipados (`f64(id)`, `int(id)`, `choice(id)`, `flag(id)`).
- **Trait `Generator`:**
  - datos: `id()` (estable, nunca cambia), `name()`, `category()`, `description()`, `params() -> Vec<ParamSpec>`;
  - `randomize(&self, rng, &mut Params, locks)`: tiene una implementación por defecto que usa la información `random` de cada `ParamSpec` y respeta los parámetros bloqueados;
  - `generate(&self, &Params, &GenContext) -> Result<GenOutput, String>`.
- **`GenContext`:** el área de dibujo en mm (rectángulo del papel menos márgenes), la semilla, el grosor de rotulador de referencia y un nivel de detalle (`Preview` / `Final`), para que la vista previa pueda ser más ligera.
- **`GenOutput`:** lista de partes `{ name, lines: Vec<Polyline> }` en mm de hoja, más un aviso opcional. Las escenas la obtienen directamente de `SceneOutput`; los generadores 2D la construyen a mano. Añade un ayudante que calcule los metros de línea por parte.
- **Registro:** `generate::all() -> &'static [&'static dyn Generator]` y `generate::find(id)`.
- **Determinismo:** con la misma semilla y los mismos parámetros, salida idéntica. Usa solo `geom::Rng` (o un ruido determinista propio si hace falta), nunca aleatoriedad del sistema.

## Paso 2 · Grupos generados en el documento (núcleo)

El modelo a imitar es la **foto viva**: `crates/vectorio-core/src/photo.rs`, `Document::photos`, `Item::group`, `Document::photo` y `Document::group_items` en `doc.rs`. Una foto guarda su imagen y sus ajustes, y sus tintas son objetos que apuntan a ella con `group`.

- `Document::generated: Vec<GeneratedGroup>` con `{ id, name, generator: String, params: Params, seed: u64, area: BBox }`. Usa `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
- **Una parte = un objeto** del documento:
  - `Geometry::Path { closed: false }`, con `group = Some(id)` y nombre `«<grupo> · <parte>»`;
  - contorno con el rotulador asignado y sin relleno.
  - Así cada parte se mueve a otra tinta con las herramientas que ya existen.
- **Asignación inicial de tintas:** cada parte va a un rotulador del documento. El generador puede sugerir un orden (por ejemplo contorno → tinta 1, sombras → la última); si no, se reparten por orden, dando la vuelta si hay más partes que tintas.
- **Regenerar** (cambiar parámetros o semilla de un grupo existente):
  - sustituye las líneas de cada objeto **por nombre de parte**;
  - conserva su rotulador, su visibilidad, su bloqueo, su `Placement` y su posición en las capas;
  - las partes nuevas crean objetos nuevos;
  - las partes que desaparecen eliminan su objeto, pero recuerda su rotulador en el grupo por si vuelven.
- Las líneas se guardan en coordenadas locales de modo que mover o escalar el grupo y luego regenerar mantenga la posición. Explica el criterio en el código.
- Pruebas:
  - un archivo antiguo se abre igual;
  - ida y vuelta de serialización;
  - regenerar conserva tintas y posiciones;
  - una parte que desaparece y vuelve recupera su tinta.

## Paso 3 · Modo «Crear» en la app

Puntos de partida:
- `ViewMode` en `crates/vectorio-app/src/app.rs`;
- el selector de modo de la barra (busca `ViewMode::Output, tr!("Salida")`);
- los atajos y el menú en `commands.rs` (`⌘1` Diseño, `⌘2` Salida);
- `output.rs::show`, como ejemplo de modo con panel propio;
- `theme.rs` y `docs/plan-liquid-glass.md` para el estilo Liquid Glass actual.

Debe quedar así:

- **Tercer modo «Crear»**, con este orden en el selector y en el menú: Diseño · Crear · Salida. Atajos: `⌘1` Diseño, `⌘2` Crear, `⌘3` Salida (actualiza los atajos y textos existentes, y `LEEME.md`).
- **Panel lateral**, con estos elementos:
  - **Galería** de generadores por categoría, con nombre y descripción.
  - **Controles construidos a partir de `params()`**: deslizador con campo numérico, desplegable, casilla y encabezados plegables. Respetan `show`, muestran `hint` como ayuda y tienen un **candado** por parámetro.
  - **Semilla**: campo numérico y botón **Aleatorio** (nuevos parámetros, salvo los bloqueados, y nueva semilla). Un atajo de teclado para nueva semilla, solo cuando el foco no esté en un campo de texto.
  - **Métricas**: metros de línea por parte y total, con aviso visual por encima de 40 m y más fuerte por encima de 80 m («el papel puede saturarse»), y el tiempo de cálculo.
  - **Botones**: **Colocar en el lienzo** (crea el grupo con un paso de deshacer y vuelve a Diseño con el grupo seleccionado) o, si se está editando un grupo existente, **Actualizar grupo** y **Cancelar**.
- **Vista previa** en la zona central: la hoja con su color y las partes dibujadas con el color de la tinta que les tocaría. Reutiliza el pintado existente del lienzo si puedes.
- **Cálculo en segundo plano**, en un hilo con contador de versión:
  - un resultado obsoleto se descarta;
  - un cambio de control relanza el cálculo sin bloquear la interfaz;
  - la vista previa usa `Preview` y la colocación `Final`.
- **`generate` dentro de `std::panic::catch_unwind`** como última red. Un error o pánico se muestra en el panel como mensaje y nunca cierra la app.
- **En Capas/inspector**, un grupo generado se muestra como fila de grupo, igual que el de una foto (`group_row` en `inspector.rs`), con la acción **«Editar en Crear…»**, que abre el modo Crear con sus parámetros y su semilla.
- **Recordar** el último generador y sus parámetros entre sesiones, en las preferencias de la app.

## Paso 4 · Dos generadores de demostración

Son para probar la infraestructura, no la dirección artística definitiva, pero deben verse limpios y trazarse bien en A4 con un rotulador de 0,3–0,5 mm, por debajo de medio segundo.

1. **«Manzanas»** (categoría Escenas): una cuadrícula isométrica de bloques sobre una base, usando el motor.
   - Parámetros: filas, columnas, altura mínima y máxima, probabilidad de hueco (plaza), ancho de calle, giro de cámara (dos o tres vistas), elevación (isométrica/dimétrica), sol (ángulo), separación de trama.
   - Cada bloque es una caja con trama por tono (`hatch_face`) y algunas torres cilíndricas con tejado cónico (`lathe` y `hatch_surface`), con sombras sobre el suelo.
   - Partes: contorno, tejados, trama clara, trama media, trama oscura, sombra.
   - La cámara se ajusta al área con `Camera::fit`. Usa `Scene::visible` para no dibujar detalles menores de 1 mm.
2. **«Espirógrafo»** (categoría Curvas), generador 2D sin motor de escena.
   - Hipotrocoide y epitrocoide con radios enteros (cierra la curva con el mcd), distancia del lápiz, número de capas con desfase y parte por capa.
   - Debe salir en un solo trazo continuo por capa.

## Paso 5 · Pruebas y comprobaciones

- Pruebas del núcleo:
  - `Params`: valores por defecto, recorte a rango, claves desconocidas y `show`;
  - determinismo de cada generador;
  - **500 semillas aleatorias** por generador con `randomize`: ninguna falla, ninguna produce NaN, todas quedan dentro del área y del límite de 80 m, y la mayor tarda menos de 1 s en release (informa del máximo).
- Pruebas del documento del paso 2.
- Antes de entregar:
  1. `cargo test --workspace`
  2. `cargo clippy --workspace --all-targets` sin avisos nuevos.
  3. `cargo run -p vectorio-core --example muestra_escena --release -- muestra_escena.svg --dimetric`: revisa que las superficies curvas tengan trama continua.
  4. `python3 scripts/install-macos.py`.
  5. Comprobación visual en `/Applications/Vectorio.app`. Solo si **no** hay una Vectorio abierta: ábrela, prueba Crear → Manzanas → Aleatorio varias veces → Colocar → mover una parte a otra tinta → Editar en Crear → Actualizar → Deshacer, y Salida con el grupo. Si ya hay una abierta, no la cierres: indica en el resumen qué no pudiste comprobar.
- Actualiza `LEEME.md` con el modo Crear y los atajos nuevos.

## Fuera de esta fase

El kit de piezas (muros, ventanas, tejados, árboles, figuras…), las escenas con dirección artística (pueblo, castillo, estadio…) y más generadores 2D. Llegarán en la fase 3.

## Qué entregar

Un resumen breve en español con:

- lo hecho en cada paso y los ficheros tocados;
- la API pública nueva (firmas);
- las decisiones donde este documento deja margen, y por qué;
- las dos mediciones del motor y el peor tiempo de los generadores;
- capturas o descripción de la comprobación visual, o qué no se pudo comprobar;
- lo pendiente o lo que no te convenza.
