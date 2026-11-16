# Referencias anteriores a la Fase A

Estos SVG se capturaron el 16 de noviembre de 2026, antes de modificar el motor, ejecutando:

```sh
cargo run -p vectorio-core --release --example volumetric -- /tmp/vectorio-phase-a-before/volumetric
cargo run -p vectorio-core --release --example volumetric_arte -- /tmp/vectorio-phase-a-before/volumetric_arte
cargo run -p vectorio-core --release --example sombra_plana -- /tmp/vectorio-phase-a-before/sombra_plana
```

Los tres ejemplos usaron el toro y los parámetros predeterminados del código existente. La receta `../recipe-v1.json` es el `atlas.json` generado por la primera ejecución, sin modificar.

`tests/volumetric_phase_a.rs` compara los bytes completos con estas referencias. No se deben regenerar para aceptar accidentalmente cambios del algoritmo durante un refactor.
