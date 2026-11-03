# Spec: organización y navegabilidad del código

- **Fecha:** 2026-11-05
- **Estado:** Propuesta (preguntas abiertas cerradas, ver "Decisiones")
- **Ámbito:** documentación, estructura de crates y proceso (sin cambio funcional)
- **Complementa:** `docs/specs/2026-10-14-loom-architecture-overview.md` (visión general)
  y `AGENTS.md` (reglas operativas)

## Problema

CodeLoom ha crecido hasta el punto de que **entrar en el código cuesta más que
cambiarlo**. Los datos medidos:

| Métrica | Valor |
|---|---:|
| Archivos `.rs` | 278 |
| Líneas de Rust | **115.603** |
| Archivos de más de 1.000 líneas | 25 |
| % del código en esos 25 archivos | **54,5 %** (62.978 líneas) |
| Specs / Plans | 62 / 44 (a la medición; 64 / 46 el 2026-11-05) |
| Tests (`#[test]`/`#[tokio::test]`) | 2.137 |

Por crate: `codeloom-core` 42.918 · `batten` 30.873 · `codeloom-tui` 23.328 ·
`codeloom-a2a` 11.913 · `codeloom-cli` 4.990 · `codeloom-e2e` 782 ·
`codeloom-mcp` 684 · `codeloom-herdr` 115.

Los peores archivos:

```
10769  codeloom-tui/src/app.rs
 7510  codeloom-core/src/subagent.rs
 5613  codeloom-core/src/agent/mod.rs
 4810  codeloom-tui/src/render.rs
 4377  codeloom-tui/src/run.rs
 3228  codeloom-cli/src/server.rs
```

Cifras medidas al redactar; derivan con cada commit (p. ej. `app.rs` ya tiene 10.794
líneas), así que los checks y el INDEX deben contarlas, no hardcodearlas.

**El hallazgo que define el problema: no existe `ARCHITECTURE.md`, ni mapa de código,
ni `README.md` por crate.** Un agente nuevo solo tiene `AGENTS.md` y 106 documentos
sueltos en `docs/specs` y `docs/plans`, sin índice ni estado.

### Efecto concreto sobre un agente de IA

1. **Contexto quemado en orientación.** Para saber dónde vive el composer hay que
   abrir un archivo de 10.769 líneas; el presupuesto de contexto se gasta antes de
   empezar a trabajar.
2. **Riesgo de tocar el sitio equivocado.** Sin mapa de responsabilidades, es fácil
   implementar en `codeloom-core` algo que ya existe en `batten`, o duplicar una
   abstracción.
3. **Specs obsoletos que se leen como vigentes.** 62 specs sin estado: un agente
   puede implementar un diseño ya superado.
4. **Reglas duras invisibles.** "core sin ratatui", "tools que escriben son
   `Exclusive`", "nada sin cota" están en `AGENTS.md`, pero no hay forma de saber si
   el código actual las cumple ni de detectar la violación.

Ejemplo real y reciente de este coste: el bug del composer (`load_session` en
`app.rs:2802` no toca `self.composer`) vivió sin detectarse porque **no hay frontera
visible donde mirar** dentro de `app.rs`.

## Objetivos y no-objetivos

**Objetivos medibles:**

- Un agente localiza el punto de cambio de cualquier feature en **3 saltos o menos**,
  sin leer archivos de más de 1.000 líneas.
- Existe una guía por tarea ("quiero añadir un tool") que nombra los archivos exactos.
- Los archivos de más de 1.000 líneas bajan del 54,5 % a menos del 30 % del código y de
  25 a menos de 20 archivos en este ciclo.
- Ningún archivo nuevo supera el techo acordado, con check automático.
- Todo spec y plan declara su estado (vigente, histórico, superado, obsoleto).
- Un agente nuevo puede responder "¿dónde va esto?" leyendo **un** documento.

**No-objetivos:**

- Reescribir la arquitectura de crates (la dirección de dependencias se mantiene).
- Cambiar comportamiento funcional. El refactor es de forma, con los 2.137 tests como
  red de seguridad.
- Documentar cada función (eso ya lo hace el código). Se documenta **estructura y
  puntos de entrada**.
- Unificar los 106 documentos en uno.

## Capa de navegación para agentes

Cinco artefactos nuevos, con contenido fijo. `AGENTS.md` pasa a ser la puerta de
entrada que **apunta** a todos ellos (no los duplica).

### 1. `docs/ARCHITECTURE.md`

Mapa del sistema, no tutorial. Contenido obligatorio:

- **Tabla de crates**: nombre · responsabilidad · entrada principal · qué **no** va
  aquí. Debe incluir la regla dura: `codeloom-core` sin `ratatui`/`crossterm`.
- **Diagrama de dependencias** (texto, sin herramientas): quién puede depender de
  quién, y las prohibiciones.
- **Flujo end-to-end**: mensaje del usuario → sesión → provider → tools → bus de
  eventos → TUI / servidor. Con el archivo real donde se cruza cada frontera.
- **Los cinco registros/subsis­temas** que hay que conocer: `Provider`, registry de
  tools, bus de eventos (`LiveEvent`), `SessionStore`, y el orquestador de subagentes.

### 2. `README.md` por crate (los 8)

Plantilla fija y corta (una pantalla):

```markdown
# codeloom-core
**Responsabilidad:** motor puro: sesión, agentes, tools, providers, eventos.
**No va aquí:** nada de TUI, crossterm, ratatui, terminal.
**Tipos clave:** `SessionStore`, `LiveEvent`, `Agent`, `ToolRegistry` (con ruta).
**Puntos de entrada:** `Agent::run`, `ToolRegistry::dispatch` (con ruta).
**Cómo se testea:** tests puros + `wiremock` para providers. Sin red real.
**Trampas:** las tools que escriben son `ExecutionMode::Exclusive`.
```

### 3. `docs/CODE-MAP.md` — índice **orientado a tareas**

Lo que más ahorra a una IA: en vez de "qué es cada módulo", "dónde toco para X".

```markdown
## Quiero añadir una herramienta (tool)
1. Implementa el trait en `codeloom-core/src/tools/<nombre>.rs`
2. Regístrala en `codeloom-core/src/registry/mod.rs`
3. Declara `ExecutionMode` (Exclusive si escribe)
4. Añade test con provider mock
5. Añade el átomo visual si necesita render propio: `batten/src/state/tool_atom.rs`

## Quiero añadir un evento al bus
1. Variante en `codeloom-core/src/event/mod.rs` (`LiveEvent`)
2. Clasifícalo en `is_child_relevant()` (lista blanca)
3. Consumidor en TUI: `codeloom-tui/src/app.rs`
4. ¿Debe persistir? `session/sqlite.rs`
```

Entradas obligatorias: añadir tool · añadir provider · añadir evento · añadir fase de
subagente · añadir widget · añadir comando de composer · cambiar permisos · tocar A2A
(cliente/servidor) · añadir columna persistida.

### 4. `docs/INVARIANTS.md` — reglas duras y cómo se detecta su violación

| Invariante | Cómo se comprueba |
|---|---|
| `codeloom-core` no depende de `ratatui`/`crossterm` | `grep` en `Cargo.toml` de core + revisión de imports |
| Tools que escriben son `ExecutionMode::Exclusive` | revisión del registro + test por tool |
| Nada sin cota (outputs, prompts, loops) | decálogo de contexto; test de truncado |
| Sin red real en tests | revisión de tests; `wiremock` obligatorio |
| Errores de librería con `thiserror` | convención de crate |

Cada invariante con **el comando exacto** que lo verifica, no solo la proclamación.

### 5. `AGENTS.md` como puerta de entrada

Ya existe. Se le añade una sección "Mapa de lectura" al principio:

> - ¿Cómo está organizado? → `docs/ARCHITECTURE.md`
> - ¿Dónde toco para X? → `docs/CODE-MAP.md`
> - ¿Qué no puedo romper? → `docs/INVARIANTS.md`
> - ¿De qué crate es esto? → `crates/<crate>/README.md`
> - ¿Qué specs están vigentes? → `docs/INDEX.md`

## Índice y ciclo de vida de specs y plans

**El problema del "spec rot":** 62 specs y 44 plans sin estado. Un agente que lee un
spec superado implementa lo incorrecto, y no hay señal de aviso.

**Diseño:** `docs/INDEX.md` generado y mantenido, con una fila por documento:

| Documento | Tipo | Estado | Sustituye a | Sustituido por | Fecha |
|---|---|---|---|---|---|
| 2026-10-29-...-live-subagent-supervision | spec | parcialmente superado | — | 2026-11-04-estados-de-subagente | 2026-10-29 |

Estados permitidos: `vigente` · `en curso` · `histórico` (implementado y cerrado) ·
`superado` (otro documento manda) · `obsoleto` (no implementar).

Además, frontmatter mínimo en cada documento nuevo:

```yaml
---
estado: vigente
sustituye: []
sustituido_por: []
---
```

**Regla:** un agente que va a implementar algo **debe** consultar `docs/INDEX.md`
primero, y si el documento que iba a leer está `superado`, lee el que lo sustituye.

## Convenciones de tamaño y estructura

**Umbral recomendado: 500-600 líneas por archivo. Techo duro: 1.000.**

Justificación, no dogma:

- 500-600 líneas es lo que cabe en una pantalla de razonamiento sin perder el hilo de
  la responsabilidad del módulo.
- 1.000 es el punto donde un archivo deja de ser "un módulo" y pasa a ser un cajón:
  deja de tener un nombre que lo describa. `app.rs` no se llama "app", se llama
  "todo".
- **Excepciones legítimas:** tablas de datos (`theme.rs`), generadores de provider
  (`anthropic.rs`, `openai.rs`), y código generado. Un archivo grande y **cohesivo**
  con una sola responsabilidad clara es aceptable; uno grande y **heterogéneo** no.
  El criterio es "¿puedo describir el archivo en una frase sin usar 'y'?".

**Estructura canónica en `codeloom-core`:** un módulo con más de ~300 líneas se
convierte en directorio con `mod.rs` de re-exports y archivos por responsabilidad.

**Cómo partir sin romper:** re-exports (`pub use`) mantienen la ruta pública; el
refactor se hace en commits que solo mueven código, sin cambiar comportamiento, con
los tests verdes como prueba. Nunca mezclar un move con un cambio de lógica.

## Plan de refactor de monolitos

Ordenado por **riesgo/beneficio**, no por tamaño bruto.

| Orden | Archivo | Qué extraer | Riesgo | Por qué en esta posición |
|---:|---|---|---|---|
| 1 | `subagent.rs` (7.510) | ejecución, publicación de eventos, input (las fases y el watchdog ya son módulos hermanos propios: `subagent_phase.rs`, `subagent_watchdog.rs`) | Medio | Es donde vive el trabajo de A2A en curso; hacerlo pronto evita conflicto |
| 2 | `render.rs` (4.810) | un módulo por zona (header, transcript, rail, composer, footer) | Medio | Alto beneficio y bajo acoplamiento |
| 3 | `server.rs` (3.228) | IPC · ciclo de sesión · snapshot · suscripciones | Bajo | Aislado del resto |
| 4 | `app.rs` (10.769) | estado de sesión · estado de hijos · scroll/selección · comandos · layout | Alto | El más grande y donde vive el bug del composer, pero el más arriesgado: va tras los anteriores y tras la Fase 6 del plan de estados |
| 5 | `run.rs` (4.377) | bucle de eventos · teclas por contexto · driver de tick | Medio | Depende de que `app.rs` ya esté partido |
| 6 | `agent/mod.rs` (5.613) | turno · tool loop · compactación · dispatch | Alto | Motor central: máximo cuidado, posponible |

Estos seis suman 36.307 líneas (31,4 % del código). Partirlos en piezas por debajo de
1.000 baja el porcentaje por debajo del 30 % (objetivo 1), pero deja ~19 archivos
mayores de 1.000: el objetivo de **número** de archivos es "< 20 en este ciclo", y el
resto se aborda en una oleada posterior con la lista blanca decreciente.

**Qué NO partir:** `theme.rs`, `anthropic.rs`, `openai.rs` y las tablas de `batten`.
Son grandes pero cohesivos; partirlos añadiría indirección sin ahorrar contexto.

**Verificación de "no cambia comportamiento":** commits de movimiento puro,
`cargo test` verde (2.137 tests), y `cargo clippy -D warnings` como red adicional.

## Guardarraíles automáticos

Un check que reporte archivos por encima del techo, para que no vuelva a crecer:

- **Opción A (preferida):** test de Rust en el workspace que enumere `src/**/*.rs` y
  falle si alguno supera 1.000 líneas, con lista blanca explícita de las excepciones
  justificadas (decreciente). Ventaja: corre en `cargo test`, sin CI adicional.
- **Opción B:** script en CI. Ventaja: no contamina el workspace.

En ambos casos, la lista blanca **nunca crece**: solo se permite retirar entradas.

Los cuatro checks siguen siendo el contrato: `cargo build`, `cargo test`,
`cargo fmt --check`, `cargo clippy -- -D warnings`.

## Alternativas consideradas

1. **Documentar todo en un único `ARCHITECTURE.md` gigante.** Descartada: repetiría el
   problema (un archivo que nadie lee entero) y quedaría obsoleto al primer cambio.
2. **Solo generar documentación automática de símbolos.** Descartada: un agente no
   necesita la lista de funciones, necesita **la intención y las fronteras**. Los
   símbolos ya los da el LSP.
3. **Refactorizar primero y documentar después.** Descartada: documentar es riesgo cero
   y beneficio inmediato; además la documentación es lo que guía el refactor sin
   perderse.
4. **Romper la API pública al partir módulos.** Descartada: `pub use` mantiene las
   rutas y evita tocar a los consumidores.

## Decisiones (antes preguntas abiertas)

1. **Techo de líneas:** 1.000 duro, sin objetivo intermedio. 500-600 queda como
   recomendación de revisión, no como check.
2. **Guardarraíl:** opción A, test de Rust en el workspace (corre en `cargo test`, sin
   CI adicional), con lista blanca decreciente.
3. **Prioridad vs. A2A:** `subagent.rs` primero; `app.rs` y `run.rs` **después de la
   Fase 6 del plan de estados** (`2026-11-04`), para no tener dos refactores del
   composer a la vez. (Sustituye la frase anterior "después de la Fase 0-1".)
4. **README por crate:** en `crates/<crate>/README.md` (viaja con el crate y lo ve
   `cargo doc`/crates.io).
5. **Índice de specs:** `docs/INDEX.md` mantenido a mano, más un test que falla si algún
   `docs/specs/*.md` o `docs/plans/*.md` no aparece en él (detecta el desincronizado sin
   exigir frontmatter retroactivo en 110 documentos). El frontmatter es obligatorio solo
   en documentos nuevos desde esta fecha.
