# Guion de prueba con usuarios — primer uso de Vectorio

Valida el bloque 4 de `auditoria-ux-2026-11-23.md`. Versión probada: 0.1.28 o posterior.

## Preparación
- Cinco personas que no hayan usado Vectorio. Sesión individual de 20–25 min.
- Mac con la app recién instalada, ventana vacía, máquina desconectada. Carpeta de prueba vacía en el Escritorio.
- Grabar pantalla (con permiso). Una persona observa y no ayuda.
- Dar un SVG de ejemplo y una foto en la carpeta de prueba.

## Lo que se dice al participante
«Voy a pedirte tareas con una app para dibujar con un plotter. No evalúo tu habilidad, sino la app. Piensa en voz alta. Si te atascas, sigue intentándolo; solo te ayudaré si lo pides.»

## Tareas (sin instrucciones sobre dónde pulsar)
| # | Tarea | Éxito |
|---|---|---|
| 1 | Dibuja una línea en rojo. | Línea con tinta roja en el lienzo. |
| 2 | Crea una capa y mueve una línea dentro de ella. | Línea movida, en la capa correcta. |
| 3 | Guarda el proyecto, ciérralo y vuelve a abrirlo. | Proyecto reabierto con el trazo. |
| 4 | Importa el SVG de ejemplo. | Aparece en el lienzo sin reemplazar el trabajo. |
| 5 | Añade una escena al lienzo y expórtala como SVG. | Archivo creado, sabe dónde está. |
| 6 | Exporta otra vez con el mismo nombre. | No se sobrescribe nada por sorpresa. |
| 7 | Explica con tus palabras la diferencia entre «Exportar ruta SVG» y «Guardar plan con pausas». | Dice que el plan conserva pausas/cambios de tinta; el SVG solo el recorrido. |

## Registro por tarea
Primera elección (menú, botón, atajo) · errores · ayuda pedida (sí/no, cuál) · tiempo · comentarios literales · ¿perdió trabajo o sobrescribió algo?

## Criterios
- Al menos 4 de 5 completan cada tarea sin ayuda.
- Nadie pierde trabajo ni sobrescribe una exportación por sorpresa.
- Cualquier tarea con 2 o más fallos se convierte en un hallazgo con la grabación como evidencia.

## Después
Resumir resultados en `docs/historial/prueba-usuarios-AAAA-MM-DD.md` con una tabla persona × tarea y los cambios propuestos. Repetir con 3 personas tras corregir.

## Pasada con VoiceOver (aparte, 1 persona)
Recorrer menús, selector de espacios, inspector de tintas y Salida con VoiceOver activado; anotar controles sin nombre o con orden de lectura incorrecto.
