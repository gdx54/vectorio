# Volumen · Fase E

16 de noviembre de 2026. Acabado artístico vectorial por pasada.

## Uso

La galería incorpora **Ondas con halo**, **Eco de contorno**, **Trazo a mano** y **Ondas con gradiente**. Elegirlos sustituye la pila y permite deshacer.

El editor de la pasada contiene **Acabado artístico**, al mismo nivel que sus otros apartados:

- **Halo al ocultarse (mm)**: abre espacio en el trazo posterior junto a una silueta más cercana. Conserva el trazo delantero y mantiene la distancia al atravesar vértices. Disponible para contorno, aristas, campo, ondas y malla con líneas visibles.
- **Ecos de contorno** y **Desplazamiento del eco (mm)**: hasta doce copias; desplazamiento positivo hacia fuera y negativo hacia dentro. Una copia a 0,2 mm permite engrosar el contorno. En contornos cerrados se respeta la orientación de exteriores y huecos; en perfiles abiertos el desplazamiento es lateral. Con gradiente, los ecos usan la tinta base elegida y el original conserva su reparto de color.
- **Temblor de mano**, suavidad, longitud irregular y pequeños huecos: ruido suave en milímetros y variación reproducible mediante la semilla de escena y una semilla propia de la pasada. Cambiar tintas, nombres u orden conserva el resultado.

En **Acciones → Aplicar gradiente de tintas**, elegir altura, profundidad o luz. Reutiliza los mapas de lectura de G con transición difuminada por alternancia vectorial; el editor del mapa permite ajustar las tintas, incluida una rampa de dos o tres. Se aplica a trazos sobre la superficie visible. Las vistas especializadas con medida propia conservan sus controles específicos.

Los halos no se ofrecen en secciones desplazadas, tramas 2D, suelo ni vistas de lectura compuestas. Los ecos son desplazamientos artísticos: un desplazamiento grande sobre una silueta cóncava puede producir cruces.

## Núcleo y persistencia

`volumetric/finishing/` contiene el acabado, los ecos y la variación sembrada. El halo amplía los intervalos analíticos de oclusión por longitud de trazo, también en curvas cerradas. El resto del acabado actúa después de las máscaras y el reparto de tintas, con recorte al área imprimible. Las leyendas conservan su geometría y los ecos fuera del papel se eliminan.

La caché de acabado reutiliza la geometría al cambiar metadatos y evita repetir los cálculos anteriores al modificar temblor o ecos. Se mantienen límites de trazos/puntos y cancelación. Toda la salida sigue siendo vectorial.

La receta sigue siendo **v2**: añade ajustes de acabado con valores predeterminados desactivados. Carga recetas v2 anteriores sin estos campos y mantiene la migración v1. Guardar estilos, recetas y deshacer/rehacer conserva los ajustes.

## Comprobaciones

- `cargo test`: **379 superadas, 9 ignoradas, ningún fallo**. Las once referencias SVG históricas siguen siendo idénticas byte a byte, incluida la migración de una receta v1 real.
- Pruebas de profundidad y distancia del halo, continuidad entre vértices y cierre, ecos exteriores/interiores y huecos, doble contorno a 0,2 mm, ruido acotado y reproducible, huecos, semillas independientes, cachés, leyendas, recorte físico y exportación finita.
- Pruebas de persistencia, deshacer/rehacer, reasignación de tintas y acceso a los cuatro estilos en español e inglés a 980 y 1.440 px.
- Capturas revisadas de los cuatro estilos y del panel estrecho: `VECTORIO_QA_FINISH=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture`. Archivos `target/volumetric/finish-*.png`.
- Detector de interfaz sin hallazgos y `git diff --check` sin errores.

Instalación mediante `python3 scripts/install-macos.py` en `/Applications/Vectorio.app`. La sesión abierta se conserva; la nueva versión se usa en el siguiente inicio.

## Pendiente

**F · Continuidad**: objeto 3D reeditable en el documento, «Abrir en Volumen» desde Crear, hoja multivista y perspectiva. Las ampliaciones de G sobre mezcla óptica y nuevas isolíneas escalares continúan pendientes según su entrega; no forman parte de E.
