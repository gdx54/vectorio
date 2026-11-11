# Vectorio

Aplicación de escritorio en Rust para hacer "cosas bonitas" con un plotter de rotuladores (EasyDraw V4 A3 / tarjeta EBB). Es la hermana creativa de Linear: texto manuscrito, dibujos multicolor con rellenos, fotos convertidas en dibujo y una vista previa fiel de cómo quedará en papel.

## Qué hace

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
  - Siluetas: formas cerradas.
  - Grabado: ondas moduladas.
  - Al añadirlo, el dibujo queda seleccionado entero para moverlo o borrarlo de una vez.
- **Selección múltiple**: arrastrar en vacío selecciona con un recuadro; ⇧ o ⌘ clic añade o quita. Se mueven, duplican, borran y editan juntos.
- **Menú nativo de macOS** con todas las órdenes y la hoja de atajos (⌘/).
- **Vista Plotter**: reproduce el orden real de dibujo con los desplazamientos en vacío y la punta del rotulador. Muestra la distancia, las subidas y el tiempo estimado.
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
- Modo claro u oscuro según el sistema.

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
| ⌘S · ⇧⌘S · ⌘O · ⌘E · ⇧⌘E · ⌘N | Guardar · Guardar como · Abrir · Exportar SVG · uno por rotulador · Nuevo |
| Flechas (⇧) | Mover 1 mm (10 mm) |
| ⌘1 · ⌘2 | Diseño · Plotter |
| ⌘= · ⌘− · Pellizco · ⌘ + rueda | Zoom |
| Espacio + arrastrar | Desplazar (en Plotter, Espacio reproduce) |
| ⌘0 · ⇧⌘0 | Ajustar a la ventana · Tamaño real |
| ⇧ al mover / girar | Bloquear eje / pasos de 15° |

## Compilar

```bash
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
cargo test -p vectorio-core
cargo run --release -p vectorio-app
```

## Instalación en macOS

```bash
python3 scripts/install-macos.py
```

Instala o actualiza `/Applications/Vectorio.app`, con el icono Relieve. Después de cada cambio del proyecto se actualiza esta misma aplicación. También comprueba el código al abrir: si ha cambiado, compila antes de iniciar; si la compilación falla, abre la última versión instalada. Los registros están en `~/Library/Logs/Vectorio/updates.log`.

La comprobación al abrir depende de que esta carpeta del proyecto y Rust sigan disponibles en este Mac. Si el proyecto no está disponible, abre el binario instalado. Una ventana ya abierta conserva su versión hasta cerrarla; guarda el proyecto antes de reiniciar para cargar una actualización.

## Estructura

- `crates/vectorio-core`: lógica pura y con tests:
  - geometría;
  - fuente;
  - rellenos;
  - formas;
  - vectorizado y mezcla óptica (`mix`);
  - optimización;
  - SVG;
  - documento.
- `crates/vectorio-app`: interfaz eframe/egui (`app`, `canvas`, `commands`, `inspector`, `vectorize_ui`, `colorpanel` (editor integrado), `theme`).
- `crates/vectorio-core/fonts`: tipografías SVG de un solo trazo.

## Próximos pasos

- Driver serie EBB para enviar directamente al EasyDraw, con pausa para cambiar de rotulador.
- Grupos con nombre.
- Recortar rellenos por capas, para que lo de abajo no se pinte bajo lo de arriba.
- Un editor de glifos.
- Exportar como PNG de vista previa.

## Créditos

- Tipografías EMS (Allure, Capitol, Casual Hand, Neato, Pepita, Society, Tech) de Evil Mad Scientist Laboratories, y Hershey Script derivada de las fuentes del Dr. A. V. Hershey. Licencia SIL Open Font License 1.1 (`crates/vectorio-core/fonts/OFL.txt`).
  - https://gitlab.com/oskay/svg-fonts
  - https://www.evilmadscientist.com/2011/hershey-text-an-inkscape-extension-for-engraving-fonts/
