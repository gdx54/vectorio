# VOLUMETRIC · exploración de lenguajes artísticos

Estudio del 15 de noviembre de 2026. **Estudio inicial y modos ya integrados**: SOMBRA PLANA, ONDAS DE SUPERFICIE, MALLA CROMÁTICA y ONDAS + MALLA. La comparación HTML conserva el prototipo inicial; la aplicación tiene ahora controles, recetas y exportación propios. El usuario se interesa por las tres direcciones (líneas sobre el volumen, malla de varias tintas y combinación) y añade iluminación con sombras proyectadas.

## Direcciones

- **Ondas e interferencias.** Distancias que se propagan por las aristas de la superficie (Dijkstra, aproximación geodésica), convertidas en isolíneas. Un campo espacial coherente altera el recorrido sin sacar las curvas de los triángulos. Controlar origen, densidad, deformación y semilla. Evolución: campos tangentes/curvatura e integración de líneas de flujo para un grabado más direccional; no confundir las ondas demostradas con esa técnica aún pendiente.
- **Malla cromática.** Dibujar las aristas de la malla de trabajo, incluidas las diagonales de triangulación. Complejidad artística independiente del límite de importación. En el prototipo se separan tintas por orientación; proponer también altura, profundidad y regiones coherentes. El modelo original permanece independiente de la malla artística simplificada. Para que sus aristas no desaparezcan como cuerdas interiores, la oclusión debe utilizar la propia malla dibujada.
- **Líneas + malla.** Superponer ondas y estructura con papeles distintos para las tintas. Controlar el equilibrio, la complejidad y la visibilidad de cada familia. Las trayectorias y las asignaciones de tinta deben seguir separadas.
- **Luz y sombra.** Luz direccional con azimut y altura; plano receptor y posición respecto al objeto; sombra como contorno o trama. El prototipo calcula sombras sobre el suelo con tres azimuts y elevación de 50°, reutilizando la proyección geométrica y unión de intervalos de `scene::cast_shadow` / `hatch_shadows`. Permite aislar la sombra o acompañarla del volumen. Las sombras propias del objeto requieren después una prueba de visibilidad respecto a la luz, no solo clasificar normales. La luz puede gobernar también la densidad de un grabado sobre superficie.

## Prueba comparativa

Mismo STL local usado para probar modelos grandes, reducido por el motor existente a su malla de previsualización (~20.000 caras). Cámara ortográfica común. Se calculan curvas, mallas artísticas de tres densidades y sombras; la comparación permite cambiar representación, complejidad, número de tintas y dirección de luz. No se cambia la app ni la fuente del modelo.

Artefactos reproducibles en el directorio de visualizaciones de este chat: `volumetric-art-study.rs`, `volumetric-art-data.json` y `volumetric-lenguajes.html`. El prototipo enlaza el núcleo Rust real, reutiliza importación, secciones y oclusión y no constituye un segundo motor de producción. Las distancias geodésicas del prototipo se han integrado después en el núcleo, con caché, controles, cancelación y pruebas.

Comprobaciones: geometría finita, tres sombras diferentes, trazados vectoriales reales y fragmento de comparación inferior a 1 MB. Las variantes son pruebas de dirección, no una certificación de rendimiento en malla Original ni una implementación completa de grabado por curvatura.

## Base técnica y orden sugerido

La separación por orientación y el sombreado proyectado pueden integrarse aprovechando la infraestructura actual. Después, añadir campos geodésicos, líneas de flujo y mezcla de familias con controles contextuales; conservar semilla, origen y luz en las recetas. Evitar acumular muchos botones de efectos que no puedan combinarse.

Referencias primarias: [Real-Time Hatching, Praun et al.](https://gfx.cs.princeton.edu/proj/hatching/) para orientación por campos de curvatura y relación entre densidad y tono; [Apparent Ridges for Line Drawings, Judd et al.](https://people.csail.mit.edu/tjudd/apparentridges.html) para líneas de forma dependientes del punto de vista. Son referencias para evolución; el estudio no incorpora sus implementaciones ni presenta tramas rasterizadas como SVG vectorial.

Verificación interactiva: los cinco modos cambian la geometría visible; densidad de malla baja/alta produce recorridos distintos; una/tres tintas generan una/tres familias cromáticas; cambiar el azimut altera la sombra. Se comprueban también ocultación del volumen y vista clara/oscura. El fragmento final ocupa unos 341 kB.


## Primera integración: SOMBRA PLANA

Implementado tras la petición de unir sombra propia y proyectada en una sola tinta, con aspecto facetado/cel shading. Blanco de papel para luz, una tinta para todas las sombras, tono intermedio y contorno opcionales. La luz direccional conserva dirección/altura; el umbral modifica la cantidad de sombra y una malla artística simplificada controla el faceteado. El original se conserva.

El modo comprueba oclusión respecto a la luz, además de orientación: un componente puede arrojar sombra sobre otro, incluso sobre caras orientadas hacia el foco. La vista de papel muestra masas planas; «Ver trazos del plotter» permite inspeccionar la salida real. SVG y lienzo reciben tramas vectoriales paralelas unidas, sin dibujar las divisiones de cada triángulo. La densidad adecuada depende del rotulador y se ajusta en «Acabado y trazos». No hay degradado ni textura raster.

La primera integración se ha completado después con los tres modos descritos a continuación. Ejemplo de sombras: `cargo run -p vectorio-core --release --example sombra_plana -- target/volumetric modelo.stl`.


## Integración de los tres lenguajes · 16 noviembre 2026

- **ONDAS DE SUPERFICIE**: distancia por el grafo de aristas desde un extremo elegido en cada componente, convertida en isolíneas sobre los triángulos. Aproximación geodésica, sin atajos por el interior del sólido. Dirección del origen, niveles, deformación espacial, frecuencia y semilla reproducible. «Interferencias» usa la diferencia de distancias desde dos extremos opuestos, generando franjas de igual diferencia de fase. Contorno opcional y principales/secundarias en tintas independientes.
- **MALLA CROMÁTICA**: todas las aristas de la triangulación artística, incluidas las diagonales coplanarias, una sola vez. Complejidad ajustable conservando el original. Una, dos o tres familias de tinta, repartidas por orientación de caras, altura, profundidad de cámara u objetos de la fuente. Un archivo con un solo objeto produce una familia con esa última regla; las otras reglas permiten dividirlo por geometría.
- **ONDAS + MALLA**: ondas en una tinta, malla en una o dos adicionales. Complejidad de la malla y niveles de ondas permiten equilibrar las dos familias. La oclusión corresponde a la propia malla artística, evitando esconder sus aristas con la geometría de mayor detalle del original.

Los tres modos son geometría Rust de producción: vista de papel y exportación con recorte físico y oclusión analítica, colocación en Diseño/Salida y recetas con parámetros y fuente completa. La vista 3D es una referencia de la superficie con curvas y aristas completas; el papel muestra la salida visible. El grabado mediante líneas de flujo de curvatura continúa siendo una dirección futura, distinta de las ondas implementadas.

Ejemplo de SVG reproducibles: `cargo run -p vectorio-core --release --example volumetric_arte -- target/volumetric-arte modelo.stl`. Genera ondas, malla, combinado, interferencias y malla de una tinta. No envía movimientos al plotter.
