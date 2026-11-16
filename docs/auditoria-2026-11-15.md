# Auditoría de Vectorio · 15 de noviembre de 2026

La auditoría inicial identificó **8 hallazgos: 4 P1 y 4 P2**. Los ocho se han corregido en la implementación y se ha actualizado `/Applications/Vectorio.app` sin cerrar ni reiniciar ventanas abiertas.

## Corrección posterior del cierre con ⌘Q

Los dos informes de macOS `vectorio-2026-11-15-175316.ips` y `vectorio-2026-11-15-175433.ips` identificaron un `SIGABRT` en `winit::ApplicationDelegate::app_will_terminate`, llamado desde `NSApplication::terminate` dentro de `VectorioApp::request_quit`. La llamada síncrona reentraba en el manejador de eventos durante el propio fotograma.

Se eliminó esa llamada y el permiso global de terminación nativa. Tras resolver los cambios pendientes, la aplicación solicita `ViewportCommand::Close` y un repintado para que eframe termine su bucle y libere los recursos fuera de `App::update`. Las solicitudes del Dock siguen llegando a la misma protección de cambios.

La nueva prueba comprueba que la salida envía `Close`, sin ocultar la ventana ni cancelar el cierre. Las 8 pruebas de sesión documental y Clippy pasaron, y se reinstaló la compilación release en Aplicaciones. Se probaron cierres con ⌘Q en la instalación real y se comprobó que el proceso terminaba sin generar nuevos informes de fallo. Se confirmó que la salida con cambios pendientes espera la confirmación del usuario. No se verificó pulsar Cancelar: la herramienta no permite controlar el aviso del sistema y el usuario estaba con el teléfono. Se cerró únicamente el proceso de prueba creado por el agente (4874, iniciado a las 17:57:49), sin documentos del usuario; no quedó una acción pendiente para el usuario.

## Correcciones aplicadas

| Hallazgo | Corrección y verificación |
| --- | --- |
| Cambios sin guardar | Estado de último guardado, indicador en el título y diálogo Guardar / Descartar cambios / Cancelar para nuevo, abrir y salir. Menú y solicitudes nativas de salida pasan por la protección. Cancelar o fallar el guardado conserva la sesión. |
| Historial entre proyectos | Abrir o crear reinicia historial, rehacer, selección, herramientas en curso y resultados pendientes. La regresión confirma que abrir B, deshacer y guardar conserva B y deja A intacto. |
| Carga inválida | Límite de archivo de 256 MB; dimensiones y producto comprobados antes de reservar; límite de 16 millones de píxeles por cobertura y 64 millones acumulados; PNG contrastado antes de decodificar. Se validan papel, tintas, posiciones, identidades y referencias; `next_id` se normaliza por encima de las identidades reservadas. |
| Accesibilidad | Herramientas y segmentos con nombre, función y estado; candados identificados por parámetro; colores y visibilidad etiquetados; foco visible y botones desactivados reconocibles. Pruebas del árbol AccessKit y activación con Enter/Espacio. Tab recorre controles fuera de la selección del lienzo. |
| Ajustes | Bloqueo común de órdenes de menú y teclado; controles de fondo desactivados; se omiten archivos soltados y atajos de Crear mientras hay una hoja activa. Esc y ⌘W cierran Ajustes. |
| Recortes SVG | Recorte vectorial de segmentos, transformaciones, unión de figuras, intersección de recortes heredados, huecos par/impar y `objectBoundingBox`. Las máscaras no soportadas se rechazan con un error explícito en vez de importar un resultado distinto. |
| Exportación desfasada | Comprobación común de las claves actuales de las tintas en exportación de Diseño, por rotulador y por páginas, incluso antes de que se haya iniciado el trabajo de cálculo. |
| Error al elegir imagen | Se muestra el fallo y se conserva la imagen anterior y sus ajustes. Regresión con un PNG corrupto. |

Verificación posterior:

- `cargo test --workspace --all-targets --offline`: **258 correctas, 0 fallidas, 8 omitidas**; 18 pruebas de regresión adicionales respecto a la auditoría inicial.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: correcto.
- Captura adicional `create::render_qa::render_create_panels -- --ignored`: correcta; revisadas las imágenes de 980 × 620 y 1440 × 900 generadas con el renderizador de pruebas.
- `python3 scripts/install-macos.py`: compilación release e instalación correctas; el instalador comprobó la firma de la aplicación antes de reemplazarla.
- La versión abierta no se reinició. La actualización se utiliza al volver a iniciar la aplicación.

La semántica accesible se verificó en el árbol generado por egui; no se realizó una sesión completa de VoiceOver ni una salida real con el plotter. El diálogo nativo de salida y la integración con el Dock no se ejercitaron sobre una sesión de usuario. La puntuación siguiente corresponde a la auditoría inicial y no se ha aumentado mediante una estimación.

## Auditoría inicial

La aplicación tenía una base técnica sólida, con riesgos en la protección del trabajo y la coherencia entre documento, archivo e historial. No se identificaron P0 dentro del alcance revisado.

## Alcance y evidencia

Se revisó el estado actual del árbol de trabajo, incluidos sus cambios locales: núcleo Rust, interfaz egui, persistencia, deshacer, importación SVG e imágenes, cálculo de trazos, exportación y empaquetado macOS. En esta primera fase no se modificó código de producto.

- `cargo test --workspace --all-targets --offline`: **240 pruebas correctas, 0 fallidas, 8 omitidas**. Las omitidas corresponden a capturas, rendimiento y barridos extensos de semillas.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: correcto. Cargo avisa de incompatibilidad futura en la dependencia `block 0.1.6`; no bloquea la compilación actual.
- `/usr/bin/codesign --verify --deep --strict /Applications/Vectorio.app`: correcto.
- La huella de fuentes del instalador coincide con `Contents/Resources/build.json` de la aplicación instalada.
- Se abrió la instalación de Aplicaciones y se inspeccionaron el árbol de accesibilidad y las vistas Diseño, Crear y Salida. Se volvió a Diseño sin añadir objetos ni modificar ajustes.
- Se prepararon programas temporales en `/tmp`, fuera del código de producto, para comprobar carga inválida, identificadores, recorte SVG, historial, trazos desfasados y atajos con Ajustes abierto. El programa del historial utilizó los módulos reales de la aplicación y una copia temporal de `app.rs` con un único método añadido para invocar `track_changes`.

La revisión visual fue limitada al tamaño de ventana existente y al tema activo. Una captura de Crear quedó incompleta y no se utilizó para declarar defectos de renderizado. No se hicieron pruebas con plotter físico, una sesión completa de VoiceOver, benchmarks extensos, barridos de todas las recetas ni consulta de vulnerabilidades de dependencias. El detector web de Impeccable no corresponde a una aplicación egui.

## Evaluación de la interfaz

La aplicación tiene estructura propia de editor de escritorio, menú nativo y atajos. La conformidad funcional con macOS queda pendiente por la salida sin protección de cambios, los controles sin semántica accesible y los atajos que atraviesan Ajustes.

Puntuación orientativa de la revisión, no certificación de accesibilidad ni medición exhaustiva de rendimiento. Se adaptan las dimensiones de Impeccable a macOS; no se aplican requisitos táctiles de iOS o Android.

| Dimensión | Puntuación | Evidencia principal |
| --- | ---: | --- |
| Accesibilidad | 2/4 | AccessKit activo y algunos controles etiquetados; herramientas, segmentos y candados aparecen como `unknown`. |
| Rendimiento | 3/4 | Cálculo de escenas, trazos y salida en segundo plano; importación y algunas exportaciones siguen siendo síncronas. Sin benchmark de latencia. |
| Apariencia y temas | 3/4 | Paleta centralizada y pruebas de contraste/materiales correctas; verificación visual limitada al tema activo. |
| Comportamiento de escritorio | 2/4 | Menú y atajos nativos; faltan protección de cambios y aislamiento completo de hojas. |
| Adaptación | 3/4 | Pruebas de disposición a 980 × 620 y tamaños mayores correctas; falta recorrido manual con escalado y contenido largo. |
| **Total** | **13/20** | **Aceptable; necesita trabajo significativo en las dimensiones débiles.** |

## Hallazgos

### 1. [P1] Salir no protege los cambios sin guardar

**Localización:** `crates/vectorio-app/src/commands.rs:393`, `crates/vectorio-app/src/app.rs:490` y `:515`. Categoría: persistencia y comportamiento de escritorio.

El menú usa directamente `PredefinedMenuItem::quit`; no existe una instantánea de último guardado ni una comprobación de documento modificado. Nuevo y abrir un proyecto también sustituyen el documento sin ofrecer guardar. El cierre rojo de ventana oculta la ventana y conserva la sesión, pero salir del programa puede perder definitivamente el trabajo no guardado.

**Verificación:** revisión del flujo de salida, guardado y sustitución del documento. No se ejecutó Salir con trabajo del usuario.

**Corrección:** mantener un estado de último guardado y encauzar salir, nuevo y abrir proyecto por una resolución Guardar / Descartar / Cancelar. Si se cancela o falla guardar, conservar la sesión actual. Añadir indicador de documento modificado. Acción sugerida: `$impeccable harden`.

### 2. [P1] Deshacer cruza proyectos y conserva la ruta del archivo equivocado

**Localización:** `crates/vectorio-app/src/app.rs:300`, `:437` y `:520`. Categoría: integridad del documento.

Abrir B cambia `doc` y `file`, pero conserva el historial y `committed` de A. El siguiente seguimiento registra la sustitución como un cambio deshacible. Deshacer recupera A sin restaurar la asociación al archivo A. Un guardado normal posterior escribiría A sobre B.

**Reproducción aislada:** crear estado A → registrar cambios → abrir B → registrar cambios → deshacer. Resultado real:

```text
Tras abrir B y deshacer: documento=Proyecto A, archivo=/tmp/vectorio-audit-project-b.vectorio
```

**Corrección:** iniciar una sesión documental coherente al abrir o crear: reiniciar historial, rehacer, instantáneas, selección y trabajos vinculados a la sesión. Si se permite deshacer una apertura, el historial debe conservar también la asociación del archivo. Acción sugerida: `$impeccable harden`.

### 3. [P1] La carga de proyectos puede provocar un pánico antes de devolver un error

**Localización:** `crates/vectorio-core/src/trace.rs:338`, `crates/vectorio-core/src/raster.rs:17`, `crates/vectorio-core/src/doc.rs:670`. Categoría: validación de entrada y estabilidad.

La cobertura reserva `w * h` elementos antes de validar el PNG y sus dimensiones. Un archivo con dimensiones desbordantes provoca un pánico; dimensiones grandes también pueden exigir memoria desproporcionada. Esta deserialización se ejecuta en el hilo de interfaz al abrir el proyecto.

**Reproducción aislada:** serializar un documento con cobertura 2 × 2 y sustituir `w` por `18446744073709551615`. `Document::from_json`, dentro de `catch_unwind`, provoca un pánico en lugar de devolver `Err`.

La validación documental también omite invariantes de identidad: otro caso aislado aceptó `next_id = 1` con un objeto existente de identificador 1, y añadir un objeto produjo dos identificadores iguales. Las operaciones que buscan por identificador pueden entonces dirigirse al primer objeto coincidente.

**Corrección:** comprobar dimensiones, multiplicación con `checked_mul`, límites de píxeles y tamaño del archivo antes de reservar; contrastar el PNG antes de construir la rejilla. Validar identificadores únicos, referencias a tintas/grupos y `next_id` al cargar. Rechazar o normalizar con un mensaje explícito. Acción sugerida: `$impeccable harden`.

### 4. [P1] Los controles personalizados principales carecen de semántica accesible

**Localización:** `crates/vectorio-app/src/app.rs:980`, `:991`, `:1047`; `crates/vectorio-app/src/create.rs:163`. Categoría: accesibilidad.

Herramientas, segmentos y candados se dibujan con `allocate_exact_size`, `Sense::click` y un pintor, sin aportar `WidgetInfo` con función, nombre y estado. El árbol de accesibilidad real expone numerosos controles de la barra y los candados de Crear como `unknown`. El texto de ayuda al pasar el puntero no proporciona la semántica que sí tienen los botones estándar.

**Impacto:** un usuario de lector de pantalla no puede identificar de forma fiable la herramienta, el modo seleccionado o el estado del candado. Los atajos cubren algunas acciones, pero no sustituyen esa información.

**Corrección:** registrar nombre, función y estado accesibles; definir activación por teclado y foco visible en los componentes compartidos. Verificar con VoiceOver. Las tarjetas de tinta ya usan `WidgetInfo` y ofrecen un precedente dentro del proyecto. Acción sugerida: `$impeccable harden`.

### 5. [P2] Los atajos del documento siguen activos con Ajustes abierto

**Localización:** `crates/vectorio-app/src/commands.rs:121`, `crates/vectorio-app/src/app.rs:892`, `crates/vectorio-app/src/prefs_ui.rs:42`. Categoría: interacción y aislamiento de hojas.

La barrera de teclado contempla color, foto y ayuda, pero no `prefs_ui.open`. Si ningún campo de texto está activo, Delete puede borrar la selección detrás de Ajustes. Las órdenes del menú tampoco excluyen esta hoja.

**Reproducción aislada:** seleccionar un objeto, abrir `PrefsUi`, entregar un evento Delete a `shortcuts`. Resultado:

```text
Con Ajustes abierto: prefs_open=true, objeto borrado por Delete=true
```

**Corrección:** resolver primero la hoja activa y limitar los comandos a sus acciones permitidas, tanto desde teclado como desde menú. Conservar Esc para cerrar Ajustes. Acción sugerida: `$impeccable harden`.

### 6. [P2] Importar SVG ignora los recortes

**Localización:** `crates/vectorio-core/src/svg_in.rs:29`. Categoría: fidelidad de importación.

El recorrido aplica transformaciones absolutas, pero no las regiones de recorte de los grupos. Un trazado parcialmente oculto mediante `clipPath` se importa completo y puede dibujar zonas que el SVG original no muestra.

**Reproducción aislada:** segmento de (0,0) a (100,100) px, recortado a un rectángulo de 10 × 10 px. El extremo importado queda en **26,4583 mm**, aunque el recorte termina en **2,6458 mm**.

**Corrección:** propagar los recortes y aplicarlos a las polilíneas. Si alguna construcción no está soportada, advertirlo al importar en lugar de aceptar una representación silenciosamente distinta. Acción sugerida: `$impeccable harden`.

### 7. [P2] Exportar desde Diseño puede utilizar trazos de una configuración anterior

**Localización:** `crates/vectorio-app/src/app.rs:582`, `:624`; `crates/vectorio-core/src/doc.rs:175`. Categoría: coherencia de exportación.

Salida comprueba que sus cálculos estén actualizados. Diseño llama directamente a `doc.layers()`, y las geometrías `Traced` utilizan sus líneas almacenadas aunque sus parámetros ya hayan cambiado. Mientras termina el recálculo de una tinta viva, se puede exportar la versión anterior. También afecta al flujo por rotulador y a las páginas que repitan esas tintas.

**Verificación:** un objeto con `made` de la configuración anterior y un ángulo cambiado se identifica como desfasado, pero `layers()` sigue devolviendo sus trazos almacenados. La revisión del flujo confirma que Diseño no espera ni comprueba esa condición antes de exportar. No se accionó un diálogo de exportación en la sesión visual.

**Corrección:** centralizar una condición de exportación que compruebe las claves actuales de las tintas; esperar su actualización o exportar una instantánea calculada con los parámetros vigentes. Acción sugerida: `$impeccable harden`.

### 8. [P2] El diálogo específico de imagen descarta los errores de carga

**Localización:** `crates/vectorio-app/src/vectorize_ui.rs:176–185`. Categoría: recuperación de errores.

`open_dialog` descarta el resultado de `load` con `let _ =`. Una imagen corrupta o ilegible elegida desde Foto → dibujo no genera aviso. La apertura general sí muestra el error, por lo que el comportamiento depende de la entrada elegida.

**Verificación:** revisión de las dos rutas y del `Result` devuelto por `image::open`. No se alteraron archivos del usuario.

**Corrección:** devolver o conservar el error para mostrar un aviso en español y permitir elegir otra imagen sin perder la configuración anterior. Acción sugerida: `$impeccable harden`.

## Patrones y puntos que conviene conservar

Los fallos se concentran en los límites entre estados: documento y archivo, parámetros y trazos calculados, hoja activa y comandos globales. Conviene resolver esos límites en funciones compartidas, para que teclado, menú y barra usen las mismas reglas.

La separación entre objetos/capas y tintas está representada explícitamente en el modelo. El guardado del proyecto usa reemplazo atómico y tiene pruebas de interrupción. La salida comprueba revisiones y cancela cálculos desfasados. Los generadores tienen pruebas de determinismo, límites y recuperación ante errores. La cobertura de disposición, traducciones y contraste es una buena base para ampliar las regresiones a los casos anteriores.

## Orden recomendado

1. **P1 · `$impeccable harden`:** proteger cambios sin guardar y corregir la asociación entre historial y archivo. Añadir regresiones de abrir, deshacer, guardar y cancelar.
2. **P1 · `$impeccable harden`:** validar proyectos antes de reservar memoria o aceptar identidades incoherentes.
3. **P1/P2 · `$impeccable harden`:** completar semántica accesible y aislar el teclado/menú de las hojas.
4. **P2 · `$impeccable harden`:** asegurar exportación actualizada, recortes SVG y mensajes de carga de imágenes.
5. **Verificación · `$impeccable audit`:** repetir las reproducciones y recorrer VoiceOver, escalado y archivos grandes tras las correcciones.
6. **Final · `$impeccable polish`:** revisar foco, mensajes y presentación cuando los flujos anteriores sean correctos.

Estas acciones pueden ejecutarse por separado, juntas o en el orden que se prefiera. Repetir `$impeccable audit` después de corregirlas permitirá actualizar la evaluación con evidencia nueva.

La fase de auditoría inicial no reinstaló la aplicación. La fase posterior de correcciones sí compiló e instaló la actualización, como se detalla al principio del informe.
