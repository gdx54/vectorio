# Volumen · Fase B

16 de noviembre de 2026. Implementación del §9 de `plan-volumen-rediseno.md`, manteniendo los algoritmos y los estilos de la Fase A. No incorpora máscaras tonales, tramas nuevas ni perspectiva.

## Interfaz

- Papel como vista inicial, con recuadro 3D de 92 × 72 pt en un espacio reservado debajo: no tapa el papel. Doble clic en el recuadro alterna la vista dividida. Se conserva la órbita que ya existía.
- Escena de 176 pt y Pasadas de 220 pt. Por debajo de 1.100 pt, Escena se convierte en una barra de iconos accesibles y Pasadas se abre como hoja flotante. Escena también se puede plegar manualmente.
- Chips para los siete estilos, con nombres en español y miniaturas vectoriales del modelo actual al pasar el ratón. Las miniaturas se generan a menor resolución en un trabajador cancelable. Guardar estilo está visible al final; los estilos propios se conservan en `estilos-volumen.json`, separados de los estilos de fotografías.
- Lista de pasadas con activación, muestra de tinta, nombre, asa para ordenar, añadir, duplicar y eliminar. También se pueden subir y bajar desde Acciones. La selección abre sus parámetros y atenúa el resto de trazos del papel; Ver todas retira ese resaltado. La exportación conserva los trazados completos y sus anchos físicos.
- Parámetros esenciales visibles y un único Avanzado por pasada. Modelo › Avanzado es el único segundo nivel plegable de Escena; no hay terceros niveles.
- Sin descripciones extensas de modos ni nombres en clave. El espacio y los objetos colocados se llaman Volumen. Los textos están en `locales/app.yml`, con traducción al inglés.
- Cálculo e importación indicados con una barra fina e indeterminada sobre el papel. Errores con acción Recalcular. Pie con trazos, tintas, minutos estimados y tiempo de cálculo; exportación secundaria y Colocar en el lienzo como único botón de acento en Volumen. El exportador global también es secundario en este espacio.

## Estado, persistencia y cálculo

`volumetric_ui.rs` conserva el estado, la importación, el trabajador y las acciones. La interfaz se reparte en `volumetric_scene_panel.rs`, `volumetric_passes_panel.rs`, `volumetric_styles.rs`, `volumetric_preview.rs` y `volumetric_stats.rs`. Se retiran los controles antiguos y los tres papeles fijos (`role_active`, `valid_pens`).

Cada pasada tiene una identidad estable en la app. Las solicitudes geométricas se ordenan por identidad y excluyen nombre, tinta, orden y activación. La vista aplica estos datos después, utilizando la respuesta geométrica que corresponda; no confunde resultados anteriores con una pila recién cambiada. Se conservan también las pasadas apagadas para activarlas sin otro cálculo. Cambiar parámetros o cámara usa las cachés del núcleo y el descarte de respuestas obsoletas.

Deshacer y Rehacer conservan la pila y sus identidades. La barra global, el menú y ⌘Z/⇧⌘Z actúan sobre la pila en Volumen, y sobre el campo activo cuando se edita texto. No deshacen accidentalmente el documento de Diseño.

Guardar, abrir, exportar y colocar utilizan las pasadas reales, incluidas combinaciones personalizadas y más de tres tintas. Las recetas v1 siguen migrándose por el núcleo. Los estilos conservan su paleta y la reutilizan o añaden al aplicarse.

La estimación de tiempo usa `output::prepare_cancellable` con las tintas, velocidades y ajustes reales de Salida, en un trabajador separado y cancelable. No bloquea la interfaz ni entra en la clave geométrica.

Al combinar sombra y otros generadores se comparte un encuadre estable, elegido sin depender del orden de pasadas. La proyección usa esa misma cámara; los estilos históricos mantienen sus SVG. Una prueba específica cubre la alineación de sombra y aristas y el reordenamiento sin nuevo cálculo.

## Verificación

- `cargo test`: 314 pruebas superadas, 9 ignoradas, ningún fallo; incluye las once referencias SVG de Fase A, idénticas byte a byte.
- Pruebas de recetas v2 personalizadas en la app, seis tintas de malla, metadatos sin invalidación geométrica, identidad estable, deshacer, almacenamiento de estilos y estimación con distintas velocidades y respuestas obsoletas.
- Capturas del renderizador de egui a 980 × 620 y 1.440 × 900: temas claro y oscuro, error, cálculo, importación, pasada seleccionada y hoja flotante. Comprobación de acciones en ambos idiomas.
- Revisión de la aplicación instalada en `/Applications/Vectorio.app`: selección y edición, octava tinta, añadir Ondas, ordenar por arrastre, deshacer, exportar SVG físico y colocar trazados. El documento de prueba se devolvió a blanco.
- Detector de diseño sin incidencias y comprobación de formato/espacios de los archivos modificados.

Pendiente: gizmo de luz y manipulación del dibujo sobre el papel de Fase C; máscaras y mejoras tonales de D, efectos y continuidad de las fases siguientes. El resaltado solicitado en el §9 queda resuelto en B.
