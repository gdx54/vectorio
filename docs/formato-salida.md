# Plan de salida · `vectorio-output-v1`

Archivo JSON que genera **Salida → Guardar plan con esperas…**. Describe el recorrido completo, con las pausas de secado, para que un futuro driver lo ejecute sin recalcular nada.

- Unidades: milímetros y segundos.
- Origen `(0, 0)` en la esquina superior izquierda del papel; las Y crecen hacia abajo (como en SVG).
- Los cambios compatibles (campos nuevos) mantienen `format`. Un cambio que rompa la lectura sube la versión a `v2`. Una prueba de `vectorio-core` falla si el esquema cambia sin tocar la versión.

## Campos de primer nivel

| Campo | Contenido |
|---|---|
| `format` | `"vectorio-output-v1"` |
| `units` | `"mm, s"` |
| `generator` | Programa y versión que lo generó, p. ej. `"Vectorio 0.1.0"` |
| `origin`, `y_axis` | `"top-left"`, `"down"` |
| `paper` | `name`, `width_mm`, `height_mm`, `margin_mm` |
| `pens` | Tintas del documento: `name`, `color` `[r, g, b]`, `width_mm`, `custom_name`. Los índices de `pen` apuntan aquí |
| `motion` | `draw_speed`, `travel_speed` (mm/s), `lift_time` (s), `optimize`, `join_tolerance` |
| `settings` | Ajustes de secado usados: `drying`, `reorder`, `dry_seconds`, `max_wet_passes`, `soak_mm`, `cell_mm`, `chunk_mm`, `pen_change_seconds` |
| `strokes` | Trazos ya ordenados y orientados: `pen` y `path` (lista de puntos `{x, y}`) |
| `steps` | Acciones en orden temporal (ver abajo) |
| `stats` | Totales del plan: `draw_mm`, `travel_mm`, `draw_seconds`, `travel_seconds`, `lift_seconds`, `wait_seconds`, `change_seconds`, `waits`, `pen_changes`, `strokes`, `total_seconds`, `outside_points` |
| `baseline_stats` | Los mismos totales del dibujo sin protección de secado, para comparar |

## Acciones de `steps`

Cada una lleva `action`, `start_s` y `end_s`. Las acciones son consecutivas: el final de una es el principio de la siguiente.

| `action` | Campos extra | Significado |
|---|---|---|
| `draw` | `stroke` | Dibuja `strokes[stroke]` con la pluma bajada, de su primer al último punto |
| `travel` | `from`, `to` | Desplazamiento con la pluma subida (incluye el tiempo de subir y bajar). Al acabar cada tinta vuelve a `(0, 0)` |
| `wait` | `at` | Espera con la pluma subida sobre `at` hasta que seque la zona que va a pisar el siguiente trazo |
| `change_pen` | `pen` | Pausa para cambiar manualmente al rotulador `pen`. `end_s - start_s` es el mínimo previsto |

## Qué no hace el plan

- No lleva órdenes de máquina (EBB u otras); solo el recorrido y los tiempos.
- Las esperas no existen en el SVG de la ruta, solo aquí.
- Los trazos fuera del papel se conservan y se cuentan en `stats.outside_points`.
