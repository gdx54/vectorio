# Vectorio

- Aplicación Rust de escritorio para plotter. Núcleo en `crates/vectorio-core` e interfaz egui en `crates/vectorio-app`.
- Conserva el idioma español en la interfaz y la separación entre objetos/capas y tintas.
- Usa `/opt/homebrew/opt/rustup/bin` en PATH si `cargo` no está disponible.
- Después de cambios de código, ejecuta las comprobaciones apropiadas y actualiza la instalación de macOS con `python3 scripts/install-macos.py`. La instalación autorizada por el usuario es `/Applications/Vectorio.app`.
- No cierres ni reinicies ventanas con trabajo del usuario. La versión que ya está abierta continúa hasta el siguiente inicio.
- La app instalada comprueba los cambios al abrir mediante `VectorioLauncher`; el instalador conserva el último binario si la compilación falla.
- Para las comprobaciones visuales, prefiere la instalación en Aplicaciones. No crees copias adicionales de la aplicación salvo que sea necesario para preservar una sesión abierta del usuario.
- El logo elegido es **E · Relieve**, paleta **A02 · Limón suave / índigo**: fondo `#F6F095` y V de curvas de nivel `#35216C`. Recursos de identidad en `assets/branding` e icono macOS en `packaging/macos/Vectorio.icns`.
