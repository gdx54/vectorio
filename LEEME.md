# Vectorio

Aplicación de escritorio en Rust para hacer "cosas bonitas" con un plotter de rotuladores (EasyDraw V4 A3 / tarjeta EBB). Es la hermana creativa de Linear: texto manuscrito, dibujos multicolor con rellenos, fotos convertidas en dibujo y una vista previa fiel de cómo quedará en papel.

## Qué hace

- **Texto manuscrito de un solo trazo**, con 11 tipografías:
  - 8 de trazo sencillo de Evil Mad Scientist y Hershey: Capitol (cursiva unida), Allure, Society, Hershey Script, Pepita, Casual, Neato y Arquitecto.
  - 3 propias: Vectorio manuscrita, cuidada y técnica.
  - Ajustes de pulso (0–2), inclinación, interlínea, alineación y ajuste de línea. «Unir letras» escribe cada palabra sin levantar el rotulador.
  - «Otra variación» cambia la letra sin cambiar el texto. Incluye acentos, ñ, ¿ y ¡.
- **Rotuladores**: color, nombre y grosor físico. Cada objeto tiene un rotulador de contorno y otro de relleno.
  - Clic para elegir, doble clic para cambiar el color con el selector de color de macOS, ⌫ para borrar.
  - Funcionan como capas: el de arriba de la lista se dibuja el último y queda encima. Al arrastrar un rotulador para reordenarlo, sus objetos se reordenan con él.
- **Rellenos**: rayado, cruzado, zigzag continuo (sin levantar el rotulador), sólido, concéntrico, puntillismo y ondas. Se pueden ajustar la separación, el ángulo y el margen.
- **Formas y generativos**: rectángulo, elipse, polígono, estrella, corazón, espiral, espirógrafo, Lissajous y rosa polar.
- **Mano alzada**: el trazo se suaviza automáticamente. Si lo cierras, se puede rellenar.
- **Foto → dibujo** (tecla I o arrastrar la foto), en una sola hoja con la vista «Dibujo» en vivo: muestra los rellenos y tramas reales con el grosor de cada rotulador.
  - Colores: k-means; cada color se convierte en un rotulador con relleno. Sin contorno por defecto y con todos los rayados en el mismo ángulo (ajustable, o uno distinto por color).
  - Mezcla: superpone tramas de rayado o puntos de varios rotuladores (CMYK, RYB o los colores de la foto) para mezclar los colores en el ojo, con colores más vivos que un relleno plano. Prefiere el negro para las sombras.
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
| V · P · T · S | Seleccionar · Mano alzada · Texto · Formas |
| I · ⇧⌘I | Foto → dibujo |
| ⌘Z / ⇧⌘Z | Deshacer / Rehacer |
| ⌘D · ⌫ | Duplicar · Eliminar (en la lista de rotuladores, borra el rotulador) |
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
- `crates/vectorio-app`: interfaz eframe/egui (`app`, `canvas`, `commands`, `inspector`, `vectorize_ui`, `colorpanel`, `theme`).
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
