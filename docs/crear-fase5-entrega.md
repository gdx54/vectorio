# Crear · fase 5 · entrega

14 de noviembre de 2026.

## Escenas

**Ciudad en fiesta** conserva sus eventos, distritos, costa, avenida diagonal,
vida en la calle, control de rótulos y juego de bolígrafos. El detalle de su
revisión visual y los ajustes sucesivos está en `plan-crear-fase5.md`.

**Cúpulas** añade una receta independiente, editable desde Crear y guardada
con el documento como las demás. Las referencias aportadas por el usuario
orientan la jerarquía de tamaños, los mundos habitados y los protagonistas;
no se incorporan imágenes raster, marcas ni personajes de las referencias.

- Cinco a nueve cúpulas semiesféricas con zócalo, nervios de vidrio y pasarelas.
- Seis interiores: pueblo con torre, jardín con fuente, cancha, mercado,
  huerto y laboratorio. La esfera recorta sus geometrías y el cristal trasero
  oculta el decorado, conservando visibles los objetos interiores.
- Mundo Subacuático con superficie, barca, peces, burbujas, arrecife, rocas,
  buzos y pequeño submarino. Ballena, calamar o reptil marino como protagonista.
- Mundo Espacial con planetas, galaxia, estrellas, horizonte lunar, cráteres,
  estación y astronautas. Cohete, estación orbital o ballena estelar.
- Se puede prescindir del protagonista, regular gente y fondo y elegir cuatro
  vistas. Los candados y la semilla siguen el comportamiento común de Crear.
- Juegos Abismo, Órbita y Bolígrafo; reducción de tintas y reutilización de
  rotuladores sin mezclar objetos con tintas. El cambio de mundo conserva la
  elección de tintas del usuario.

Las piezas compartidas ganan rasgos proyectados sobre la piel de las criaturas
para evitar ojos y bocas cortados por su propio cuerpo, y ventanillas de cohete.
El vidrio usa la parte «agua», que cada juego resuelve en su tinta correspondiente.

## Comprobaciones

- `cargo test --workspace --release`: 214 pruebas superadas (171 del núcleo,
  42 de interfaz y una de documentación), ocho comprobaciones manuales ignoradas
  por defecto. Las dos pruebas de 500 semillas se ejecutaron explícitamente.
- Barrido de 500 semillas por cada uno de los cinco generadores: coordenadas
  finitas dentro del área y menos de 80 m; sin avisos de construcción en Pueblo,
  Ciudad ni Cúpulas.
- Barrido adicional de Cúpulas: 500 semillas en Preview y Final. Máximos de
  13,50 ms y 13,15 ms respectivamente en este equipo, por debajo de 250 ms y
  1,5 s. Estos tiempos miden generación, sin renderizado de la interfaz.
- Casos específicos de ambos mundos, todos los protagonistas, cuatro vistas,
  cinco a nueve cúpulas, papel vertical/horizontal y rotulador de 0,7 mm;
  determinismo, conexiones e interiores diferentes; presupuesto de caras y
  segmentos. Regresión de ojo completo sobre el cuerpo curvado en cuatro vistas.
- Todos los ejemplos compilados en release.
- `cargo clippy --workspace --all-targets` termina correctamente. Conserva avisos
  anteriores en `doc`, `fill`, `optimize`, `raster`, `svg_in`, `vectorize`,
  `muestra_texto`, `canvas` y una prueba de `vectorize_ui`; sin avisos en Cúpulas
  ni en las piezas modificadas aquí. La ejecución con `-D warnings` queda
  bloqueada por esos avisos existentes, no por esta escena.

## Revisión visual y archivos

Láminas individuales y doce semillas revisadas por cada mundo en
`target/phase5/domes`, con hojas `contactos-underwater.svg` y
`contactos-space.svg`. No se observan fugas de fondo a través de los interiores
ni recortes inesperados en las cúpulas en estas 24 láminas. Los protagonistas
cruzan detrás de los mundos; algunas ventanillas quedan ocultas por ellos.
Capturas del propio egui, sin abrir otra instancia ni tocar la ventana del
usuario: `target/phase5/cupulas-{underwater,space}-{980,1440}.png`.

La adaptación es de dibujo lineal para bolígrafo: no reproduce las grandes
masas de color de las referencias. Los protagonistas se cruzan por detrás de
las cúpulas y quedan ocultos donde corresponde; el coral y parte de las
criaturas pueden continuar a sangre en el borde de la hoja.

Instalación completada y firma verificada; la huella de fuentes coincide con
la instalación. Se actualiza con `python3 scripts/install-macos.py` en
`/Applications/Vectorio.app`. La ventana abierta conserva su versión hasta
que el usuario vuelva a abrir la aplicación.
