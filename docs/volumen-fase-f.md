# Volumen · Fase F

16 de noviembre de 2026. Continuidad entre Crear, Volumen y Diseño, hoja multivista y perspectiva.

## Uso

- **Colocar en el lienzo** guarda un objeto 3D junto a sus trazos, con modelo, receta, pasadas y tintas. En Diseño, hacer doble clic sobre sus trazos o elegir **Editar en Volumen…** en el inspector o en el menú de su grupo en Capas.
- **Actualizar objeto 3D** sustituye los trazos en su posición de la pila y conserva la transformación aplicada en Diseño. Reabrir no modifica el documento. Las tintas retocadas en Diseño se incorporan a la receta; los objetos bloqueados deben desbloquearse antes de actualizar. Colocar y actualizar tienen un paso propio de deshacer/rehacer.
- En **Crear → Abrir en Volumen**, las seis escenas 3D se convierten en una malla real, incluidas las caras traseras. La preparación ocurre en un trabajador. El generador 2D de espirógrafos no ofrece esta acción. Se importa la geometría construida por la escena, sin intentar reconstruir volumen desde su dibujo 2D; los detalles que solo son líneas y las tramas de Crear no se convierten en superficies.
- En **Escena → Papel → Vistas en la hoja**, elegir una vista, planta/alzado/perfil/isométrica o un giro completo de 2 a 12 vistas. La separación se expresa en milímetros. Cada celda tiene cámara, recorte, pasadas y reparto de tintas; la salida sigue siendo una sola hoja vectorial. Las cuatro vistas técnicas usan proyección ortográfica.
- En **Escena → Cámara → Perspectiva**, ajustar distancia focal y plano cercano. Las distancias son relativas al tamaño del modelo. La vista única y el giro admiten perspectiva; las recetas anteriores siguen siendo ortográficas.

## Documento y recuperación

`Document.volumes` guarda las recetas y los vínculos entre pasadas, familias de tinta y objetos. Duplicar crea un grupo independiente; reordenar o eliminar tintas remapea las recetas. Guardar elimina los metadatos de grupos sin trazos, y cargar valida recetas, identificadores y transformaciones. El proyecto sigue siendo compatible con documentos anteriores sin objetos 3D.

La aplicación guarda copias automáticas atómicas en segundo plano, en `~/Library/Application Support/Vectorio/Recuperacion`, separadas del archivo del usuario y con un archivo por sesión. Al volver a iniciar, ofrece recuperar una sesión interrumpida, descartar su copia o dejarla para más tarde. No ofrece copias pertenecientes a procesos que siguen abiertos. Guardar o salir de forma confirmada limpia la copia de esa sesión. Las pruebas usan una carpeta temporal y no alteran las copias del usuario.

## Proyección y composición

`volumetric/camera.rs` define la cámara propia de Volumen y su adaptación al motor analítico compartido. La perspectiva usa profundidad proyectiva, recorta polígonos y segmentos contra el plano cercano antes de dividir y corrige los pesos de interpolación de los mapas de lectura. La conversión de unidades del motor conserva también distancia focal y plano cercano. Los generadores de Crear mantienen su cámara ortográfica y sus salidas anteriores.

`volumetric/continuity.rs` compone las vistas con cachés por cámara y conserva las familias de tinta. La captura de las superficies completas de Crear se activa solo durante la conversión; los renders normales no almacenan esa geometría adicional. Los cambios de cámara, composición y perspectiva se guardan en la receta v2 y en el historial de Volumen.

## Comprobaciones

- `cargo test`: **393 superadas, 9 ignoradas, ningún fallo**. Las once referencias SVG históricas siguen siendo idénticas byte a byte, incluida la receta v1 real.
- Persistencia del objeto 3D, actualización en la misma posición, transformación de Diseño, tintas, duplicación, bloqueo, eliminación y deshacer/rehacer.
- Recuperación automática con receta 3D completa y rechazo de proyectos inválidos sin borrarlos.
- Conversión de los seis generadores 3D; prueba de un cubo con las doce caras trianguladas y sin bordes abiertos, conservando idéntico el render habitual.
- Puntos de fuga, proyección/inversa, profundidad proyectiva, oclusión y plano cercano; los siete algoritmos históricos con perspectiva producen trazos finitos y recortados.
- Hojas técnicas y de giro, color, recorte físico, determinismo, persistencia y cancelación.
- Capturas revisadas a 1.440 × 900 y 980 × 620: `VECTORIO_QA_CONTINUITY=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture`. Archivos `target/volumetric/continuity-*.png`. Corregido el ancho de la hoja flotante del giro completo.
- Detector de interfaz sin hallazgos y `git diff --check` sin errores.

Instalación con `python3 scripts/install-macos.py` en `/Applications/Vectorio.app`, conservando la sesión abierta. La versión nueva se usa al siguiente inicio.

## Pendiente

Las fases A–F del plan están implementadas. Las ampliaciones de G sobre mezcla óptica e isolíneas escalares se completan en [Ampliación de G](volumen-ampliacion-g.md). Campos de curvatura para líneas de flujo, contornos sugestivos y GLB/glTF siguen siendo ampliaciones posteriores del plan.
