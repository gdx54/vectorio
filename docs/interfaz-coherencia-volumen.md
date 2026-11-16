# Coherencia de la interfaz · Crear y Volumen

16 de noviembre de 2026. Ajustes solicitados después de la Fase F.

- **Espacios**: el selector Diseño / Crear / Volumen / Salida usa un pomo deslizante con identidad estable, incluso cuando cambian las herramientas de la barra. Conserva sus cuatro segmentos también a 980 px; el título y el selector de formas se ajustan al espacio disponible.
- **Exportar**: mantiene la misma cápsula azul en todos los espacios. Entrar en Volumen ya no lo convierte en un botón secundario.
- **Tintas de Volumen**: el menú permite elegir una familia y asignarle directamente cualquier tinta del documento, con muestra de color y desplazamiento vertical para paletas largas. Se eliminan los desplegables anidados que podían cerrar el menú antes de aplicar una elección. Cada pasada recuerda por separado la familia activa.
- **Estilos**: desaparece la tira horizontal de efectos. El selector compartido con Crear ofrece búsqueda por nombre o categoría, lista vertical y miniatura al pasar el ratón. Volumen agrupa sus 29 estilos en Líneas y superficies, Tono y textura, Mapas de lectura y Acabado artístico, además de Mis estilos. Crear usa el mismo componente para sus generadores.

## Comprobaciones

`cargo test`: **398 pruebas superadas, 9 ignoradas, ningún fallo**. Se mantienen los SVG históricos idénticos. Las pruebas nuevas verifican la animación entre espacios, el aspecto constante de Exportar, el selector compacto en ambos idiomas, la búsqueda y selección con el puntero y la asignación de los ocho colores del documento en cada una de tres familias, en español e inglés.

Capturas revisadas a 980 × 620 y 1.440 × 900, con los temas claro y oscuro, incluidos catálogo filtrado, paleta abierta y Crear. Se regeneran con `VECTORIO_QA_COHERENCE=1 cargo test -p vectorio-app render_volumetric_workspace -- --ignored --nocapture` y la prueba ignorada `render_create_panels`. Detector de interfaz y `git diff --check` sin hallazgos.

Instalación mediante `python3 scripts/install-macos.py` en `/Applications/Vectorio.app`, sin cerrar ni reiniciar ventanas abiertas.

## Pendiente

Los juegos de color predefinidos, la mezcla óptica y las isolíneas se han entregado después en [Ampliación de G](volumen-ampliacion-g.md). Los juegos se pueden retocar por familia y conservar dentro de un estilo.
