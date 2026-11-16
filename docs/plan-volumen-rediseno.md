# Volumen · plan de rediseño (interfaz, orden y lenguaje artístico)

16 noviembre 2026. Revisión del estado de `codex/volumetric` y plan para ejecutarlo por fases.
Pensado para que otro agente (Codex, Sonnet) lo ejecute paso a paso sin tener que redescubrir el contexto.

## 1. Diagnóstico

El motor está bien construido (importación robusta, caché por etapas, trabajador cancelable,
oclusión analítica, recorte físico, recetas). Lo que falla es **el modelo mental** que ofrece.

**Siete modos excluyentes que no son del mismo tipo.** `RecipeKind`
(`crates/vectorio-core/src/volumetric/mod.rs`) mezcla tres cosas distintas en un mismo combo:
- *generadores de línea*: secciones (ATLAS/STRATA), aristas (X-RAY), ondas, malla;
- *un tratamiento tonal*: SOMBRA PLANA;
- *una combinación cableada*: ONDAS + MALLA (`Hybrid`).

Consecuencia: no se puede pedir lo artísticamente interesante, que siempre es una mezcla
(silueta + ondas solo en la sombra + malla fina en la luz). Cada combinación nueva exigiría
otro `RecipeKind`. `Hybrid` es el síntoma.

**`Settings` plano.** Un único struct con los parámetros de todos los modos (cortes, campo,
sombra, arte, STRATA, X-RAY…). La interfaz tiene que esconder y mostrar según el modo, y la
receta guarda basura de modos no usados. Las tintas están limitadas a tres papeles fijos
(`layer_pens: [usize; 3]`, `Drawing::items(pens: [usize; 3])`).

**Interfaz ordenada por la tubería técnica, no por la intención.** Un panel de 326 px con
colapsables Modelo → Transformar → Cámara → Topografía → Composición y tintas → Receta →
Luz y sombra → Superficie artística. Nombres en clave en inglés y mayúsculas (ATLAS, STRATA,
X-RAY), contrarios a la regla de interfaz en español. Cámara y luz solo con deslizadores: nada
se manipula en la vista. Las estadísticas compiten con los botones de acción.

**Sin continuidad con el documento.** «Colocar en el lienzo» deja trazados fijos: no se puede
volver a editar el objeto 3D desde Diseño. Y Volumen (⌘4) vive aislado de Crear (⌘2), que
también tiene motor de escena 3D (`scene`).

**Pobreza tonal.** Para un plotter el tono se hace con densidad y dirección de línea. Hoy solo
existe sombra binaria + un tono intermedio con trama recta paralela. No hay trama que siga la
forma, ni tramado cruzado por niveles, ni línea oculta discontinua, ni halos, ni perspectiva.

## 2. Idea central: escena + pila de pasadas

Sustituir «elige un modo» por **«una escena y una pila de pasadas de trazo»**, como capas.

- **Escena** (compartida): modelo, transformación, cámara, luz, papel y composición.
- **Pasadas** (lista ordenada): cada una genera líneas, tiene su tinta, su visibilidad y su máscara.

| Pasada | Qué dibuja | Viene de |
|---|---|---|
| Contorno | Silueta y perfil exterior; opción «eco» (n desplazamientos hacia fuera) | X-RAY |
| Aristas | Aristas vivas por ángulo diedro | X-RAY |
| Secciones | Cortes por planos (dirección, número, rango, desplazamiento por estrato) | ATLAS, STRATA |
| Curvas de campo | Isolíneas de altura, profundidad o distancia | ATLAS |
| Ondas | Ondas geodésicas / interferencias | ONDAS |
| Malla | Aristas de la malla artística | MALLA CROMÁTICA |
| Trama | Tramado por niveles de luz (ver §4) | SOMBRA PLANA |
| Sombra arrojada | Contorno o trama de la sombra sobre el suelo | SOMBRA PLANA |

Propiedades comunes de cada pasada:
- **Tinta**: cualquier tinta del documento (se acaba el límite de 3).
- **Visibilidad**: visible · oculta (discontinua opcional) · todo.
- **Máscara tonal**: toda la superficie · solo en luz · solo en sombra · banda de luz [a, b].
  Esto es lo que hace que combinar sea artístico: ondas solo en la sombra, malla solo en la luz.
- **Reparto de tintas** (opcional): por orientación, altura, profundidad u objeto (hoy solo en Malla).
- Activa/desactivada, nombre, orden.

Los siete modos actuales pasan a ser **estilos** (pilas guardadas) con nombres en español:
- *Curvas de nivel* (antes ATLAS): Curvas de campo + Contorno.
- *Estratos* (STRATA): Secciones con desplazamiento + Contorno.
- *Rayos X* (X-RAY): Aristas visibles + Aristas ocultas discontinuas en otra tinta.
- *Sombra plana*: Trama (2 niveles) + Sombra arrojada + Contorno.
- *Ondas*, *Malla de color*, *Ondas y malla*: equivalentes directos.
- Nuevos (§4): *Grabado*, *Aguafuerte cruzado*, *Plano técnico*, *Topografía en sombra*.

## 3. Interfaz propuesta

```
┌ barra ─────────────────────────────────────────────────────────────┐
│ Estilos: [miniatura][miniatura][miniatura]…   [Guardar estilo]      │
├ Escena (izq., plegable) ┬──── papel grande ───────┬ Pasadas (der.) ─┤
│ Modelo  ▸ cubo ▾ Import │                         │ 👁 ● Contorno    │
│ Cámara  ▸ planta alzado │      vista del papel    │ 👁 ● Ondas       │
│         iso · persp.    │   (trazos reales)       │ 👁 ● Trama       │
│ Luz     ▸ esfera-sol    │                ┌─────┐  │ [+ Añadir]       │
│ Papel   ▸ A4 · márgenes │                │ 3D  │  │ ── Ondas ──      │
│                         │                └─────┘  │ parámetros de la │
│                         │                         │ pasada elegida   │
├─────────────────────────┴─────────────────────────┴─────────────────┤
│ 12.430 trazos · 3 tintas · ~18 min de plotter     [Exportar] [Colocar]│
└─────────────────────────────────────────────────────────────────────┘
```

Principios:
1. **El papel es protagonista.** La vista 3D pasa a recuadro (alternable a pantalla dividida).
2. **Manipulación directa.** Arrastrar en el recuadro 3D orbita la cámara; un **gizmo de sol**
   (esfera pequeña) fija la luz; arrastrar en el papel mueve el dibujo, rueda = escala. Los
   deslizadores quedan como ajuste fino.
3. **Panel de pasadas como Capas.** Ojo, muestra de tinta, nombre, arrastrar para reordenar;
   debajo solo los parámetros de la pasada seleccionada (nada de esconder/mostrar por modo).
   Clic en una pasada resalta sus trazos en el papel.
4. **Estilos con miniatura.** Galería en la barra superior; miniaturas generadas con el modelo
   actual a baja resolución en el trabajador. Elegir un estilo sustituye la pila (deshacer posible).
5. **Pie limpio.** Estadísticas en una línea (trazos, tintas, tiempo estimado reutilizando
   `output`), avisos de calidad solo si los hay, botones a la derecha.
6. Nombres en español en todo: el espacio se llama **Volumen** (⌘4).
7. Reutilizar `theme::glass_panel`, `accent_button` y la estética Liquid Glass existente.

## 4. Mejoras artísticas (por impacto/coste)

1. **Trama por niveles de luz** (alto impacto, coste bajo). Hoy la luz es binaria + tono
   intermedio. Calcular `N·L` (más la oclusión de luz ya existente) y dividirlo en 2–4 bandas:
   cada banda añade una familia de trama con su ángulo (0°, 45°, 90°, 135°) → tramado cruzado
   clásico de aguafuerte. Reutiliza `shadow/mask.rs` (unión de intervalos y fase global en mm).
2. **Trama que sigue la forma** (alto impacto, coste medio). Secciones por planos en espacio
   del objeto **recortadas por la máscara tonal** = trazos que abrazan el volumen solo donde hay
   sombra, con espaciado que se estrecha en las bandas oscuras. Es combinar `contours` + máscara,
   sin campos de curvatura todavía. Estilo *Grabado*.
3. **Línea oculta discontinua en tinta propia** (medio/bajo). Los intervalos ocultos ya se
   calculan al recortar; conservarlos en vez de descartarlos, con patrón de guion en mm.
4. **Halos** (alto impacto visual, coste medio). Cortar la línea de detrás unos mm a cada lado
   donde pasa por detrás de una silueta más cercana. Muy de ilustración técnica; da profundidad
   sin tono.
5. **Contorno con eco y grosor** (bajo). Silueta repetida n veces hacia fuera o hacia dentro, o
   doble pasada desplazada 0,2 mm para engrosar.
6. **Perspectiva** (medio/alto). Cámara con distancia focal y plano cercano. Ya está en la hoja
   de ruta; desacoplar de `scene::Camera` sin tocar los generadores de Crear.
7. **Variación sembrada con efecto real** (bajo). La semilla hoy apenas actúa. Usarla para
   temblor de mano determinista (desplazamiento por ruido suave en mm), longitud de trazo de
   trama irregular y pequeños huecos: aspecto dibujado sin perder reproducibilidad.
8. **Hoja multivista** (medio). Rejilla de N cámaras sobre la misma hoja (giro completo, planta +
   alzado + perfil + iso) — reutiliza composición y recorte.
9. **Gradiente de tintas por campo** (bajo). Repartir cualquier pasada entre 2–3 tintas según
   altura/profundidad/luz con frontera difuminada por alternancia de trazos.
10. Más adelante: campos de curvatura y líneas de flujo (Praun), contornos sugestivos / apparent
    ridges (Judd), GLB/glTF.

## 5. Orden del código

Núcleo (`crates/vectorio-core/src/volumetric/`):
- `scene_settings.rs`: `SceneSettings { model, camera, light, composition, detail }`.
- `pass.rs`: `Pass { name, enabled, pen: usize, visibility, mask, coloring, kind: PassKind }`,
  `PassKind` con sus parámetros propios (enum con datos, no struct plano).
- `passes/` un archivo por tipo: `outline.rs`, `edges.rs`, `sections.rs`, `field.rs`,
  `waves.rs`, `mesh.rs`, `hatch.rs`, `ground.rs`. Cada uno: `fn lines(ctx: &PassCtx, p: &Params)
  -> Vec<Line3 o Polyline>`. `PassCtx` da acceso a las cachés compartidas (malla de trabajo,
  malla artística, grafo, campo de luz, oclusores, cámara).
- `engine.rs`: orquesta pasadas, cachea por (identidad de malla, hash de parámetros de la pasada),
  aplica máscara → oclusión → recorte → simplificación. Salida `Drawing { parts: Vec<Part> }`
  con una parte por pasada (o por familia de color).
- `style.rs`: estilos integrados (los siete actuales + nuevos) como pilas de pasadas.
- Receta **versión 2**; `Recipe::from_json` migra v1 → v2 traduciendo `RecipeKind` a su pila.
  Mantener una prueba con una receta v1 real.
- `pipeline.rs`, `art/`, `shadow/` se desmontan en `passes/` conservando sus algoritmos y pruebas.

App (`crates/vectorio-app/src/`):
- `volumetric_ui.rs` → solo estado, trabajador y acciones.
- `volumetric_scene_panel.rs` (izq.), `volumetric_passes_panel.rs` (der.),
  `volumetric_styles.rs` (galería), `volumetric_preview.rs` (papel + recuadro 3D + gizmos).
- Eliminar `volumetric_art_controls.rs` y la lógica `role_active`/`valid_pens` de 3 papeles.

Documento:
- `Item` nuevo o metadato «Objeto 3D» que guarda la receta junto a los trazados. Doble clic en
  Diseño → abre Volumen con esa receta; «Actualizar» reemplaza los trazados. Recuperación
  automática del proyecto incluida.
- Crear (⌘2): acción «Abrir en Volumen» que convierte la escena generada en malla.

## 6. Fases

Cada fase deja la app funcionando, con pruebas y `python3 scripts/install-macos.py`.

**Fase A · Pasadas en el núcleo, sin cambiar la interfaz.** `Pass`, `PassKind`, `engine.rs`,
`passes/` envolviendo los algoritmos actuales; los siete `RecipeKind` se traducen a pilas y deben
producir SVG idénticos (pruebas de equivalencia con los ejemplos `volumetric` y `volumetric_arte`).
Receta v2 + migración. Fin del límite de 3 tintas.
*Modelo recomendado: Opus, razonamiento alto (diseño de tipos y cachés).*

**Fase B · Interfaz nueva.** Tres zonas, panel de pasadas tipo Capas, galería de estilos con
nombres en español, pie limpio, papel protagonista y recuadro 3D. Sin efectos nuevos aún.
*Modelo recomendado: Sonnet o Codex, razonamiento medio; revisión visual en la app instalada.*

**Fase C · Manipulación directa.** Órbita en el recuadro, gizmo de sol, mover/escalar en papel,
resaltado de la pasada seleccionada.
*Modelo recomendado: Sonnet o Codex, razonamiento medio.*

**Fase D · Tono.** Máscaras tonales por pasada, trama por niveles (cruzada), trama que sigue la
forma, oculta discontinua. Estilos *Grabado*, *Aguafuerte cruzado*, *Plano técnico*,
*Topografía en sombra*.
*Modelo recomendado: Opus, razonamiento alto para máscaras y recorte; el resto Sonnet.*

**Fase E · Acabado artístico.** Halos, eco de contorno, temblor sembrado, gradiente de tintas.
*Modelo recomendado: Sonnet, razonamiento medio (halos: alto).*

**Fase F · Continuidad.** Objeto 3D re-editable en el documento, «Abrir en Volumen» desde Crear,
hoja multivista, perspectiva.
*Modelo recomendado: Opus para el modelo de documento y la perspectiva; Sonnet para lo demás.*

## 7. Criterios de aceptación

- Ningún estilo antiguo cambia su SVG en Fase A (pruebas de equivalencia).
- Cualquier combinación de pasadas es posible sin tocar código.
- Cambiar tinta, visibilidad u orden de una pasada no recalcula geometría.
- Cambiar parámetros de una pasada solo recalcula esa pasada.
- Todo el texto de la interfaz en español (y en `locales/app.yml`).
- La salida siempre es vectorial con recorte físico; nada raster incrustado.

## 8. Mapas de lectura: el color como información

### Qué falla hoy

`art/edges.rs` decide la tinta de cada arista **entera** por reglas gruesas: el eje dominante del
valor absoluto de la normal (X/Y/Z), tres franjas fijas de altura/profundidad o el objeto módulo
el número de tintas. Resultado: saltos bruscos que no coinciden con ninguna frontera real, sin
orden entre tintas, sin leyenda. Parece aleatorio porque no codifica ninguna magnitud legible.

### Modelo: medida → codificación → rampa

Separar tres decisiones, aplicables a **cualquier pasada** (§2):

1. **Medida (campo escalar por vértice/cara)**, calculada en el núcleo y cacheada por malla:
   - Altura, profundidad de cámara, distancia geodésica (ya existen).
   - **Luz** `N·L` con oclusión de luz (ya hay medio camino en `shadow`).
   - **Curvatura media** (convexo/cóncavo, Laplace-Beltrami discreto) y **gaussiana** (defecto
     angular). Muestra crestas, valles, sillas.
   - **Pendiente** (ángulo con la vertical) y **orientación** de la pendiente (brújula, como un
     mapa de exposición en geología).
   - **Oclusión ambiental** (cuánto cielo ve cada punto; muestreo de hemisferio contra los
     oclusores): rincones y huecos en oscuro. Es la medida que más «lee» el volumen.
   - **Espesor** (rayo hacia dentro): zonas finas y gruesas, útil para piezas impresas en 3D.
   - Combinaciones: `a·campo1 + b·campo2`, umbrales, invertir, ecualizar por histograma.
2. **Codificación** (cómo se dibuja el valor con líneas):
   - **Rampa de tintas con frontera exacta**: los trazos se cortan donde el campo cruza el umbral
     (interpolando sobre el segmento, como ya hace `contours`), no por arista entera.
   - **Transición difuminada**: entre dos tintas vecinas, alternancia ordenada de trazos
     (dithering por índice de línea) en proporción al valor. Sin bandas duras.
   - **Densidad**: el valor controla el espaciado de la trama → mapa de calor en una sola tinta.
   - **Mezcla óptica**: el valor se convierte en un color de un mapa de color y `mix.rs` (ya
     usado para fotos) lo descompone en coberturas de los rotuladores reales → tramas cruzadas
     superpuestas, recortadas por caras proyectadas.
   - **Isolíneas del campo**: curvas de nivel de curvatura, de oclusión, de luz (como un plano
     térmico), con valores rotulados opcionalmente.
3. **Rampa**: secuencial (ordenar automáticamente las tintas elegidas por luminosidad OKLab),
   divergente (curvatura: cóncavo en una tinta, convexo en otra, papel en el cero), categórica
   solo para objetos. Rampas tipo «térmica» con 4–5 rotuladores de la biblioteca del usuario.

### Cartela técnica en el papel

Opcional, en el margen: barra de leyenda con la rampa real de tintas y valores, nombre de la
medida y unidades, escala gráfica, azimut/elevación de cámara y luz, modelo y fecha. Texto
con el motor de letra existente (trazo sencillo). Convierte la lámina en un documento técnico.

### Efectos 3D → 2D nuevos (pasadas)

- **Hachuras de relieve** (estilo Lehmann, cartografía antigua): trazos cortos en la dirección
  de máxima pendiente, más largos/densos cuanto más inclinado. Muy técnico y muy plotter.
- **Mapa de agujas**: rejilla regular en papel con un pequeño trazo por celda orientado según la
  normal proyectada (o el gradiente de cualquier campo); longitud = magnitud.
- **Líneas de horizonte** (perfiles apilados): barridos horizontales en pantalla desplazados por
  la profundidad, con ocultación por horizonte flotante. La profundidad por cara es afín y ya
  hay BVH 2D en `shadow/project.rs`.
- **Crestas y valles**: líneas de curvatura extrema en dos tintas (convexo/cóncavo).
- **Anaglifo**: dos cámaras separadas 2–4°, una en rojo y otra en cian → relieve real con gafas.
  Variante: par estereoscópico lado a lado.
- **Desarrollo** (despliegue de la malla artística en plano): pliegues de monte y valle en dos
  tintas, pestañas de pegado. Se puede recortar y montar: el 3D vuelve a ser 3D.
- **Estela de giro**: varias rotaciones superpuestas con la rampa de tintas recorriendo el tiempo.
- **Sección técnica**: corte por un plano con rayado normalizado en la cara cortada, cotas del
  volumen envolvente y vista oculta discontinua.

### Fase G · Mapas de lectura (antes de la E)

Medidas (luz, curvaturas, pendiente, oclusión ambiental), codificaciones (rampa con frontera
exacta, difuminado, densidad), rampas ordenadas por luminosidad y cartela. Después, efectos:
hachuras de relieve, líneas de horizonte y anaglifo primero (alto impacto, coste bajo-medio);
desarrollo y sección técnica al final.
*Modelo recomendado: Opus, razonamiento alto para curvatura, oclusión ambiental y corte exacto;
Sonnet o Codex para cartela, rampas y anaglifo.*

## 9. Jerarquía visual de la interfaz (detalle de la fase B)

Sustituye y concreta el §3. Una sola pregunta por zona; el ojo recorre: estilo → papel → pasadas → salida.

**Cinco zonas, en orden de importancia**
1. **Papel (centro, máximo espacio).** Es el resultado; nunca lo tapa un panel. Recuadro 3D
   pequeño abajo a la derecha (92×72 pt aprox.), ampliable a pantalla dividida con doble clic.
   La cartela (§8) se ve dentro del papel porque se plotea.
2. **Pasadas (derecha, 180–220 pt).** Lo que más se toca. Arriba la lista tipo Capas
   (ojo · muestra de tinta o rampa · nombre · asa para arrastrar; `+` para añadir). Debajo,
   solo los parámetros de la pasada seleccionada, en tres niveles:
   - *esencial* siempre visible: medida/tipo, densidad o niveles, tintas, máscara;
   - *Avanzado* plegado: precisión, semilla, simplificación, rangos;
   - nada de colapsables anidados dentro de colapsables (hoy hay hasta tres niveles).
3. **Estilo (barra superior, junto al selector de espacios).** Chips con nombre en español y
   miniatura al pasar el ratón. «Guardar estilo» al final. Elegirlo reemplaza la pila (deshacer).
4. **Escena (izquierda, 150–180 pt, plegable entera).** Se ajusta una vez y se olvida: Modelo
   (nombre + nº de triángulos, importar, calidad), Cámara (chips Planta/Alzado/Iso/Persp. +
   orbitar en el recuadro), Luz (gizmo de esfera con sol arrastrable), Papel (resumen de una
   línea, despliega tamaño/márgenes/posición). Transformar modelo va dentro de Modelo › Avanzado.
5. **Pie (una línea).** Izquierda: trazos · tintas · tiempo estimado · tiempo de cálculo, en
   texto secundario. Avisos solo si los hay (color de aviso, una línea, con acción). Derecha:
   Exportar SVG (secundario) y Colocar en el lienzo (único botón de acento de la pantalla).

**Reglas**
- Un único botón de acento por pantalla: «Colocar en el lienzo».
- Tres tamaños de texto: título de zona (secundario, pequeño), etiqueta (normal), valor/ayuda
  (atenuado). Nada en mayúsculas sostenidas; nada de nombres en clave en inglés.
- Ayudas de una línea bajo el control solo cuando no es obvio («Arrastra en la vista 3D para
  orbitar»). Las descripciones largas de cada modo desaparecen: el estilo se explica con la miniatura.
- Estado de cálculo como fina barra de progreso sobre el papel, no como texto en el panel.
- Pasada seleccionada: sus trazos resaltados en el papel y el resto atenuados mientras se edita.
- Ventana estrecha: Escena se pliega a iconos; Pasadas pasa a hoja flotante sobre el papel.
- Reutilizar `theme::glass_panel`, `accent_button` y tokens de `theme.rs`; textos en `locales/app.yml`.
