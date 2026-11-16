# VOLUMETRIC · verificación

## Comandos

```sh
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cargo test --workspace
cargo test -p vectorio-core volumetric
cargo test -p vectorio-app volumetric_ui
cargo run -p vectorio-core --release --example volumetric -- target/volumetric
# Con un archivo real exportado por Blender u otro modelador:
cargo run -p vectorio-core --release --example volumetric -- target/volumetric modelo.stl
python3 scripts/install-macos.py
```

El ejemplo genera ATLAS, STRATA y X-RAY como SVG físicos y recetas JSON sin interfaz ni plotter. Las pruebas son locales y no llaman a servicios externos.

## Casos automáticos

- Cubo: aristas sin diagonales, varias cámaras, cortes horizontales/verticales/oblicuos, cortes coincidentes con caras coplanarias y proyección ortográfica invariante a profundidad.
- Esfera: continuidad/cierre en tres orientaciones, densidad; longitud visible menor que completa; resultado invariante al invertir todas las caras.
- Toroide: dos anillos horizontales, secciones verticales con dos componentes, transición a una componente y agujero conservado.
- OBJ: polígono cóncavo, índices relativos, objetos, caras degeneradas y repetidas; errores con NaN, índices ausentes/cero. Malla abierta, desconectada, vértices duplicados y aristas no manifold.
- STL: fixtures de disco ASCII y binario, binario con cabecera «solid», contador de caras/tamaño, truncamiento, NaN, límite de triángulos y varios sólidos ASCII. Importación real asíncrona y fallo que conserva el modelo previo.
- Siete primitivas: validación y límites de tamaño.
- SVG: parseo con usvg, unidades físicas 420 × 297 mm, capas Inkscape, sin imágenes incrustadas ni NaN/inf; todas las trayectorias dentro del margen incluso con zoom, desplazamiento y giro; salida determinista en los tres presets.
- Receta: igualdad exacta de f64, polilíneas y parámetros tras guardar/reabrir una malla curva; serialización/deserialización, rechazo de versiones o parámetros inválidos y recursos acotados.
- Cachés: órbita/composición conservan malla y curvas del objeto; transformar el modelo o cambiar el campo invalida lo necesario. Cambio de tinta no cambia la versión geométrica.
- Trabajo asíncrono: cancelación del núcleo, trabajador que conserva solo el pedido más reciente y descarte de respuestas obsoletas.
- Interfaz: «Según cámara» usa los ángulos actuales sin esperar una respuesta anterior; selector distingue modelos importados y permite volver a la primitiva previa; errores contrastan al menos 4,5:1 en panel claro y oscuro. Acciones accesibles a 980 × 620 y 1440 × 900, español e inglés; regresión completa de funciones anteriores mediante la batería del workspace.

## Alcance de la evidencia

Las tolerancias son geométricas, no comparaciones de píxeles. Los triángulos con winding invertido no cambian la oclusión de secciones; normales incoherentes siguen siendo un límite de la clasificación de siluetas X-RAY. Las pruebas no aseguran reparación de mallas arbitrarias, ausencia de autointersecciones en modelos defectuosos, perspectiva, rendimiento GPU ni exactitud del campo de distancia dentro de cada cara.

Los benchmarks y capturas preexistentes que están marcados `ignored` no se consideran ejecutados por `cargo test --workspace`. Las verificaciones visuales y tiempos medidos de esta entrega se anotan al final de este documento.

## Comprobaciones de la entrega (15 noviembre 2026)

- Batería final: 66 pruebas de app, 216 de núcleo y 1 doctest (283 en total); 9 ignoradas no forman parte de ese total. Incluye las dos pruebas añadidas tras revisión independiente.
- Captura manual ejecutada: `cargo test -p vectorio-app render_volumetric_workspace -- --ignored`. Resultados en `target/volumetric/workspace-980.png` y `workspace-1440.png`; capturas adicionales de error oscuro y modelo importado para revisar correcciones.
- Ejemplo release, toroide de 3.072 triángulos, 48 niveles / 96 curvas: ATLAS 22,0 ms, STRATA 4,5 ms y X-RAY 3,7 ms en esta máquina, caché compartida en las dos últimas llamadas. Es una medición de ese ejemplo, no una promesa para cualquier importación.
- Instalación de `/Applications/Vectorio.app` compilada en release y firma ad hoc verificada por el instalador. No se cerraron sesiones del usuario ni se enviaron movimientos al plotter.
- Comprobación con CUA en la instalación: abrir VOLUMETRIC, importar `cube-binary.stl`, cambiar eje Z→X y densidad 48→24; ver el cambio; colocar las dos partes en Diseño y obtener en Salida una ruta de 5,3 m, dos tintas y unos tres minutos estimados. La ejecución física no se realiza.
- Se intentó abrir Illustrator para comprobar compatibilidad externa, pero la aplicación no llegó a abrirse y la llamada agotó su tiempo. La compatibilidad externa queda sin verificar; los SVG sí pasan parseo y controles vectoriales de la batería.

- SVG exportado desde el diálogo de la app: `target/volumetric/stl-cube-ui.svg`, XML leído después de guardar: 420 × 297 mm, 24 trazados, 2 capas y 0 imágenes raster. El nombre introducido por automatización quedó inicialmente concatenado al sugerido; se renombró el artefacto de prueba en disco.
- La revisión independiente dio `ship` en su pase de los tres hallazgos: contraste de error, selector tras importar y dirección de cámara actual. Las capturas y pruebas respaldan esas correcciones; no se interpreta como certificación de capacidades futuras.


## Arrastre y ampliación de modelos

- Arrastre probado mediante eventos reales `egui::RawInput::dropped_files` para OBJ y STL (también mayúsculas) desde Diseño, Crear, Salida y VOLUMETRIC. Se comprueba acceso a Solo3D, carga de la malla, cierre de la importación y conservación íntegra del documento. Una segunda importación sustituye la anterior y no recibe su error obsoleto.
- Calidad de malla: esfera subdividida de más de 40.000 triángulos; reducción geométrica, contornos cerrados, vuelta a todos los triángulos con Original, proxy de pantalla independiente y receta que conserva la fuente completa. Validación y lectura cancelables probadas.
- Límites actuales: 512 MB por archivo/receta, 5 millones de caras y 15 millones de vértices de entrada (STL sin indexar). Los vértices coincidentes se compactan. Los avisos de 16 MB/8.000 polígonos ya no son los límites vigentes.
- Medición release con dos STL locales del usuario, leídos sin modificarlos: 1.499.605 triángulos / 71,5 MiB → carga/validación 1,19 s, ATLAS 2,03 s y 40.000 caras de trabajo; 209.872 triángulos / 10 MiB → carga 0,11 s, ATLAS 0,15 s y 39.998 caras de trabajo. Se generan SVG con los tres presets en `target/volumetric-large` y `target/volumetric-calpon`; no se duplican recetas grandes para esta comprobación.
- Capturas del modelo grande y del acceso a Solo3D: `target/volumetric/large-1440.png` y `drop-980.png`. Generación reproducible: `VECTORIO_QA_MODEL=/ruta/modelo.stl cargo test -p vectorio-app render_volumetric_workspace -- --ignored`. No toca ventanas de trabajo.
- Batería de esta ampliación: 68 pruebas de aplicación + 218 de núcleo + 1 doctest = 287 pasan; nueve ignoradas. La captura manual ignorada sí se ejecuta aparte.
- No se ha medido el extremo de cinco millones de caras en Original. Esa opción conserva geometría completa y puede tardar sustancialmente más que la calidad interactiva. La llamada individual de simplificación nativa no se interrumpe internamente; su resultado se descarta si la petición quedó obsoleta.

- Revisión de esta ampliación: aviso permanente de calidad/reducción junto a las acciones cuando corresponde, comprobado en 980 × 620 y 1440 × 900. La revisión independiente da `ship` en su pase de ese hallazgo.


## SOMBRA PLANA · entrega de iluminación

- Siete pruebas específicas del núcleo: recorte de oclusión total/parcial; una cara orientada hacia la luz que recibe sombra de otra componente; sombra propia y suelo en una sola capa SVG; orientación coherente de normales en componentes cerradas con winding mezclado; campo de luz en caché al orbitar; cambio de luz y cancelación; faceteado reversible y salida determinista conservando el original. La unión de intervalos no repasa las juntas y elimina sombra de suelo tapada por el objeto.
- Interfaz: las pruebas de 980 × 620 y 1440 × 900, en español/inglés, incluyen el nuevo preset. Alternar masas/trazos y cambiar tinta no invalida geometría; cambiar luz sí. Batería final: 69 app + 225 núcleo + 1 doctest = 295 pruebas pasan; nueve ignoradas. La captura manual se ejecuta además por separado.
- Capturas reales de triángulos egui, sin abrir ventanas: `VECTORIO_QA_SHADOW=1 VECTORIO_QA_MODEL=/ruta/modelo.stl cargo test -p vectorio-app render_volumetric_workspace -- --ignored`. Archivos `target/volumetric/shadow-*.png`: pequeño oscuro, grande claro, error, STL importado, STL grande, vista Solo 3D, trayectorias y segundo tono/contorno. El último caso abre «Acabado y trazos» mediante entrada de puntero sobre el control observado.
- Ejemplo: `cargo run -p vectorio-core --release --example sombra_plana -- target/volumetric [modelo.stl]`. Genera tres SVG: una tinta, luz lateral y dos tonos con contorno. En toroide de 3.072 caras: 1.200 facetas, 1.089 trazos, 16,5 ms iniciales. Con STL local del usuario de 1.499.605 caras: carga 0,83 s, interpretación inicial 975,8 ms, 1.200 facetas / 2.177 trazos; cambio posterior de luz 19,7 ms y segundo tono/contorno 20,2 ms. Medidas de esta máquina y estos modelos, no garantía general. Los archivos de entrada no se modifican.
- La revisión visual independiente de los siete estados del nuevo modo dio `ship`, sin hallazgos materiales. Es una revisión de esta extensión; no sustituye una auditoría completa de fuentes ni una ejecución física en plotter.
- `/Applications/Vectorio.app` actualizada por el instalador, firma verificada; las secciones de código/datos Mach-O coinciden con la compilación release. Se mantiene intacta la sesión abierta del usuario, que conserva su versión hasta el siguiente inicio. La verificación visual de este modo usa capturas headless reales de egui, no se presenta como un recorrido CUA de la aplicación instalada.


## Tres lenguajes artísticos · 16 noviembre 2026

- ONDAS DE SUPERFICIE, MALLA CROMÁTICA y ONDAS + MALLA usan el núcleo de producción, parámetros en recetas y tinta por función. Seis pruebas de arte: distancias que siguen el toroide y origen por componente desconectada; independencia del winding; variación determinista, cambios de origen e interferencias; cada arista y diagonal del cubo exactamente una vez con los cuatro criterios cromáticos; SVG físico, recorte con giro/zoom/posición, documentos y reconstrucción exacta tras abrir recetas con motor nuevo; ocultación de la malla dibujada, número de familias, caché y cancelación. También se comprueba la apertura de recetas antiguas sin parámetros artísticos.
- Las pruebas de interfaz de ambas lenguas y 980 × 620 / 1440 × 900 incluyen los tres modos. Se comprueban roles activos de tintas y que cambiar la asignación de tinta no solicita geometría. Batería completa final: 70 app + 231 núcleo + 1 doctest = 302 pruebas pasan, nueve ignoradas. La captura ignorada se ejecuta expresamente además.
- Capturas reales de egui: `VECTORIO_QA_ART=1 VECTORIO_QA_MODEL=/ruta/modelo.stl cargo test -p vectorio-app render_volumetric_workspace -- --ignored`. Diez estados en `target/volumetric`: tres modos claros a 1440, malla/combinado oscuros a 980, interferencias con controles avanzados, asignaciones de tinta y cada modo con STL grande. La apertura y desplazamiento de los grupos se hace mediante eventos reales de puntero y rueda de egui. No se alteran ventanas del usuario.
- Ejemplo release `volumetric_arte`: STL local de 1.499.605 caras, carga 0,85 s; ondas 986,4 ms con 2.700 caras y 275 curvas; malla 4,1 ms / 4.049 aristas / tres capas; combinado 8,7 ms / tres capas; interferencias 6,7 ms; malla de una tinta 4,1 ms / una capa. Las últimas llamadas comparten la malla y parte de la caché; son medidas locales, no garantías para cualquier archivo. SVG de 420 × 297 mm sin imágenes raster en `target/volumetric-arte`.
- La revisión visual independiente de estos diez estados dio `ship`, sin correcciones materiales, al alcance de esta extensión. Muestreó controles y núcleo y no repitió la auditoría completa de pruebas. Conserva el sistema visual existente; PRODUCT.md/DESIGN.md y un nuevo mundo visual no forman parte de este alcance.
- Instalación de `/Applications/Vectorio.app` actualizada con el instalador autorizado, firma verificada y 14 secciones Mach-O de código/datos coincidentes con el binario release. La sesión ya abierta conserva su versión hasta el próximo inicio. No se afirma un recorrido CUA de esta versión instalada ni una prueba física con plotter.
