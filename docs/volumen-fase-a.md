# Volumen · Fase A

16 de noviembre de 2026. Alcance: núcleo de pasadas, sin rediseño de interfaz ni efectos artísticos nuevos.

## Contrato

`Pass` guarda nombre, activación, tinta arbitraria, visibilidad y un `PassKind` con sus parámetros. Se admiten contorno, aristas, secciones, campo, ondas, malla, trama y suelo. `Settings::passes()` traduce los siete modos anteriores a pilas; conserva sus nombres de salida, familias vacías, orden y reparto histórico. Atlas no añade contornos que no existían y Rayos X no añade líneas discontinuas nuevas.

`Engine::draw_passes` acepta una pila editable, repetible y ordenable. `Engine::draw` conserva la entrada anterior para la app y los ejemplos; `Engine::draw_recipe` utiliza la pila almacenada. `passes/` adapta los algoritmos anteriores de `pipeline.rs`, `art/` y `shadow/`, conservando sus tolerancias, oclusión, simplificación y recorte físico. Su desmontaje completo queda para trabajo posterior: esta fase los envuelve.

La trama histórica de Sombra plana conserva en una misma pasada la unión de objeto y suelo: separarlos cambiaría las juntas y el orden del SVG. La pasada de suelo independiente reutiliza las máscaras y la fase de trama existentes. La visibilidad ofrece lo ya disponible: visible o completo. Máscaras tonales y líneas ocultas discontinuas quedan para la Fase D.

La caché de salida usa identidad de fuente, parámetros del programa y papel. Nombre, activación, tinta y orden quedan fuera de esa clave. Los contextos geométricos se comparten por faceteado y guardan secciones, campos, niveles, aristas y luz por parámetros; cambiar oclusión reutiliza la geometría tridimensional. Las cachés conservan los programas activos y podan el historial para no acumular indefinidamente ediciones anteriores. La proyección se vuelve a calcular cuando cambia su visibilidad o composición.

`DrawingPart.pen` identifica una tinta sin limitarla a tres papeles. `Drawing::items` y `document` aceptan una asignación de longitud arbitraria; una asignación vacía utiliza las tintas propias de las pasadas. Las familias de malla permiten hasta 256 tintas (límite de recursos), con distribución por altura, profundidad u objetos. La clasificación histórica por orientación sigue distinguiendo sus tres ejes: no se inventa un algoritmo cromático nuevo. El estilo histórico Ondas y malla conserva dos familias de malla; una pila personalizada puede tener más.

## Persistencia

La receta v2 guarda `passes` como pila autoritativa. `layer_pens` deja de formar parte del formato nuevo; `set_layer_pens` adapta las asignaciones antiguas al guardar. `Recipe::from_json` migra v1 traduciendo `RecipeKind` y sus tres asignaciones, valida parámetros y referencias de tintas y conserva la malla original. Se rechazan versiones futuras.

La app conserva el selector y los controles históricos. Solo se adaptan la escritura de asignaciones y su lectura al nuevo tipo. La edición y representación de pilas personalizadas en la interfaz queda pendiente de la Fase B; para trabajar con ellas en esta fase se usa la API del núcleo (`draw_passes` / `draw_recipe`).

## Evidencia

`tests/volumetric_phase_a.rs` compara los SVG completos byte a byte con once archivos capturados antes del refactor: tres de `volumetric`, cinco de `volumetric_arte` y tres de `sombra_plana`. Cada comparación comprueba la entrada histórica, la pila explícita y la receta v2 guardada y reabierta.

`tests/fixtures/volumetric/recipe-v1.json` es la receta real emitida por el ejemplo anterior al refactor, con toro, parámetros y tintas. La prueba la migra y compara su SVG con `atlas.svg`. Otras pruebas cubren los siete migradores, versiones y tintas inválidas, una pila mixta con seis tintas de malla y una séptima de ondas, caché por pasada, cambio de visibilidad, pila vacía, cancelación y suelo independiente.

Quedan pendientes las fases B–G del plan: interfaz, manipulación directa, máscaras y mejoras tonales, efectos artísticos, mapas de lectura y continuidad del objeto 3D con el documento.
