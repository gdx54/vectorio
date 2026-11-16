# Volumen · Fase G · Mapas de lectura

16 de noviembre de 2026. La fase G se ejecuta después de D, antes de E y F, según el orden del plan actualizado.

## Medida, codificación y rampa

Una pasada puede conservar su generador de líneas y añadir un **Mapa de lectura** desde **Acciones**. También hay pasadas y estilos de lectura en la galería y en **Añadir pasada › Añadir mapa de lectura**.

Las medidas disponibles son altura, profundidad de cámara, distancia geodésica, luz con sombras entre componentes, curvatura media, curvatura gaussiana, pendiente, orientación de pendiente, oclusión ambiental, espesor y objetos. Se calculan sobre la malla de trabajo transformada y se cachean sin asignación de tintas.

- Curvatura media firmada: Laplace–Beltrami cotangente, masa baricéntrica y normales coherentes; positivo indica convexo y negativo cóncavo.
- Curvatura gaussiana: defecto angular dividido por área. Distingue regiones convexas y sillas; no equivale al signo de la curvatura media.
- Pendiente: ángulo de la normal con la vertical, entre 0° y 90°; orientación: azimut horario desde +Y, con continuidad circular en el paso 360°/0°.
- Oclusión ambiental: entre 8 y 128 rayos hemisféricos de distribución coseno, deterministas, contra un BVH de triángulos. Cero significa abierto al cielo y uno cubierto.
- Espesor: primera intersección de un rayo hacia dentro siguiendo la normal; cero significa que no encontró otra superficie.
- Distancia geodésica: aproximación por caminos de aristas, desde el extremo superior de cada componente.

Las codificaciones de esta fase son **Fronteras exactas**, **Transición difuminada** y **Densidad de trazos**. La primera interpola el campo sobre cada segmento y lo corta en los umbrales. La segunda reparte dos tintas contiguas por índice de trazo, mediante una secuencia ordenada reproducible. La tercera adelgaza la familia de líneas con 16 niveles de cobertura y una sola tinta; el espaciado base sigue perteneciendo al generador.

La rampa secuencial ordena las tintas seleccionadas de oscuro a claro por L de OKLab. La divergente conserva el orden elegido y reserva papel alrededor de cero, con rango simétrico. La categórica solo se ofrece para objetos: usa su pertenencia por cara, sin interpolar identificadores. **Tintas** abre el selector de familias; la muestra de rampa permanece visible en el editor y en la lista.

En Avanzado están la combinación ponderada de dos medidas, inversión, umbral, rango manual y ecualización de histograma. En mezclas de luz con otra medida, el rango automático toma los valores de los vértices; las regiones de sombra recortadas pueden saturar el extremo del rango. Puede fijarse un rango manual. La cartela identifica las mezclas mediante sus dos medidas y pesos.

## Cartela vectorial

**Cartela en el papel** reserva una franja de 26 mm por cartela dentro del área imprimible. Contiene rampa real, valores o categorías, medida/unidades, modelo, fecha, cámara/luz y escala gráfica. La tipografía es de trazo único del motor existente: el SVG no lleva texto ni imágenes raster incrustadas.

La fecha se fija al activar la cartela, en UTC, es editable y se guarda en la receta. No cambia al reabrir el dibujo. La escala del desarrollo corresponde a su empaquetado físico; el par estereoscópico tiene su propia escala reducida. Los horizontes son una deformación artística: su escala se interpreta en horizontal.

Mover o escalar la composición conserva la cartela en su franja. El zoom usa el encuadre efectivo del modelo y mantiene el punto señalado. Ocultar una pasada conserva la reserva de su cartela para no mover las demás; desactivar la opción Cartela libera el espacio.

## Efectos de lectura

- **Hachuras de relieve**: dirección de máxima pendiente, con longitud y densidad crecientes en las zonas inclinadas.
- **Mapa de agujas**: rejilla física de pequeños trazos orientados por la normal proyectada.
- **Líneas de horizonte**: perfiles de profundidad y envolventes superior/inferior de horizonte flotante, con cruces analíticos. Los tramos contiguos se unen para evitar levantamientos innecesarios.
- **Crestas y valles**: aristas de extremos discretos de curvatura media, separadas por signo y umbral. No es una implementación de crestas de curvatura principal continuas.
- **Anaglifo**: cámaras separadas entre 0,5° y 8°, con estilo rojo/cian y variante de par estereoscópico.
- **Desarrollo**: bosque de caras desplegadas isométricamente, priorizando las uniones coplanarias y cortando uniones que producirían solape. Empaqueta las islas a una escala común, conserva pliegues de monte/valle en tintas distintas y añade pestañas y números de unión.
- **Estela de giro**: entre dos y doce cámaras, con tintas repartidas según el momento de la vista.
- **Sección técnica**: semiespacio recortado, tapa rayada con huecos, contorno, líneas ocultas discontinuas y dimensiones XYZ de la envolvente resultante.

La galería incorpora 14 estilos de lectura. La rampa rojo/cian del estilo Anaglifo reutiliza o añade esas tintas al documento; las tintas de los objetos existentes se conservan.

## Contratos y límites

`reading/fields.rs`, `rays.rs`, `surface.rs`, `encode.rs`, `effects.rs`, `unfold.rs`, `section.rs` y `legend.rs` separan medidas, consultas geométricas y salida física. `Pass.reading` es opcional; las recetas v2 anteriores se abren sin este campo, y la migración v1 continúa funcionando.

`DrawingPart` conserva una parte editable por pasada y expone sus familias de tinta. Exportación, colocación, miniaturas, selección y estimación usan esas familias. Cambiar asignaciones de tinta no solicita geometría; la ordenación por luminosidad se aplica al materializar la salida con la paleta real. Cambiar la cantidad de familias sí cambia los umbrales.

La lectura de líneas utiliza el campo de la superficie visible proyectada, igual que las máscaras de D. En una pasada con líneas ocultas o todas las líneas, ese campo sigue siendo el de la superficie visible. Las vistas que cambian la proyección (horizontes, anaglifo, giro, desarrollo y sección) no admiten la máscara tonal de la cámara principal; la interfaz la oculta y las recetas incompatibles se rechazan. Las pasadas de lectura propias generan regiones visibles; la selección visible/oculta/todo se conserva en las pasadas de líneas 3D.

Las magnitudes se expresan en unidades del modelo normalizado (**u**), no se interpretan las unidades OBJ como milímetros. Los trazos, guiones, separación, pestañas y cartelas sí se miden en milímetros de papel. Las curvaturas dependen de la calidad de la triangulación; en fronteras o aristas no manifold se dejan a cero para no interpretar la curvatura del borde como curvatura superficial. Espesor en mallas abiertas y AO con pocas muestras son aproximaciones.

El horizonte ordena los perfiles por profundidad media; no sustituye a un renderizador general de volúmenes con voladizos. El desarrollo admite parámetros de hasta 512 facetas y un máximo efectivo de 1.024 caras; requiere una malla manifold y puede producir varias islas. No optimiza el corte ni el pegado como una aplicación de papercraft especializada. La lectura admite hasta 100.000 caras y 200.000 vértices de trabajo, y la salida hasta 250.000 trazos; los excesos se rechazan con un mensaje. Hay cancelación y cachés acotadas.

## Comprobaciones

- `cargo test`: **365 pruebas superadas, 9 ignoradas y ningún fallo**.
- Se conservan las once referencias SVG históricas byte a byte, incluida la receta v1 real.
- Curvatura y escala de una esfera, integral de Gauss–Bonnet, signos en un toroide, plano abierto, AO bajo una cubierta y espesor de una esfera.
- Fronteras exactas sin pérdida de longitud, coberturas reproducibles, continuidad de orientación 360°/0°, rampa OKLab y cero divergente como papel.
- Todos los estilos generan geometría finita, recortada y reproducible, con receta v2 y exportación vectorial. Se prueban cachés, cancelación, paletas personalizadas y referencias de tinta al guardar estilos/deshacer.
- Desarrollo de cubo sin diagonales de corte artificiales; triángulos coincidentes frente a pliegues compartidos; hueco del toroide en la sección; cámaras estereoscópicas separadas; horizonte flotante con cruces exactos.
- Cartela que conserva el dibujo y no pierde el anclaje del zoom; controles y acciones de salida a 980 × 620 y 1.440 × 900 en español e inglés.
- Capturas: `VECTORIO_QA_READING=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture`; resultados `target/volumetric/lecture-*.png`.
- `git diff --check`, formato Rust y detector de interfaz sin errores.
- En la instalación de Aplicaciones: selección de mapa térmico, cuatro tintas, cartela y ⌘Z/⇧⌘Z. Escape cierra primero los menús de Volumen; se añade una prueba de regresión para conservar el espacio activo.

Referencias de las fórmulas: [libigl, cantidades geométricas discretas](https://libigl.github.io/tutorial/#chapter-2-discrete-geometric-quantities-and-operators) y [OKLab, formulación de su autor](https://bottosson.github.io/posts/oklab/). Se implementan las operaciones en Rust; no se incorpora otra biblioteca geométrica.

## Pendiente

E y F se entregaron posteriormente. La mezcla óptica, los juegos de tintas y las isolíneas de los nuevos campos se completan en [Ampliación de G](volumen-ampliacion-g.md). Siguen pendientes los campos de curvatura principal continuos y las líneas de flujo.
