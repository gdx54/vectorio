# Encargo para Sol · «Ciudad en fiesta», corrección 4 (relevo)

A partir de aquí sigues tú con la ciudad. Este documento resume en qué estado
está y qué falta, para que no tengas que reconstruirlo leyendo todo.

Las normas de `docs/plan-paso1-sol.md` siguen valiendo:
- español en la interfaz, y cada texto nuevo de la receta con su traducción en
  `crates/vectorio-app/locales/app.yml`;
- sin `cargo fmt`, sin dependencias nuevas y sin commit;
- `--test-threads=1` en las pruebas;
- `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"` si no encuentras `cargo`;
- no cierres ni reinicies Vectorio si está abierto;
- rótulos inventados y genéricos en español, sin marcas.

Va por hitos que dejan las pruebas en verde: si se acaba el margen, para al
terminar uno.

## 0. Estado de partida

Rama `feat/crear-motor-escena`, con muchos cambios **sin confirmar** (no los
confirmes ni los descartes). Todas las pruebas pasan y clippy no avisa de nada
en `generate/` ni en `scene/`. La app instalada en `/Applications/Vectorio.app`
lleva todo lo que se cuenta aquí.

### Mapa de archivos

| Archivo | Qué hay |
|---|---|
| `generate/festival.rs` | Receta, `City` (cámara, calles, manzanas, hitos), parámetros y pruebas |
| `generate/festival/events.rs` | Eventos de la plaza: verbena, mercadillo, feria, cabalgata |
| `generate/festival/stories.rs` | «Vida en la calle»: obra con grúa, globo, palomas, tendederos, andamios |
| `generate/festival/coast.rs` | Costa: playa o puerto en uno de los lados delanteros |
| `generate/festival/avenue.rs` | Avenida diagonal, edificios en cuña y parque |
| `generate/village.rs` | Pueblo blanco; comparte con la ciudad figuras, palomas, vendedor y vegetación |
| `generate/kit/` | Piezas comunes (`landscape.rs` tiene `vegetation` con `Plant::{Cypress, Olive, Shrub, Palm, Pine}`) |

### Parámetros actuales de la ciudad

`people`, `landmarks`, `event`, `stories`, `circuit` («Carrera urbana»),
`coast` («Costa»: none/beach/port), `avenue` («Avenida diagonal», solo sin
carrera), `yaw`, `sun`, `shadow`. La versión de la receta sigue en 1.

### Lo que hay hecho (correcciones 1 a 3)

Encontrarás el detalle en el paso 1 de `docs/plan-crear-fase5.md`.

- **Base:** ciudad a sangre, carrera urbana, sombras proyectadas, los cuatro
  eventos y la vida en la calle.
- **Costa:**
  - playa con paseo de palmeras, sombrillas, socorrista, vóley y barcas;
  - puerto con muelle, lonja, espigón de escollera con faro y pantalán.
- **Avenida diagonal:**
  - pasa delante de la plaza, o detrás si hay costa;
  - mediana con árboles y farolas, y cuatro carriles;
  - edificios en cuña con la proa achaflanada;
  - un parque con fuente, cipreses en el borde, pinos al fondo y olivos delante.
- **Cruce con el pueblo:** pinares ralos y palmeras a la entrada de la plaza.

## Hito 1 · Rematar la avenida

Mira `avenida_{225,315,45,135}.svg` y `av_costa.svg` antes de tocar nada; la
orden para generarlos está en «Cómo mirar una lámina».

1. **Hueco en la mediana.** En el cruce central faltan unos 30 m de mediana,
   porque allí llegan dos calles de la rejilla. `free_runs` evita las calles con
   `in_street`. Cierra el tramo entre las dos calles con mediana y árboles, y
   pon un paso de cebra con gente cruzando la avenida en cada calle que llega.
2. **Con costa casi no se ve.** La avenida queda al fondo y solo asoma por el
   borde superior. La cámara suma los desplazamientos `COAST_SHIFT` y
   `AVENUE_SHIFT` en `camera()` de `festival.rs`. Busca un encuadre en el que
   se vean a la vez el mar y la mediana, aunque sea de lado.
3. **Las proas no se leen.** Las cuñas del lado de la plaza se ven de frente y
   parecen cajas. Dale a la proa algo que la distinga: un torreón cilíndrico
   bajo (`scene.lathe`) o un chaflán más ancho con escaparate.
4. **Pruebas.** Ya existe `festival_coasts_and_avenue_are_clean_in_every_view`.
   Añade una que compruebe que, con la avenida activa, hay trazo de
   vegetación cerca de su eje en las cuatro vistas.

## Hito 2 · Distrito dominante

Es el parámetro pendiente del plan. Añade `district` («Distrito»), una elección
entre «Comercial», «Residencial» y «Ocio», con sorteo aleatorio:

- **Comercial:** lo que hay ahora, con escaparates en todas las fachadas.
- **Residencial:** alturas más bajas y parejas, más balcones y tendederos, pocos
  escaparates y algún patio con árboles en el centro de la manzana.
- **Ocio:** terrazas con mesas en las aceras, más azoteas con gente, carteles y
  un hito más probable de tipo escenario.

Cambia las proporciones, no la geometría de la rejilla. El núcleo alrededor de
la plaza sigue igual en los tres distritos.

Pruebas: el barrido de semillas y vistas ya existe. Añade el distrito al bucle
de `check_seeds` y comprueba en las tres opciones que no hay avisos y que no se
pasa del presupuesto.

## Hito 3 · Globos y banderas

- **Banderolas** colgadas en vertical de las farolas de la avenida y de las
  calles del núcleo, cada 2 o 3 farolas.
- **Banderas** en mástil sobre algunas azoteas del núcleo y en el faro del
  puerto.
- **Racimos de globos** sueltos atados a los puestos y a las barandillas de la
  plaza. El vendedor de globos ya existe en `kit::life`; reutiliza su dibujo
  del globo.

Son trazos y no tapan nada: no los conviertas en oclusores, o el detector de
costuras protestará. Tinta «línea», o «rótulos» si queda mejor.

## Hito 4 · Repaso lámina a lámina

Genera las semillas 0 a 11 en la vista 225 y otras cuatro en cada una de las
demás vistas, con parámetros aleatorios. Anota en
`docs/plan-crear-fase5.md`, en una línea por lámina, qué falla:
- algo flotando o cortado;
- asfalto vacío;
- la plaza tapada;
- gente sobre un edificio;
- texto encima de otro.

Arregla lo evidente y deja anotado lo que no dé tiempo.

## Hito 5 · Cierre

- `cargo test --workspace --release -- --test-threads=1` y
  `cargo clippy --workspace --release --all-targets`. El código nuevo no debe
  dar ningún aviso.
- Hoja de contactos:
  `cargo run -p vectorio-core --example hoja_contactos --release -- target/phase5/contactos.svg`.
  **No** le pases `qlmanage` a la hoja entera, porque se cuelga (pesa 23 MB).
  Recorta cada lámina en su propio SVG, como en el encargo anterior.
- `python3 scripts/install-macos.py`. Tarda unos minutos: lánzalo en segundo
  plano.
- En `docs/plan-crear-fase5.md`, una nota «Corrección 4» bajo las anteriores.

## Cómo mirar una lámina

```bash
cargo run -q -p vectorio-core --example muestra_receta --release -- festival_city target/phase5/x.svg 4 avenue=si circuit=no coast=port yaw=225
```

```bash
cd target/phase5 && qlmanage -t -s 1400 -o . x.svg
```

Esto deja `x.svg.png`. Las opciones de elección se pasan por su clave
(`coast=port`, `event=fair`).

## Trampas conocidas (aprendidas en la corrección 3)

- **Interruptores en `muestra_receta`:** van como `si`/`no`. `true` y `false`
  se leen como texto y se ignoran, y la lámina sale con los valores por defecto,
  con la carrera puesta y sin avenida.
- **`scene.lathe`** no dibuja nada si las alturas del perfil no crecen
  estrictamente. Para un escalón, sube 0,02 m en lugar de repetir la altura.
- **El papel abarca poco.** A la escala por defecto, la hoja solo cubre unos
  ±35 m en horizontal alrededor del centro. Todo lo que se coloque lejos de la
  plaza tiene que comprobarse con `on_paper`.
- **El margen de `on_paper` va en milímetros y amplía el área de la hoja.** Un
  margen negativo la encoge: en `avenue_span` puse `-40` y la avenida
  desapareció sin dar ningún error.
- **Calles y avenida:** las calles de la rejilla pasan por `self.pieces(a, b)`,
  que recorta la tierra firme y la avenida, y por `self.street_band(a, b)`, que
  remata el bordillo en oblicuo contra la avenida. Si añades otra calle, pásala
  por las dos.
- **Coordenada diagonal:** `Avenue::offset(p)` da los metros al eje de la
  avenida y `side(origin)` el lado de una manzana. `cut_lots` lista las celdas
  que corta la avenida, y esas no llevan manzana normal: llevan cuña o parque.
- **Orden de pintado:** la cámara se calcula una vez en `City::new`, primero la
  costa y después la avenida. Si un elemento depende de la vista (lo que va
  delante o detrás), usa `scene.camera().axes().2` aplanado con `flat`, como
  `park_trees`.
