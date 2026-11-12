# Plan · Material Liquid Glass coherente (claro y oscuro)

Plan para implementarlo por fases (Sonnet). Cada fase compila, pasa `cargo test` y
`cargo clippy --all-targets -- -D warnings` y se instala con `python3 scripts/install-macos.py`.
Conserva el español como idioma fuente, `tr!/ts!/tm!` en cualquier texto nuevo y la
separación objetos/capas ↔ tintas. No cambia ningún comportamiento, solo el aspecto.

## 1. Diagnóstico (capturas del 12-11-2026)

Lo que hay hoy en `theme.rs` es un panel con relleno en **degradado vertical**
(`glass_top` → `glass`), un **brillo solo arriba** (`rim_top` → `rim_bottom`), un trazo
de 1 px y una sombra. No es un único material; cada pieza se pinta a su manera:

| Pieza | Cómo se pinta hoy | Problema |
|---|---|---|
| Barra superior (`app.rs::toolbar`) | `TopBottomPanel` opaco `pal.bar` y línea separadora | Es una barra clásica de macOS 14. En Liquid Glass la barra es transparente sobre el contenido y cada grupo de controles flota en su propia cápsula de vidrio. |
| Paneles laterales y de estado | `glass_shape` | En **oscuro**, el degradado 66→34 sobre un lienzo de 30 se lee como una caja gris turbia con un bisel arriba, más estilo 2010 que vidrio. En **claro**, el vidrio casi blanco sobre el lienzo 232 queda como una tarjeta blanca plana, y el brillo blanco sobre blanco no se ve. |
| Lienzo | Color liso, y `canvas::fit` aparta el papel de los paneles | No hay nada detrás del vidrio. Como egui no desenfoca, la translucidez no aporta nada y solo ensucia. |
| Segmentados (`segmented`, `choice`, `text_segment`) | Fondo opaco `pal.control` y selección en rectángulo r = 8 | En oscuro es gris sobre gris y la selección no destaca. No son cápsulas. |
| Botón de acento (`accent_button`) | Rectángulo azul plano | En macOS 26 es una cápsula de vidrio teñido con brillo. |
| Tarjetas de tinta (`inspector::paint_pen_card`) | `pal.card` 64,64,70 casi opaco en oscuro | Bloques pesados que compiten con la banda de color. |
| Campos y desplegables | Rectángulos `pal.control` opacos y el ▼ de egui | Son opacos sobre el vidrio, y el triángulo no es de Mac. |
| Hojas (Ajustes, Atajos, Color, Foto) y avisos | `glass_frame` = `pal.bar` opaco | Cada una con un margen y un radio propios. |
| Radios | Panel 18, margen interior 14, control 8, segmentado 10, tarjeta 12 | No son concéntricos: 18 − 14 = 4 ≠ 8. |
| Texto secundario | Gris opaco 152 (oscuro) o 110 (claro), a 10,5–11,5 pt | Le falta contraste en oscuro, sobre todo en las tarjetas y en «Ideas para empezar». |

En resumen, el claro «casi llega» porque la sombra separa el panel del fondo. El oscuro
falla porque en oscuro la sombra no se ve, y lo único que define el panel es un
degradado gris con un brillo arriba.

## 2. Principios del material

egui 0.29 no puede leer ni desenfocar lo que hay debajo, así que el vidrio se simula con
cuatro capas, **siempre las mismas** para todo lo que flote:

1. **Sombra**: doble.
   - Una de contacto, corta y nítida, para definir el borde.
   - Otra ambiental, amplia y suave, para despegarlo del fondo.
   - En oscuro, la de contacto pasa a ser un trazo exterior negro, porque una sombra negra sobre negro no se ve.
2. **Cuerpo**: un tinte **casi uniforme** y translúcido, con un degradado vertical muy leve (≤ 4 % de luminancia), no el actual de ~30 unidades.
3. **Borde especular**: un anillo de 1 px por **todo** el perímetro, cuya intensidad depende de la orientación de la normal.
   - Máximo arriba a la izquierda, un segundo máximo más débil abajo a la derecha y casi nada en los costados.
   - Esa doble luz es lo que hace que se lea como vidrio y no como plástico.
4. **Línea interior de profundidad**: 1 px justo dentro del anillo, oscura y muy tenue, solo en la mitad inferior. Da grosor sin bisel.

Los controles dentro del vidrio usan **rellenos con alfa** (blanco en oscuro, negro en
claro), no grises opacos. Así funcionan sobre cualquier vidrio y sobre el lienzo.

## 3. Fases

### Fase 1 · Un único material en `theme.rs`

1. Sustituye `glass_shape` por una API de materiales:
   ```rust
   pub enum Material { Panel, Capsule, Sheet, Popup, Tinted(Color32) }
   pub fn glass(rect: Rect, radius: f32, m: Material, pal: &Palette) -> egui::Shape
   pub fn glass_panel_with(ui, pal, radius, margin, m: Material, add) -> InnerResponse<R>  // la de ahora más el material
   ```
   - `Panel`: los paneles laterales, el de estado y los de Salida.
   - `Capsule`: los grupos de la barra (radio = alto / 2).
   - `Sheet`: hojas modales, más opaca.
   - `Popup`: menús y desplegables.
   - `Tinted`: el botón de acento.
2. **Borde especular**: reutiliza `rounded_outline` (sube a 12 tramos por esquina para que el anillo no se vea facetado).
   - Para cada vértice calcula la normal exterior `n` y define `k = 0.5 + 0.5·dot(n, (-0.6,-0.8))`. La luz viene de arriba a la izquierda.
   - Intensidad del anillo: `rim_hi·k^3 + rim_lo·(1-k)^3`. El segundo término es el contrabrillo abajo a la derecha.
   - Banda de 1,0 px (no 1,25), con alfa por vértice como ya se hace.
3. **Línea interior**: segundo anillo entre `shrink(1.0)` y `shrink(2.0)` con `pal.inner_shade`, multiplicado por `smoothstep(0.4, 1.0, y_rel)`.
4. **Sombra doble**: contacto `Shadow{offset:(0,1), blur:3}` con `shadow_contact`, y ambiental `Shadow{offset:(0,10), blur:32}` con `shadow`.
5. **Paleta nueva**. Sustituye `glass_top`, `rim_top`, `rim_bottom` y `card` por los campos de esta tabla. Son valores de partida, a ajustar con capturas.

   | Campo | Claro | Oscuro |
   |---|---|---|
   | `canvas` (arriba → abajo, degradado muy suave) | 238,238,242 → 230,230,235 | 22,22,25 → 16,16,18 |
   | `glass` (Panel) | 255,255,255 α 0,74 | 44,44,50 α 0,72 |
   | `glass_sheet` | 250,250,252 α 0,94 | 38,38,42 α 0,94 |
   | `glass_tint_top` (degradado del cuerpo, se mezcla arriba) | blanco α 0,10 | blanco α 0,04 |
   | `rim_hi` / `rim_lo` | blanco α 1,0 / blanco α 0,55 | blanco α 0,34 / blanco α 0,14 |
   | `inner_shade` | negro α 0,05 | negro α 0,35 |
   | `outline` (trazo exterior) | negro α 0,07 | negro α 0,60 |
   | `shadow` / `shadow_contact` | negro α 0,12 / 0,08 | negro α 0,45 / 0,50 |
   | `fill_control` (campos, segmentado sin seleccionar) | negro α 0,05 | blanco α 0,08 |
   | `fill_hover` | negro α 0,08 | blanco α 0,12 |
   | `fill_card` (tarjeta de tinta) | blanco α 0,70 | blanco α 0,07 |
   | `knob` (segmento seleccionado) | blanco opaco, con sombra de contacto | blanco α 0,20, con anillo `rim_hi` |
   | `text` | 29,29,31 | 245,245,247 |
   | `secondary` | 96,96,104 | 174,174,182 |
   | `tertiary` (pistas, «Ideas…») | 132,132,140 | 128,128,136 |
   | `separator` | negro α 0,08 | blanco α 0,10 |
   | `accent` | 0,122,255 | 10,132,255 |

   - El oscuro baja el lienzo a 16–22 y sube el vidrio a 44–50: el panel queda **por encima** del fondo, no al mismo nivel.
   - El claro baja un poco el lienzo para que el vidrio blanco contraste.
   - Aplica el degradado del lienzo con un `Mesh` de 4 vértices en `canvas::show` y `output::show`, en lugar de `fill(pal.canvas)`. Deja `pal.canvas` como color medio para los usos que ya tiene (atenuado de controles desactivados, `vectorize_ui.rs:602`).
6. **Radios concéntricos**:
   - `PANEL_RADIUS = 22`, `PANEL_PAD = 12` (sustituye a `PANEL_MARGIN + 4`) y `CONTROL_RADIUS = PANEL_RADIUS - PANEL_PAD = 10`.
   - Las tarjetas de tinta usan `CONTROL_RADIUS + 2`.
   - Las cápsulas usan alto / 2.
   - Las hojas usan `SHEET_RADIUS = 26` con `SHEET_PAD = 20`.
   - Pon todo en `theme.rs` y quita los números sueltos: `22.0` del estado, `20.0` del aviso y `Margin::same(22.0)` y `18.0` de las hojas.
7. **Pruebas** en `theme.rs`:
   - Contraste WCAG: compón `text`, `secondary` y `tertiary` sobre `glass` encima del `canvas` (alfa sobre opaco).
     - `text` ≥ 7:1.
     - `secondary` ≥ 4,5:1.
     - `tertiary` ≥ 3:1.
     - Comprueba claro y oscuro.
   - El anillo es simétrico respecto a la diagonal y el máximo cae arriba a la izquierda.

### Fase 2 · Barra transparente con cápsulas (`app.rs`)

1. Quita el `TopBottomPanel` opaco.
   - El lienzo (`CentralPanel`) ocupa toda la ventana.
   - La barra pasa a ser un `egui::Area` con `Order::Middle` y alto `TOOLBAR_H`.
   - Mantén el arrastre de ventana y el doble clic sobre el hueco vacío: ahora el `interact` va sobre el rectángulo de la barra **antes** de los controles, como hoy.
2. **Efecto de borde superior**: un degradado del color del lienzo de α 0,85 a 0 en los primeros `TOOLBAR_H + 12` px.
   - Va pintado en la capa del lienzo, por encima del papel y por debajo de la barra.
   - Así el título y los botones se leen aunque el papel pase por debajo.
3. Cada grupo de la barra se pinta con `Material::Capsule`, alto 34 y radio 17: las herramientas, Foto/Abrir, Deshacer/Rehacer y Diseño/Salida.
   - Reescribe `segmented` con el fondo de cápsula de vidrio, `fill_control` dentro y padding 3.
   - Reescribe `paint_segment_bg` con un **pomo** en forma de cápsula (`knob`): sombra de contacto en claro y anillo especular en oscuro.
   - Anima el pomo entre segmentos con `ctx.animate_value_with_time(id, x, 0.18)`. Es opcional pero muy de macOS 26.
   - El texto del segmento seleccionado va en `text` y semibold. El resto, en `secondary`.
4. El título «Vectorio / Sin título» se queda sin cápsula, en `text` y `secondary`, como en Finder.
5. `canvas::fit`: el papel se ajusta debajo de la barra (añade `TOOLBAR_H` al margen superior), porque ahora el lienzo empieza en y = 0.
   - Revisa `canvas.rs:121` y las coordenadas que restan la barra.
   - Revisa también `help_sheet`, el ancla del aviso y `floating_panels` (`top`).
6. **Modo Salida**: lo mismo. `output::show` no debe dejar una franja opaca.

### Fase 3 · Controles y contenido sobre el vidrio

1. **`visuals()`**:
   - `widgets.inactive` = `fill_control`, `hovered` = `fill_hover` y `active` = `accent` α 0,18.
   - `rounding` = `CONTROL_RADIUS` en general; botones pequeños (`small_button`, «Ajustar») en cápsula.
   - `window_fill`/`menu` usan el color del material `Popup`.
   - `menu_rounding` = 12 y `popup_shadow` = la sombra ambiental.
   - `extreme_bg_color` (campos de texto) = `fill_control` también, no blanco ni negro puros.
2. **`accent_button`**: pasa a cápsula con `Material::Tinted(accent)`.
   - Cuerpo con el acento, la mitad superior mezclada un 10 % con blanco y el anillo especular encima.
   - Texto blanco semibold.
   - Al pasar el ratón, ilumina un 6 %. Al pulsar, oscurece un 8 %.
   - Úsalo también en el «Hecho» de `colorpanel.rs:129`, que hoy lleva su propio `Button`.
3. **Desplegables**: un `icon` propio para `ComboBox` con un chevron arriba/abajo (dos «V» de 4 px en `secondary`) en lugar del ▼.
   - Hazlo como función `theme::combo_icon` y aplícalo a todos los `ComboBox` con `.icon(theme::combo_icon)`.
4. **Tarjetas de tinta** (`paint_pen_card`):
   - Fondo `fill_card` con el mismo anillo especular, en una versión fina, y sin `glass_stroke`.
   - La banda de color se queda, pero el brillo (`sheen`) pasa a un degradado de blanco α 0,22 a 0 en lugar de un rectángulo.
   - El código hex y el grosor van en `tertiary` (11 pt). El nombre, en `text` 12,5 semibold.
   - En `Ghost` (arrastrando) usa `Material::Popup`, no el color de la tarjeta opaco.
5. **`section()`**: 11 pt semibold en `secondary`, con +0,3 de interletraje si egui lo permite. Si no, déjalo así.
   - Los textos de ayuda bajo las secciones van en `tertiary`.
   - Revisa `inspector.rs`, `output.rs` y `prefs_ui.rs`: cualquier `pal.secondary` que sea una pista y no una etiqueta pasa a `tertiary`.
6. **`swatch`** (papel): el anillo de selección en `accent` a 2 px con 2 px de separación, como ahora. El trazo de borde pasa a `outline`.
7. **`close_button`**: cápsula `fill_control`.

### Fase 4 · Hojas, menús y avisos

1. `glass_frame` desaparece y todas las hojas usan `Material::Sheet` con `SHEET_RADIUS` y `SHEET_PAD`.
   - Afecta a Ajustes (`prefs_ui.rs:55`), Atajos (`app.rs:837`), Color (`colorpanel.rs:61`) y Foto (`vectorize_ui.rs:313`).
   - Para ello `glass_panel_with` acepta un `Frame`/margen, o se añade `glass_sheet(ui, pal, add)`.
   - Hoy todas hacen `.fill(pal.bar.to_opaque())`: esa opacidad la da ahora el α 0,94 del material.
2. El velo de las hojas pasa a negro α 0,25 en claro y α 0,45 en oscuro, más un degradado radial suave si es barato. Si no, sin degradado.
3. **Aviso (`toast`)**: `Material::Capsule`, centrado bajo la barra.
4. Los menús de egui (Exportar, desplegables) toman `window_fill`, `window_stroke` = `outline` y la sombra de `visuals()`. No pueden llevar el anillo de malla; con el trazo y la sombra basta.
5. Los círculos de `vectorize_ui.rs:517/523` (`pal.bar`) pasan a `Material::Capsule` circular o a `glass_sheet`.

### Fase 5 · Lo que hay detrás del vidrio (opcional, después de validar 1–4)

egui no desenfoca. Pero sí sabemos qué hay bajo cada panel: el papel, que es un
rectángulo de color conocido.

1. **Papel «visto a través»**: si el rectángulo del papel en pantalla corta un panel o la barra, pinta dentro de la forma del vidrio y **antes** del cuerpo:
   - un `Shadow` con `blur` ≈ 24 y `spread` 0 del rectángulo del papel, recortado al panel;
   - con el color del papel y α 0,55.
   
   Así se ve una mancha clara de bordes difusos, que es el desenfoque del papel. Lo pinta `glass_panel_with` si recibe `backdrop: Option<(Rect, Color32)>`, que `app.rs` calcula a partir de `self.view`.
2. **Ajuste del papel**: `canvas::fit` deja de reservar todo el ancho de los paneles. Encaja el papel entre ellos, pero permite que, al hacer zoom o desplazar, pase por debajo. Ya lo hace; solo cambia que ahora se verá.
3. Mide el coste con un proyecto grande. Si un fotograma tarda > 1 ms más, desactívalo durante el arrastre.

### Fase 6 · Comprobación visual

Haz capturas en claro y oscuro sin tocar las preferencias ni las ventanas del usuario:

```bash
H=$(mktemp -d); mkdir -p "$H/Library/Application Support/Vectorio"
echo '{"appearance":"oscuro"}' > "$H/Library/Application Support/Vectorio/preferencias.json"
HOME="$H" /Applications/Vectorio.app/Contents/MacOS/vectorio &   # instancia propia; ciérrala con kill <pid> al acabar
```

- Usa `screencapture -x -l <windowid>` con el id de la ventana de ese pid. Este proyecto ya ha usado un pequeño Swift con `CGWindowListCopyWindowInfo`.
- **Nunca** cierres otras instancias de Vectorio.

Lista de revisión, en claro y en oscuro:

- [x] Los paneles se separan del lienzo sin mirar la sombra.
- [x] El borde brilla arriba a la izquierda y un poco abajo a la derecha, nunca como un bisel.
- [x] La barra no tiene franja ni línea. Los grupos son cápsulas y el pomo del segmentado se distingue a primera vista.
- [x] Exportar es una cápsula azul con brillo.
- [x] Las tarjetas de tinta pesan menos que su banda de color.
- [x] Los textos de pista se leen en oscuro.
- [x] Ajustes, Atajos, Color y Foto comparten radio, margen y material.
- [x] El modo Salida no tiene franjas opacas.
- [x] Revisa a escala de interfaz 0,85 y 1,25 (Ajustes → General).

Resultado de la revisión (12-11-2026): se corrigieron picos en la sombra de las cápsulas pequeñas, el pomo hueco de los deslizadores, las casillas redondas casi invisibles en claro, el gris de una pasada invisible en el mapa de tinta y el ajuste del papel al arrancar con otra escala.

## 4. Fuera de alcance

- Desenfoque real con un pase propio de wgpu (`egui_wgpu::CallbackTrait` sobre una copia del framebuffer). Es posible, pero cuesta y es frágil en eframe 0.29. Sería un plan aparte si la fase 5 no basta.
- `NSVisualEffectView` nativo detrás de la ventana: solo desenfocaría el escritorio, no el lienzo.
- Cambios de disposición o de flujo, y cambios de iconos (salvo el chevron de los desplegables).

## 5. Reparto sugerido

| Fase | Riesgo | Modelo / razonamiento |
|---|---|---|
| 1 Material y paleta | Medio: es la base de todo | Sonnet, razonamiento alto |
| 2 Barra transparente | Medio: coordenadas y arrastre | Sonnet, alto |
| 3 Controles | Bajo | Sonnet, medio |
| 4 Hojas | Bajo | Sonnet, medio |
| 5 Fondo a través | Medio-alto | Sonnet, alto, u Opus si la 1–4 no convencen |
| 6 Capturas y ajuste fino | — | Opus para el juicio visual final |
