# Crear · dirección artística y arquitectura procedural

14 de noviembre de 2026.

## Resultado

El registro de Crear pasa de cinco a siete generadores. **Megastructuras** ofrece
Torre estratificada, Patio interior y Gran pórtico. **Puerto y ciudadela** ofrece
Pesquero, Almacenes y Ciudadela. Las recetas conservan semilla, parámetros,
candados, objetos e identidad de tintas al guardarse en el documento.

`generate/composition.rs` aporta semillas por parcela, ritmos agrupados con
pausas y una envolvente de altura relativa a la cámara. El puerto deja libres
la plaza y la calle transversal, agrupa casas y amarres y reserva el lugar de
las torres. Megastructuras alterna bandas de alturas desiguales, retranqueos,
galerías, lamas y huecos. Manzanas recibe cubiertas y una jerarquía de alturas;
la ciudad organiza mejor la sucesión de alturas vecinas.

## Motor y materiales

`generate/massing.rs` construye una masa por celdas, con relleno y vaciado.
Extrae su piel en seis direcciones y fusiona celdas coplanarias del mismo
material en rectángulos. Las aristas se calculan antes de fusionar las caras:
las juntas en T y las celdas interiores no producen una cuadrícula de líneas.
Los tramos contiguos se unen antes de pasar por la ocultación analítica del
motor existente. La sombra es la unión de las sombras de la piel, conservando
los vacíos del edificio; no se sustituye por la envolvente de toda la torre.

El estilo compartido añade **Intensidad de tinta**, de 0 a 1,5. Aumenta la
presencia de cubiertas, vegetación y sombras; adapta la trama principal a un
mínimo de 1,45 veces el ancho del rotulador. Los faldones oscuros reciben una
segunda familia más abierta. Las caras al sol conservan papel blanco.
Las cubiertas horizontales también reciben color y las ventanas siguen
apartando la trama del muro.

Las escenas densas reservan el tejado al papel cálido de la gente y la sombra
al papel frío del agua, en lugar de devolver ambos a la misma trama.
Los juegos clásicos de bolígrafo siguen usando negro, azul, rojo y verde,
con amarillo opcional. Las escenas sencillas nuevas ofrecen cinco tintas de
serie. La elección y los cambios de tintas del usuario se siguen conservando.

Referencia técnica estudiada: [SVGeometric, de jarvisar](https://github.com/jarvisar/geometric),
especialmente el motor ortográfico, la masa por rejilla y la composición por
bandas y familias de detalles. La implementación añadida aquí es Rust propio;
no incorpora archivos ni una traducción literal del código JavaScript.

## Validación

- `cargo test --workspace --release`: 223 pruebas superadas; ocho pruebas de
  comprobación manual ignoradas por defecto.
- Barrido explícito de 500 semillas por cada uno de los siete generadores:
  3.500 dibujos, finitos, dentro del área y por debajo de 80 m. Peores tiempos
  en ese barrido: Megastructuras 35,76 ms y Puerto 13,66 ms.
- Casos adicionales de las seis composiciones nuevas, las cuatro vistas,
  papeles verticales y apaisados y rotuladores de 0,25 y 0,7 mm. Se comprueba
  la recuperación de geometría, partes y tintas después de guardar.
- Prueba de masa unida: nueve aristas en un bloque sin juntas internas;
  el vaciado de una esquina revela geometría adicional y es determinista.
- Prueba de riqueza: más longitud de tinta que a intensidad cero y colores
  diferentes para cubierta y sombra en los juegos de serie.
- Todos los ejemplos compilados en release. Clippy finaliza con avisos
  existentes fuera de los módulos añadidos.
- Nueve láminas revisadas en `target/art-direction`; SVG y PNG con las tintas
  y anchos reales del juego. El ejemplo `muestra_receta` admite `palette=`
  y `pens=`. Capturas del propio egui para Megastructuras en 980 y 1440 px
  en `target/phase5`, sin tocar la sesión abierta del usuario.

En la muestra cuadrada de Pueblo blanco, semilla 4, se pasa de 22,37 m antes
del cambio a 30,62 m, con más presencia de tejados y sombras. No es una medida
universal: longitud y encuadre cambian con el papel y los parámetros.

## Instalación

Se ejecutó `python3 scripts/install-macos.py`. La compilación release terminó
correctamente, pero la creación de la carpeta de preparación en `/Applications`
fue bloqueada por los permisos del entorno (`Operation not permitted`).
La aplicación anterior no se reemplazó y no se cerraron ventanas.

Para completar la instalación desde Terminal en el Mac:

```sh
python3 /Users/raullorenzo/Documents/Vectorio/scripts/install-macos.py
```
