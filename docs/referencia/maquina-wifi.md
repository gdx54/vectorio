# Máquina por Wi-Fi · Raspberry Pi junto al plóter

Vectorio puede trazar en la EasyDraw/MakerQ (placa EiBotBoard) a través de una Raspberry Pi Zero 2 W conectada a la placa por USB. Es la misma Raspberry y el mismo servicio que usa Linear (`linear-bridge`, puerto 8765), así que las dos apps comparten la máquina sin pisarse: solo una la usa a la vez.

## Uso

1. Máquina → «Máquina: pluma, origen y recorrido» → **Conexión: Wi-Fi (Raspberry Pi)** y el nombre de la Raspberry (`pizero.local`) o su IP. Se guarda en las preferencias (`machine.wifi`, `machine.wifi_host`).
2. **Conectar** o **Trazar en la máquina**, como por USB. La primera vez macOS pide permiso de **red local** para Vectorio: hay que aceptarlo (Ajustes del Sistema → Privacidad y seguridad → Red local).
3. Pausar, Continuar, Parar, cambios de tinta, botón de la placa, secado, pausa programada, Volver al origen y Desconectar al terminar funcionan igual que por USB.

## Cómo funciona

La sesión de la máquina (`vectorio_core::machine::session`) es un hilo que recibe órdenes (`Command`) y devuelve avisos (`Notice`). La interfaz la usa igual por los dos caminos:

- **USB**: `session::run` con `SerialLink` en el Mac.
- **Wi-Fi**: `remote::run` abre una sesión en la Raspberry, reenvía las órdenes por HTTP y recoge los avisos. En la Raspberry, `vectorio-machine` ejecuta ese mismo `session::run` con la placa en `/dev/ttyACM0`.

El trazado corre en la Raspberry: si se corta el Wi-Fi, sigue hasta terminar. Pausa, sigue y para se leen del `Control` del trazado y se reenvían; las pausas que decide la máquina (cambio de tinta, botón) vuelven como avisos y se reflejan en ese `Control`. Si la Raspberry no responde durante 30 s, la interfaz da la conexión por perdida.

Protocolo del servicio (solo red local):

| Petición | Qué hace |
| --- | --- |
| `POST /session` `{"app":"vectorio","settings":{…}}` | Abre la sesión en la primera placa `/dev/ttyACM*`. 409 si la máquina está ocupada (también por Linear), 503 sin placa, 501 sin `vectorio-machine`. |
| `POST /session/<id>/send` | Una orden `Wire` en JSON (`"Home"`, `{"Pen":true}`, `{"Plot":{"plan":…,"footprints":…,"motion":…}}`, `"Pause"`, `"Resume"`, `"Stop"`, `"Disconnect"`…). |
| `GET /session/<id>?after=N` | Avisos desde el número N, si sigue abierta y el número siguiente. |

El JSON del plan no lleva las huellas de secado, así que van aparte (`footprints`) y la Raspberry las devuelve al plan. Al salir, Vectorio para el trazado y cierra la sesión, así la placa queda libre al momento (para Linear, por ejemplo). Si no llega a hacerlo (se cuelga o pierde la red), una sesión que nadie consulta durante 45 segundos recibe `Idle`: suelta los motores, termina el trazado en curso o para uno en pausa.

## Instalar o actualizar en la Raspberry

```bash
cargo zigbuild -p vectorio-core --release --target aarch64-unknown-linux-gnu.2.36 --bin vectorio-machine
```

```bash
scp target/aarch64-unknown-linux-gnu/release/vectorio-machine raul@pizero.local:linear-plotter/
```

Necesita `zig` y `cargo-zigbuild` (`brew install zig`, `cargo install cargo-zigbuild`) y el destino `rustup target add aarch64-unknown-linux-gnu`. No hace falta reiniciar el servicio: cada sesión arranca `vectorio-machine` de nuevo. No lo sustituyas mientras traza. El servicio (`bridge.py`) vive en el repositorio de Linear (`scripts/easydraw/`).

`curl http://pizero.local:8765/info` indica `"sessions": true` cuando el motor de Vectorio está instalado.

## Pruebas

- `machine::remote::tests`: cliente contra un servicio simulado (órdenes, avisos, pausas en los dos sentidos).
- `crates/vectorio-core/tests/machine_helper.rs`: `vectorio-machine` sin placa avisa y termina.
- En Linear, `tests/easydraw-bridge.py` prueba las sesiones del servicio con un motor simulado.
