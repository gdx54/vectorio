# VOLUMETRIC · hoja de ruta

## Entrega actual

Fase 0 y flujo funcional de Fase 1 implementados. Motor independiente en Rust, siete primitivas, OBJ y STL binario/ASCII reales y diagnóstico acotado; cámara ortográfica orbital y transformaciones del modelo; intersecciones reales con planos X/Y/Z o dirección personalizada; unión de contornos, componentes y casos coplanarios; número/espaciado/rango de cortes; vista dual o individual; composición A3 vertical/apaisada; tintas por función; colocación en Diseño y exportación SVG por el sistema existente. Recetas independientes con malla incorporada permiten repetir el cálculo.

Se adelantan partes de las fases 2–3: campos de profundidad de cámara/distancia, oclusión analítica, principales/secundarias, ATLAS y STRATA, y extracción estructural inicial X-RAY. Esto no equivale a dar por terminadas esas fases.

## Prioridades siguientes

1. Fase 2: cámara con vistas exactas en los extremos y perspectiva con clipping de plano cercano; desacoplarla sin cambiar generadores. Curvatura, iluminación como campo para grabado y combinaciones de campos, suavizado consciente de superficie, intervalos no uniformes, deformación reproducible y vista de planos de sección. Ampliar mediciones de modelos grandes y valorar BVH para el modo Original.
2. Fase 3: clasificación explícita por tipo, trazo oculto discontinuo en una tinta propia y superposición de estructura con topografía. X-RAY actual permite todo/solo visible, sin separar aristas ocultas por estilo. Las siluetas dependen de normales coherentes; la oclusión topográfica no.
3. Persistencia: grupos de recetas 3D enlazados a los objetos, con edición desde Capas, duplicación y remapeo de tintas integrado en el documento. Actualmente se guardan recetas JSON independientes y trazados fijos en `.vectorio`.
4. Importación: GLB/glTF con transformaciones, unidades, jerarquía y materiales. OBJ y STL no tienen unidades: orientación Z arriba y normalización explícita, con ajustes XYZ. Materiales/texturas/normales importadas y colores de atributo STL se ignoran; objetos conservan nombres y pertenencia pero se interpretan juntos.
5. Fase 4: grabado por campos tangentes e iluminación sobre superficie. ENGRAVING no aparece como un preset vacío.
6. Fase 5: RUIN, PARALLAX, multivista, composición múltiple, semillas con efectos artísticos definidos y variación controlada. La semilla se conserva en parámetros, sin efectos aleatorios durante esta entrega.
7. Fase 6 opcional: metadatos y sugerencias de IA, siempre con funcionamiento geométrico local.

## Límites actuales

- Cámara superior: elevación 0,01°–89,99°, sin vistas inferiores ni perspectiva.
- Visor 3D de referencia mediante orden de profundidad, sin selección de objetos, edición de malla, z-buffer ni planos manipulables. Las curvas superpuestas muestran la familia completa para comprender la sección; la vista 2D muestra la visibilidad final.
- Hay simplificación geométrica de trabajo ajustable y se conserva el original. Los límites de importación son ahora 512 MB y 5 millones de triángulos. Las mallas abiertas y no manifold producen advertencias; no se reparan agujeros automáticamente.
- Recetas y controles se conservan durante la sesión; antes de cerrar, guardar la receta para conservar fuente 3D. No se incorporan a la recuperación automática del proyecto.
- A3 y recorte están comprobados por pruebas; compatibilidad sintáctica SVG se valida con usvg. La apertura en Illustrator/Inkscape se registra por separado solo si se comprueba realmente.
- No se mandan movimientos a ningún drawbot.


## Ampliación posterior al uso inicial

Implementados arrastre global OBJ/STL a Solo 3D, límites ampliados a 512 MB/5 millones de triángulos, compactación de vértices STL y tres niveles de detalle. Se ensaya el flujo con modelos locales reales de 209.872 y 1.499.605 triángulos. El original se conserva; la calidad interactiva produce una interpretación aproximada explícita. Pendiente de optimización adicional: modo Original con millones de caras (especialmente su oclusión), progresos porcentuales de reducción y reanudación de recetas muy grandes.


## Lenguaje artístico: primera integración

SOMBRA PLANA implementado: luz orientable, sombra propia por orientación y oclusión real de luz, sombra sobre suelo, malla facetada conservando la fuente y una tinta compartida. Vista de masas o trayectorias; segundo tono y contorno opcionales. El SVG contiene trama vectorial continua por regiones. Persistencia de parámetros en recetas. El grabado direccional continúa como paso siguiente. Las ondas geodésicas, la malla cromática y la combinación de familias se integran en la ampliación siguiente.


## Tres lenguajes de superficie integrados · 16 noviembre 2026

ONDAS DE SUPERFICIE, MALLA CROMÁTICA y ONDAS + MALLA implementados con controles contextuales, tintas por familia, recetas, SVG y colocación en el lienzo. Ondas por distancia de grafo con origen por componente, deformación sembrada e interferencias de dos extremos. Malla triangulada artística de complejidad ajustable, una/tres tintas por orientación/altura/profundidad/objeto. Combinado con ondas y hasta dos tintas estructurales independientes. El original se conserva y el papel usa la oclusión de la propia superficie dibujada. Pendientes: flujo por campos tangentes/curvatura, múltiples puntos de origen editados sobre el visor y fuentes 3D enlazadas desde el documento.
