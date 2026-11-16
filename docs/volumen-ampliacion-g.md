# Volumen · Ampliación de G

16 de noviembre de 2026. Juegos de tintas, mezcla óptica, isolíneas escalares y cotas opcionales de sección.

## Uso

En el editor del mapa, **Juego de tintas** aplica Grafito, Cuatricromía, Colores primarios, Térmica o Tierra y añil. Reutiliza las tintas idénticas del documento y añade las que falten; conserva las tintas y los objetos anteriores. Después puede modificarse cualquier familia desde **Tintas**. La elección pasa a mostrarse como Personalizado cuando se cambia una tinta. Guardar el estilo conserva la selección y su paleta.

La galería incorpora **Mezcla óptica térmica**, **Isolíneas de curvatura** e **Isolíneas de luz**. Hay 32 estilos integrados, organizados en el selector con búsqueda ya existente.

En una pasada de **Mapa de lectura** de superficie, **Codificación** ofrece:

- **Mezcla óptica**: convierte el campo en colores de un mapa térmico, de grises o frío/cálido y los separa entre los rotuladores reales. Genera tramas cruzadas sobre las regiones visibles. **Escala de trama** ajusta el patrón físico; el grosor de las plumillas limita su detalle. La muestra del editor representa el mapa de color; la cartela reproduce su separación en tintas. La distribución divergente conserva papel sin tinta en el cero.
- **Isolíneas del campo**: de 2 a 96 niveles de cualquier medida escalar disponible. Admite rango, inversión, combinación de medidas, ecualización y un único umbral. **Rotular valores** añade etiquetas vectoriales, evitando solaparlas entre sí. Un campo constante no produce curvas.

Las dos codificaciones generan su propia geometría y se ofrecen en mapas de superficie. Las vistas especializadas —anaglifo, desarrollo, sección, horizontes y otras— conservan su codificación particular. Los identificadores de objetos siguen siendo categorías, no un campo escalar interpolable.

En **Sección técnica**, desactivar **Mostrar cotas XYZ** quita la anotación de dimensiones del papel y del SVG. El corte, las líneas ocultas y el rayado permanecen. El interruptor está entre los controles esenciales, se guarda en recetas y estilos y participa en deshacer/rehacer. Las recetas anteriores conservan las cotas activadas.

## Implementación y límites

`reading/optical.rs` reutiliza la separación de densidad óptica de `mix.rs` mediante una entrada analítica por color. No crea una imagen intermedia ni incrusta ráster. Una tabla de 129 valores y un patrón vectorial físico producen coberturas reproducibles con ángulos distintos por familia. La reproducción es aproximada y depende de los colores, grosores y comportamiento real de los rotuladores.

Cambiar una tinta física obliga a actualizar la separación y sus trazos. Se reutilizan las mallas, campos y regiones visibles; nombres y orden de pasadas siguen siendo metadatos. El modo óptico conserva el orden de sus familias y no las vuelve a ordenar por luminosidad al exportar. La cartela usa la tinta más oscura para sus anotaciones.

`reading/scalar.rs` obtiene intersecciones del campo con cada región visible, interpola en el espacio del modelo y proyecta después, también con perspectiva. Une extremos coincidentes sin puentes entre ramas y elimina segmentos repetidos. Las curvas de luz interpolan el campo por vértices en las regiones iluminadas y se recortan contra las sombras analíticas; las regiones de sombra permanecen a cero. La calidad de curvatura y de sus isolíneas sigue dependiendo de la triangulación de trabajo.

Se conservan recorte físico, cancelación y límites de salida. Las nuevas opciones tienen valores predeterminados compatibles con recetas v2 anteriores; continúa la migración v1.

## Comprobaciones

- `cargo test`: **408 superadas, 9 ignoradas, ningún fallo**. Las once referencias SVG históricas siguen siendo idénticas byte a byte, incluida la receta v1 real.
- Separación analítica equivalente al motor de fotos para colores de referencia, papel sin tinta en el cero divergente y actualización óptica sin reconstruir la geometría del modelo.
- Isolíneas de altura verificadas sobre un plano inclinado, con proyección ortográfica y perspectiva; todas las medidas escalares producen coordenadas finitas.
- Juegos de tintas reutilizables sin modificar objetos anteriores; selección con el puntero y edición posterior de una familia.
- Cotas de sección activables por interfaz, salida sin anotación, conservación del rayado y líneas ocultas, persistencia y carga de recetas anteriores.
- Salida vectorial finita y recortada, determinismo, cancelación y exportación sin texto ni imágenes incrustadas.
- Capturas revisadas a 980 × 620 y 1.440 × 900, temas claro y oscuro: `VECTORIO_QA_EXTRA=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture`. Archivos `target/volumetric/extra-*.png`.
- Detector de interfaz y `git diff --check` sin hallazgos.

Instalación con `python3 scripts/install-macos.py` en `/Applications/Vectorio.app`, sin cerrar ni reiniciar ventanas abiertas.

## Pendiente

Quedan las ampliaciones posteriores del plan: campos de curvatura principal y líneas de flujo, contornos sugestivos y GLB/glTF. Los juegos personalizados se conservan dentro de estilos; no se añade una biblioteca independiente de juegos de tintas.
