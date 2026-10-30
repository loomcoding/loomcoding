# Plan: runtime local acotado de skills

Spec: [`docs/specs/2026-10-30-codeloom-runtime-local-skills.md`](../specs/2026-10-30-codeloom-runtime-local-skills.md)

## Fases

1. Confirmar las raíces `.agents/skills` del proyecto y usuario, y su precedencia.
2. Añadir un loader de solo Markdown con IDs seguros, límites explícitos,
   detección de rutas fuera de raíz y errores aislados por skill.
3. Aplicar únicamente los IDs de `scope.skills` del agente local propietario;
   no heredar scopes ni cargar por defecto. Mantener el contenido estable por
   sesión, sin hot-reload.
4. Incorporar los fragmentos cargados al prompt en un punto acotado y estable.
   Emitir estado por agente: declarada, encontrada, cargada o error.
5. Calcular tokens y coste de forma informativa para las peticiones que incluyan
   cada fragmento; mostrarlo separado del total provider-reported y del budget.
6. Verificar aislamiento, validación de IDs/rutas, topes, error parcial,
   atribución de coste y persistencia con temporales y Provider falso; sin red ni
   ejecutar contenido de skills.
7. Ejecutar `cargo fmt --check`, `cargo build`, `cargo test` y
   `cargo clippy -- -D warnings`.

## Límites

- No implementar todavía clasificación/activación automática ni discovery gate.
- No ejecutar scripts ni cargar skills de agentes A2A remotos.
- Los tokens/costes son informativos; no controlan límites ni permisos.
- Los IDs del perfil no autorizan lectura fuera de las raíces confirmadas.

## Estado

- [x] Crear spec y plan separados del coste informativo MCP.
- [x] Aprobación e implementación solicitadas por el usuario.
- [x] Confirmar raíces, precedencia y límites.
- [x] Loader aislado e integración con agentes locales.
- [x] Estado y coste informativo por skill.
- [x] Pruebas del loader, SQLite y estado de skills en el widget.
- [x] `cargo build`, `cargo fmt --all -- --check` y
  `cargo clippy -- -D warnings`.
- [ ] `cargo test --workspace`: 2 fallos ajenos a estos planes del composer/slash menu
  en `codeloom-tui`; 557 tests de `codeloom-core` pasan.
