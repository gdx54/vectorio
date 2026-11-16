# VOLUMETRIC · auditoría y arquitectura

## Base observada (15 noviembre 2026)

Workspace Rust, núcleo puro `vectorio-core` y aplicación eframe/egui 0.29 sobre wgpu/Metal. `Document` almacena objetos editables separados de `Pen`; sus polilíneas y coordenadas físicas se miden en mm, Y abajo. `Document::layers` reúne por tinta, une y ordena recorridos. `svg_out` exporta SVG vectorial físico con capas Inkscape; `output` prepara rutas y esperas. No se necesita un controlador nuevo.

`scene` ya implementa sólidos procedurales, cámara ortográfica (mundo en metros, Z arriba), tramas, sombras, rejilla espacial y oclusión analítica de segmentos contra caras convexas proyectadas. Antes de esta entrega no disponía de un importador de mallas ni de secciones generales. `topo` interpreta fotografías mediante Fast Marching; no es topografía de superficies 3D. Los generadores arquitectónicos de `generate` funcionan, con sus propias recetas y trabajadores versionados. No se cambian sus algoritmos.

Antes de editar: rama inicial `feat/crear-motor-escena`, numerosos cambios locales y archivos nuevos; conservados. Se crea `codex/volumetric` sobre ese mismo estado, sin stash, reset ni commits ajenos. La batería inicial completa pasa (aplicación, núcleo y doctest; pruebas manuales/benchmarks ignorados). No hay migración de lenguaje ni cambios en LINEAR.

## Decisión

Biblioteca modular dentro de `vectorio-core::volumetric`, utilizable sin interfaz. Adaptador independiente en `vectorio-app::volumetric_ui`, accesible como cuarto espacio. Se reutilizan `scene::V3`, `Camera`, `Scene`, `Polyline`, `Item`, `Document`, las tintas y la salida existente. Se puede retirar la entrada de interfaz y el módulo sin cambiar los dibujos de otros motores. La simplificación de mallas grandes usa `meshopt` 0.6.2 (meshoptimizer nativo, sin servicio externo). Se activa `serde_json/float_roundtrip` para conservar exactamente las coordenadas f64 al abrir proyectos y recetas.

Contratos: modelo triangulado validado → transformación normalizada → campo escalar/planos → polilíneas 3D con nivel → cámara ortográfica → oclusión y recorte en mm → partes vectoriales por función → objetos del documento → capas por tinta y salida existente. La geometría no contiene colores. Las recetas conservan el modelo original y los parámetros, sin depender de la ruta del OBJ.

## Referencias investigadas

Se inspeccionan las implementaciones reales de [hidden-line.js](https://github.com/neurofuzzy/three-plotter-renderer/blob/develop/src/hidden-line.js) y [optimize.js](https://github.com/neurofuzzy/three-plotter-renderer/blob/develop/src/optimize.js) de three-plotter-renderer. El primero empareja posiciones, clasifica perfiles/ángulos, divide cruces y contiene alternativas de ray casting y profundidad GPU. Sus tolerancias de pantalla y clasificación parcial por puntos no se trasladan al SVG físico. El segundo elimina duplicados, une colineales y delega la ordenación; algunas pasadas comparan pares. Vectorio ya tiene rejilla y optimización propias. Se reutilizan estas últimas y se implementa Marching Triangles en Rust, sin copiar código JavaScript ni añadir Three.js.

[Licencia MIT](https://github.com/neurofuzzy/three-plotter-renderer/blob/develop/LICENSE), copyright 2020 Geoff Gaudreault: si en el futuro se copia código o una porción sustancial, deben conservarse copyright y permiso. Esta entrega consulta técnicas, no incorpora código del referente. La investigación de `perspective-hatch.js` no pudo completarse por fallo de acceso; no se presenta como verificada. Freestyle/Grease Pencil y vpype son referentes para fases futuras, no dependencias ni capacidades implementadas.

## Módulos y contratos implementados

- `mesh.rs`: malla indexada original, nombres de objetos y pertenencia por cara; validación y diagnóstico de fronteras/no manifold. Transformación centrada a extensión mayor de dos unidades; escala XYZ y rotaciones XYZ. Cubo/prisma, esfera, cilindro, cono, plano, toroide y composición de tres volúmenes.
- `import.rs`: OBJ UTF-8, índices positivos/negativos y caras `v/vt/vn`. Las normales se recalculan de la geometría; materiales y texturas se ignoran. Ear clipping de polígonos simples planos, hasta 256 vértices por cara. No se ejecutan instrucciones ni se abren recursos externos del OBJ.
- `stl.rs`: STL binario little-endian y ASCII estricto; detección por tamaño y contador binario, incluso con cabecera «solid». Normales y colores de atributo se ignoran, sin convertirlos en tintas. Varios sólidos ASCII conservan su pertenencia. Formatos incompletos, contadores inválidos y exceso de triángulos se rechazan antes de recorrer o reservar geometría.
- `contours.rs`: valores escalares por vértice y Marching Triangles. Altura direccional, profundidad ortográfica de cámara y distancia al centro (interpolación lineal). Curvas con identificador/valor de nivel. Regiones coplanarias conservan su frontera, no sus diagonales. Soldadura de extremos con distancia real `1e-7` en coordenadas normalizadas, consultando celdas vecinas. Grafos de grado dos forman ciclos; bifurcaciones terminan trazos, nunca se cierran artificialmente.
- `pipeline.rs`: caché del modelo transformado y de curvas; cámara y composición; adaptación a `Scene` y al documento. ATLAS separa principales/secundarias, STRATA desplaza cortes completos y X-RAY filtra diagonales mediante adyacencia y normales. Las curvas 3D se conservan antes de proyectar; simplificación final en mm. Las tintas se asignan después.
- `volumetric_ui.rs`, `volumetric_controls.rs`, `volumetric_preview.rs`: estado/trabajador, controles contextuales y dos vistas respectivamente. No almacenan algoritmos de secciones ni importación. Navegación propia con ⌘4; en ventanas estrechas el selector de espacios es desplegable. No cambia ⌘1–3.

La cámara guarda azimut/elevación y usa la base ortonormal existente, sin distorsiones 2D. Un encuadre esférico deja espacio a giros del dibujo; escala, posición y giro de composición se aplican antes del recorte de salida. Planta/alzado/perfil se aproximan a 0,01° de los extremos: el contrato existente de `Camera` excluye 0° y 90° y se conserva para no alterar los generadores. No hay perspectiva en esta fase.

El campo de profundidad se calcula geométricamente en coordenadas de cámara, sin rasterizar. En proyección ortográfica sus isolíneas son equivalentes a planos perpendiculares a la cámara; se mantienen como campo independiente de los planos definidos en el objeto. La distancia al centro es una aproximación lineal por vértices, no la intersección exacta con esferas: puede ser constante en mallas de vértices equidistantes. No se afirma que exista curvatura ni iluminación escalar.

## Visibilidad y papel

Se registran todos los triángulos utilizables como oclusores, incluyendo los que tienen normales invertidas. El motor existente recorta intervalos de cada segmento por el contorno proyectado de cada cara y su plano de profundidad; no decide la visibilidad solo por normales ni por un punto medio. La rejilla espacial reduce candidatos y funciona entre componentes distintos. Se conserva el sentido estable de los vértices en cortes/oclusores para no hacer depender el resultado del winding. No se incluye ningún raster en el SVG.

La malla normalizada se amplía internamente ×1000 y la escala de cámara se divide por ese factor al reutilizar `Scene`. Así su tolerancia arquitectónica de 0,002 m equivale aquí a `0,000002` unidades normalizadas, sin modificar la tolerancia de otros motores. Hay casos tangentes, caras casi alineadas con la cámara y superficies extremadamente finas que siguen dependiendo de tolerancias numéricas. STRATA muestra deliberadamente secciones completas y desactiva la oclusión del sólido original, que ya no coincide con los estratos desplazados.

Se recorta al rectángulo del papel menos márgenes antes de colocar/exportar; no se saturan coordenadas contra el borde (eso crearía líneas falsas). Se simplifica por RDP, por defecto 0,025 mm. El SVG conserva el ancho del rotulador asignado. La vista 2D usa 0,5 pt ≈ 0,1764 mm; no promete que ese sea el ancho físico de la herramienta. `Document::layers` conserva su unión/orden de rutas por tinta.

## Recursos, cancelación y cachés

OBJ y STL: 512 MB, 15 millones de vértices de entrada, 5 millones de triángulos; receta JSON: 512 MB. Hasta 160 cortes y dos millones de segmentos de sección; las acciones interactivas ofrecen 4–128 cortes. Un modelo que supera el límite se rechaza con un aviso de límite de recursos; no se descartan caras al azar ni se bloquea el hilo gráfico. La lectura usa `take(límite + 1)` antes de parsear.

Un trabajador persistente conserva los resultados intermedios; las importaciones se leen en otro hilo y nunca se repiten al cambiar tinta. Versiones atómicas cancelan cálculos superados, con consulta en triángulos, niveles, preparación de caras, trazos y eliminación de duplicados. La construcción de la rejilla y una llamada individual de simplificación no son interrumpibles, pero están limitadas por los presupuestos. Solo un resultado de la versión actual habilita colocar o exportar; el anterior se conserva atenuado mientras se calcula.

Cambiar cámara conserva cortes del objeto; invalida cortes del campo de profundidad. Cambiar composición conserva la malla y secciones. Cambiar tinta/revelado/vista dual no solicita cálculo geométrico. Cambiar el modelo o su transformación invalida malla derivada y curvas. El sólido 3D es una previsualización con pintado por profundidad, no un visor de edición de mallas con z-buffer; el SVG utiliza la oclusión analítica.

## Persistencia y compatibilidad

Las recetas `.json` versionadas incluyen malla, parámetros, papel y tintas separadas de la geometría. Al abrirlas, las tintas se reutilizan o añaden sin recolorear objetos anteriores. Colocar añade objetos de trazados editables a `Document`, compatibles con guardar, deshacer, SVG y Salida. El proyecto `.vectorio` conserva esos trazados; la fuente 3D se guarda mediante **Guardar receta…**, no como una nueva variante de `Geometry`. Esta decisión evita migrar el documento y tocar sus múltiples consumidores durante el MVP. La edición 3D enlazada desde una capa queda pendiente.


## Arrastre global y mallas grandes

Un OBJ o STL soltado en Diseño, Crear, Salida o VOLUMETRIC abre el módulo en **Solo 3D** y carga el modelo en segundo plano. Los documentos y las recetas de otros espacios se conservan. Un nuevo arrastre sustituye la importación pendiente, invalidando su respuesta y cancelando lectura/parseo/validación por bloques. Con varios modelos en un gesto se abre el último; otros tipos de archivo del gesto no tapan el modelo. Los diálogos modales conservan su bloqueo existente.

`mesh.rs` compacta los vértices coincidentes al validar, lo que evita conservar tres copias por triángulo de los STL. Los mapas de soldadura y diagnóstico usan hashing; no se depende de su orden de iteración para construir geometría. `reduce.rs` aplica simplificación por colapso de aristas con meshoptimizer, por objeto y conservando los vértices seleccionados del original. No se eliminan triángulos por muestreo aleatorio. La malla original limpia permanece en memoria y en la receta.

Tres calidades reproducibles: **Interactiva**, objetivo 40.000 triángulos/error relativo 0,001; **Detallada**, objetivo 160.000/error 0,0001; **Original**, todos los triángulos. El objetivo puede excederse cuando la topología o el error impiden reducir más. La calidad elegida se aplica a las secciones, la oclusión y el SVG: se informa expresamente en Modelo y también junto a Exportar (fuera del acordeón), con calidad, número original/de trabajo y alcance sobre SVG. No se presenta una simplificación como geometría original. El visor sólido tiene su propia malla de referencia de unas 20.000 caras, independiente del detalle de cálculo. El cálculo y la receta conservan la calidad elegida.

La validación original y la reducción se almacenan en caché por modelo/calidad. Cambiar cámara, color, escala de papel o número de cortes no vuelve a importar ni simplificar el modelo. La llamada nativa individual a meshoptimizer no es interrumpible; se comprueba cancelación antes y después y nunca se publica una respuesta obsoleta.

Referencias: [API y código de meshopt-rs](https://github.com/gwihlidal/meshopt-rs), [simplificación de meshoptimizer](https://github.com/zeux/meshoptimizer). meshopt-rs ofrece MIT/Apache-2.0; se utiliza MIT. El código nativo empaquetado tiene licencia MIT. Los avisos correspondientes se incluyen en `packaging/macos/VOLUMETRIC_NOTICES.txt` y se copian al bundle instalado.


## SOMBRA PLANA: iluminación y regiones de tinta

`volumetric/shadow` añade un camino alternativo a las secciones. `ShadowSettings`, incluido con valores por defecto en recetas antiguas, conserva luz direccional (dirección y altura de 5° a 85°), umbral, facetas objetivo (64–8.000), suelo, separación del suelo, segundo tono, contorno y trama en mm. La malla artística se obtiene por colapso de aristas de la malla de trabajo con error relativo máximo 0,04; el objetivo puede excederse por restricciones topológicas. El número real se muestra junto a Exportar y el original se mantiene. La vista 3D usa esta misma malla, para hacer coincidir facetas y sombras.

`shadow/project.rs` proyecta triángulos y construye un BVH 2D. La profundidad sobre cada cara es afín; recortar contra los tres semiplanos del oclusor y su plano de profundidad produce la región exacta que tapa. La sustracción convexa calcula visibilidad de cámara. El mismo procedimiento desde la luz calcula regiones de sombra arrojadas sobre caras iluminadas, también entre componentes separados. No se decide la sombra únicamente por normales ni por un punto central, ni se usa un mapa raster de profundidad.

Las normales se orientan coherentemente por adyacencia; en componentes cerradas se elige el exterior por volumen firmado. Las componentes abiertas conservan el lado inicial de sus caras y no se convierten automáticamente en sólidos. Las mallas no manifold o autointersectadas siguen sin reparación general. El umbral artístico añade sombra a la clasificación geométrica por orientación. El segundo tono opcional corresponde a una franja de coseno de 0,28 sobre el umbral.

`shadow/mod.rs` calcula las sombras del suelo sobre un plano horizontal bajo todo el modelo. En la vista final se eliminan las partes del suelo tapadas por la silueta completa del objeto. El encuadre incluye objeto y sombra. `shadow/mask.rs` une los intervalos de todas las sombras, resta el suelo oculto y separa el tono intermedio. La trama tiene fase global en mm: dos facetas contiguas o la sombra propia y la del suelo forman un mismo segmento cuando se tocan. No se repasan sus juntas. Se recorta al margen físico; la parte de sombra utiliza una sola tinta por defecto. El contorno opcional reutiliza la visibilidad de segmentos de `Scene` y omite las aristas internas.

La vista de papel dibuja las masas con polígonos convexos, sin bordes de triangulación. «Ver trazos del plotter» muestra las trayectorias efectivas; exportar o colocar siempre usa esas trayectorias, nunca un relleno raster. La separación predeterminada de 0,22 mm se puede ajustar al ancho del rotulador. Las asignaciones de sombra, tono y contorno son funciones independientes de las tintas del documento.

El campo de luz se almacena por identidad de malla, luz, umbral y segundo tono. Cambiar cámara, posición de papel o trama conserva ese campo; cambiar tinta o alternar masas/trazos no lanza trabajo geométrico. Se conserva el trabajador versionado/cancelable. Presupuestos defensivos: 512 fragmentos por cara en recorte, 250.000 regiones o trazos; el exceso devuelve error explícito, sin suprimir geometría silenciosamente. La vista 3D sigue usando orden de profundidad como referencia; el papel y el SVG usan recorte analítico.


## ONDAS DE SUPERFICIE, MALLA CROMÁTICA y ONDAS + MALLA

El módulo `volumetric/art` comparte importación, simplificación, cámara ortográfica, recorte, `Scene`, documentos y tintas. Los nuevos presets de `RecipeKind` usan `ArtSettings` con valores por defecto para recetas antiguas: objetivo de 64–20.000 triángulos (2.700 iniciales), 4–160 niveles, patrón, origen angular, deformación, frecuencia, criterio cromático, una/tres familias de malla y contorno opcional. La reducción artística usa la malla de trabajo con error relativo 0,04; el objetivo puede excederse por topología/error. Se informa el número real y su alcance sobre SVG junto a las acciones. La fuente completa no se modifica.

`art/field.rs` forma el grafo de aristas únicas, con longitudes reales. Cada componente conexa recibe una semilla en el vértice extremo según azimut/elevación. Dijkstra calcula la distancia a lo largo de la superficie triangulada; es una aproximación por aristas, no una geodésica exacta ni un campo de curvatura. Interferencias resta las distancias de los extremos opuestos. La deformación suma armónicos espaciales con fases derivadas de la semilla mediante hash estable; altera el campo escalar y nunca desplaza puntos fuera de la superficie. `isolines` ensambla las curvas sobre triángulos y conserva componentes separadas y agujeros. Un nivel puede producir varias curvas.

`art/edges.rs` emite cada par de vértices compartido una sola vez, incluyendo diagonales de triangulación. La orientación usa el valor absoluto de normales adyacentes, sin depender del winding. Altura/profundidad clasifican el punto medio de cada arista en bandas; profundidad se recalcula al orbitar. Objetos utiliza la pertenencia importada y repite la paleta entre objetos si hay más que tintas. Una arista compartida se asigna a una sola familia, conservando el presupuesto físico de una pasada.

`art/mod.rs` mantiene el grafo, campo, curvas y aristas en caché por identidad de malla y parámetros pertinentes. Cambiar cámara, composición o color no recalcula distancias ni curvas; profundidad de cámara sí reclasifica colores. Cambiar densidad recalcula niveles, cambiar semilla/deformación/origen recalcula el campo y cambiar objetivo de malla invalida sus derivados. Se consulta cancelación en grafos, búsqueda, niveles, preparación de caras, aristas y renderizado; se mantiene el descarte versionado del trabajador.

En combinado, parte 0 es ondas y partes 1/2 son la malla. En malla, las tres partes son familias cromáticas; en ondas son principales, secundarias y silueta. Estos papeles se traducen a tintas del documento después del cálculo. La interfaz permite elegir las familias activas y oculta las asignaciones que no se utilizan. Las recetas conservan `ArtSettings`, semilla, tintas y fuente, con compatibilidad por valores por defecto. La salida sigue siendo trazados editables en Diseño y trayectorias de Salida, no objetos 3D enlazados nuevos.

La vista 3D añade curvas/aristas a la referencia completa; la vista 2D y SVG eliminan partes ocultas con la propia malla dibujada. Usar como oclusor la fuente detallada produciría falsas ocultaciones de cuerdas simplificadas: no se hace. Los tres modos conservan márgenes físicos, rotación, zoom y posición, sin generar imágenes raster incrustadas. No se añaden dependencias externas.

## Actualización · Fase G (16 noviembre 2026)

Las fases A–D sustituyen los modos excluyentes por una pila de pasadas y conservan la interfaz de escena/papel/pasadas. La fase G añade `volumetric/reading/` para medidas, rampas y efectos de lectura, y `volumetric_reading.rs` para su editor. `DrawingPart` mantiene una parte por pasada, con familias de tinta que el documento reúne por rotulador. La receta sigue en versión 2 con nuevos campos opcionales y conserva la migración desde v1. El detalle vigente, las comprobaciones y las limitaciones se describen en [docs/volumen-fase-g.md](docs/volumen-fase-g.md).
