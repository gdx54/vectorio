# Crear · entrega de la fase 2

## Implementación

- **Paso 0 — motor:** `scene/{mod,camera,solids,hidden,hatch,shadow,tests}.rs` y
  su `README.md`. Tramas de superficie con vértices de profundidad por faceta;
  rechazo de entradas degeneradas, direcciones proyectadas al plano y sombras
  opcionales; recorrido DDA con supercobertura; filtro relativo para caras de
  canto; torno con orientación exterior por construcción y perfiles con cuello;
  eliminación de oclusores traseros solo en sólidos convexos. El ejemplo
  `examples/muestra_escena.rs` agrupa los tornos por tono y encuadra los trazos
  reales en una segunda pasada, manteniendo la separación física de trama.
- **Pasos 1 y 4 — generadores:** `generate/{mod,blocks,spiro,tests}.rs`, registrados
  en `lib.rs`. Esquemas declarativos, valores serializables saneados, condiciones
  de visibilidad, bloqueos, semilla y contexto, registro y dos generadores.
  Manzanas entrega seis partes; Espirógrafo, una curva cerrada por capa.
- **Paso 2 — documento:** `generate/document.rs` y `doc.rs`. Recetas persistentes,
  una parte por objeto abierto sin relleno, regeneración por identidad de parte,
  memoria de tintas ausentes, duplicación independiente y remapeo de tintas al
  borrar o reordenar la paleta. Los documentos antiguos siguen omitiendo el campo
  `generated` cuando está vacío.
- **Paso 3 — aplicación:** `create.rs`, `app.rs`, `commands.rs`, `inspector.rs`,
  `main.rs`, `prefs.rs`, `i18n.rs` y `locales/app.yml`. Selector y menú Diseño ·
  Crear · Salida; ⌘1/⌘2/⌘3. Galería, controles, candados, semilla de 64 bits,
  Aleatorio/N, métricas, vista previa por tinta, colocación/actualización y
  cancelación, edición desde Capas, preferencias y traducción al inglés.
  `LEEME.md` recoge el modo y los atajos.
- **Paso 5 — validación:** pruebas del motor, migración de parámetros, grupos,
  identidad/transformaciones/tintas, deshacer/rehacer, resultados obsoletos,
  captura de pánico, traducciones y disposición en 980 × 620 en ambos idiomas.
  Las dos mediciones del motor y las 500 semillas por generador son pruebas
  ignoradas por defecto que se ejecutaron expresamente en release.

## API pública añadida

```text
Scene::hatch_surface(&mut self, faces: &[Vec<V3>], angle_deg: f64, spacing_mm: f64)
Scene::solid_oriented(&mut self, vertices: &[V3], faces: &[Vec<usize>], smoothing: &[u16], quiet: bool) -> Vec<Vec<V3>>
V3::finite(self) -> bool

Params(pub BTreeMap<String, ParamValue>)
ParamValue::{Number(f64), Text(String), Bool(bool)}
Params::{f64(&str)->f64, int(&str)->usize, choice(&str)->&str, flag(&str)->bool}
Params::set(&mut self, id: &str, value: ParamValue)
Params::normalized(&self, specs: &[ParamSpec]) -> Params
ParamSpec::{Section, Range, Choice, Toggle}
ParamSpec::{id()->Option<&'static str>, label()->&'static str, hint()->Option<&'static str>, shown(&Params)->bool}
Show { id: &'static str, equals: &'static str }
Detail::{Preview, Final}
GenContext { area: BBox, seed: u64, pen_mm: f64, detail: Detail }
GenContext::valid(&self) -> bool
GenPart { name: String, lines: Vec<Polyline>, pen_order: Option<usize> }
GenOutput { parts: Vec<GenPart>, warning: Option<String> }
GenOutput::{lengths_m(&self)->Vec<f64>, total_m(&self)->f64}
From<SceneOutput> for GenOutput
Generator: Sync {
  id(), name(), category(), description() -> &'static str
  params() -> Vec<ParamSpec>
  randomize(&self, rng: &mut Rng, params: &mut Params, locks: &BTreeSet<String>)
  generate(&self, params: &Params, context: &GenContext) -> Result<GenOutput, String>
}
generate::all() -> &'static [&'static dyn Generator]
generate::find(id: &str) -> Option<&'static dyn Generator>
generate::randomize_specs(specs: &[ParamSpec], rng: &mut Rng, params: &mut Params, locks: &BTreeSet<String>)

GeneratedGroup::new(name: String, generator: String, params: Params, seed: u64, area: BBox) -> GeneratedGroup
Document::generated: Vec<GeneratedGroup>
Document::generated_group(&self, id: u64) -> Option<&GeneratedGroup>
Document::put_generated(&mut self, group: GeneratedGroup, output: GenOutput, replacing: Option<u64>) -> Vec<u64>
```

Las variantes de `ParamSpec` están descritas por sus campos en `generate/mod.rs`.
Los nuevos campos de `GeneratedGroup` incluyen tablas `parts` (nombre → objeto),
`inks` (nombre → tinta opcional) y `placement` de referencia.

## Criterios elegidos

- La orientación explícita de `solid_oriented` conserva todos los oclusores. La
  variante convexa sigue orientando por centroide y solo guarda caras delanteras.
- Abrir otro grupo de sombras **descarta** el pendiente; no dibuja pasadas
  inesperadas. Sin sol no se abre un grupo. Las caras no planas o cóncavas se
  descartan. Los perfiles de torno pueden cambiar de convexidad, conservando
  alturas estrictamente crecientes y radios nulos solo en extremos.
- DDA se usa tanto para consultar oclusores como para indexar/consultar pasadas
  coincidentes; conserva sellos por consulta y cubre fronteras y esquinas.
  Evita cuantizar direcciones y separar por error líneas casi iguales.
- La entrada de la interfaz es un entero textual de 64 bits, evitando pérdidas
  de precisión al convertir semillas grandes a un control numérico flotante.
- `Params::normalized` aplica el esquema al usarse: rellena ausentes, elimina
  desconocidos y sanea tipos/rangos. `random: None` excluye un parámetro; en
  rangos, `Some((min,max))` permite una banda más estrecha. `show.equals` admite
  texto, números o booleanos y el prefijo `!` para desigualdad.
- Las alturas de Manzanas se interpretan como extremos de un intervalo, incluso
  si se introducen en orden inverso. El espirógrafo cierra con el mcd; radios
  iguales en una hipotrocoide se comunican como error porque producirían un
  punto. Aleatorio evita ese caso respetando los candados.
- El papel de la receta es su área original. Las líneas locales se calculan
  restando `area.min`; cada objeto conserva su `Placement`. Los nuevos objetos
  heredan la transformación de referencia del grupo. Los existentes mantienen
  identificador, nombre, tinta, visibilidad, bloqueo, transformación y orden.
- Un solo trabajador recibe pedidos versionados, descarta la cola obsoleta y
  comprueba la versión después del cálculo. Preview usa menos detalle; Final
  se calcula antes de modificar el documento. Los pánicos se convierten en
  errores. El documento solo cambia al recibir el Final vigente, en un paso
  de deshacer; Cancelar invalida resultados pendientes.
- No se añadieron dependencias ni se consultó código de SVGeometric.

## Mediciones y revisión visual

Mediciones release de esta entrega, con las pruebas ejecutadas secuencialmente:

| Carga | Tiempo |
|---|---:|
| 5.000 caras / 20.000 segmentos cortos | 12,933 ms de mediana (7 renders) |
| 500 cajas tramadas + sombras + 2.000 diagonales largas | 12,601 ms de mediana (5 renders) |
| Manzanas, 500 semillas aleatorias | 3,084 ms máximo |
| Espirógrafo, 500 semillas aleatorias | 1,120 ms máximo |

Las 1.000 muestras generadas terminaron correctamente, con coordenadas finitas,
dentro del área A4 y menos de 80 m por hoja. Los tiempos dependen del equipo.

Se abrió `/Applications/Vectorio.app` tras comprobar que no había una Vectorio
abierta. Se probó Crear → Manzanas → Aleatorio varias veces → Colocar → seleccionar
sombra → asignarle rojo desde la paleta → Editar en Crear → Aleatorio → Actualizar.
La sombra conservó el rojo. Deshacer recuperó la geometría anterior en un paso.
⌘3 calculó y mostró el recorrido del grupo en Salida; ⌘2 permitió revisar también
el espirógrafo. Los dos SVG del bodegón se inspeccionaron con trama continua en
cilindro, cono y cúpula.

La captura detectó que el panel Crear invadía la barra superior. Se corrigió la
reserva de espacio y se añadió una prueba de posición del encabezado y del pie
para ambos idiomas. Esta última corrección, el campo de semilla exacto y los
rótulos de partes se instalan para el siguiente inicio: la ventana de prueba se
conserva abierta, sin reiniciarla. Esa revisión final de aspecto nativo queda
pendiente del siguiente inicio; la disposición corregida está comprobada con
el renderizado de egui en pruebas.

Fuera de alcance: el kit de piezas y los generadores con dirección artística de
la fase 3. La protección de recursos del motor limita resoluciones extremas;
no se garantiza un tiempo máximo para geometría arbitraria o toda superpuesta.
