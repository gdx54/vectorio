# Vectorio · identidad

Concepto elegido por el usuario: **E · Relieve**, con la paleta **A02 · Limón suave / índigo**. Una V de curvas de nivel vinculada al trazo continuo y a los recorridos de un plotter. Fondo de referencia `#F6F095`, contornos `#35216C`.

- `propuestas.png`: exploración de seis conceptos.
- `vectorio-icon.png`: icono elegido, 1024 × 1024, con transparencia exterior.
- `vectorio-icon-relieve-marfil.png`: versión anterior conservada.
- `paletas-a-amarillos.png`: comparativa en la que se eligió A02.
- `../../packaging/macos/Vectorio.icns`: icono multirresolución instalado en macOS.

Imágenes generadas y refinadas con la herramienta integrada **image_gen**. El icono se convierte a `.icns` durante su preparación para macOS. La aplicación y el instalador utilizan los archivos guardados en este proyecto.

## Prompts utilizados

### Exploración

```text
Use case: logo-brand. Asset type: one polished visual brainstorming board for the Mac app Vectorio. Create a landscape 3-column by 2-row contact sheet presenting SIX genuinely different logo and app-icon concepts, clearly labelled A, B, C, D, E, F in reading order, and a small wordmark "Vectorio" beneath each symbol. Vectorio is a creative pen-plotter desktop tool: single-line handwriting, beautiful generative curves, multicolor physical marker pens, hatching, stippling, vector paths. Spirit: precise and playful, hand meets machine, a creative atelier rather than a corporate vector editor.
A: "Trazo" — an elegant memorable V drawn with one continuous rounded ink line, a small ending loop recalling handwriting.
B: "Trama" — compact bold V made of interwoven cyan, magenta and warm yellow plotter strokes, graphic and confident.
C: "Punta" — distinctive stylized felt-tip pen nib and plotted curve suggesting V, in ink-black and deep ultramarine.
D: "Órbita" — a disciplined spirograph rosette constructed from only a few clean continuous loops, warm coral against ink.
E: "Relieve" — a sculptural V formed by evenly spaced nested contour lines, indigo on ivory, crisp rather than overly intricate.
F: "Vuelo" — a fluid single-line folded bird whose wings subtly form a V, dark teal with one small orange stroke.
For each cell show a large primary symbol and a very small corresponding rounded-square macOS app-icon adaptation at its lower right, so the user can judge Dock readability. A restrained cream-white presentation background, generous whitespace, subtle neutral hairline separators between cells, consistent scale, exceptionally clean edges, minimal text. Symbols must feel professionally designed, recognisable and vector-friendly. Make the six silhouettes clearly distinct. Avoid generic app sparkles, arbitrary arrows, cliché bezier-node diagrams, fake 3D extrusion, metallic effects, excessive shadows, stock-logo clip art, watermarks, or any names other than the exact word "Vectorio" and labels A-F. The app icon thumbnails can have flat softly rounded tile backgrounds; the main symbols remain unboxed.
```

### Icono elegido E · Relieve

```text
Use case: logo-brand. Edit the reference brainstorming board into ONE finished macOS application icon for Vectorio, using ONLY concept E "Relieve" from the bottom middle. Preserve that exact concept: a wide softly rounded V made from about seven evenly spaced nested indigo contour strokes, with smooth rounded U-shaped turns at its bottom and rounded upper ends. Keep its distinctive silhouette, rhythm, stroke weight, and deep indigo-on-ivory colors. Output a single square 1024 by 1024 icon image. Center the V inside a warm ivory rounded-square macOS-style tile, occupying about 67% of the full canvas width. The tile itself fills about 88% of the canvas, with softly continuous rounded corners and transparent exterior margins. Very subtle surface depth at most; clean and flat enough for the finely drawn mark to be clear in the Dock. No external drop shadow. No typography, labels, wordmark, captions, letters A-F, separators, contact sheet, extra icons, or other concepts. Transparent background outside the ivory tile. This is the final production icon, crisp edges and perfectly regular spacing, faithful to E.
```

### Refinamiento del contorno

```text
Use case: precise-object-edit. Refine this approved Vectorio macOS icon for production. Keep the indigo nested-line V mark exactly unchanged, its proportions, stroke count, spacing, position, size, and ivory tile color. Change ONLY the outer silhouette/alpha cutout: the current image has irregular white speckles and ragged residual pixels outside and around the rounded-square tile, especially along the bottom and right. REMOVE ALL those artifacts. Deliver a perfectly smooth continuous rounded-square tile with clean anti-aliased alpha edges and fully transparent exterior, zero stray pixels, zero white fringe, no paper texture, no external shadow. The tile must be precisely centered with even transparent margin on all four sides. Flat solid ivory surface. Square canvas, suitable for conversion to a macOS icns. No text and no new elements.
```

