# Encargo: panel «Crear», fase 3 (estilo propio, kit de piezas y «Pueblo blanco»)

Eres un agente de programación trabajando en **Vectorio**, una aplicación de escritorio en Rust para dibujar con plotter (núcleo en `crates/vectorio-core`, interfaz egui en `crates/vectorio-app`). Las fases 1 y 2 dejaron hechos estos módulos, que se han revisado y funcionan:

- el motor de escenas `crates/vectorio-core/src/scene/`: ocultación exacta, tramas, sombras y tornos con cuello;
- la infraestructura de generadores `crates/vectorio-core/src/generate/`: parámetros declarativos, grupos regenerables en el documento y dos generadores de prueba;
- el modo **Crear** de la app (`crates/vectorio-app/src/create.rs`).

Esta fase da el salto de «funciona» a **«merece la pena dibujarlo»**. Tiene tres partes:

1. pequeñas correcciones de la revisión;
2. un **estilo de dibujo propio** y un **kit de piezas** reutilizable;
3. la primera escena con dirección artística: **«Pueblo blanco»**.

Después revisaré tu trabajo con otro agente, así que deja el código claro, probado y bien explicado.

## Reglas

Las mismas de la fase 2:

- Lee `AGENTS.md`, `crates/vectorio-core/src/scene/README.md` y `docs/crear-fase2-entrega.md` antes de empezar.
- La interfaz va en español, con su traducción al inglés en `locales/app.yml`. Se mantiene la separación entre objetos/capas y tintas.
- **Trabajo original.** SVGeometric (`jarvisar/geometric`) no tiene licencia. No abras, copies ni traduzcas su código ni calques sus escenas, sus piezas o su paleta.
- Identificadores en inglés; comentarios y documentación en español.
- **No ejecutes `cargo fmt`.** Sin dependencias nuevas salvo que sean imprescindibles.
- Si falta `cargo`: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- No hagas commit salvo que el usuario lo pida. **No cierres ni reinicies la Vectorio que esté abierta.**
- Los documentos guardados con la fase 2 tienen que seguir abriéndose y regenerándose igual.

---

## Paso 0 · Correcciones de la revisión de la fase 2

1. **Recetas huérfanas.** `Document::to_json` ya descarta las fotos que se han quedado sin tintas. Haz lo mismo con los `generated` sin objetos. Prueba: borrar todas las partes de un grupo y guardar no deja la receta en el archivo.
2. **Editar no debe pisar lo recordado.** Ahora `CreateUi::edit` escribe los parámetros del grupo en las preferencias del generador y borra los candados. Separa ambas cosas:
   - mientras se edita un grupo, se trabaja sobre una copia;
   - las preferencias solo se actualizan al crear grupos nuevos;
   - al cancelar o actualizar, vuelven los parámetros y candados recordados.
3. **Vista previa:**
   - con un error, la hoja muestra el error (o la última vista previa válida atenuada), no «Preparando vista previa…»;
   - mientras se calcula el Final, se mantiene la última vista previa en lugar de dejar la hoja en blanco.
4. **`randomize_specs`:** un `step` menor o igual que 0, o no finito, no puede producir NaN; en ese caso no se cuantiza.
5. **Encuadre de las escenas.** Manzanas solo ocupa alrededor del 63 % del ancho útil porque encuadra una caja con 3 m de margen. Añade a `generate` un ayudante reutilizable con este patrón:
   - construye la escena con una cámara provisional;
   - mide la caja real de la salida;
   - ajusta escala y centro, y la **vuelve a construir**, para que la separación de trama siga siendo física en mm.

   El bodegón ya lo hace a mano. Úsalo en Manzanas y en Pueblo blanco. Prueba: con los valores por defecto, la salida ocupa al menos el 90 % del ancho o del alto útil.
6. **Luz.** El sol de Manzanas tiene la altura fija (`z = 1.5`): los tejados siempre salen claros y las paredes nunca, así que el abanico de tonos es corto. Añade un parámetro **altura del sol** (por ejemplo de 20° a 70°) y aplícalo también a la escena nueva.
7. **Tejado cónico de Manzanas.** Su pendiente (54,7°) coincide con el complemento de la elevación isométrica. Las caras traseras quedan de canto y el cono parece una peonza con la elipse entera. Usa una pendiente que no coincida con ninguna de las elevaciones ofrecidas y documenta por qué.

## Paso 1 · Estilo propio (`generate/style.rs`)

Las escenas comparten un **estilo Vectorio** definido en un solo sitio, para que se reconozcan como de la misma familia y se puedan retocar juntas. La propuesta de partida es esta; si al probarla algo no funciona en papel, ajústalo y explica por qué:

- **Tono con densidad, nunca con grosor.** Tres niveles:
  - claro: sin trama, es el blanco del papel;
  - medio: trama sencilla;
  - oscuro: trama más densa o cruzada.

  En un pueblo blanco, la cal se lee precisamente porque las caras al sol quedan vacías.
- **La trama sigue a la superficie:**
  - paredes: paralela a las hiladas, es decir, a la arista horizontal;
  - tejados inclinados: según la línea de máxima pendiente;
  - suelo y terreno: un ángulo fijo de papel;
  - superficies de revolución: horizontal en la hoja (`hatch_surface`).
- **Bandas de tono limpias en superficies curvas.** Hoy la frontera entre bandas sigue las facetas y sale en escalera (se ve en la cúpula y los conos). Clasifica por tiras verticales completas del torno (todas las facetas de un mismo meridiano con el mismo tono, decidido por la normal en la mitad de su altura). Otra opción equivalente: suaviza la frontera. Explica la elección.
- **Sombra arrojada** con su propia parte y un ángulo de trama distinto del de las caras, para que se distinga aunque vaya con la misma tinta.
- **Maqueta cortada.** Toda escena se apoya en un bloque de terreno cortado a cuchillo. Sus caras laterales llevan los **estratos** como líneas de nivel horizontales con ligeras ondulaciones deterministas. Es el sello visual de la serie: la escena flota en la hoja como una maqueta.
- **Partes estándar** con nombres fijos, para que el usuario reparta tintas igual en todas las escenas: `contorno`, `detalle` (ventanas, puertas, tejas, almenas), `trama` (media y oscura; o separadas si el resultado lo pide), `sombra`, `vegetación`, `terreno` (bloque cortado y estratos). Sugiere tintas con `pen_order`: contorno y detalle con la primera tinta, vegetación con la segunda y sombra con la última.
- **Nivel de detalle.** Nada por debajo de 1 mm en la hoja (`Scene::visible`). La vista previa puede omitir detalles finos.

## Paso 2 · Kit de piezas (`generate/kit/`)

Funciones puras que dibujan sobre una `Scene` con el estilo del paso 1. Reciben su posición, sus medidas y un `&mut Rng` cuando tienen variación; son deterministas y descartan entradas degeneradas sin entrar en pánico. Como mínimo:

- **Casa** (prisma) con tejado a una o dos aguas, de cuatro aguas o plano con pretil.
- **Huecos:**
  - ventanas y puertas como líneas `whole`: o se ven enteras o no se dibujan;
  - en las paredes en sombra, el hueco puede ir relleno de trama oscura;
  - los huecos se reparten por la fachada sin pisar esquinas ni otros huecos.
- **Chimenea, pretil, alero simple.**
- **Torre-campanario:** fuste prismático, cuerpo de campanas con huecos, remate de cúpula o chapitel (torno con cuello del paso 0.5 de la fase 2).
- **Escalera** entre dos niveles.
- **Muro de contención y bancal:** plataformas escalonadas que siguen una ladera.
- **Vegetación:** ciprés (torno alargado), olivo o árbol de copa (copa convexa irregular a partir de un torno con perfil aleatorio suave), arbusto bajo.
- **Terreno cortado** con estratos (paso 1).

Cada pieza tiene al menos una prueba geométrica:

- número de aristas visibles esperado en una vista conocida;
- una ventana tapada por otra casa no se dibuja a medias;
- la pieza no produce NaN con medidas extremas;
- los detalles menores de 1 mm se omiten.

Escribe un ejemplo `examples/kit_piezas.rs` que dibuje un muestrario de todas las piezas en una hoja A4.

## Paso 3 · Escena «Pueblo blanco» (categoría Escenas)

Un pueblo de casas encaladas que se escalona en bancales sobre una ladera, con su iglesia, sus calles en cuesta y sus cipreses, todo sobre la maqueta cortada. La idea es la de los pueblos blancos andaluces, pero interpretada con el estilo del paso 1, no como copia de ninguna referencia.

**Composición procedural, con criterio:**

1. **Ladera:** de 3 a 6 bancales de altura y fondo variables; la pendiente es un parámetro.
2. **Calles:** una o dos calles principales siguen las curvas de nivel y una escalera o cuesta las une. Las casas dan la fachada a la calle.
3. **Casas:**
   - agrupadas en manzanas compactas a lo largo de las calles, con variación de altura (1 a 3 plantas), ancho y tipo de tejado;
   - medianeras compartidas: las casas vecinas se tocan, sin huecos absurdos entre ellas;
   - las cajas pegadas no deben duplicar trazos; el motor ya elimina pasadas repetidas.
4. **Iglesia:**
   - una nave con su campanario en el punto más alto o en una plaza;
   - es el foco de la composición: más alta y con más detalle;
   - opcional.
5. **Vegetación:** cipreses junto a la iglesia y en los bordes; olivos en los bancales vacíos. La densidad es un parámetro.
6. **Ritmo y aire:** deja huecos (plazas, huertos, patios). Un pueblo saturado se lee peor en papel. Limita la longitud total por defecto a unos 40 m en A4.

**Parámetros:**
- tamaño;
- bancales;
- pendiente;
- densidad de casas;
- plantas máximas;
- proporción de cada tipo de tejado;
- iglesia (sí/no);
- vegetación;
- vista (cuatro giros);
- elevación (isométrica/dimétrica);
- sol (azimut y altura);
- separación de trama;
- profundidad del terreno cortado.

Usa `show` cuando un control solo tenga sentido con otro (por ejemplo los del campanario, solo con iglesia).

**Calidad:**
- con los valores por defecto y con al menos 8 de cada 10 semillas aleatorias, el resultado tiene que verse bien sin retocar;
- trazado final por debajo de 300 ms en release; vista previa por debajo de 100 ms.

## Paso 4 · Miniaturas en la galería (app)

Cada generador de la galería muestra una miniatura pequeña:

- su salida con los valores por defecto, en `Preview`;
- calculada en segundo plano con el mismo trabajador o uno aparte;
- guardada en memoria durante la sesión.

Mientras no existe, la fila se muestra como ahora. No debe retrasar la vista previa principal.

## Paso 5 · Hoja de contactos y comprobaciones

- Ejemplo `examples/hoja_contactos.rs`: dibuja en un SVG grande una cuadrícula de 4 × 3 semillas de cada generador, con la semilla escrita debajo de cada una. Es la herramienta principal de la revisión visual: inclúyela en tu entrega y comenta qué semillas te parecen flojas y por qué.
- Pruebas:
  - las del paso 0;
  - las del kit;
  - determinismo de Pueblo blanco;
  - **500 semillas** con `randomize`: sin fallos ni NaN, dentro del área, por debajo de 80 m y peor tiempo por debajo de 1 s en release.
- Antes de entregar:
  1. `cargo test --workspace`
  2. `cargo clippy --workspace --all-targets` sin avisos nuevos.
  3. Ejecuta los dos ejemplos nuevos y revisa los SVG.
  4. `python3 scripts/install-macos.py`.
  5. Comprobación visual en `/Applications/Vectorio.app`, solo si no hay una Vectorio abierta: Crear → Pueblo blanco → Aleatorio varias veces → Colocar → cambiar la tinta de `vegetación` → Editar en Crear → Actualizar (la tinta se conserva) → Salida.
- Actualiza `LEEME.md` si cambia algo visible.

## Fuera de esta fase

El castillo, el estadio y otras escenas reutilizarán este kit en fases posteriores. Sombras sobre otros objetos (no solo sobre el suelo), figuras humanas y texto en las escenas tampoco entran ahora.

## Qué entregar

Un resumen breve en español (puede ir en `docs/crear-fase3-entrega.md`) con:

- lo hecho en cada paso y los ficheros tocados;
- la API pública nueva (firmas);
- las decisiones de estilo y por qué;
- los tiempos (Pueblo blanco por defecto, peor de las 500 semillas, vista previa);
- la hoja de contactos y tu valoración sincera de qué funciona y qué no;
- lo pendiente o lo que no te convenza.
