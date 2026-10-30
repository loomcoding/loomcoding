# Verificación del runtime compartido

> Estado: **implementado** (verificado el 2026-10-30; véase el apartado «Estado» del final y el plan gemelo). Estado original: ejecutada el 2026-10-27; aceptación satisfecha.

## Alcance

Cerrar la aceptación de R1–R5 con pruebas sin red/provider real y ejecutar el contrato global del repositorio.

## Aceptación

1. Cobertura de protocolo A2A/tenant/tarjetas, ejecución y aislamiento entre agentes, consentimiento y cancelación.
2. Cobertura del LLM supervisor ausente, con fallo y timeout, verificando continuidad determinista.
3. Cobertura de IPC, attach, desconexión, carrera de arranque, ownership SQLite y aislamiento de configuración.
4. Pasan `cargo build`, `cargo test`, `cargo fmt --check` y `cargo clippy -- -D warnings`.
5. Se registran los comandos y resultados exactos en el plan de ejecución.
