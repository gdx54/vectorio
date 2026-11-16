# Volumen · Fase D

16 de noviembre de 2026. Tono vectorial, trama por niveles, trama que sigue la forma y línea oculta discontinua.

## Uso

La galería incorpora cuatro estilos:

- **Grabado**: secciones en el espacio del objeto, con densidad creciente en tres bandas oscuras y contorno.
- **Aguafuerte cruzado**: cuatro familias acumulativas de trama, a 0°, 45°, 90° y 135°, más contorno. Las zonas más claras quedan como papel.
- **Plano técnico**: aristas visibles y aristas ocultas discontinuas en pasadas y tintas independientes.
- **Topografía en sombra**: curvas de altura recortadas a la sombra, más contorno.

En **Añadir pasada** están disponibles **Trama por niveles** y **Trama de forma**. Se pueden elegir de dos a cuatro niveles. La primera permite ajustar separación en milímetros y ángulos por nivel en Avanzado; la segunda permite elegir cortes y dirección X/Y/Z. Sus planos acompañan la rotación y la escala XYZ del objeto, en vez de estar ligados a la pantalla.

Todas las pasadas tienen **Máscara tonal**: Toda la superficie, Solo en luz, Solo en sombra o Banda de luz, con límites entre 0 y 1. La intensidad es N·L por cara, con normales coherentes y oclusión de luz: las sombras que otro componente arroja sobre el objeto valen cero. Luz y sombra usan el umbral de la escena. En las fronteras compartidas manda la cara más oscura para evitar un repaso de la misma arista por ambas máscaras.

Las máscaras describen la superficie visible sobre el papel. Por tanto, una máscara sobre una pasada con líneas ocultas o todas las líneas recorta contra esa superficie proyectada. Las tramas de regiones y del suelo no generan líneas traseras; el selector de ocultas discontinuas se ofrece en las pasadas de líneas 3D.

Para línea oculta, elegir **Ocultas discontinuas** en Visibilidad y ajustar **Guion (mm)** y **Hueco (mm)**. El patrón mantiene su fase al pasar por los vértices de cada polilínea. Los intervalos se conservan durante la oclusión analítica; no se aproximan restando dibujos 2D.

## Núcleo y persistencia

- `pass.rs`: `TonalMask`, `DashPattern`, `ToneHatchParams`, `HatchMode`, `PassKind::ToneHatch` y `Visibility::Hidden`.
- `shadow/tone.rs`: regiones visibles y de sombra, índice espacial para recortar segmentos, unión de intervalos, tramas tonales y guiones físicos. Reutiliza los polígonos analíticos y la fase global de la sombra existente; no introduce rasterización.
- `engine.rs`: caché de resultados tonales independiente de nombres, tintas y orden; cambiar una máscara reutiliza la geometría de la pasada. La luz y los parámetros de máscara invalidan los resultados afectados. Cachés y fragmentación acotadas, con cancelación.
- El renderizador de `scene` conserva opcionalmente los intervalos ocultos. El comportamiento predeterminado de los generadores de Crear no cambia.
- Receta **v2**, compatible con recetas v2 anteriores sin los nuevos campos y con la migración v1 existente. Máscaras, patrón de guion y parámetros de trama se guardan también en estilos. Deshacer/rehacer conserva los cambios.
- Controles en español e inglés, dentro del editor contextual existente, sin nuevos niveles de plegables. La hoja estrecha permite acceder a los controles mediante desplazamiento vertical.

## Comprobaciones

- `cargo test`: **341 pruebas superadas, 9 ignoradas, ningún fallo**.
- Las once referencias SVG históricas conservan coincidencia byte a byte, incluida la migración desde una receta v1 real.
- Partición luz/sombra en ondas, malla y curvas; fronteras y huecos exactos; sombras entre componentes; familias acumulativas por nivel; secciones curvas y transformación de sus planos; visibilidad/ocultación complementarias y guiones en milímetros.
- Exportación vectorial finita y recortada al papel, determinismo, caché sin dependencia de tinta/nombre/orden, validación de rangos y compatibilidad con v2 anteriores.
- Selección de estilos, edición de máscara, receta y deshacer; acciones y máscaras accesibles a 980 × 620 y 1.440 × 900 en español e inglés.
- Capturas de los cuatro estilos en ambos tamaños, temas claro y oscuro: `VECTORIO_QA_TONE=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture`. Archivos en `target/volumetric/{grabado,aguafuerte,tecnico,topografia}-{980,1440}.png`.
- Detector de interfaz sin hallazgos y `git diff --check` sin errores.

La instalación se actualiza con `python3 scripts/install-macos.py` en `/Applications/Vectorio.app`, sin cerrar ni reiniciar la sesión abierta. Las capturas se generan con el mismo código egui y geometría del núcleo; la versión nueva estará disponible en el siguiente inicio.

## Pendiente

El tono de esta fase es constante por cara; no implementa campos de curvatura ni líneas de flujo. Según el orden actualizado del plan, sigue **G · Mapas de lectura** antes de **E · Acabado artístico**; después queda **F · Continuidad**. No se añaden halos, eco de contorno, temblor sembrado, gradientes de tintas, perspectiva ni objetos 3D reeditables en el documento.
