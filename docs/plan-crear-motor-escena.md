# Encargo: motor de escenas isométricas (fase 1 del panel «Crear»)

Eres un agente de programación trabajando en **Vectorio**, una aplicación de escritorio en Rust para dibujar con plotter. El núcleo está en `crates/vectorio-core` (sin interfaz) y la interfaz egui en `crates/vectorio-app`. Tu trabajo en este encargo es **solo el núcleo**: un motor que convierte una escena 3D sencilla en líneas 2D listas para el plotter, con las líneas ocultas eliminadas de forma exacta. Después revisaré tu trabajo con otro agente, así que deja el código claro, probado y bien explicado.

## Contexto: para qué sirve

Vamos a añadir un tercer modo a la app, **Crear**, junto a Diseño y Salida. Tendrá generadores procedurales de dibujos: escenas isométricas (un pueblo, un castillo, un estadio…) y otros generadores 2D. Cada escena se planifica en metros, se construye con piezas 3D simples (cajas, prismas, tejados, cilindros, conos) y se dibuja con una cámara ortográfica. Este encargo es la base de todo eso: el motor de escena. **No** hagas todavía el panel, los generadores ni la integración con el documento; vendrán en fases posteriores.

## Reglas

- Lee `AGENTS.md` antes de empezar.
- **Trabajo original.** La idea está inspirada en el proyecto SVGeometric (`jarvisar/geometric`), que **no tiene licencia**. No abras, copies ni traduzcas su código. Implementa a partir de la descripción de este documento.
- Identificadores en inglés, como el resto del núcleo (`Document`, `Item`, `fill`, `order`…). Comentarios y documentación en **español**, en el tono del código existente (frases cortas que explican el porqué).
- Usa los tipos que ya existen: `vectorio_core::geom::{P, p, Polyline, BBox, Rng}`. Todo lo que sale del motor está en **milímetros sobre la hoja, con y hacia abajo**, como el resto del núcleo.
- **Sin dependencias nuevas.** El algoritmo descrito abajo no necesita ninguna librería de operaciones entre polígonos.
- No toques `crates/vectorio-app` ni otros módulos del núcleo, salvo para registrar el módulo nuevo en `crates/vectorio-core/src/lib.rs`.
- **No ejecutes `cargo fmt`**: el proyecto usa líneas densas a propósito (hasta unos 150 caracteres) y el formateo ensuciaría todo el diff.
- Si `cargo` no está disponible: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Trabaja en una rama nueva, `feat/crear-motor-escena`, creada desde la rama actual. No hagas commit salvo que el usuario lo pida. Si lo pide, el mensaje en español y terminando con la línea de coautoría que te indique tu entorno.
- No cierres ni reinicies la Vectorio que esté abierta.

## Qué hay que construir

Un módulo nuevo `crates/vectorio-core/src/scene/` (con `mod.rs` y los ficheros que veas razonables, por ejemplo `camera.rs`, `solids.rs`, `hidden.rs`, `hatch.rs`, `shadow.rs`). Registra `pub mod scene;` en `lib.rs`.

### 1. Tipos básicos

- `V3 { x, y, z }` en **metros**, z hacia arriba, con las operaciones vectoriales mínimas (suma, resta, escala, producto escalar y vectorial, normalizar).
- `Camera`: ortográfica, definida por
  - `yaw_deg`: giro alrededor de la vertical,
  - `elev_deg`: inclinación hacia abajo desde el horizonte (35,264° es isométrica verdadera; 30° y 26,57° dan dimétricas habituales),
  - `scale`: milímetros de papel por metro,
  - un centro de la escena en metros y el centro de la hoja en mm.

  Métodos:
  - `project(V3) -> (P, f64)`: punto en la hoja (mm, y abajo) y profundidad en metros (mayor = más lejos).
  - `facing(normal) -> bool`: si una cara con esa normal exterior mira a la cámara.
  - `ground(P, z) -> V3`: el punto a la altura `z` que se ve en esa posición de la hoja. Servirá a los generadores para recortar el mundo a lo que se ve.
  - `fit(bbox_mundo, área_hoja, margen)`: elige escala y centro para que una caja del mundo quepa en un rectángulo de la hoja.
- Partes: cada línea pertenece a una **parte** con nombre (por ejemplo «contorno», «tejados», «sombra», «trama clara»). Usa un `PartId(u16)` y una tabla de nombres en la escena: `scene.part("tejados") -> PartId` devuelve el mismo id para el mismo nombre. Hay una parte activa (`scene.set_part(id)`) que se aplica a todo lo que se añade después. Esto es lo que más adelante se convertirá en tintas: el usuario podrá mover cada parte a otra tinta, así que no mezcles partes.

### 2. La escena

`Scene` guarda la cámara, el área de la hoja (para recortar), las caras que ocultan y las líneas que se dibujan. API mínima (los nombres son orientativos; elige los mejores y documéntalos):

- `line(&[V3])`: polilínea abierta que se dibuja donde nada la tape.
- `closed(&[V3])`: polilínea cerrada.
- `whole_line(&[V3])`: un detalle pequeño que se dibuja **entero o nada**. Si cualquier parte queda tapada o se sale de la hoja, se descarta, porque un detalle cortado queda feo.
- `occluder(&[V3])`: polígono plano y convexo que solo oculta, sin dibujar nada.
- `solid(vertices, caras, grupos_suavizado)`: sólido convexo cerrado a partir de caras dadas como índices (acepta cualquier sentido de giro; oriéntalas hacia fuera con el centroide). Cada cara oculta. Las aristas se dibujan así:
  - una arista se dibuja solo si **al menos una** de sus dos caras mira a la cámara;
  - si las dos caras comparten un grupo de suavizado mayor que 0 (facetas de una superficie curva), la arista solo se dibuja si es **silueta**, es decir, una cara mira a la cámara y la otra no;
  - cada arista se emite **una sola vez**, aunque la compartan dos caras. Las aristas duplicadas son el error más típico: el plotter las dibujaría dos veces.
  - opción `quiet` para que el sólido oculte sin dibujar aristas.
- Atajos construidos sobre `solid`:
  - `box3(min, max)` (caja alineada con los ejes) y una variante girada sobre la vertical;
  - `extrude(polígono_convexo, vector)`: prismas, muros, tejados a dos aguas;
  - `lathe(centro, perfil: &[(radio, altura)], suave: bool)`: sólido de revolución vertical (cilindros, conos, cúpulas, torres). Un radio 0 es un vértice. El número de segmentos depende del radio proyectado, de forma que el error de cuerda no pase de unos 0,04 mm en el papel. Con `suave = true` usa grupos de suavizado para que solo se vea la silueta; con `false` se ven todas las facetas (útil para árboles o sombrillas de estilo facetado). Con buenos segmentos, la silueta poligonal es suficiente en esta fase.
- **Detalle según la escala**, para que los generadores no dibujen lo que no se va a ver:
  - `paper_mm(metros) -> mm`;
  - `visible(metros, mínimo_mm) -> bool`, para saltarse detalles demasiado pequeños para el rotulador;
  - `circle_segments(radio_m) -> usize`.

### 3. Eliminación exacta de líneas ocultas

Este es el corazón del motor. No uses z-buffer ni rasterizado: en el plotter deja líneas cortadas a trozos. El método:

1. **Caras.** Cada cara se proyecta a la hoja. Guarda:
   - su polígono convexo en pantalla, como semiplanos;
   - su caja en la hoja;
   - su profundidad mínima;
   - su **plano de profundidad sobre la pantalla**: `profundidad = a·x + b·y + c`, sacado de tres vértices no alineados.

   Las caras de canto (área proyectada casi nula) no ocultan nada y se descartan.
2. **Índice espacial.** Una rejilla uniforme sobre la hoja (celdas de unos 4 mm) con la lista de caras que toca cada celda. Al consultar un segmento, recorre las celdas de su caja y evita revisar dos veces la misma cara (un sello por consulta en lugar de un `HashSet`).
3. **Por cada segmento** A→B de cada línea (con su profundidad en los extremos, que varía linealmente a lo largo del segmento):
   - descarta las caras cuya profundidad mínima no sea menor que la máxima del segmento menos la tolerancia;
   - recorta el segmento contra el polígono de la cara con **Cyrus-Beck**, lo que da un intervalo `[t0, t1]`;
   - dentro de ese intervalo, quédate solo con la parte donde la cara está **más cerca** que el segmento (la diferencia de profundidades es lineal en `t`, así que es otro corte);
   - acumula los intervalos ocultos, ordénalos y emite los huecos que quedan entre ellos;
   - une los tramos visibles consecutivos a través de los vértices de la polilínea, para que una línea sin interrupciones salga como **un solo trazo**.
4. **Tolerancias.** Hacen falta dos, explicadas en el código:
   - una de profundidad (del orden de 2 mm en metros): una cara tiene que estar al menos así de cerca para ocultar;
   - un encogimiento minúsculo del polígono de la cara en pantalla, para que una cara nunca oculte sus propias aristas ni las de su vecina.

   Las pruebas deben demostrar que con ellas no se pierden aristas legítimas y que no aparecen huecos falsos.
5. **Recorte a la hoja**, descartar tramos de menos de 0,05 mm y aplicar la regla de `whole_line`.

### 4. Tramas en caras y luz

- `hatch_face(&[V3] cara, dirección_mundo, separación_mm)`: líneas paralelas sobre una cara plana y convexa, **separadas lo indicado en el papel** sea cual sea el ángulo de la cara respecto a la cámara. Lo más sencillo: proyecta la cara a la hoja, trama el polígono 2D en el ángulo que da la proyección de `dirección_mundo` y recupera la profundidad de cada punto con el plano de profundidad de la cara. Las líneas de trama pasan por la eliminación de ocultas como cualquier otra, así que una cara tapada por otra pierde su trama. Solo se traman caras que miran a la cámara.
- Luz: `Scene` tiene un sol opcional (dirección). Un ayudante `tone(normal) -> Tone` clasifica cada cara en `Lit`, `Mid` o `Dark` según la luz, con umbrales configurables. Así un generador puede asignar la parte y la separación de la trama según el tono: arriba clara, un lado media, el otro oscura.

### 5. Sombras arrojadas

- `begin_shadows(z_suelo, recorte_mundo: Option<[x0, y0, x1, y1]>, ángulo_trama)`: a partir de aquí, cada sólido que se añada proyecta su sombra sobre el plano horizontal `z_suelo` siguiendo el sol. Para un sólido convexo, la sombra es la **envolvente convexa** de sus vértices proyectados.
- `hatch_shadows(separación_mm)`: rellena todas las sombras del grupo con líneas paralelas sobre el suelo. **Las sombras que se solapan se fusionan** para no dibujar dos veces: recorre las líneas de trama y, en cada una, une los intervalos que aporta cada sombra antes de emitir. Como son polígonos convexos, no hace falta ninguna operación general entre polígonos. Las líneas resultantes están en el plano del suelo y pasan por la eliminación de ocultas, de modo que los objetos de delante las tapan. Las líneas de distintos grupos con el mismo ángulo deben quedar alineadas, para que sombras vecinas se tramen como una sola.

### 6. Salida

`scene.render() -> SceneOutput`, con las polilíneas visibles **por parte**, en mm. Dentro de cada parte, conserva el orden en que se añadieron las líneas. No optimices el recorrido: eso ya lo hace `vectorio_core::optimize` (`order`, `join`) más adelante en la canalización. Incluye en `SceneOutput` un método para la longitud total de línea por parte, en metros. Sirve para avisar de dibujos que saturan el papel: el objetivo es de 1 a 40 m por hoja, y por encima de 80 m el papel sufre.

## Pruebas obligatorias

Pruebas unitarias en el propio módulo (`#[cfg(test)]`), como en el resto del núcleo:

1. **Cubo en isométrica:** exactamente 9 aristas visibles (el hexágono exterior y las 3 interiores), sin duplicados, con la longitud total esperada.
2. **Caja detrás de caja:** la arista tapada se corta en el punto exacto (compara con el valor calculado a mano, con tolerancia de 1e-6 mm).
3. **Sin auto-ocultación:** las aristas de un sólido y las tramas de sus caras no desaparecen por culpa de sus propias caras ni de las caras vecinas coplanarias (por ejemplo, dos cajas pegadas formando un muro).
4. **Cara de canto:** no oculta nada.
5. **`whole_line`:** se descarta entera si algo la tapa en parte y se conserva si está libre.
6. **Silueta de cilindro suave:** se ven las dos generatrices de la silueta y la tapa superior, pero ninguna arista entre facetas.
7. **Sombras solapadas:** dos cajas cuyas sombras se pisan producen líneas de trama sin tramos solapados ni duplicados.
8. **Ningún trazo duplicado:** una función de prueba que detecte segmentos colineales solapados en la salida, aplicada a una escena mixta.
9. **Determinismo:** la misma escena produce exactamente la misma salida dos veces.

**Medición de rendimiento** (prueba `#[ignore]` o dentro del ejemplo): una escena sintética de unas 5.000 caras y 20.000 segmentos debe resolverse en menos de 100 ms en `--release`. Indica el tiempo medido en tu resumen.

## Ejemplo visual

Crea `crates/vectorio-core/examples/muestra_escena.rs`, siguiendo el estilo de los otros ejemplos (`muestra_trazos.rs`: comentario de uso en la cabecera y ruta de salida como argumento):

```
cargo run -p vectorio-core --example muestra_escena --release -- muestra_escena.svg
```

Debe dibujar en A4 un **bodegón de pruebas**, no una escena artística:

- dos o tres cajas de alturas distintas, una tapando parcialmente a otra;
- una casa con tejado a dos aguas (caja y prisma);
- una torre cilíndrica con tejado cónico;
- una cúpula;
- un tramo de muralla con almenas (cajas pequeñas en fila);
- un par de detalles `whole_line` (ventanas), uno libre y otro tapado;
- tramas por tono en las caras;
- sombras arrojadas sobre el suelo.

Cada parte va en un grupo SVG con un color distinto. Si una variable de entorno o un argumento lo indica, que genere también una versión en dimétrica, para comprobar que nada depende de la isométrica exacta. Imprime por consola el tiempo y la longitud por parte.

## Comprobaciones antes de entregar

1. `cargo test --workspace`
2. `cargo clippy --workspace --all-targets` sin avisos nuevos.
3. Genera `muestra_escena.svg` y míralo: aristas limpias, nada visible que debería estar oculto, ninguna arista legítima perdida, tramas y sombras sin dobles.
4. `python3 scripts/install-macos.py` (lo exige `AGENTS.md` tras cambios de código, aunque esta fase no cambie la interfaz).

## Qué entregar

Un resumen breve en español con:

- los ficheros creados y la API pública final (firmas);
- las decisiones tomadas donde este documento deja margen, y por qué;
- los valores de tolerancia elegidos y cómo los validan las pruebas;
- el tiempo medido en la prueba de rendimiento;
- lo que haya quedado pendiente o que no te convenza.

Ese resumen, junto con el diff, es lo que se revisará.
