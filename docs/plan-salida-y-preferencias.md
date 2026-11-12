# Plan: pulido de Salida, Preferencias e idiomas

Plan para ejecutarlo por fases. Cada fase deja la app compilando, con pruebas en verde e instalada.

## Reglas para todas las fases

- Lee `AGENTS.md` antes de empezar. La interfaz sigue en español. Se mantiene la separación entre objetos/capas y tintas.
- **No ejecutes `cargo fmt`**: el proyecto usa líneas densas a propósito y el formateo ensuciaría todo el diff.
- Si `cargo` no está disponible: `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
- Al terminar cada fase:
  1. `cargo test --workspace`
  2. `cargo clippy --workspace --all-targets` (sin avisos nuevos)
  3. `python3 scripts/install-macos.py`
- No cierres ni reinicies la Vectorio que esté abierta.
- Actualiza `LEEME.md` cuando cambie algo visible.
- Un commit por fase, solo si el usuario lo pide. Termina el mensaje con la línea de coautoría que indique la sesión.
- **Paso 0 (antes de la fase A):** el trabajo de Salida está sin commit en `feat/tipografias-y-menu`. Pide al usuario que lo confirme o autorice el commit antes de tocar nada, para poder comparar.

---

## Fase A · Ajustes al plan de salida

La base es buena y conviene conservarla: modelo conservador, rutas seguras intactas, cálculo en segundo plano, capa propia para el papel y buenas pruebas. Los cambios van por orden de prioridad. A1–A5 son obligatorios. A6–A9 solo se hacen si las mediciones lo justifican.

### A1. Una sola comprobación de «salida lista»
Hoy hay tres versiones distintas:
- `output::show` usa `rendered.rev == rev && !tracing() && !is_computing() && ready()`.
- `export_svg` y `export_per_pen` no comprueban `is_computing()`.
- «Plan con esperas…» en la barra solo comprueba `output_ui.ready()`.

Cambios:
- Añade `pub fn output_ready(&self) -> bool` en `VectorioApp` (`app.rs`) con la condición completa y úsala en todos esos sitios.
- `output::show` calcula `current` y `ready` con ella.
- Prueba: con `is_computing()` verdadero, `output_ready()` es falso.

### A2. Avisar del resultado de las exportaciones
Ahora `OutputUi::export_plan` y `export_svg` guardan el error en `self.error`, que solo se ve en el inspector. Exportando desde la barra, el usuario no se entera ni del éxito ni del fallo.
- Haz que devuelvan `Result<Option<PathBuf>, String>`. `None` significa que se canceló el diálogo.
- En `app.rs`, muestra un aviso con `notify`: «Plan guardado en …» o el error.
- Mantén el error en el inspector cuando la exportación se lanza desde el inspector.

### A3. El recorrido usa el color del papel
`map()` pinta el papel siempre `Color32::WHITE` e ignora `app.paper_tint`. Con papel negro y tinta blanca, la simulación no muestra nada.
- En `Map::Route`, pinta el papel con `PAPER_TINTS[paper_tint]`. Habrá que pasar `paper_tint` a `map()`.
- Los mapas de calor siguen sobre blanco, que es como se leen.
- El rótulo de la barra no cambia.

### A4. Quitar el modo Plotter antiguo y renombrar
`ViewMode::Plotter` ya no llega al lienzo, así que hay código muerto:
- `canvas.rs`: la rama `ViewMode::Plotter` de `show`, `simulate` y `paint_plot` (sección «Plotter», hacia la línea 718).
- `app.rs`, `status_bar`: el bloque `if self.mode == ViewMode::Plotter` con Reproducir, velocidad y «Desplazamientos» (hacia las líneas 789–803).
- `Sim.show_travel`, si deja de usarse. `OutputUi` ya tiene el suyo.

Después renombra `ViewMode::Plotter` → `ViewMode::Output` en todo el crate. Es un renombrado mecánico.

### A5. Recalcular sin tirones
Al arrastrar un deslizador se encadenan cálculos completos, y cada resultado pone `sim.progress = 1.0`.
- **Espera previa:** cuando cambian los ajustes, guarda el instante del cambio en `OutputUi`. No lances el trabajo hasta 250 ms después del último cambio. Pide un repintado con `ctx.request_repaint_after`.
- **Cancelación:** añade `output::prepare_cancellable(..., cancel: &AtomicBool)`. `prepare` pasa a llamarla con una bandera que nunca se activa. Comprueba la bandera en el bucle de `schedule`, cada pocas decenas de trazos, y en el bucle de huellas de `prepare`. Si se activa, devuelve un `Err` interno que la UI ignora. `OutputUi` guarda un `Arc<AtomicBool>` por trabajo y lo activa cuando lanza uno nuevo.
- Conserva la posición de la simulación al llegar un plan nuevo. Solo pon `progress = 1.0` cuando no había plan.
- Prueba en core: con la bandera activada, `prepare_cancellable` termina enseguida con error.

### A6. Fragmentar solo donde hace falta (algoritmo)
Ahora, si una sola zona supera el límite, se trocean **todos** los trazos en tramos de `chunk_mm`. Eso añade subidas y viajes en zonas que nunca iban a superar el límite.

Una celda con `total_heat ≤ max_wet_passes` nunca puede superarlo, así que:
- En `prepare`, cuando `needs_protection` sea verdadero, trocea solo los trazos cuya huella toque alguna celda con `total_heat.counts[i] > max_wet_passes`. Los demás pasan enteros.
- Prueba nueva: una zona con 5 pasadas sobre la misma línea y una línea larga de 200 mm lejos de ella. La línea larga queda en un solo tramo y `wet_after.max() <= cap`.
- Las pruebas existentes deben seguir pasando, en especial `many_crossings_keep_all_geometry…`.

### A7. Formato del plan JSON (pensando en el driver)
- Añade a `OutputPlan`:
  - `generator: "Vectorio <versión>"`. El core no conoce la versión de la app: recíbela como parámetro de `prepare` o rellénala en la app antes de serializar.
  - `origin: "top-left"` y `y_axis: "down"`.
  - El margen ya va dentro de `paper`.
- Crea `docs/formato-salida.md` con el esquema `vectorio-output-v1`: unidades, acciones (`change_pen`, `travel`, `wait`, `draw`) y qué significa cada campo.
- Prueba de esquema: serializa un plan pequeño y comprueba la lista de claves de primer nivel y las de cada acción. Si alguien cambia el formato sin cambiar la versión, falla.

### A8. Mapa de calor como textura (medir antes)
`map()` clona la malla cada fotograma (`egui::Shape::Mesh(mesh.clone())`). Con una foto puede tener millones de vértices.
- Mide primero: añade a `examples/muestra_secado.rs` un caso con una foto o muchas líneas, y cuenta los vértices de `heat_mesh`.
- Si pasa de unos 200 000 vértices, cambia a textura: `grid.pixels(2048)` → `egui::ColorImage` → `ctx.load_texture(..., TextureOptions::NEAREST)`, pintada con `painter.image` sobre `paper_rect`. Los colores de `HEAT_COLORS` no dependen del tema, así que la textura no hay que regenerarla al cambiar de aspecto.
- Adapta la prueba `exports_and_simulation_stay_visible…`: en vez de `has_heat` con malla, comprueba que hay una forma de imagen o rectángulo con la textura del mapa.

### A9. Errores del core como tipo (prepara la fase C)
- En `vectorio-core/src/output.rs`, cambia `Result<OutputPlan, String>` por `Result<OutputPlan, OutputError>`, con un enum de variantes: `InvalidSettings`, `MissingPen`, `InvalidPenWidth`, `InvalidCoordinates`, `InvalidLength`, `Cancelled`.
- El texto en español pasa a la app (`output.rs` de la app) con una función `error_text(&OutputError) -> String`.

### Fuera de alcance por ahora
Rendimiento de `draw_route` con fotos enormes (cachear las formas ya dibujadas). Apúntalo en «Próximos pasos» de `LEEME.md`.

---

## Fase B · Preferencias

### B1. Modelo y archivo
- Crea `crates/vectorio-app/src/paths.rs` con `pub fn support_dir() -> Option<PathBuf>`, que devuelve `~/Library/Application Support/Vectorio`. Úsalo en `styles.rs` y en `theme::Appearance` en lugar de repetir la ruta.
- Crea `crates/vectorio-app/src/prefs.rs`:
  ```rust
  #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
  #[serde(default)]
  pub struct Preferences {
      pub appearance: theme::Appearance,
      pub language: Language,          // Sistema · Español · English (la fase C lo activa)
      pub ui_scale: f32,               // 0.8..=1.5, ctx.set_zoom_factor
      pub paper_tint: usize,           // hoy se pierde al cerrar
      pub plotter_name: String,        // "EasyDraw"; sustituye el texto fijo
      pub show_tips: bool,             // consejos del inspector de documento
      pub new_document: NewDocument,   // papel, PlotSettings y OutputSettings por defecto
  }
  ```
- Usa `Default` con los valores actuales. `NewDocument::default()` sale de `Document::default()`.
- Archivo: `preferencias.json` en `support_dir()`. Escritura atómica, igual que hoy en `Appearance::save`: temporal y `rename`.
- **Migración:** si no existe `preferencias.json` pero sí `apariencia.json`, toma de ahí el aspecto. Guarda el archivo nuevo y deja el antiguo sin borrar.
- Mueve `Appearance::load/save` a `Preferences::load/save`. `Appearance` conserva `label()` y `apply()`.
- En `VectorioApp`, sustituye `appearance` por `prefs: Preferences`. Añade `fn set_prefs(&mut self, ctx, new)`, que aplica los cambios (aspecto, escala, tinta del papel) y guarda. Si falla, avisa con `notify` como hace hoy `set_appearance`.
- Pruebas:
  - ida y vuelta por JSON;
  - un JSON vacío `{}` da los valores por defecto;
  - la migración desde `apariencia.json`. Usa un directorio temporal: haz que `load`/`save` reciban la ruta en una función interna probable.

### B2. Ventana de Ajustes
- Crea `crates/vectorio-app/src/prefs_ui.rs` con un `egui::Window` centrado, sin redimensionar, de unos 520 pt de ancho, con el estilo de `theme::glass_panel` y `theme::close_button`. Esc lo cierra.
- Pestañas con `segmented` / `text_segment`, como en la barra:
  - **General:**
    - Aspecto (Sistema · Claro · Oscuro).
    - Idioma (Sistema · Español · English). Hasta la fase C, desactivado con la nota «Próximamente».
    - Tamaño de la interfaz, deslizador 80–150 %.
    - Mostrar consejos.
  - **Documento nuevo:**
    - Papel: tamaño, ancho, alto y margen. Reutiliza los controles de papel del inspector de documento si se pueden extraer a una función.
    - Color del papel de vista previa.
    - Velocidades: dibujo, viaje, subir/bajar.
    - Botones «Usar los de este documento» y «Restablecer».
  - **Salida:**
    - Nombre del plotter.
    - Valores iniciales de secado: tiempo, pasadas húmedas, empape, pausa al cambiar de tinta.
    - Nota: solo afectan a documentos nuevos; cada proyecto guarda los suyos.
- Los cambios se aplican al momento, como en macOS: no hay botón «Aceptar». Se guarda al soltar un deslizador o cambiar un valor, no en cada fotograma.

### B3. Abrir los Ajustes
- Añade `Command::Preferences` con atajo ⌘, en `COMMAND_KEYS` (`Key::Comma`).
- Menú nativo: en el menú «Vectorio», después de «Acerca de Vectorio», añade un separador y `item("Ajustes…", Some("CmdOrCtrl+Comma"), Command::Preferences)`.
- Añade «⌘,» a la hoja de atajos (`CHEATSHEET`).
- **Quita el menú «Aspecto» de la barra superior** (`app.rs`, `ui.menu_button("Aspecto", …)`). El aspecto se cambia en Ajustes, y se conserva el submenú Visualización → Apariencia como atajo.

### B4. Usar las preferencias
- `new_document()` y `blank()` crean el documento con `prefs.new_document`: papel, plot y output.
- `paper_tint` se lee de `prefs` y se guarda al cambiar, tanto desde el menú como desde Ajustes.
- `"EasyDraw"` en `output.rs` y en la barra («Preparación · EasyDraw») pasa a ser `prefs.plotter_name`.
- `show_tips` oculta los consejos de `document_settings` en el inspector.
- `ui_scale` → `ctx.set_zoom_factor(prefs.ui_scale)` al arrancar y al cambiarlo.
- Actualiza `LEEME.md`: sección «Ajustes (⌘,)». Quita la mención al menú Aspecto de la barra.

---

## Fase C · Idiomas: infraestructura y primeras pantallas

Objetivo: español (por defecto y de reserva) e inglés, listos para publicar el código. En esta fase se traducen por completo Ajustes, los menús nativos, la barra, la hoja de atajos y Salida. El resto va en la fase D.

### C1. Biblioteca y archivos
- Añade a `vectorio-app`:
  - `rust-i18n = "3"`;
  - `sys-locale = "0.3"`, para detectar el idioma del sistema;
  - `serde_yaml` como dependencia de desarrollo, para la prueba de claves.
- En `main.rs`: `rust_i18n::i18n!("locales", fallback = "es");`.
- Crea `crates/vectorio-app/locales/es.yml` y `locales/en.yml`.
- Usa claves semánticas por zona: `menu.archivo.abrir`, `salida.guardar_plan`, `ajustes.general.idioma`… Para interpolar: `t!("salida.secando", tiempo = …)`.
- `Language` (de `prefs.rs`) gana `fn resolve(self) -> &'static str`, que devuelve `"es"` o `"en"`. `Sistema` usa `sys_locale::get_locale()`: si empieza por `es`, `"es"`; en otro caso, `"en"`.
- `set_prefs` llama a `rust_i18n::set_locale(...)` cuando cambia el idioma.

### C2. Números y unidades
- Crea `crates/vectorio-app/src/fmt.rs` con `decimal(x, decimales)`: coma en español, punto en inglés.
- Mueve allí `duration`, `distance` y `pct` desde `output.rs`, con unidades traducidas («min», «s», «h»).
- Sustituye los `.replace('.', ",")` que haya en la app.

### C3. Menú nativo y hoja de atajos
- `CHEATSHEET` pasa de `const` a `fn cheatsheet() -> Vec<(String, Vec<(String, String)>)>`.
- Al cambiar el idioma, reconstruye el menú: guarda `menu = NativeMenu::install(ctx)` de nuevo. Comprueba que `init_for_nsapp` sustituye el menú anterior y que el manejador de eventos queda bien. Si en macOS da problemas, avisa con «El menú cambiará de idioma al volver a abrir Vectorio» y no lo reconstruyas.
- `AboutMetadata` (créditos, comentario) también se traduce.

### C4. Pantallas de esta fase
Traduce todos los textos visibles en:
- `prefs_ui.rs`;
- `commands.rs` (menús y hoja);
- la barra superior de `app.rs` y los avisos de `notify` de ese archivo;
- `output.rs` (app) y `error_text` de A9;
- `theme.rs` (`Appearance::label`, tooltips).

En el `en.yml`, los términos de dominio van así:

| Español | English |
|---|---|
| tinta / rotulador | pen |
| Salida | Output |
| secado | drying |
| pasadas húmedas | wet passes |
| empape | bleed |
| viaje | travel |

### C5. Pruebas
- Prueba de paridad: carga `es.yml` y `en.yml` con `serde_yaml` y comprueba que tienen las mismas claves.
- Prueba de claves usadas: recorre `src/*.rs` y extrae los literales de `t!("…")` con una expresión regular sencilla o `str::find`. Cada clave debe existir en `es.yml`. `rust-i18n` devuelve la propia clave cuando falta, y así no pasa en silencio.
- Repite la prueba de visibilidad de Salida (`exports_and_simulation_stay_visible…`) en inglés. Los textos son más largos y no deben salirse del panel en 980×620.

---

## Fase D · Idiomas: resto de la app

Es mecánica, pero larga: unos 900 textos. Haz un commit por archivo y pasa las pruebas de C5 en cada uno.

1. `inspector.rs` (unos 390 textos). Empieza por los encabezados de `theme::section`.
2. `vectorize_ui.rs`.
3. `colorpanel.rs`, `canvas.rs`, `styles.rs`. Los nombres de los estilos predefinidos se traducen; los guardados por el usuario, no.
4. **Etiquetas del core** (`label()` en `fill.rs`, `font.rs`, `mix.rs`, `shapes.rs`, `vectorize.rs` y `trace.rs`): el core no debe depender de `rust-i18n`.
   - Añade a cada enum `fn key(self) -> &'static str`, un identificador estable en ASCII.
   - En la app, traduce con `t!(format!("core.trazo.{}", s.key()))`.
   - Deja `label()` en el core para el ejemplo y las pruebas, o elimínala si ya nadie la usa.
   - Pasa `Err(String)` de `doc.rs`, `shared.rs` y `trace.rs` a enums, como en A9, o como mínimo tradúcelos en la app.
5. **No se traduce:**
   - el contenido de los documentos ya creados (nombres de rotuladores, objetos, papel);
   - el texto de muestra «Hola, Vectorio».
   
   Los rotuladores por defecto de un documento nuevo sí salen en el idioma activo: `default_pens()` recibe los nombres desde la app, o la app los renombra tras crear el documento.
6. `LEEME.md`: menciona el idioma en Ajustes. Para la publicación en abierto, propón al usuario un `README.md` en inglés. No lo crees sin pedirlo.

---

## Decisiones tomadas por defecto (cámbialas si el usuario dice otra cosa)

- El menú «Aspecto» desaparece de la barra. Se conserva Visualización → Apariencia.
- Idiomas: español e inglés. «Sistema» elige español si el sistema está en español; en cualquier otro caso, inglés.
- Los Ajustes son una ventana dentro de la app (`egui::Window`), no una ventana nativa aparte.
- Al arrancar se sigue abriendo un papel en blanco. No hay preferencia de «abrir el último proyecto», porque obligaría a leer Documentos al iniciar.
