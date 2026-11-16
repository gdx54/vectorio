# Plan: panel «Crear», fase 5 (escenas de alta densidad)

Objetivo: dos escenas nuevas que nazcan con la riqueza de las ilustraciones de
referencia (ciudad isométrica, festival, cúpulas bajo el mar y en el espacio) y no
la ganen a base de retoques como el Pueblo blanco.

**Regla de origen.** Son ilustraciones con derechos de autor (y con marcas
reales). Sirven solo de análisis: no se copian composiciones, personajes, rótulos,
logotipos ni marcas (nada de Royal Enfield, Toyota, etc.). Las escenas son
originales, con temas y rótulos propios. Mismo criterio que con SVGeometric.

## 1. Qué lleva esa riqueza (análisis)

1. **Tres escalas a la vez.** Un hito enorme (calamar, escenario, torre de cristal,
   cohete, circuito), bloques medios (edificios, cúpulas, tribunas) y un polvo de
   detalle (personas, coches, motos, globos, banderas, árboles). Quita una escala y
   la imagen se vacía.
2. **Gente por cientos, y legible.** La densidad no viene de dibujar mucho por
   persona sino de muchas personas muy simples, en grupos con intención: una cola,
   un paso de cebra, una grada, un corrillo, un baile. Cada grupo cuenta algo.
3. **Cada superficie hace algo.** Azoteas con terraza, balcones con gente,
   fachadas con escaparate, toldo, rótulo y ventanas con vida. No hay tejado liso
   ni suelo vacío: el vacío es una decisión (una plaza), no un olvido.
4. **Escenas dentro de la escena.** Las cúpulas de cristal enseñan un interior
   completo (un pueblo, una cancha, un jardín). Es el truco que multiplica la
   densidad sin llenar la hoja: un volumen simple con un mundo dentro.
5. **Distritos temáticos.** Cada zona del plano tiene su gramática (puerto, parque,
   calle comercial, circuito). La variedad entre zonas da lectura; la repetición
   dentro de una zona da orden.
6. **Gran forma que cruza capas.** Un animal gigante o una vía que atraviesa
   cúpulas, agua y cielo une la composición y marca el recorrido del ojo.
7. **El fondo también está lleno.** Mar, cielo, estrellas, campo: nunca papel en
   blanco en los bordes. Todo llega a sangre.
8. **Color como jerarquía.** Pocas familias de color por zona y un acento vivo. En
   el plotter eso son tintas: no hace falta gama, hace falta reparto.

## 2. Qué hay que añadir al motor y al kit (infraestructura común)

Se hace primero, porque las dos escenas dependen de ella.

- **Multitud (`kit::crowd`).** Figura mínima de 5–8 trazos más cabeza, con 4–6
  variantes de pose y altura, dibujada como sprite con oclusión (no como sólido de
  revolución). Generadores de grupo: `queue`, `crossing`, `stands` (rejilla por
  gradas), `cluster`, `dance`. Presupuesto: 1.000–3.000 personas por hoja.
- **Vehículos (`kit::vehicle`).** Coche, moto, bus, barca, tren: cajas con perfil
  extruido y ruedas. Colocación sobre una polilínea (calle, circuito, vía) con
  separación y sentido.
- **Gramática de fachada (`kit::shopfront`, `kit::tower`).** Rejilla de ventanas
  con ritmos, bajos con escaparate y toldo, balcones con gente, rótulos y torres
  de cristal con plantas visibles. Reemplaza el `facade()` actual en las escenas
  densas.
- **Cinta curva (`kit::ribbon`).** Calles, circuitos y raíles a partir de una
  polilínea suave: banda con borde, bordillo, marcas y peralte. Es lo que hoy no
  puede hacer el pueblo (solo trabaja en ejes).
- **Cúpula (`kit::dome`).** Casquete en alambre sin ocluir el interior (se dibuja
  con `whole_line`), con un contenido que se recibe como cierre: la cúpula
  recorta el mundo interior y dibuja su cubierta por encima.
- **Criaturas y grandes formas (`kit::creature`).** Cuerpo de revolución sobre un
  eje curvo, con aletas o extremidades; tres especies al principio (cetáceo,
  cefalópodo, saurio). También cohete y estación.
- **Rotulación.** Los rótulos usan el texto vectorial que ya existe en el núcleo,
  proyectado sobre un plano de fachada.
- **Presupuesto y medida.** `Scene` hasta 8.000 caras y 40.000 segmentos (el banco
  actual mide 12 ms con 5.000 y 20.000). Se añade una métrica de densidad por
  escena: personas, metros trazados y metros por decímetro cuadrado.
- **Tintas.** Los juegos pasan a 5–7 rotuladores: contorno, trama, vegetación,
  terreno, agua, gente y rótulos. Cada parte conserva su tinta; con menos
  rotuladores se agrupan como ahora.

## 3. Escena A · «Ciudad en fiesta»

Isométrica, a sangre, con barrios y un circuito que la cose.

- **Plano.** Cuadrícula de calles con una avenida diagonal; cuatro o cinco
  distritos sorteados: comercial, residencial, parque, puerto o playa, y
  recinto de espectáculo con escenario y gradas.
- **Hitos.** Una torre de cristal con plantas visibles, un escenario con gradas
  llenas y una noria, un depósito circular o un faro. Dos o tres por hoja, nunca
  más.
- **Circuito.** Una cinta curva que sube y baja sobre los bloques, con coches,
  neumáticos en las curvas y gradas.
- **Micro.** Pasos de cebra con multitud, colas, toldos, terrazas en azoteas,
  globos, banderas y árboles de tres especies.
- **Parámetros.** Distrito dominante, densidad de gente, número de hitos, presencia
  de circuito, de playa, de parque; giro del sol y vista.
- **Cómo se lee con una sola tinta.** Blanco en las fachadas de la calle (la cal
  de la fase 4), trama abierta en los costados, gente y coches en trazo fino.

## 4. Escena B · «Cúpulas»

Corte vertical con una línea de agua (o un horizonte espacial) y burbujas.

- **Mundos.** Subacuático (agua, fondo marino, cetáceo, saurio, barco arriba) y
  espacial (luna, planetas, cohete, estación, galaxia). Mismo motor con otro
  decorado y otro juego de tintas.
- **Cúpulas.** Entre cinco y nueve, de tamaños distintos, unidas por túneles. Cada
  una contiene una microescena sorteada: jardín, pueblo, cancha, mercado, granja,
  laboratorio.
- **Criatura que cruza.** Una gran forma (calamar, ballena, cohete) pasa entre
  cúpulas y las une compositivamente.
- **Fondo lleno.** Burbujas, peces o estrellas por campos de densidad, algas o
  nubes de polvo, siempre a sangre.
- **Parámetros.** Mundo, número de cúpulas, criatura, densidad de fondo, vista.

## 5. Pasos

0. **Infraestructura** (la parte pesada): `crowd`, `vehicle`, `ribbon`, `dome`,
   `creature`, fachada nueva, presupuesto y métrica de densidad, pruebas de cada
   pieza (sin costuras, sin recortes, deterministas, sin NaN).
   *Hecho.* `kit::crowd` (6 poses, colas, pasos, gradas, corrillos, baile y
   explanada; tope de 3.000 figuras, simplificadas por debajo de 2,6 mm),
   `kit::ribbon` (cinta con bordes, discontinua, pianos, vía y tablero elevado),
   `kit::vehicle` (coche, moto, autobús, barca y tren enganchado), fachada
   (`window_grid` con 4 ritmos, balcones con gente, escaparates con toldo,
   rótulos con la letra del núcleo, torre de cristal, azotea con sombrillas),
   `kit::dome` (vidrio que no tapa e interior recortado a la esfera con
   `Scene::clip_to_sphere`), `kit::creature` (cetáceo, cefalópodo, saurio,
   cohete y estación), `Scene::load` con el presupuesto de 8.000 caras y
   40.000 segmentos, `kit::density` (personas, metros y metros por dm²) y el
   juego de tintas `inks::DENSE` de 5 a 7 rotuladores (línea, gente,
   vegetación, agua, trama, rótulos, terreno). Pruebas en
   `kit/dense_tests.rs`: deterministas, sin NaN, nada con entradas imposibles,
   sin costuras ni caras solapadas, interior de la cúpula dentro del vidrio,
   vista previa que aclara sin mover a nadie, 3.000 personas dentro del
   presupuesto (39.652 segmentos, 15 ms) y lámina en el tope en 20 ms.
   Muestrario: `cargo run -p vectorio-core --example kit_denso --release`.
1. **Ciudad en fiesta** completa, con las 12 semillas valoradas una a una.
   *Hecho (versión acotada).* Generador `festival_city` registrado, con juego
   `inks::DENSE`, controles de gente, 1–3 hitos, circuito, vista y giro del sol.
   Incluye un núcleo de nueve manzanas (una reservada para la plaza), periferia, comercios y azoteas,
   cruces con peatones, colas, coches, motos y autobuses; torre de cristal,
   escenario con dos gradas y depósito circular, sorteados sin repetirse, y
   carrera urbana por las calles, con vallas, público, grada y meta. La ciudad sigue a sangre.
   Se comprueban determinismo, geometría finita dentro del papel, ausencia de
   avisos y presupuesto en las semillas 0–11, con los tres recuentos de hitos y
   ambos estados del circuito; también las cuatro vistas y los extremos de gente.
   *Mejora:* manzanas de fachada continua (una o dos filas con patio), hitos altos
   detrás de la plaza y escenario delante, plaza con fuente, corros, olivos y verbena
   (farolas y banderines que cruzan las calles), escaparates con rótulos variados,
   paseantes y tráfico por toda la ciudad, y acabado por presupuesto (completo,
   comercios, volumen o caja) para no pasar nunca de los topes.
   Hoja de contactos en `target/phase5/contactos.svg` y detalle de ciudad en
   `target/phase5/ciudad.svg`.
   Quedan pendientes la noria, el faro, la playa y el puerto, el parque con tres
   especies, globos y banderas, la avenida diagonal, el distrito dominante y la
   valoración detallada de cada semilla. Esta entrega cubre la revisión visual
   de errores evidentes, no esa valoración artística completa.
   Corrección 1: ciudad a sangre y carrera urbana; cámara a 2,4–4 mm/m,
   cobertura del suelo verificada y detalle de periferia ajustado al presupuesto.
   Revisadas las doce láminas individuales: edificios hasta el borde y escaparates
   visibles. Con este encuadre cercano la carrera periférica se ve por fragmentos
   en los bordes; no se presenta el rectángulo completo en una sola vista.
   Puerto o paseo marítimo, avenida diagonal, noria, parques y globos quedan
   para el siguiente paso.
   Corrección 2: sombras proyectadas como en el pueblo (control «Densidad de
   sombras») y control «Evento» para la plaza: verbena; mercadillo con doce
   puestos con toldo y rótulo; feria con noria, tiovivo, casetas y cola; y
   cabalgata por la calle de la plaza más cercana a la cámara, con carroza,
   gigantes, cabezudos, banda y público (ese tramo se queda sin tráfico). Nuevo
   control «Vida en la calle»: obra con estructura a medias y grúa torre (solo
   donde la grúa cabe en la hoja y sin tapar la plaza de frente), globo
   aerostático, bandadas de palomas, vendedores de globos, tendederos con vecina
   en las azoteas, pintores en andamios colgados y bancos con gente sentada al
   pie de los olivos.
   Cruce con el pueblo: el pueblo usa ya las figuras de la ciudad (con postura,
   niños y sombra gracias a `Scene::cast_shadow`), y un vendedor de globos y una
   bandada de palomas en la plaza; palomas, vendedor y figura con sombra viven
   ahora en `kit::life`, compartidos por las dos escenas.
   Lámina suelta con parámetros a mano: `cargo run -p vectorio-core --example
   muestra_receta --release -- festival_city ciudad.svg 3 event=fair stories=1`.
   Corrección 3: control «Costa» (sin costa, playa o puerto) en uno de los lados
   delanteros. Playa con paseo de palmeras, sombrillas, bañistas, socorrista,
   vóley y barcas; puerto con muelle, lonja, espigón de escollera con faro y
   pantalán con barcas. Control «Avenida diagonal» (no con la carrera): cruza la
   cuadrícula delante de la plaza (detrás si hay costa), con mediana de árboles
   y farolas, cuatro carriles, edificios en cuña con proa achaflanada (bajos los
   que quedan entre la avenida y la cámara) y un parque con fuente, cipreses en
   el borde, pinos al fondo y olivos delante. La cámara se corre hacia el mar y
   hacia la avenida para que entren en la hoja. Cruce con el pueblo: pinares
   ralos entre los cultivos y una pareja de palmeras a la entrada de la plaza.
   Con parámetros a mano los interruptores van como `si`/`no`:
   `... festival_city avenida.svg 4 avenue=si circuit=no coast=port`.
   Corrección 4: mediana continua entre las llegadas de las calles, con pasos
   de cebra y peatones en cada llegada; proas con chaflán de tres metros y
   escaparate. Con costa y avenida, desplazamiento conjunto y un 10 % más de
   campo para mostrar el agua y la mediana a la vez. Los tendederos respetan
   ahora el grosor del pretil a esa escala y no se duplican en las azoteas.
   «Distrito» sortea Comercial, Residencial u Ocio: cambian las proporciones
   de alturas, balcones, comercios, patios, tendederos y terrazas de la periferia;
   el núcleo conserva su organización y Ocio favorece el escenario. Banderolas
   en farolas, banderas en azoteas y faro, y racimos atados a puestos y plaza.
   Los nuevos adornos son trazos; comparten el dibujo de globos del vendedor
   sin añadir oclusores. Pruebas de vegetación visible junto al eje en las cuatro
   vistas, y barridos de los tres distritos con costa y avenida.

   Crear: panel flotante Liquid Glass de 360 puntos, sin divisor redimensionable;
   selector compacto y galería desplegable, leyenda de tintas sin recortes laterales.
   Zoom con pellizco y botones, desplazamiento con trackpad y arrastre; doble clic
   o «Ajustar» recuperan la hoja. La cámara de vista previa no altera la receta.
   Capturas de egui fuera de pantalla en `target/phase5/crear-{980,1440}.png`,
   sin cerrar ni reiniciar la sesión abierta. El material usa la implementación
   de vidrio de `theme.rs`, incluido su fondo translúcido y reflejos.

   Revisión visual de 24 láminas (`target/phase5/revision4`), con `--random` y
   vista fijada. No se aprecian personas flotantes sobre edificios ni textos
   superpuestos en esta revisión. Los recortes de edificios en los bordes son
   intencionados; no se han retirado para simular una maqueta.

   | Vista / semilla | Observación |
   |---|---|
   | 225 / 0 | Playa y mercadillo legibles; plaza visible, sin asfalto vacío. |
   | 225 / 1 | Cabalgata y avenida visibles; proas todavía discretas de frente. |
   | 225 / 2 | Escenario y plaza se separan bien; costa sin elementos flotantes. |
   | 225 / 3 | Noria y carrusel ocultan parte del pavimento, propio del evento. |
   | 225 / 4 | Puerto y faro enteros en el papel; lonja y escenario legibles. |
   | 225 / 5 | Noria parcialmente cortada por el encuadre, sin corte estructural nuevo. |
   | 225 / 6 | Obra y grúa a un lado de la plaza; edificio de obra se lee abierto. |
   | 225 / 7 | Noria y parque compiten por atención; queda pendiente afinado de densidad. |
   | 225 / 8 | Copas de la mediana tapan parte de un paso; troncos fuera del cruce. |
   | 225 / 9 | Puerto abierto y escenario claro; sin huecos de ciudad en el borde. |
   | 225 / 10 | Grúa recortada por arriba, a sangre; plaza reconocible a su lado. |
   | 225 / 11 | Feria densa, con noria y escenario; rótulos separados. |
   | 315 / 0 | Obra y playa visibles; plaza desplazada, sin taparla. |
   | 315 / 1 | Avenida y cabalgata claras; proas mejoradas pero sobrias en esta vista. |
   | 315 / 2 | Escenario y playa despejados; plaza abierta. |
   | 315 / 3 | Feria densa y depósito recortado arriba; plaza cubierta por atracciones. |
   | 45 / 0 | Playa y escenario completos; edificios cortados en el borde. |
   | 45 / 1 | Mediana continua y parque visibles; copa grande reduce la lectura del paso. |
   | 45 / 2 | Obra cerca de la plaza, que queda libre; grúa en el borde derecho. |
   | 45 / 3 | Noria tapa parte de la feria; no hay cuerpos flotando sobre las casas. |
   | 135 / 0 | Obra y playa ordenadas; el borde corta casas, no calles desiertas. |
   | 135 / 1 | Mediana y parque claros; cruce con público visible. |
   | 135 / 2 | Plaza y escenario abiertos; obra ocupa el otro costado. |
   | 135 / 3 | Noria domina el evento; rótulos y público legibles alrededor. |

   Pulido final de ciudad: los árboles de mediana guardan 5,5 m con los cruces
   para dejar espacio a la copa; las copas del parque también se comprueban
   proyectadas en papel para no tapar los pasos. Con feria, el parque espacia
   su arbolado y baja
   pinos y olivos para que no compitan con la noria. Pequeños aparcabicis de dos
   bicicletas en algunas aceras del núcleo, ligados a «Vida en la calle», sin
   oclusores ni consumo de aleatoriedad adicional. Revisadas las cuatro vistas
   de feria con avenida y una vista con bicicletas.

2. **Cúpulas — implementado**, con los dos mundos y las referencias visuales
   aportadas el 14 de noviembre. Jerarquía de cinco a nueve cúpulas, conectadas
   por un árbol de pasarelas y con seis interiores: pueblo con torre, jardín,
   cancha, mercado, huerto y laboratorio. Vidrio en tinta de agua y contorno
   oscuro; el interior se recorta a la esfera y la mitad trasera oculta el
   decorado. Un cetáceo, calamar o reptil marino cruza el fondo submarino; un
   cohete, estación o ballena estelar, el espacial. Ambos pueden desactivarse.
   Agua con superficie, barca, bancos de peces, burbujas, coral, rocas, buzos y
   pequeño submarino; espacio con planetas, galaxia, estrellas, horizonte lunar,
   cráteres, estación pequeña y astronautas. Controles de gente, fondo, número
   de cúpulas y cuatro vistas. Juegos Abismo, Órbita y Bolígrafo, con reducción
   de tintas según el número de rotuladores. Las referencias se adaptan a trazos
   para plotter, sin copiar sus marcas ni personajes.
3. **Comprobaciones y entrega — completado:** 500 semillas por escena sin NaN
   ni coordenadas fuera de la hoja,
   tiempos por debajo de 250 ms en Preview y de 1,5 s en Final, clippy, ejemplos,
   instalación, hojas de contactos y `docs/crear-fase5-entrega.md`. Barrido
   adicional de Cúpulas en Preview y Final (máximos 13,50 y 13,15 ms); 24 láminas
   revisadas de los dos mundos y panel egui comprobado en dos tamaños. Detalle
   de las pruebas, avisos previos de clippy y archivos en la entrega.

Se hace «de una» en este orden: primero la gente y la cinta (lo que más
densidad aporta), y la ciudad con ellas, antes de abrir la segunda escena.

## 6. Riesgos y qué no prometo

- **Tiempo de plotter.** Una hoja con 2.000 personas y fachadas ricas puede pasar
  de 300 m de trazo. Hay que dar un control de densidad, y que Preview dibuje la
  mitad.
- **Líneas finas.** Una persona a 3 mm solo se lee con rotulador de 0,2–0,3 mm.
  Se fija el tamaño mínimo por rotulador, como ya hace `Style::quantum`.
- **Las cúpulas con interior** necesitan que el interior sea un mundo recortado;
  es lo más delicado del motor y puede costar más que todo lo demás junto.
- **Textura vs. limpieza.** Más densidad no es mejor: si la multitud no tiene
  agrupación intencionada, se lee como ruido. La valoración semilla a semilla
  vuelve a ser parte de la entrega.
