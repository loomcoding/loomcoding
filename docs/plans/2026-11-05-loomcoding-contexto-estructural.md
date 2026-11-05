# Contexto estructural Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** que un modelo pequeño reciba, en una sola llamada o ya en el prompt, el contexto estructural
(símbolos, usos, pruebas y sección del mapa) que hoy reconstruye con 10 a 25 rondas de `grep` y `view`.

**Architecture:** módulo nuevo `crates/loomcoding-core/src/context/` con un índice de símbolos
(`tree-sitter` + `tags.scm` de Rust), un ordenador por PageRank personalizado y un compositor de paquete
acotado. Lo consumen una herramienta `context` y una orientación previa que la oficina adjunta al final
del turno (no al prompt del sistema, para no invalidar la caché de prefijo de Strata y `llama-server`).

**Tech Stack:** Rust, `tree-sitter`, `tree-sitter-rust` (0.24.2 en el clon de referencia), `tokio`.

**Spec:** `docs/specs/2026-11-05-loomcoding-contexto-estructural.md` (marco:
`2026-11-05-loomcoding-mejorar-el-sistema-del-modelo-pequeno.md`).

## Global Constraints

- `loomcoding-core` NO depende de `ratatui` ni de `crossterm`.
- Los cuatro checks en cada commit: `cargo build`, `cargo test`, `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`.
- Toda salida y todo bucle con cota (decálogo `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`):
  paquete con tope de caracteres y máximo duro, profundidad y número de símbolos fijos.
- Herramienta de solo lectura: `ExecutionMode::Parallel`.
- Sin red real en tests; ficheros de prueba en directorios temporales.
- Comentarios en español pijo; identificadores y mensajes de error sin tocar.
- La orientación previa viene **apagada por defecto** (`[context] enabled = false`) hasta que la medición
  de la tarea 8 la justifique.
- Un solo commit por tarea, con `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

- Fichero de Rust con sintaxis rota: el índice lo omite o lo indexa parcialmente, nunca entra en pánico.
- Repositorio grande o con `target/`: se ignora lo que `.gitignore` y `target/` excluyen; indexar tiene
  plazo y no bloquea el turno.
- Consulta sin coincidencias, o con un nombre que existe en muchos sitios: lo dice y lista los más
  próximos, con tope.
- Fichero editado durante el turno: el índice no devuelve líneas desfasadas.
- Paquete que no cabe: se recorta por prioridad y lo declara («recortado»), no se corta a mitad de línea.

## Tareas (orden de dependencia)

Las tareas 1 a 4 son funciones puras con sus pruebas (aptas para el junior local). Las 5 a 7 tocan el
motor y las hago yo. La 8 mide.

### Task 0: Sondeo de coste (sin código en el repositorio)

**Files:** ninguno del repositorio; borrador en el directorio temporal de la sesión.

- [ ] **Step 1:** en un proyecto desechable que dependa de `tree-sitter` y `tree-sitter-rust`, medir
  (a) tiempo de `cargo build` en limpio con y sin esas dependencias y tamaño del binario, (b) tiempo de
  analizar todos los `.rs` de este repositorio (629 ficheros `.rs` versionados a fecha de hoy).
- [ ] **Step 2:** anotar las cifras en la sección «Riesgos» del spec. **Puerta:** si el análisis completo
  supera 2 s en frío o el build crece de forma desproporcionada, parar y replantear (índice solo de los
  ficheros del encargo, o consultas por expresión regular).
- [ ] **Step 3:** commit del spec con los números.

### Task 1: Índice de símbolos

**Files:** crear `crates/loomcoding-core/src/context/mod.rs` y `context/index.rs`; declarar `pub mod context;`
en `lib.rs`; dependencias en `crates/loomcoding-core/Cargo.toml`; pruebas en `context/tests/index.rs`.

**Interfaces — Produces:**
- `pub struct Symbol { pub name: String, pub kind: SymbolKind, pub path: PathBuf, pub line: u32, pub signature: String }`
- `pub enum SymbolKind { Function, Method, Struct, Enum, Trait, Const, Module, Other }`
- `pub struct SymbolIndex` con `pub fn build(root: &Path, deadline: Duration) -> SymbolIndex`,
  `pub fn definitions(&self, name: &str) -> &[Symbol]`, `pub fn references(&self, name: &str) -> &[Reference]`,
  `pub fn files(&self) -> impl Iterator<Item = &Path>`.
- `pub struct Reference { pub path: PathBuf, pub line: u32 }`

- [ ] **Step 1: pruebas primero** (`context/tests/index.rs`): `indexes_functions_structs_and_methods`,
  `records_references_with_file_and_line`, `ignores_target_and_gitignored_files`,
  `a_file_with_broken_syntax_does_not_panic_nor_hide_its_neighbours`, `the_deadline_stops_the_build_and_says_so`
  (índice con un indicador `truncated: bool`).
- [ ] **Step 2:** ver que fallan (no compila sin el módulo).
- [ ] **Step 3:** implementar `SymbolIndex::build` con las consultas de `tree-sitter-rust` (`tags.scm`
  incluido con `include_str!` desde la dependencia o copiado con su licencia MIT declarada), recorriendo
  con `ignore` (o el recorrido ya usado por `glob`, si lo hay: reutilizarlo).
- [ ] **Step 4:** las cuatro comprobaciones; commit.

### Task 2: Caché por fecha de fichero

**Files:** `context/index.rs`; pruebas en `context/tests/refresh.rs`.

**Interfaces — Produces:** `pub fn refresh(&mut self, root: &Path, deadline: Duration)` que reanaliza solo
los ficheros con fecha o tamaño distintos y elimina los desaparecidos.

- [ ] **Step 1: pruebas:** `refresh_reparses_only_the_changed_file` (contador de ficheros analizados
  expuesto con `pub(crate)` para la prueba), `refresh_drops_deleted_files`, `an_edited_file_never_returns_stale_lines`.
- [ ] **Step 2–4:** implementar, comprobar, commit.

### Task 3: Ordenación por PageRank personalizado

**Files:** crear `context/rank.rs`; pruebas en `context/tests/rank.rs`.

**Interfaces — Produces:** `pub fn rank(index: &SymbolIndex, seeds: &[String]) -> Vec<(String, f64)>`
(símbolos de mayor a menor importancia, determinista ante empates por nombre y ruta).

- [ ] **Step 1: pruebas:** `a_seed_ranks_above_unrelated_symbols`, `callers_of_a_seed_rank_above_strangers`,
  `ties_are_broken_by_name_so_the_order_is_stable`, `no_seeds_falls_back_to_global_importance`.
- [ ] **Step 2–4:** PageRank iterativo propio (≤ 30 iteraciones, amortiguación 0,85, sin dependencias
  nuevas); comprobar; commit. Referencia de diseño: `G:\loom-references\aider\aider\repomap.py`
  (`get_ranked_tags`).

### Task 4: Compositor del paquete acotado

**Files:** crear `context/pack.rs`; pruebas en `context/tests/pack.rs`.

**Interfaces — Produces:**
`pub struct PackOptions { pub max_chars: usize, pub max_symbols: usize, pub body_lines: usize }` (valores
por defecto: 6000, 12, 20; `max_chars` tiene un máximo duro de 20000) y
`pub fn compose(index: &SymbolIndex, root: &Path, query: &str, opts: &PackOptions) -> String`.

Contenido por símbolo: `ruta:línea`, firma, hasta `body_lines` del cuerpo, quién lo usa (ruta:línea, tope 5),
pruebas que lo nombran (tope 3). Añade al final, si existe, la sección de `docs/CODE-MAP.md` cuyo título
(«Quiero…») comparta palabras con la consulta (tope 1500 caracteres). Si recorta, termina con la línea
`[recortado: N símbolos más; afina la consulta]`.

- [ ] **Step 1: pruebas:** `the_pack_never_exceeds_max_chars`, `the_hard_maximum_clamps_a_huge_request`,
  `an_unknown_symbol_says_so_and_lists_the_closest_names`, `a_truncated_pack_declares_it_and_ends_on_a_whole_line`,
  `the_code_map_section_is_attached_when_its_title_matches`.
- [ ] **Step 2–4:** implementar; comprobar; commit.

### Task 5: Herramienta `context`

**Files:** crear `crates/loomcoding-core/src/tools/context.rs`; registrar en `tools/mod.rs`
(`project_registry`); pruebas en `tools/context.rs` (módulo `tests`); actualizar `docs/CODE-MAP.md`.

**Interfaces — Consumes:** `SymbolIndex`, `compose`, `PackOptions`. **Produces:** `pub struct ContextTool;`
con esquema `{query: string (obligatoria), budget?: integer}`.

- [ ] **Step 1: pruebas:** `returns_a_pack_for_an_existing_symbol`, `reports_when_nothing_matches`,
  `the_budget_is_clamped_to_the_hard_maximum`, `it_is_registered_in_the_project_registry`.
- [ ] **Step 2–4:** el índice vive en un `OnceCell`/`Mutex` por directorio de trabajo dentro de la
  herramienta, con `refresh` en cada llamada; plazo de indexado 2 s con respuesta parcial declarada;
  respetar cancelación. Comprobar; commit.

### Task 6: Orientación previa en la oficina

**Files:** `crates/loomcoding-core/src/config/` (sección `[context]`: `enabled: bool = false`,
`max_chars: usize = 6000`); `agent/` (función nueva `orientation.rs`, enganche en `run_with_origin`);
pruebas en `agent/tests/orientation.rs`.

**Interfaces — Produces:** `pub(super) fn orientation_for(&self, prompt: &str) -> Option<String>`; devuelve
`None` si está apagada o si el encargo no cita ninguna ruta ni símbolo que exista en el índice.

Reglas: se calcula **una vez por turno de usuario** (no en `run_woken`), a partir de rutas y nombres que el
encargo cita y que existen; se adjunta **tras** el mensaje del usuario como sección rotulada
`[orientación automática: origen = índice de símbolos]`, nunca en `system_prompt()`.

- [ ] **Step 1: pruebas:** `it_is_off_by_default`, `it_is_attached_after_the_user_message_not_in_the_system_prompt`
  (afirma que `system` no cambia entre turnos), `it_is_not_attached_on_a_wake_turn`,
  `it_is_skipped_when_the_prompt_names_nothing_known`, `it_respects_max_chars`.
- [ ] **Step 2–4:** implementar; comprobar; commit. Línea nueva en la guía de herramientas del prompt:
  «antes de `grep`/`view` para localizar código, usa `context`».

### Task 7: Documentación

**Files:** `docs/ARCHITECTURE.md`, `docs/CODE-MAP.md` («Quiero tocar el contexto estructural»),
`docs/INDEX.md` (estado), `docs/INVARIANTS.md` (cotas del paquete), `CHANGELOG`.

- [ ] **Step 1:** actualizar los cuatro documentos con nombres exactos del código; **Step 2:** comprobar
  con el guion de rutas del repositorio que cada ruta y símbolo citados existen; **Step 3:** commit.

### Task 8: Medición (puerta de aceptación)

**Files:** guion de recuento en `G:\temp\loom-qwen\autonomia\` (fuera del repositorio).

- [ ] **Step 1:** con el binario nuevo, correr las tareas 6 y A1 en Qwen3.5-9B y Strata, con
  `[context] enabled = false` y `= true`, tres repeticiones cada combinación cuando el tiempo lo permita;
  servidores levantados por el usuario y vigilancia de RAM como hasta ahora.
- [ ] **Step 2:** tabla por ejecución: llamadas de orientación (`grep`+`view`+`glob`), rondas, tokens de
  entrada y salida, tiempo, si compila y si el diff cumple el encargo.
- [ ] **Step 3: criterio:** si la orientación no baja de forma clara y la corrección no empeora, se
  mantiene apagada y se documenta el resultado en el spec; si baja, se propone activarla por defecto en
  los perfiles de modelos pequeños.
