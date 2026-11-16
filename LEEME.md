# Vectorio

Aplicación de escritorio en Rust para hacer "cosas bonitas" con un plotter de rotuladores (EasyDraw V4 A3 / tarjeta EBB). Es la hermana creativa de Linear: texto manuscrito, dibujos multicolor con rellenos, fotos convertidas en dibujo y una vista previa fiel de cómo quedará en papel.

## Qué hace

- **Volumen (⌘4)**: primitivas y modelos OBJ/STL, pasadas combinables con tintas independientes, secciones reales, ondas, malla, luz y sombras. Incluye mapas de curvatura, pendiente, oclusión ambiental y espesor; mezcla óptica, isolíneas, halos, eco y temblor reproducible. Tiene cámara orbital ortográfica o con perspectiva, hojas multivista, cartelas vectoriales y 32 estilos organizados por categorías. Las recetas se guardan con los objetos 3D para reabrirlos desde Diseño. Las escenas de Crear pueden abrirse en Volumen. Las coordenadas de Sección técnica se ocultan con «Mostrar cotas XYZ». Conserva el modelo original, admite OBJ/STL de hasta 512 MB y 5 millones de triángulos y ofrece calidad interactiva, detallada u original. [Plan de Volumen](docs/plan-volumen-rediseno.md) y [última ampliación](docs/volumen-ampliacion-g.md).

- **Texto manuscrito en cajas**, con 11 tipografías de un solo trazo:
  - 8 de trazo sencillo de Evil Mad Scientist y Hershey: Capitol (cursiva unida), Allure, Society, Hershey Script, Pepita, Casual, Neato y Arquitecto.
  - 3 propias: Vectorio manuscrita, cuidada y técnica.
  - Con T, arrastra una caja y escribe o pega un texto completo. Los tiradores derecho, inferior y de esquina cambian el ancho y el alto sin escalar la letra; el contenido se redistribuye.
  - Márgenes interiores, interlineado, espacio entre párrafos, sangría de primera línea y alineación izquierda, centrada, derecha o justificada. El guionado automático usa un diccionario español incluido.
  - La caja conserva todo el texto y lo pagina automáticamente según su alto. «Anterior/Siguiente» recorre las páginas; «Salto de página» (⌘Enter) inserta un corte en el cursor. «Exportar páginas…» genera un SVG por hoja y repite los demás objetos del diseño.
  - «Ocupar hoja» ajusta la caja al área útil; «Folio A4» prepara papel A4 vertical y su caja de texto. Los textos antiguos pueden convertirse en cajas desde el inspector.
  - Ajustes de pulso (0–2), inclinación, interlínea, alineación y ajuste de línea. «Unir letras» escribe cada palabra sin levantar el rotulador.
  - «Otra variación» cambia la letra sin cambiar el texto. Incluye acentos, ñ, ¿ y ¡.
- **Capas**: una lista plana, con un objeto por fila. Arriba queda delante.
  - Arrastra uno o varios objetos para cambiar el orden, sin cambiar sus tintas. Traer adelante/Enviar atrás y los atajos funcionan sobre toda la selección.
  - Doble clic en el nombre para renombrar; Enter confirma y Esc cancela.
  - Cada fila muestra el color del contorno (aro) y del relleno (círculo lleno). Un clic abre el editor integrado en español, con RGB, hexadecimal y vista previa. La X, Cancelar o Esc descartan el cambio; Hecho confirma. Las acciones están en un pie fijo y permanecen visibles.
  - Editar un color desde la fila afecta solo a ese objeto y a ese contorno o relleno. Las demás tintas y objetos se conservan.
- **Paleta de rotuladores**: independiente de las capas, con nombre, color y grosor físico de cada tinta.
  - Cada tinta es una tarjeta con su color, una muestra del trazo con su grosor real, el nombre, el hexadecimal y los milímetros.
  - Clic para usarla al dibujar; doble clic en el color para editarlo y en el nombre para renombrarla. El inspector permite cambiar el grosor.
  - Arrastra una tarjeta: las demás se apartan para mostrar dónde caerá. Suéltala sobre una capa o un objeto del lienzo para asignarle esa tinta al contorno; con ⌥, al relleno.
  - La paleta no agrupa objetos ni determina el orden visual del dibujo. El inspector muestra el color real en hexadecimal y permite editarlo directamente; elegir una tinta compartida queda en «Elegir de la paleta…».
  - Los nombres automáticos de los colores se actualizan al recolorear; los nombres escritos por el usuario se conservan.
- **Rellenos**: rayado, cruzado, zigzag continuo (sin levantar el rotulador), sólido, concéntrico, puntillismo y ondas. Se pueden ajustar la separación, el ángulo y el margen.
- **Formas y generativos**: rectángulo, elipse, polígono, estrella, corazón, espiral, espirógrafo, Lissajous y rosa polar.
- **Mano alzada**: el trazo se suaviza automáticamente. Si lo cierras, se puede rellenar.
- **Foto → dibujo** (tecla I o arrastrar la foto), en una sola hoja con la vista «Dibujo» en vivo: muestra los rellenos y tramas reales con el grosor de cada rotulador.
  - Vista previa navegable: pellizco para ampliar alrededor del puntero, dos dedos o arrastre para desplazar, botones Acercar/Alejar y Ajustar (también doble clic). Original, Dibujo y Superpuesto comparten el encuadre; el zoom no cambia el tamaño en el papel.
  - Colores: k-means; cada color se convierte en un rotulador con relleno. Sin contorno por defecto y con todos los rayados en el mismo ángulo (ajustable, o uno distinto por color).
  - Mezcla: superpone tramas de rayado o puntos de varios rotuladores (CMYK, RYB o los colores de la foto) para mezclar los colores en el ojo, con colores más vivos que un relleno plano. Prefiere el negro para las sombras. Las capas importadas se identifican como tramas de relleno, con su tinta en la muestra de relleno. La importación conserva los mismos trazos y grosores que la vista previa.
  - Líneas: centro de los trazos.
  - Retrato: campo de direcciones calculado con el tensor estructural RGB suavizado; las líneas de corriente siguen los bordes y formas de la imagen. Las sombras reciben pasadas adicionales, cruzadas si se desea, y las luces conservan el papel. Funciona en negro, RYB (amarillo, rojo, azul y negro), CMYK, colores de la foto o rotuladores elegidos. Densidad, fluidez, seguimiento de formas, ángulo de las zonas sin estructura, pasadas, largo y reserva de luces ajustables. Genera trazos abiertos por tinta; la foto y sus ajustes se conservan para regenerarla desde Capas. Para empezar: RYB, 240 de densidad, 3 pasadas, punta de 0,35 mm; conviene una foto nítida y con contraste. La densidad es relativa al tamaño de imagen: al reducir mucho el dibujo en el papel también hay que reducirla para que el grosor físico no cierre las luces.
  - Siluetas: formas cerradas.
  - Grabado: ondas moduladas.
  - Topografía: adaptación nativa en Rust de FastMarchingTopoPlot. Una onda recorre la imagen con una velocidad guiada por el brillo y sus tiempos generan curvas de nivel, más juntas en las sombras. Número de curvas, gamma, velocidad mínima, suavizado y origen ajustables; también se puede colocar el origen pulsando sobre la imagen. Conserva el alfa de PNG para omitir zonas transparentes y guarda foto y parámetros para regenerar el dibujo. Sale como trazos por tinta, compatibles con SVG y Salida. Detalles y atribución en `docs/topografia.md`.
  - Al añadirlo, el dibujo queda seleccionado entero para moverlo o borrarlo de una vez.
- **Selección múltiple**: arrastrar en vacío selecciona con un recuadro; ⇧ o ⌘ clic añade o quita. Se mueven, duplican, borran y editan juntos.
- **Menú nativo de macOS** con todas las órdenes y la hoja de atajos (⌘/).
- **Crear** (⌘2): generadores con vista previa y cálculo en segundo plano. Pueblo blanco compone una ladera de bancales con calles, callejones con escaleras, farolas y gente, una plaza con fuente, árboles, bancos e iglesia de nave alargada con el campanario en una esquina de los pies y una pareja de cipreses; cada bancal tiene una hilera de casas adosadas a lo largo de la calle, de una o dos plantas y alturas escalonadas (con balcones, zócalo, la puerta a la calle y el tejado volado sobre la fachada), que de vez en cuando se abre en una casa con patio o en un huerto tapiado con olivos, y en las azoteas hay pérgolas, tendederos, casetones de escalera y depósitos de agua, huertos tapiados y, alrededor, parcelas de surcos, olivares y cipreses. En «Cartel» la fachada al sol queda blanca y los costados reciben sombra fría; tejados, sombras, plantas y terreno van cada uno en su tinta; en «Grabado» todas las caras llevan una trama ligera. El sol de serie depende de la vista: deja en blanco las fachadas de la calle y en tono medio, con una trama abierta, la otra cara visible; «Giro del sol» lo gira a partir de ahí. Los estratos, irregulares, ondulados y con alguna lente, solo van en las caras del borde de la maqueta; entre bancales hay muros de contención con albardilla. «Terreno» elige entre bancales y llano, «Calles» cuántas hay y «Densidad de sombras» junta o abre las líneas de las sombras arrojadas. El encuadre «A sangre» llena toda la hoja de terreno, y «Campo alrededor» aleja la cámara para rodear el pueblo de huertas y olivares; «Maqueta» lo corta como un bloque de terreno con margen en la hoja. Manzanas combina bloques y torres; Espirógrafo dibuja curvas continuas por capa. Las escenas nuevas comparten estilo de cal blanca y tramas por superficie; cada casa, escalera, tapia o banco es una sola pieza, sin juntas entre muro, hastial, pretil o peldaños (las casas adosadas sí conservan su junta); las ventanas y puertas apartan la trama de su pared y pueden quedar en blanco o rellenas («Ventanas oscuras»). La galería muestra miniaturas calculadas en segundo plano. Ajusta parámetros, bloquea los que quieras conservar y usa Aleatorio (N cuando no estás escribiendo). Cada generador trae juegos de tintas a juego con la escena (Cal y añil, Tierra y olivo, Tinta china, Atardecer, Vectorio…) y eliges cuántos rotuladores usar: con menos, las partes se agrupan en los que hay (la vegetación antes que la trama, la trama antes que el terreno). «Rotuladores del documento» reparte las tintas existentes. Colocar añade un grupo con una parte por objeto y suma al documento los rotuladores del juego que falten, reutilizando los del mismo color; cada parte conserva su propia tinta, y al actualizar con el mismo juego se respetan las tintas que hayas cambiado. «Editar en Crear…» en Capas permite actualizar la receta conservando tintas, posición, escala, visibilidad y bloqueo. El último generador, sus parámetros y los candados se recuerdan entre sesiones. Editar un grupo trabaja sobre una copia y conserva los ajustes recordados al actualizar o cancelar.
  **Megastructuras** construye una masa continua con estratos desiguales, galerías, lamas, vaciados, cubiertas y maquinaria: Torre estratificada, Patio interior o Gran pórtico. **Puerto y ciudadela** añade un frente marítimo Pesquero, de Almacenes con grúa, o Ciudadela, con plaza, muelles, veleros, faro y fachadas con toldos. «Intensidad de tinta», compartida por las escenas, refuerza cubiertas y sombras respetando el ancho del rotulador; las escenas densas separan tejados cálidos y sombras frías.
  En Ciudad en fiesta, «Mostrar rótulos» permite quitar todos los textos de tiendas, puestos, recintos y meta conservando la arquitectura y los adornos. Los dos personajes altos de «Cabalgata» son los gigantes del desfile. «Vida en la calle» también añade pequeños aparcabicis en las aceras del núcleo; las copas de la mediana dejan libres los pasos y el parque se aligera durante la feria. El juego «Bolígrafo», disponible en todos los generadores, usa negro, azul, rojo y verde con cuatro rotuladores; con cinco añade amarillo para el terreno. Contornos y textos quedan negros, las tramas y sombras azules, los acentos rojos y la vegetación verde.
  Cúpulas conecta de cinco a nueve pequeños mundos: pueblo, jardín, cancha, mercado, huerto y laboratorio, con gente en su interior. «Mundo» cambia entre Subacuático (peces, coral, buzos, barca y una ballena, calamar o reptil marino) y Espacial (planetas, galaxia, astronautas y un cohete, estación o ballena estelar). Se pueden quitar los protagonistas, regular la gente y el fondo, y girar la vista. El vidrio deja ver los interiores y oculta el fondo; las pasarelas llegan a los bordes de las cúpulas. Juegos de tintas Abismo, Órbita y Bolígrafo.
- **Salida** (⌘3): espacio dedicado a preparar el EasyDraw, sin driver ni conexión todavía. Usa el mismo lienzo y paneles de vidrio que Diseño: inspector a la derecha con resumen, ajustes y exportación, controles de vista sobre el papel y simulación en el panel inferior.
  - **Mapa de tinta total**, con el criterio y la escala de Linear: gris (1 pasada), amarillo (2), naranja (3), rojo (4–5), carmín (6–9) y morado (10 o más). Cuenta la huella física de cada rotulador más el empape ajustable; permanecer dentro de una celda no multiplica las pasadas. Los focos conservan su máximo al alejar la vista.
  - **Mapas de humedad estimada**, antes y después del plan: muestran el máximo de pasadas aún húmedas por zona. Esperar no reduce la tinta total depositada.
  - **Protección de secado**: tiempo de secado y límite de pasadas húmedas ajustables. El plan busca otros trazos dentro de la misma tinta y espera con la pluma levantada cuando hace falta. Divide trazos largos o que se pisan repetidamente, sin simplificar su geometría; mantiene el orden de las tintas de la paleta.
  - **Simulación por tiempo**, incluidos dibujo, viajes, subidas/bajadas, secado y pausas para cambiar de rotulador. Dos pistas de tiempo paralelas —acciones y tinta activa— con indicador compartido de alto contraste y navegación por clic o arrastre, lista de pausas y comparación de tiempo y viajes frente al recorrido sin protección. Zoom, pellizco y arrastre sobre el papel.
  - **Plan JSON con esperas** y **SVG del recorrido**. El SVG conserva los trazos y su orden, pero no ejecuta pausas; estas quedan en el plan JSON para el futuro driver (formato documentado en `docs/formato-salida.md`). Cada exportación avisa de dónde se guardó o de por qué falló. Los ajustes de salida se guardan en el proyecto `.vectorio`.
  - El recorrido se simula sobre el color de papel elegido. Al cambiar ajustes se espera un instante a que dejen de cambiar, se interrumpe el cálculo anterior y se conserva el punto de la simulación. Solo se dividen los trazos que cruzan zonas que superan el límite de pasadas húmedas.
  - El modelo es conservador: considera húmeda la huella hasta que seca el final de cada tramo. Los 20 s de secado, 3 pasadas húmedas y 0,3 mm de empape iniciales son valores orientativos para ajustar con una prueba real de papel y rotuladores. La pausa de cambio de tinta representa un mínimo previsto; el cambio sigue siendo manual. El análisis avisa de trazos fuera del papel y se calcula en segundo plano.
- **Vista previa del papel**: blanco, marfil, kraft, gris o negro.
- **Archivos**:
  - Proyecto `.vectorio` en JSON.
  - Importación de SVG.
  - Exportación SVG en mm con una capa de Inkscape por rotulador, o un SVG por rotulador.

## Rendimiento (M5)

- El dibujo se pinta en la GPU mediante wgpu/Metal.
- Rellenos, vectorizado (umbral, EDT, contornos, adelgazado y k-means) y optimización de recorridos se reparten entre todos los núcleos con rayon.
- Todo se calcula en hilos en segundo plano, así que la interfaz nunca se bloquea. Mientras arrastras un objeto, su último cálculo se transforma al vuelo.

## Interfaz (líneas de macOS 27)

- Barra de herramientas uniforme integrada con los botones de la ventana.
- Paneles flotantes de vidrio legible.
- Radios concéntricos (panel 18 pt, control 8 pt).
- Tipografía SF Pro.
- **Ajustes** (⌘, o menú Vectorio → Ajustes…): aspecto Claro, Oscuro o Sistema (también en Visualización → Apariencia), tamaño de la interfaz, ideas para empezar, color del papel, nombre del plotter y los valores con los que empieza cada documento nuevo (papel, velocidades y secado; «Usar los de este documento» copia los del proyecto abierto). Se aplican al momento y se guardan en `~/Library/Application Support/Vectorio/preferencias.json`. **Idioma**: Sistema, Español o English. Están traducidos todos los paneles, menús, avisos, tamaños de papel, estilos, tipografías y tintas de serie (en documentos nuevos); lo que ya está guardado en un proyecto no cambia. Los mensajes de error internos del núcleo siguen en español. Los textos viven en `crates/vectorio-app/locales/app.yml` (la clave es el texto en español; una prueba falla si falta alguna traducción).

## Atajos

La lista completa está en la app con ⌘/.

| Tecla | Acción |
|---|---|
| V · P · T · S | Seleccionar · Mano alzada · Caja de texto · Formas |
| T + arrastrar · ⌘Enter | Dibujar caja de texto · Salto de página en el editor |
| I · ⇧⌘I | Foto → dibujo |
| ⌘Z / ⇧⌘Z | Deshacer / Rehacer |
| ⌘D · ⌫ | Duplicar · Eliminar (en la lista de rotuladores, borra el rotulador) |
| ⌘W · ⌘Q | Cerrar la ventana (la app sigue abierta; el Dock la recupera con el documento) · Salir |
| ⌘A · arrastrar en vacío · ⇧/⌘ clic | Seleccionar todo · Recuadro · Añadir o quitar |
| ⇥ · ⇧⇥ | Objeto siguiente · anterior |
| Esc | Cancelar, cerrar, deseleccionar |
| ⌘] · ⌘[ · ⌘L · ⌘R | Adelante · Atrás · Bloquear · Otra variación |
| ⌘, | Ajustes |
| ⌘S · ⇧⌘S · ⌘O · ⌘E · ⇧⌘E · ⌘N | Guardar · Guardar como · Abrir · Exportar SVG · uno por rotulador · Nuevo |
| Flechas (⇧) | Mover 1 mm (10 mm) |
| ⌘1 · ⌘2 · ⌘3 · ⌘4 | Diseño · Crear · Salida · Volumen |
| N (en Crear, fuera de campos de texto) | Parámetros y semilla aleatorios, respetando candados |
| ⌘= · ⌘− · Pellizco · ⌘ + rueda | Zoom |
| Espacio + arrastrar | Desplazar (en Salida, Espacio reproduce la simulación) |
| ⌘0 · ⇧⌘0 | Ajustar a la ventana · Tamaño real |
| ⇧ al mover / girar | Bloquear eje / pasos de 15° |

## Compilar

```bash
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cargo test --workspace --locked
cargo run --release -p vectorio-app
```

## Instalación en macOS

```bash
python3 scripts/install-macos.py
```

Instala o actualiza `/Applications/Vectorio.app`, con el icono Relieve. La app instalada abre directamente su binario: no lee el proyecto ni recompila al arrancar. Si falla una compilación, el instalador conserva la versión anterior. Una ventana ya abierta conserva su versión hasta el siguiente inicio.

Los perfiles de desarrollo y pruebas no generan símbolos de depuración ni copias incrementales para limitar el espacio ocupado. Las cachés de `target/` se pueden regenerar y no se publican en GitHub. `cargo clean` las elimina, incluidos los ejemplos y las capturas guardados allí; conserva antes los resultados que quieras guardar.

## Estructura

- `crates/vectorio-core`: lógica pura y con tests:
  - geometría;
  - fuente;
  - rellenos;
  - formas;
  - vectorizado y mezcla óptica (`mix`);
  - optimización;
  - mapa de acumulación y plan de salida con secado (`output`);
  - SVG;
  - documento y grupos generados (`generate`);
  - motor de escenas (`scene`).
- `crates/vectorio-app`: interfaz eframe/egui (`app`, `canvas`, `commands`, `create` (generadores), `inspector`, `vectorize_ui`, `output` (preparación de salida), `prefs` y `prefs_ui` (ajustes), `colorpanel` (editor integrado), `theme`).
- `crates/vectorio-core/fonts`: tipografías SVG de un solo trazo.

## Próximos pasos

- Driver serie EBB para enviar directamente al EasyDraw, con pausa para cambiar de rotulador.
- Grupos con nombre.
- Recortar rellenos por capas, para que lo de abajo no se pinte bajo lo de arriba.
- Un editor de glifos.
- Exportar como PNG de vista previa.
- Cachear el dibujo del recorrido en Salida para fotos con muchísimos trazos.

## Créditos

- Topografía: algoritmo de [FastMarchingTopoPlot](https://github.com/rolandblok/FastMarchingTopoPlot) de Roland Blok, adaptado a Rust. Licencia Unlicense, conservada en `third_party/FastMarchingTopoPlot/UNLICENSE`.

- Tipografías EMS (Allure, Capitol, Casual Hand, Neato, Pepita, Society, Tech) de Evil Mad Scientist Laboratories, y Hershey Script derivada de las fuentes del Dr. A. V. Hershey. Licencia SIL Open Font License 1.1 (`crates/vectorio-core/fonts/OFL.txt`).
  - https://gitlab.com/oskay/svg-fonts
  - https://www.evilmadscientist.com/2011/hershey-text-an-inkscape-extension-for-engraving-fonts/
