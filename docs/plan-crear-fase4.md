# Encargo: panel «Crear», fase 4 (sin costuras y dirección artística del Pueblo blanco)

Eres un agente de programación trabajando en **Vectorio**, una aplicación de escritorio en Rust para dibujar con plotter (núcleo en `crates/vectorio-core`, interfaz egui en `crates/vectorio-app`). La fase 3 dejó hechos y revisados:

- las aristas de intersección del motor;
- el estilo común en `generate/style.rs`;
- el kit en `generate/kit/`;
- la escena «Pueblo blanco» en `generate/village.rs`;
- las miniaturas de la galería.

Técnicamente funciona: las pruebas pasan, los tiempos son buenos y los encuentros entre volúmenes ya tienen su línea. Pero el Pueblo blanco **todavía no parece un pueblo blanco**. Esta fase tiene dos partes:

1. corregir unos pocos fallos de la revisión;
2. la pasada artística que convierte la escena en algo que apetezca plotear.

No se añaden escenas nuevas.

## Reglas

Las mismas de la fase 3:

- Lee `AGENTS.md`, `crates/vectorio-core/src/scene/README.md` y `docs/crear-fase3-entrega.md` antes de empezar.
- La interfaz va en español, con su traducción al inglés en `locales/app.yml`. Se mantiene la separación entre objetos/capas y tintas.
- **Trabajo original.** SVGeometric (`jarvisar/geometric`) no tiene licencia. No abras, copies ni traduzcas su código ni calques sus escenas, sus piezas o su paleta.
- Identificadores en inglés; comentarios y documentación en español.
- **No ejecutes `cargo fmt`.** Sin dependencias nuevas salvo que sean imprescindibles.
- Si falta `cargo`: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- No hagas commit salvo que el usuario lo pida. **No cierres ni reinicies la Vectorio que esté abierta.**
- Los documentos guardados con las fases 2 y 3 tienen que seguir abriéndose y regenerándose igual. Si cambias el aspecto del Pueblo blanco, versiona su receta igual que hiciste con Manzanas.
- **Legibilidad.** El código nuevo de la fase 3 está muy apretado: líneas de más de 200 caracteres y varias sentencias por línea. En los ficheros que toques (`style.rs`, `kit/`, `village.rs` e `intersections.rs`), parte las funciones largas y usa una sentencia por línea. No cambies el formato del resto del proyecto.

---

## Paso 0 · Correcciones de la revisión de la fase 3

1. **El encuadre recorta el dibujo.** Lo he medido con 150 semillas en tres áreas distintas, y cerca del 17 % de las salidas del Pueblo blanco (76 de 450) llegan al borde del área a menos de 0,05 mm. El margen es de 1 mm, así que esos dibujos salen **cortados** contra el borde. En Manzanas no le pasa a ninguna.

   La causa: `fit_scene` mide la primera pasada y reconstruye con otra escala, pero la geometría depende de la escala. `Style::quantum` y `snap`, `Scene::visible`, el paso de los estratos y el número de segmentos de los tornos cambian, y la segunda pasada sale más grande que la medida. Corrígelo así:
   - todo lo que dependa de la escala se calcula **una vez**, con la escala de la primera pasada, y se pasa fijo a la segunda. Por ejemplo, un `Style` con el cuanto ya resuelto;
   - después de la segunda pasada, comprueba que la caja cabe en el área con su margen. Si no cabe, haz una tercera pasada reduciendo la escala, nunca recortando;
   - la primera pasada se construye sobre un papel holgado, para que su medida no salga ya recortada.

   Prueba: con 500 semillas y tres áreas (cuadrada, A4 vertical y apaisada), ninguna salida queda a menos de medio margen del borde.
   > **Hecho.** `fit_scene` construye las pasadas de medida sobre un papel holgado e itera (hasta cuatro veces) hasta que la salida cabe con medio margen; la escena final se construye con la escala ya ajustada. En 900 salidas (150 semillas, tres áreas, Pueblo blanco en maqueta y Manzanas) la más cercana queda a 0,59 mm del borde.
2. **El alero son dos líneas sueltas.** `kit::eave` dibuja dos segmentos a 0,3 m de la fachada, sin nada que los sostenga. En el muestrario sobresalen de la silueta de la casa como palos. Sustitúyelo por un vuelo real: el faldón (o una losa fina) sobresale de los muros, con un grosor visible de al menos 1,5 mm en la hoja. La junta con el muro la pone el motor.
   > **Hecho.** El faldón vuela sobre la fachada de la calle (0,4 m o 1,6 mm en la hoja, lo que sea mayor) con un canto vertical en blanco de al menos 1,6 mm; los costados y la trasera quedan a haces porque tocan a las vecinas y al muro del bancal. Las casas de la fila trasera y la nave de la iglesia no vuelan: tienen delante, muy cerca, otro volumen. `kit::eave` es ahora una losa fina que no se dibuja si su canto no llega a 1,5 mm.
3. **La chimenea puede quedarse flotando.** Según el código, su base está a `top + rise·0,15` y su cara exterior a `min.x + q`. En un tejado a dos o a cuatro aguas, el faldón está a esa altura en `x = min.x + q` solo si `w ≤ 13,3·q`. Con q = 0,5 m, en casas de más de unos 6,7 m de ancho la chimenea quedaría por encima del tejado en su lado bajo. Ahora no ocurre porque las casas son estrechas, pero ocurrirá con las proporciones del paso 2. Calcula la base a partir de la altura del faldón en el punto más bajo de la huella de la chimenea, menos un encastre claro, y colócala cerca de la cumbrera, no junto al alero. Prueba con una casa de 9 m.
   > **Hecho.** La base sale de la altura del faldón en el punto bajo de su huella, menos un encastre, y la chimenea solo aparece si su fondo guarda 1,6 mm en papel con el hastial.
4. **Huecos oscuros.** `opening(..., dark: true)` solo añade un montante. O se rellena de trama oscura, como pedía el plan de la fase 3, o se le cambia el nombre. Prefiero el relleno: es lo que hace que una ventana se lea en una pared blanca (paso 2).
   > **Hecho.** `OpeningKind::Dark` (y `Twin`) rellena el hueco con una trama vertical apretada (0,6 veces la separación) recortada a su marco; la prueba `openings_clear_wall_hatching_on_every_side_and_dark_ones_get_filled` lo comprueba en las cuatro fachadas.
5. **Versión de la receta.** El parámetro oculto `scene_version`, con `show: "__internal__" = "never"`, es un apaño. Pasa la versión a un campo propio de `GeneratedGroup` (`#[serde(default)]`), o crea una variante de `ParamSpec` que nunca se muestre. Mantén la migración de las recetas de Manzanas de la fase 2.
   > **Hecho.** `ParamSpec::Version { current }` (`SCENE_VERSION`) nunca se muestra ni se sortea y, si falta, toma la versión vigente. Manzanas va por la 3 y conserva la migración a 2; Pueblo blanco empieza en la 1 con el pueblo denso (el de la fase 3 no llegó a guardarse en el repositorio, así que no se puede reproducir).
6. **`Style::lathe` depende del orden interno de las caras de `Scene::lathe`:** primero las n facetas de cada tramo y las tapas al final, con el mismo `circle_segments`. Que `lathe` devuelva esa estructura de forma explícita (facetas por meridiano y tapas aparte), o deja al menos una prueba que falle si el orden cambia.
   > **Hecho.** `Scene::lathe_parts` devuelve un `Lathe` con las caras por tramo y meridiano y las tapas aparte; `Style::lathe` ya no cuenta caras, y una prueba comprueba la altura y el ángulo de cada faceta.
7. **Pruebas del kit.** El plan pedía al menos una prueba geométrica por pieza y hay cuatro para doce piezas. Completa las que faltan, sobre todo la escalera, el bancal, el terreno, el pretil y el campanario.
   > **Hecho.** `every_kit_piece_draws_repeatably_and_cleanly_on_its_own` cubre las piezas que faltaban: cada una dibuja, es determinista, da coordenadas finitas y, sola, no deja avisos de construcción.

## Paso 1 · Sin costuras

Es el defecto más visible del muestrario. Cada pieza está hecha con varios sólidos pegados, y cada pegote deja una línea en medio de lo que debería ser **una sola superficie**:

- **Azotea con pretil:** son cuatro cajas sobre la casa. Hay una línea horizontal a lo largo de todo el muro, en la base del pretil, y juntas verticales en las esquinas del pretil. En el pueblo se ve en casi todas las casas.
- **Tejado a un agua:** la cara vertical alta del tejado continúa el muro, pero los separa una línea.
- **Hastiales:** el triángulo del tejado a dos aguas también queda separado del muro por una línea.
- **Escalera:** cada peldaño es una columna, y el lateral de la escalera sale como una fila de pilares con juntas verticales, no como un único zanco escalonado.

Las juntas entre **edificios distintos** (casas adosadas) sí deben verse. Las de **una misma pieza**, no.

**Qué hacer:**

- **Motor: caras coplanares de un mismo sólido.** Entre dos caras adyacentes, coplanares y con la misma normal exterior de un mismo sólido, no se dibuja la arista. Así se puede partir una cara no convexa en trozos convexos sin que se note. La trama de una cara partida sigue siendo una sola familia continua de líneas: compruébalo con una prueba.
- **Motor: prismas de perfil no convexo.** Añade `Scene::prism(profile: &[V3], vector: V3)`, que acepte un perfil plano simple no convexo, como el perfil de una escalera o de una casa con tejado. Descompone las tapas en trozos convexos, construye el sólido con `solid_oriented` y devuelve las caras exteriores, agrupadas por cara lógica, para poder tramarlas.
- **Kit: cada pieza es un único sólido siempre que sea posible.**
  - Casa con tejado a dos aguas: prisma de perfil pentagonal, muro y hastial en un solo sólido.
  - Casa con tejado a un agua: prisma de perfil trapecial.
  - Casa con tejado a cuatro aguas: un solo sólido convexo.
  - Azotea con pretil: un sólido no convexo, con muros exteriores de altura completa, la corona del pretil, los muros interiores y el suelo de la azotea hundido.
  - Escalera: prisma del perfil escalonado.
- **Prueba de costuras.** Para cada pieza del kit, ninguna arista visible separa dos caras coplanares con la misma normal. Las juntas entre casas adosadas siguen apareciendo.

> **Hecho.**
> - **Motor.** `build_solid` no dibuja la arista entre dos caras de un mismo sólido con la misma normal (ni como borde ni como línea). `Scene::hatch_faces` trama una cara partida en trozos sin holgura entre ellos: cada línea cruza los trozos de un solo trazo, y una prueba compara la trama de una cara entera con la de la misma cara en dos triángulos.
> - **Prismas y poliedros.** `Scene::polyhedron(faces)` recibe caras lógicas planas y simples (no convexas incluidas), une vértices, orienta el giro por adyacencia y por el volumen, parte cada cara en trozos convexos (orejas y Hertel–Mehlhorn) y construye con `solid_oriented`; devuelve los trozos agrupados por cara lógica, o nada si la cáscara no es cerrada. `Scene::prism(profile, vector)` se apoya en él.
> - **Kit.** La casa (plana, con pretil, a un agua, a dos y a cuatro aguas, con o sin vuelo), el pretil suelto, la escalera, la tapia (anillo o tramos seguidos, con o sin puerta), el banco y los pórticos de la pérgola son un solo sólido cada uno. `Style::shade_groups` trama juntas las caras coplanarias de un sólido.
> - **Prueba.** `no_kit_piece_draws_a_seam_between_coplanar_faces_of_its_own` recorre 20 piezas a tres escalas y seis semillas con el detector `Scene::seams` (solo en pruebas) y comprueba que dos casas adosadas siguen mostrando su junta con las cuatro cubiertas.

## Paso 2 · Dirección artística del Pueblo blanco

Comparado con la hoja de contactos actual, esto es lo que falla y lo que pido:

- **La cal no se ve.** Con los valores de serie, las dos fachadas visibles de casi todas las casas salen tramadas, así que el pueblo se lee gris y no blanco. Es justo lo contrario de la idea del estilo.
  - El ángulo del sol tiene que ser **relativo a la vista**: con cualquier «Vista», el sol de serie ilumina de lleno una de las dos orientaciones de fachada visibles, que queda en blanco.
  - La otra orientación se queda en tono medio, con una trama más abierta que ahora: unas 2,2 veces la separación base.
  - El oscuro se reserva para las sombras arrojadas, los huecos y las caras que de verdad miran lejos del sol.
  - Prueba: con los valores de serie y en cada vista, al menos el 40 % del área visible de muro queda sin trama.
- **Proporciones de casa, no de torre.** Hoy las casas tienen unos 3 m de fachada, 2,6 m de fondo y hasta 5 m de alto más el pretil, y se leen como torres.
  - Pide fachadas de 4 a 9 m, de 1 a 2 plantas de unos 2,8 m y fondos de 5 a 7 m. Si hace falta, haz los bancales más profundos.
  - Las casas se adosan **en hilera a lo largo de la calle**, con saltos de altura de al menos un cuanto. De vez en cuando, un patio o un solar rompe la hilera.
- **Huecos que se lean.** Las ventanas son pequeñas y profundas sobre la cal: hueco relleno de trama oscura (paso 0.4). La puerta, en la planta baja y hacia la calle. El ritmo de los huecos es regular dentro de cada casa y distinto entre casas. Los hastiales y medianeras visibles pueden llevar algún hueco aislado.
- **Iglesia con jerarquía.** La nave es claramente más larga que ancha (de 1:2 a 1:3) y el campanario va en una esquina de los pies, no en medio de la nave. Prueba que su encuentro con el faldón o con el hastial se dibuja bien.
- **Chimeneas con mesura.** Menos y solo en tejados inclinados. En la semilla 5 casi cada tejado tiene una.
- **Estratos con carácter.** Hoy son rayas paralelas casi rectas a distancia constante, y parecen un rayado, no un terreno cortado. Es el sello visual de la serie, así que merece cuidado:
  - grosor de capa irregular y determinista;
  - ondulación de baja frecuencia más visible, de un 25 a un 35 % del paso;
  - de vez en cuando, una capa que se acuña y desaparece (lente);
  - los estratos solo van en las **caras exteriores** del bloque cortado, las del borde de la maqueta;
  - los muros interiores entre bancales son muros de contención: llevan una albardilla (línea de coronación) y, como mucho, hiladas muy abiertas, nunca estratos.
- **Calles.** La calle es el blanco entre hileras. Basta con un bordillo o un escalón donde cambia el nivel. El eje de escaleras conecta de verdad las calles de bancales sucesivos.
- **Vegetación.** Los cipreses van en parejas o en hilera junto a la iglesia o en el borde de la maqueta. Los olivos se agrupan en los huertos de los solares, no sueltos uno a uno.

Cuida que todo esto se siga leyendo con una sola tinta: la diferencia entre blanco, medio y oscuro tiene que funcionar sin color.

> **Hecho.**
> - **Cal visible.** El sol de serie se calcula desde la vista: la fachada de la calle visible queda en Lit (0,74) y la otra cara visible en Mid (0,35) a 35° de altura; «Giro del sol» (−90 a 90°) lo gira a partir de ahí, sin subir la versión de la receta. En «Grabado», el tono medio usa 2,2 veces la separación (`Style::gap`). La prueba de cobertura (`Scene::lit_wall_fraction`, solo en pruebas) da entre el 47 y el 65 % de muro sin trama en las cuatro vistas, a sangre y en maqueta.
> - **Casas.** Fachadas de 4 a 9 m que llenan la hilera sin huecos (se parte en dos un solar de más de 9,5 m), fondos de 5 a 7 m, una planta y, con probabilidad 0,55 por planta, una segunda de 2,8 m; si una casa iguala a su vecina, sube un cuanto. Los bancales son más profundos (calle de 2,4 m más 7 m de casas). Un 40 % de los solares con fondo suficiente es casa con patio (retranqueada, con tapia y puerta) y el resto huerto tapiado con surcos y olivos agrupados.
> - **Huecos.** La puerta va en la planta baja y hacia la calle; las ventanas siguen un paso y un ancho propios de cada casa (iguales dentro de ella, distintos entre vecinas) y, en las de dos plantas, a veces hay un hueco aislado en la medianera, que solo se ve donde la vecina es más baja.
> - **Iglesia.** `kit::church` con `Church { width, length, height, tower }`: nave de 1:2,2 a 1:2,8, portada, óculo y ventanas altas en bahías de unos 3 m, y el campanario encastrado en una esquina de los pies, que avanza sobre la fachada y entra en la nave lo justo para guardar 1,6 mm en la hoja. `Church::ahead` dice cuánto avanza, para que bancos, árboles y gente de la plaza no lo toquen. La prueba `the_bell_tower_stands_at_a_front_corner_and_its_meeting_with_the_nave_is_drawn` recorre tres escalas, cuatro vistas y los dos lados.
> - **Chimeneas.** Solo en tejados inclinados y con menos probabilidad; las azoteas tienen casetón, pérgola, tendedero o depósito.
> - **Estratos.** `kit::Strata` es común a todo el terreno: grosor de 0,7 a 1,6 pasos, ondulación compartida de baja frecuencia más otra propia (hasta un 30 % del paso) y lentes en un 15 % de las capas. `kit::terrace` recibe un `Cut` con las caras exteriores y la cota desde la que se ve el muro delantero; los muros interiores llevan albardilla y, en el dibujo final, hiladas muy abiertas.
> - **Calles y escaleras.** La calle es el blanco de 2,4 m delante de cada hilera; el cambio de nivel es el canto del bancal. La escalera central sale de la calle de abajo y llega a la de arriba; los callejones llevan sus propias escaleras.
> - **Vegetación.** Pareja de cipreses en el costado de la iglesia sin campanario, hilera de cipreses solo en el borde del núcleo, olivos agrupados en huertos y olivares; las hileras de cipreses en los campos son raras.
> - **Comprobado.** 135 pruebas de núcleo; en release, 500 semillas por generador sin avisos (el peor Pueblo blanco tarda 296 ms), encuadre a 0,54 mm del borde como mínimo y Pueblo blanco de serie con mediana de 84 ms en vista previa y 86 ms en final, ejecutando las pruebas de una en una. Muestras en `target/phase4/paso2/`.

## Paso 3 · Comprobaciones y entrega

- **Pruebas:**
  - las de los pasos 0 y 1;
  - ningún recorte con 500 semillas en tres áreas;
  - cobertura de cal ≥ 40 % con los valores de serie;
  - determinismo;
  - 500 semillas sin NaN, por debajo de 80 m y con un peor tiempo por debajo de 1 s en release.
- **Rendimiento:** el Pueblo blanco de serie, por debajo de 100 ms en Preview y de 300 ms en Final.
- **Antes de entregar:**
  1. `cargo test --workspace`
  2. `cargo clippy --workspace --all-targets` sin avisos nuevos.
  3. Ejecuta `kit_piezas`, `hoja_contactos` y `muestra_escena`.
  4. `python3 scripts/install-macos.py`.
  5. Comprobación visual en `/Applications/Vectorio.app`, solo si no hay una Vectorio abierta.
- **Comparación:** guarda en `target/phase4/` la hoja de contactos de la fase 3 y la nueva, con las mismas 12 semillas, para comparar antes y después.
- **Entrega:** escribe `docs/crear-fase4-entrega.md` con:
  - lo hecho en cada paso y la API nueva;
  - los tiempos;
  - tu valoración sincera semilla a semilla de la hoja nueva: cuáles plotearías y cuáles no, y por qué;
  - lo que no te convenza.

> **Hecho.** Encuadre a 500 semillas en tres áreas, 500 semillas por generador sin avisos ni NaN y por debajo de 80 m, cobertura de cal y determinismo en verde; tiempos, comparación y valoración en [crear-fase4-entrega.md](crear-fase4-entrega.md).

## Fuera de esta fase

Castillo, estadio y otras escenas; sombras sobre otros objetos; figuras y texto. Volveremos a ellos cuando el Pueblo blanco esté a la altura.
