# Plan: organización y navegabilidad del código

- **Fecha:** 2026-11-05
- **Spec:** `docs/specs/2026-11-05-codeloom-organizacion-y-navegabilidad.md`
- **Relacionado:** `docs/plans/2026-11-04-codeloom-estados-de-subagente-y-composer-compartido.md`
- **Principio de orden:** primero lo de riesgo cero y beneficio inmediato (documentar),
  después los guardarraíles, y al final el refactor de monolitos de mayor a menor riesgo.

## Global Constraints

Los cuatro checks en cada commit:

```bash
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

Reglas que este plan no puede romper:

- `codeloom-core` **no** depende de `ratatui` ni `crossterm`.
- Errores de librería con `thiserror`; el binario puede usar `anyhow`.
- Nada sin cota (decálogo de contexto).
- Sin red real en tests.
- **Un commit que mueve código no cambia comportamiento.** Nunca mezclar movimiento
  con lógica: si algo se rompe, hay que saber cuál de los dos fue.
- Los specs y READMEs son **español**; el código sigue el idioma del archivo que toca.

---

## Fase 0 — Navegación (riesgo cero, beneficio inmediato)

Ningún archivo de `crates/` se toca. Al terminar esta fase, un agente nuevo puede
orientarse leyendo cinco documentos.

### Paso 0.1 — `docs/ARCHITECTURE.md`

- Tabla de los 8 crates: responsabilidad · entrada · qué **no** va aquí.
- Diagrama de dependencias permitidas y prohibidas (core sin TUI).
- Flujo end-to-end: mensaje → sesión → provider → tools → `LiveEvent` → TUI/servidor,
  con el archivo real en cada frontera.
- Los cinco subsistemas: `Provider`, registry de tools, bus de eventos,
  `SessionStore`, orquestador de subagentes.

**Aceptación:** un agente responde "¿dónde entra un mensaje del usuario?" sin abrir
código, solo con el documento.

### Paso 0.2 — `README.md` por crate (8 archivos)

Plantilla fija del spec: responsabilidad · no va aquí · tipos clave · puntos de
entrada · cómo se testea · trampas. Una pantalla cada uno.

**Aceptación:** los 8 existen y siguen la misma plantilla (verificable a ojo: mismas
secciones en el mismo orden).

### Paso 0.3 — `docs/CODE-MAP.md`

Índice orientado a tareas. Entradas mínimas obligatorias:

- añadir una tool
- añadir un provider
- añadir un evento al bus
- añadir una fase de subagente
- añadir un widget de TUI
- añadir un comando de composer
- cambiar el flujo de permisos
- tocar A2A (cliente y servidor)
- añadir una columna persistida

**Aceptación:** cada entrada nombra archivos exactos con ruta, y el listado se
contrasta contra el código real (no inventar rutas: verificar que existen).

### Paso 0.4 — `docs/INVARIANTS.md`

Tabla invariante → **comando exacto o procedimiento** que lo verifica. No basta la
proclamación: cada fila debe ser comprobable por un agente.

**Aceptación:** cada invariante tiene una comprobación ejecutable o un procedimiento
concreto con ruta.

### Paso 0.5 — `docs/INDEX.md`

Índice de los 62 specs y 44 plans con estado (`vigente` / `en curso` / `histórico` /
`superado` / `obsoleto`) y relaciones `sustituye` / `sustituido_por`.

**Aceptación:** los 106 documentos aparecen; los estados son coherentes (ningún
documento `superado` sin destino que lo sustituya).

### Paso 0.6 — `AGENTS.md` como puerta de entrada

Añadir al principio la sección "Mapa de lectura" con los cinco punteros. **No
duplicar** contenido: solo enlazar.

**Aceptación:** `AGENTS.md` sigue siendo corto y no repite lo que hay en
`ARCHITECTURE.md`.

---

## Fase 1 — Guardarraíles

### Paso 1.1 — Check de tamaño

Implementar la opción elegida (test de Rust o script). Lista blanca inicial: los 25
archivos actuales de más de 1.000 líneas, **explícitamente justificados**. La lista
solo puede decrecer.

**Aceptación:** el check falla si se añade un archivo nuevo de más de 1.000 líneas;
pasa en el estado actual.

### Paso 1.2 — Check de la regla de dependencias

Verificar que `codeloom-core/Cargo.toml` no declara `ratatui`/`crossterm` y que no hay
imports en el código.

**Aceptación:** el check falla si alguien añade la dependencia.

---

## Fase 2 — Refactor de `subagent.rs` (7.510 líneas)

Va primero porque el trabajo de A2A de `2026-11-04` crea módulos hermanos
(`subagent_phase.rs`, `subagent_watchdog.rs`): conviene hacer el movimiento antes de
que el archivo crezca más.

### Paso 2.1 — Extraer fases y watchdog

Crear `subagent_phase.rs` y `subagent_watchdog.rs`, mover las responsabilidades y
mantener `pub use` para no romper rutas públicas.

**Aceptación:** `cargo test` verde sin cambios de comportamiento; ningún consumidor
toca sus imports.

### Paso 2.2 — Convertir `subagent.rs` en `subagent/`

`subagent/mod.rs` con re-exports y archivos por responsabilidad: ejecución,
publicación de eventos, input. Movimiento puro.

**Aceptación:** 0 archivos de `subagent/` superan 1.000 líneas; la API pública se
mantiene vía `pub use`.

---

## Fase 3 — Refactor de `render.rs` (4.810 líneas)

Bajo acoplamiento, alto beneficio: un módulo por zona visual (header, transcript,
rail, composer, footer).

**Aceptación:** cada zona en su archivo, ninguno sobre 1.000 líneas, salida visual
idéntica (los tests de render existentes como red).

---

## Fase 4 — Refactor de `server.rs` (3.228 líneas)

Aislado del resto: IPC · ciclo de sesión · snapshot · suscripciones.

**Aceptación:** ninguno sobre 1.000 líneas; tests de servidor verdes.

---

## Fase 5 — Refactor de `app.rs` (10.769 líneas)

El más arriesgado, se hace al final y **después** de la Fase 6 del plan de estados
(para no chocar con el composer).

Extraer por responsabilidad: estado de sesión · estado de hijos · scroll/selección ·
comandos · layout. El objetivo no es estético: es que el bug del composer
(`load_session` en `app.rs:2802` no toca `self.composer`) tenga una frontera visible.

**Aceptación:** `app.rs` por debajo de 1.000 líneas; tests de TUI verdes;
`load_session` vive en el módulo de sesión con su test de borrador.

---

## Fase 6 — Refactor de `agent/mod.rs` (5.613 líneas)

Motor central. Turno · tool loop · compactación · dispatch. El máximo cuidado y el
último de este ciclo: si hay prisa por features, esta fase es la que se puede posponer.

**Aceptación:** ninguno sobre 1.000 líneas; tests de motor verdes.

---

## Orden y dependencias

```
Fase 0 (docs) ──→ Fase 1 (checks) ──→ Fase 2 (subagent) ──→ Fase 3 (render)
                                                              ├──→ Fase 4 (server)
                                                              └──→ Fase 5 (app)  [tras Fase 6 de estados]
                                                                     └──→ Fase 6 (agent)
```

- **Fase 0 puede empezar hoy** y no depende de nada.
- **Fase 5 (app.rs)** debe ir **después** de la Fase 6 del plan de estados
  (`2026-11-04`), para no tener dos refactores del composer a la vez.

## Métricas de éxito

| Métrica | Hoy | Objetivo |
|---|---:|---:|
| % de código en archivos > 1.000 líneas | 54,5 % | < 30 % |
| Archivos > 1.000 líneas | 25 | < 10 |
| `app.rs` | 10.769 | < 1.000 |
| Documentos de navegación | 0 | 5 + 8 READMEs |
| Specs con estado declarado | 0 | 106 |
| Saltos para localizar un punto de cambio | ≥ 5 | ≤ 3 |

## Riesgos

| Riesgo | Mitigación |
|---|---|
| El refactor de `app.rs` rompe la TUI | Se hace al final, con commits de movimiento puro y los tests de render como red |
| La documentación se queda obsoleta | `CODE-MAP.md` y `ARCHITECTURE.md` se actualizan en el mismo PR que el cambio que los invalida (regla en `AGENTS.md`) |
| `docs/INDEX.md` se desincroniza | Preferir generación desde frontmatter si se elige esa opción |
| Choque con el trabajo de A2A en curso | `subagent.rs` se parte primero (Fase 2), antes de que crezca más |
| Refactor eterno sin features | Cada fase es un commit cerrado; la Fase 6 es posponible |

## Fuera de alcance

- Reescribir la arquitectura de crates o fusionar/splitear crates.
- Cambiar el modelo de datos de sesión.
- Refactorizar archivos grandes pero cohesivos (`theme.rs`, providers, tablas): están
  justificados como excepción.
- Migrar a otra herramienta de documentación (mdbook u otra).
- Cobertura de tests nueva: se usa la existente (2.137) como red.

## Preguntas abiertas (repetidas del spec para decisión)

1. Techo de líneas: ¿1.000 duro u 800 objetivo con 1.000 de alarma?
2. Guardarraíl: ¿test de Rust o script de CI?
3. ¿README por crate dentro de `crates/` o centralizados en `docs/crates/`?
4. ¿`docs/INDEX.md` generado desde frontmatter o mantenido a mano?
