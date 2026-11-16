# Volumen · Fase C

16 de noviembre de 2026. Manipulación directa de cámara, luz y composición; se mantiene la interfaz de la Fase B y el núcleo de pasadas existente.

## Uso

- Arrastrar en el recuadro 3D orbita la cámara. Doble clic conserva la alternancia con la vista dividida. El bloqueo de cámara impide el giro, y las flechas permiten ajustar los ángulos cuando la vista tiene el foco.
- Luz incorpora una esfera con un sol arrastrable: el ángulo alrededor del centro determina el azimut y la distancia al centro determina la elevación. Cerca del centro el sol está alto; cerca del borde, bajo. La elevación se conserva entre 5° y 85°. Las flechas ajustan la luz y los controles numéricos siguen disponibles.
- Arrastrar sobre el papel desplaza el dibujo en milímetros. La rueda y el pellizco escalan alrededor del cursor, conservando el punto que se está señalando. La escala de composición queda entre 0,05 y 3; no cambia la escala XYZ del modelo.
- Los gestos no actúan sobre el papel al usar la hoja flotante de Pasadas ni al desplazar el puntero sobre los paneles. Las vistas exponen etiquetas accesibles, foco de teclado y ayudas breves en español e inglés.
- El resaltado de la pasada seleccionada, ya realizado en B, se conserva también mientras se actualiza el dibujo. No modifica el SVG exportado.

## Estado y respuesta

`volumetric_interaction.rs` concentra la conversión de puntos de pantalla a milímetros, el zoom anclado, la composición de la vista provisional, la órbita y el control de luz.

Mientras el trabajador termina la proyección exacta, el último dibujo se transforma inmediatamente para acompañar el desplazamiento y la escala. Los coeficientes de esa transformación se calculan una vez por fotograma. La salida sigue esperando una respuesta válida del trabajador y mantiene el recorte físico definitivo; no se exporta la vista provisional.

La luz compartida actualiza las pasadas de trama, suelo y contorno de sombra, incluidas las apagadas. La receta v2 conserva los valores de cámara, luz, escala y posición sin cambiar su formato.

El historial incluye los ajustes de escena además de la pila. Un arrastre es un único paso de deshacer aunque se mantenga pulsado y quieto; la rueda agrupa cambios consecutivos. Deshacer y Rehacer no se registran como una nueva edición. ⌘Z/⇧⌘Z actúan sobre la escena cuando el foco está en el papel, la vista 3D o el sol; los campos de texto conservan su propio deshacer. Elegir o importar un modelo nuevo descarta el historial de la escena anterior.

## Comprobaciones

- `cargo test`: 325 pruebas superadas, 9 ignoradas y ningún fallo.
- Arrastre físico del papel, deshacer/rehacer de un gesto con pausa, órbita con y sin bloqueo, ausencia de movimiento acumulado al mantener el cursor quieto, rueda y pellizco locales al papel.
- Anclaje del zoom incluso al alcanzar sus límites, conversión de coordenadas de luz y límites sobre el horizonte, arrastre y teclado de la esfera, propagación de luz a las pasadas y recuperación desde receta v2.
- Arrastre de luz dentro del panel desplazable y pulsaciones consecutivas de flechas sin perder el foco. En la app instalada se comprobaron la orientación por clic y el ajuste repetido con teclado; el arrastre queda cubierto por las pruebas de eventos de egui.
- La hoja flotante consume sus gestos antes que el papel; un escenario con cambios de idioma toma el bloqueo de idioma para evitar interferencias entre pruebas.
- Capturas de egui en los tamaños 980 × 620 y 1.440 × 900, incluyendo luz en panel y hoja, dibujo desplazado y escalado, temas claro y oscuro.
- Las once referencias SVG de las fases anteriores siguen coincidiendo byte a byte. No se cambian los algoritmos geométricos del núcleo.

Pendiente: Fase D, con máscaras tonales, trama por niveles, trama que sigue la forma y línea oculta discontinua. Los demás efectos artísticos y la continuidad con el documento permanecen en sus fases posteriores.
