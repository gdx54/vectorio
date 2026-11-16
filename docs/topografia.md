# Topografía · Fast Marching en Rust

En **Foto → dibujo → Estilo → Topografía**, abre una foto, ajusta el número de
curvas y elige dónde nace la onda con Origen X/Y o «Elegir origen en la imagen».
Gamma alta concentra las curvas en los medios tonos oscuros. Velocidad mínima
baja aprieta las sombras. «Suavizar foto» reduce el detalle fino antes de resolver
el campo. «Simplificar» reduce puntos de las polilíneas, sin añadir rellenos.

El motor se ejecuta en el hilo de vectorización existente. No necesita navegador,
JavaScript, Python, skfmm, d3 ni nuevas dependencias de Cargo para funcionar.
Cada resultado es un objeto de trazos en una tinta; se puede recolorear, escalar,
exportar a SVG y planificar desde Salida con las herramientas normales de Vectorio.
El grosor indicado se aplica a las tintas nuevas; una tinta existente conserva el
grosor físico que tenga en el documento, que también utiliza la vista previa.

## Algoritmo y procedencia

Adaptación de [FastMarchingTopoPlot](https://github.com/rolandblok/FastMarchingTopoPlot)
de Roland Blok. Consultado el 14 de noviembre de 2026:

- `worker.js`, blob `71fd1c43dbeb48cbf0bdcf9d7bc30f363f5e946b`: gaussiano
  separable, velocidad por brillo y resolución Fast Marching de la ecuación
  eikonal `|∇T| F = 1` con cola de prioridad.
- `script.js`, blob `63c641ecc7d58d29f60db1a07692834ad130d6ab`: composición del
  alfa sobre blanco, gris Rec. 601, niveles uniformes en el intervalo de tiempos y
  recorte de la salida a las zonas opacas.
- Licencia **Unlicense**, conservada en
  `third_party/FastMarchingTopoPlot/UNLICENSE`.

Implementación en `crates/vectorio-core/src/topo.rs`, con `BinaryHeap`, mapas
contiguos, gaussiano y extracción de curvas en paralelo con Rayon. Reutiliza el
marching squares de `raster.rs` para evitar depender de d3. Las comprobaciones
numéricas incluyen un mapa variable de tiempos calculado con el JavaScript
original y un campo constante con simetría y velocidad conocidas.

Se mantiene la condición upwind causal: si la solución cuadrática quedase antes
del mayor vecino aceptado, se usa la actualización de un solo eje. Esto corrige
el caso límite `1/F < |Tx−Ty| <= sqrt(2)/F` del original.

Las curvas que llegan al borde quedan abiertas. No se añaden las aristas del
rectángulo exterior que d3 utiliza para cerrar los polígonos de supernivel: así
el plotter no repasa el mismo marco una vez por nivel.

## Transparencia y proyectos

El alfa se conserva al reducir la imagen. El color transparente se compone sobre
blanco para calcular la velocidad, y la onda puede atravesar ese fondo. Si está
activado «Omitir zonas transparentes», las curvas se recortan al alfa >= 0,5.
El recorte se hace después de simplificar y examina también el interior de los
segmentos, para que una línea larga no salte a través de un agujero transparente.

Las fotos con transparencia se guardan como PNG dentro de `.vectorio`; las opacas
siguen usando JPEG. El lector reconoce ambos por su contenido y abre también
los documentos anteriores. Los parámetros de Topografía y la foto se conservan
en el grupo para regenerarlo con «Volver a separar colores…» desde Capas.

Los valores iniciales son 200 niveles, gamma 1, velocidad adicional 0,1 y origen
en el centro. La resolución es la común del importador. Con muchos niveles,
valores bajos de velocidad o un dibujo pequeño, conviene comprobar la huella
real de la punta y el mapa de tinta en Salida.

## Muestra reproducible

```sh
cargo run -p vectorio-core --release --example muestra_topografia -- foto.png salida.svg
```

El ejemplo escribe un SVG de 200 mm de lado mayor y una vista PNG de sus mismos
trazos y grosores. Los argumentos opcionales son niveles, gamma y velocidad:

```sh
cargo run -p vectorio-core --release --example muestra_topografia -- foto.png salida.svg 200 1.0 0.1
```
