# Vectorio

Aplicación de escritorio en Rust para crear dibujos vectoriales con plotter de rotuladores.

![Vectorio · Relieve](assets/branding/vectorio-icon.png)

Cuatro espacios comparten el mismo documento:

- **Diseño**: texto de un solo trazo, formas, mano alzada, capas y tintas independientes.
- **Crear**: escenas y generadores con semillas reproducibles, juegos de color y vista previa.
- **Volumen**: modelos OBJ/STL, pasadas combinables, sombras, mapas de lectura, mezcla óptica, isolíneas, perspectiva y hojas multivista. Los objetos 3D conservan su receta editable.
- **Salida**: recorridos por tinta, simulación, planificación con tiempos de secado y exportación SVG en milímetros.

La interfaz está disponible en español e inglés. La instalación y los controles nativos están preparados para macOS. El envío directo al plotter todavía no está implementado.

## Compilar y probar

Necesitas Rust estable, Python 3 y las herramientas de línea de comandos de Xcode en macOS.

```sh
cargo test --workspace --locked
cargo run --release -p vectorio-app
```

Si Rust está instalado mediante Homebrew y `cargo` no aparece en el terminal:

```sh
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
```

## Instalar en macOS

```sh
python3 scripts/install-macos.py
```

Instala o actualiza `/Applications/Vectorio.app`. La aplicación instalada abre directamente su binario: no necesita leer el proyecto ni recompilar al arrancar. Una ventana abierta conserva su versión hasta el siguiente inicio. El instalador mantiene la versión anterior si la compilación falla.

El paquete generado tiene firma local ad hoc; todavía no se publica un instalador notarizado por Apple.

## Espacio en disco

`target/` contiene archivos generados y está excluido de Git. El perfil de desarrollo desactiva los símbolos de depuración y las copias incrementales para limitar su tamaño. Las dependencias compiladas siguen ocupando espacio y se pueden regenerar.

```sh
cargo clean
```

Este comando elimina también los ejemplos y capturas generados dentro de `target/`. Conserva antes cualquier resultado que quieras guardar. No elimina la aplicación instalada en Aplicaciones ni los proyectos guardados fuera de `target/`.

## Documentación

- [Guía de funciones, instalación y atajos](LEEME.md).
- [Plan y arquitectura de Volumen](docs/plan-volumen-rediseno.md).
- [Juegos de tintas, mezcla óptica e isolíneas](docs/volumen-ampliacion-g.md).
- [Formato del plan de salida](docs/formato-salida.md).

El núcleo está en `crates/vectorio-core` y la interfaz egui en `crates/vectorio-app`. Los proyectos `.vectorio` conservan sus objetos, tintas y recetas.

## Licencias y créditos

El código de Vectorio se distribuye bajo [MIT](LICENSE). Las tipografías de terceros conservan [SIL Open Font License 1.1](crates/vectorio-core/fonts/OFL.txt). La adaptación de FastMarchingTopoPlot conserva su [Unlicense](third_party/FastMarchingTopoPlot/UNLICENSE). Los avisos de meshopt-rs y meshoptimizer se incluyen en [VOLUMETRIC_NOTICES.txt](packaging/macos/VOLUMETRIC_NOTICES.txt).
