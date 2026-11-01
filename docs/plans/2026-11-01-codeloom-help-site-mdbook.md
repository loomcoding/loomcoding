# Plan — Web de ayuda con mdBook

> Estado: propuesta, pendiente de aprobación (2026-11-01).
> Spec: `docs/specs/2026-11-01-codeloom-help-site-mdbook.md`. Léase junto con este plan.
>
> Para quien ejecute: usar `superpowers:subagent-driven-development` (recomendado) o `superpowers:executing-plans`.
> Los pasos son casillas (`- [ ]`).

**Objetivo:** un libro mdBook en español en `docs/book/`, con esquemas antes que prosa, cuyo contenido derivable del código se
genera y se verifica con tests.

**Arquitectura:** un crate nuevo `codeloom-book` (`publish = false`) genera páginas desde el código real y verifica los ejemplos
del libro; el libro vive en `docs/book/src/`; un job `book` de CI construye y sube el artefacto. El registro de herramientas se
mueve a `codeloom-core` para que lo compartan el binario y el generador.

**Tecnología:** mdBook y `mdbook-mermaid` (herramientas externas, no dependencias del workspace), `serde_ignored` (dev), Rust.

## Restricciones globales

- Las cuatro comprobaciones pasan en cada commit: `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`.
- Sin red en los tests. `mdbook` no se ejecuta desde `cargo test`; solo en el job `book` y a mano.
- Instalar `mdbook` y `mdbook-mermaid` (`cargo install mdbook mdbook-mermaid --locked`) **se pide a la persona usuaria** antes de
  hacerlo. Hasta entonces, las tareas 1, 2 y 4 se verifican solo con `cargo test`.
- Las páginas siguen la plantilla de la spec §5 y caben en una pantalla (**≤ 70 líneas**, salvo `generated/`).
- Ningún ejemplo lleva secretos reales: las claves van como `$OPENROUTER_API_KEY` o `<tu-clave>`.
- Conservar el final de línea de cada fichero que se edite (`git ls-files --eol`); los ficheros nuevos van en LF.
- No tocar `docs/README.md` ni los ficheros sin commitear de la persona usuaria; no commitear con `-a`.
- `codeloom-core` no depende de `ratatui` ni `crossterm`.

## Foco de revisión

1. **Fichero generado a mano o viejo:** el test falla diciendo el comando para regenerar (`UPDATE_BOOK=1 cargo test -p codeloom-book`);
   la comparación ignora `\r\n` (el repositorio mezcla finales de línea).
2. **Clave con errata en un ejemplo** (`delgation = "off"`): el test falla nombrando fichero y línea del bloque.
3. **Herramienta nueva sin ficha:** añadirla al registro sin regenerar rompe `generated_files_are_up_to_date`.
4. **Enlace interno roto:** una entrada de `SUMMARY.md` sin fichero, o un enlace relativo que no resuelve, rompe el lint.
5. **Secreto o página fuera de plantilla:** un `sk-` en un ejemplo, una página de más de 70 líneas o sin `## Ver también` rompen el lint.

---

### Tarea 1: Registro de herramientas compartido

**Ficheros:** modificar `crates/codeloom-core/src/tools/mod.rs`, `crates/codeloom-cli/src/main.rs:198-227`.

**Interfaces — produce:** `pub fn project_registry(project_env: HashMap<String, String>) -> ToolRegistry` y
`pub const CHILD_ONLY_TOOLS: &[&str] = &["artifact_publish"]`, ambos en `codeloom_core::tools`.

- [ ] Prueba en `tools/mod.rs`: `project_registry_exposes_the_expected_tools` (21 herramientas; contiene `bash`, `edit`, `subagent`,
      `artifact_read`, `development_workflow`; **no** contiene `artifact_publish`) y `child_only_tools_are_registered_for_children`
      (`CHILD_ONLY_TOOLS` contiene `artifact_publish`).
- [ ] Ejecutarla y comprobar que falla (la función no existe).
- [ ] Mover el cuerpo de `build_project_tools` a `project_registry` sin cambiar el orden ni los tipos registrados; `main.rs` la llama.
- [ ] `cargo test -p codeloom-core --lib tools::` en verde y las cuatro comprobaciones.
- [ ] Commit: `refactor(core): el registro de herramientas del proyecto vive en codeloom-core`.

### Tarea 2: Crate generador `codeloom-book`

**Ficheros:** crear `crates/codeloom-book/{Cargo.toml,src/lib.rs,src/tools.rs,src/commands.rs,src/states.rs,tests/generated.rs}`;
modificar `Cargo.toml` raíz (miembro) y `.gitignore` (`docs/book/book/`); salida en `docs/book/src/generated/`.

**Interfaces — produce (`codeloom_book`):**
- `pub fn tool_cards() -> String` — índice (nombre, modo, descripción de una línea) y una ficha por herramienta del
  registro más `CHILD_ONLY_TOOLS`: descripción, modo (`Paralela`/`Exclusiva` según `execution_mode(&Value::Null)`), tabla de
  parámetros (nombre, tipo, obligatorio, descripción) leída del esquema JSON, y «llamada mínima» con los obligatorios.
- `pub fn command_table() -> String` — tabla de `codeloom_tui::commands::COMMANDS` (orden, descripción).
- `pub fn state_table() -> String` — los ocho `TaskState::ALL` con `proto_name()`.
- `pub fn render_all() -> Vec<(&'static str, String)>` — pares (nombre de fichero, contenido) con cabecera
  `<!-- Generado por codeloom-book. No editar a mano: UPDATE_BOOK=1 cargo test -p codeloom-book -->`.
- Notas escritas a mano opcionales: si existe `docs/book/notes/tools/<nombre>.md`, la ficha lo incluye al final.

- [ ] Pruebas en `tests/generated.rs`:
      `tool_cards_cover_every_registered_tool` (cada nombre de `project_registry` y `CHILD_ONLY_TOOLS` aparece en una ficha),
      `parallel_and_exclusive_are_reported` (`view` Paralela, `edit` Exclusiva), `state_table_lists_the_eight_a2a_states`
      (contiene `input-required`, `auth-required`), `command_table_matches_the_commands_const`,
      `generated_files_are_up_to_date` (compara `render_all()` con el disco ignorando `\r\n`; con `UPDATE_BOOK=1` escribe y pasa).
- [ ] Ejecutar y comprobar que fallan.
- [ ] Implementar los generadores y `render_all`; usar `Tool::schema()` y `ToolRegistry::specs()`. Sin red ni reloj en la salida
      (determinista, orden alfabético).
- [ ] `UPDATE_BOOK=1 cargo test -p codeloom-book`, luego `cargo test -p codeloom-book` en verde y las cuatro comprobaciones.
- [ ] Commit: `feat(book): generador de fichas de herramientas, órdenes y estados A2A`.

### Tarea 3: Esqueleto del libro y lint

**Ficheros:** crear `docs/book/book.toml`, `docs/book/src/SUMMARY.md`, `docs/book/src/portada.md`,
`docs/book/src/plantilla.md`, `crates/codeloom-book/tests/lint.rs`, `scripts/book.sh`.

**Interfaces — produce:** convención de marca de página: título `# …`, línea `**Qué es:** …`, sección `## Ver también`.

- [ ] Pruebas en `tests/lint.rs`: `summary_entries_have_files` (cada enlace de `SUMMARY.md` existe),
      `internal_links_resolve`, `pages_follow_the_template` (título, `**Qué es:**`, `## Ver también`; excluye `generated/`,
      `portada.md` y apéndices que lo declaren), `pages_fit_one_screen` (≤ 70 líneas), `examples_carry_no_secrets` (ningún
      `sk-`, `Bearer ` ni valor tras `api_key =`).
- [ ] Ejecutar y comprobar que fallan o se saltan vacías según el caso.
- [ ] Crear `book.toml` (idioma `es`, `src = "src"`, buscador activado, preprocesador `mermaid`, título `CodeLoom`) y
      `scripts/book.sh`: construye con `MDBOOK_BOOK__TITLE="CodeLoom v<versión del workspace> (<commit corto>)" mdbook build docs/book`.
- [ ] `portada.md`: tres puertas (Usar, Configurar, Entender) y el mapa de conceptos en Mermaid. `plantilla.md`: la plantilla de
      la spec §5 como ejemplo vivo (excluida del lint de contenido).
- [ ] Pedir a la persona usuaria que instale `mdbook` y `mdbook-mermaid`; con ellos, `scripts/book.sh` termina sin avisos y el
      HTML muestra el diagrama dibujado. Comprobarlo abriendo la página con el navegador.
- [ ] `cargo test -p codeloom-book` y las cuatro comprobaciones; commit: `feat(book): esqueleto mdBook, plantilla y lint de páginas`.

### Tarea 4: Ejemplos verificados

**Ficheros:** crear `crates/codeloom-book/src/examples.rs`, `crates/codeloom-book/tests/examples.rs`; añadir `serde_ignored`
a `[dev-dependencies]`.

**Interfaces — produce:** convención en Markdown: una línea `<!-- verifica: config -->` o `<!-- verifica: perfil <nombre> -->`
inmediatamente antes de un bloque. `pub fn extract_blocks(markdown: &str) -> Vec<Block>` con
`Block { kind: Kind, name: Option<String>, text: String, line: usize }`.

- [ ] Pruebas: `extracts_marked_blocks_with_their_line`; `a_config_block_with_a_typo_is_rejected`
      (`delgation = "off"` bajo `[agents]` → error que nombra la clave y la línea); `a_valid_config_block_passes`
      (`[agents]` con `delegation = "off"`, `max_depth = 2`); `a_profile_block_loads_without_warnings`
      (`parse_profile` devuelve `Ok` y la lista de avisos vacía); `every_marked_block_in_the_book_passes` (recorre
      `docs/book/src/**/*.md`).
- [ ] Ejecutar y comprobar que fallan.
- [ ] Implementar la verificación de configuración envolviendo cada sección en un tipo de prueba con los tipos públicos reales
      (`CodeLoomConfig` y `ProviderConfig`, `AgentsConfig`, `A2aConfig`, la configuración de compactación y
      `ProviderCacheConfig` para `[provider] prompt_cache`) y deserializando con `serde_ignored`: cualquier clave ignorada falla.
      Perfiles con `codeloom_core::registry::profile::parse_profile`.
- [ ] Cuatro comprobaciones y commit: `feat(book): los ejemplos del libro se verifican con los cargadores reales`.

### Tarea 5: Parte 1 — Usar

**Ficheros:** crear bajo `docs/book/src/usar/`: `instalacion.md`, `primer-arranque.md`, `proveedores.md`, `la-tui.md`,
`ordenes-y-atajos.md` (incluye `generated/ordenes.md`), `sesiones-y-cola.md`, `contexto-y-coste.md`, `lanzar-un-agente.md`,
`limites-conocidos.md`; añadirlas a `SUMMARY.md`.

**Fuentes que hay que leer, no recordar:** `README.md`, `codeloom.toml`, `crates/codeloom-cli/src/main.rs` (flags),
`crates/codeloom-tui/src/commands.rs`, `crates/codeloom-tui/src/run.rs` (atajos), `docs/specs/2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`,
`docs/specs/hechos/2026-11-01-codeloom-user-invoked-agents.md`, `docs/visual-e2e.md`.

- [ ] Redactar cada página con la plantilla; esquemas y listas numeradas, sin párrafos largos. `lanzar-un-agente.md` cubre
      `/agent <nombre> [--skills a,b] <tarea>` y `@nombre …` con ejemplos exactos de la spec.
- [ ] `limites-conocidos.md`: solo lo que consta como no implementado (memoria, OAuth interactivo, TLS propio) con enlace al
      inventario.
- [ ] Las capturas, si hay, se generan con el E2E visual y `CODELOOM_HOME` temporal (`docs/visual-e2e.md`); nunca contra el
      daemon real.
- [ ] `cargo test -p codeloom-book` (lint y ejemplos) en verde; `scripts/book.sh` sin avisos; commit: `docs(book): parte Usar`.

### Tarea 6: Parte 2 — Configurar

**Ficheros:** crear bajo `docs/book/src/configurar/`: `codeloom-toml.md`, `perfiles-de-agente.md`, `skills.md`, `mcp.md`,
`catalogos-externos.md`, `agentes-remotos-a2a.md`, `delegacion-y-presupuestos.md`, `aprobacion-y-seguridad.md`.

**Fuentes:** `crates/codeloom-core/src/config/mod.rs`, `registry/profile.rs`, `registry/trust.rs`, `skills.rs`,
`crates/codeloom-mcp/src/lib.rs`, `crates/codeloom-a2a/src/config.rs`, specs `hechos/2026-11-01-codeloom-external-catalog-profiles.md`,
`hechos/2026-10-31-codeloom-delegation-control.md`, `hechos/2026-10-31-codeloom-recursive-delegation.md`,
`hechos/2026-10-31-codeloom-budget-hard-stop.md`.

- [ ] `codeloom-toml.md`: una tabla por sección (`[provider]`, `[compaction]`, `[a2a]`, `[agents]`) con clave, tipo, valor por
      defecto y efecto, y un ejemplo marcado `<!-- verifica: config -->` por sección.
- [ ] `perfiles-de-agente.md`: frontmatter completo con un perfil marcado `<!-- verifica: perfil <nombre> -->`; dejar dicho cuáles
      de `scope.*` se aplican y cuáles solo se muestran.
- [ ] `aprobacion-y-seguridad.md` recoge los cinco puntos de la spec §7, con cada afirmación comprobada en el código.
- [ ] Tests en verde, `scripts/book.sh` sin avisos; commit: `docs(book): parte Configurar`.

### Tarea 7: Parte 3 — Características

**Ficheros:** crear bajo `docs/book/src/caracteristicas/`: `mapa-de-conceptos.md`, `bucle-del-agente.md`, `herramientas.md`
(índice; incluye `generated/herramientas.md`), `comunicacion-con-subagentes.md`, `estados-a2a.md`, `jobs.md`,
`eventos-y-observabilidad.md`, `contexto-y-coste.md`, `cache-de-prompt.md`; notas a mano en `docs/book/notes/tools/` para
`bash`, `edit`, `subagent`, `subagent_control`, `artifact_publish`.

**Fuentes:** `crates/codeloom-core/src/agent/mod.rs`, `subagent.rs`, `subagent_input.rs`, `task.rs`, `task_events.rs`, `job.rs`,
`event/mod.rs`, `tools/*.rs`, y las specs hechas de A2A (`hechos/2026-10-22-codeloom-a2a-task-model-design.md`,
`hechos/2026-10-23-codeloom-a2a-follow-up-design.md`, `hechos/2026-11-01-codeloom-child-progress-artifacts.md`).

- [ ] `comunicacion-con-subagentes.md` con un diagrama de secuencia Mermaid y los pasos numerados: lanzar (`subagent` o
      `/agent`), qué ve el hijo, resultado, `input-required`, despertar del padre, cancelación, avances (`artifact_publish` y
      `artifact_read`), recursión (`max_depth`). Rotular lo que es A2A y lo que es nuestro, con el estado exacto entre paréntesis.
- [ ] `mapa-de-conceptos.md`: un diagrama con agente, subagente, job, tarea A2A, herramienta, skill, perfil y presupuesto, cada
      nodo enlazado a su página.
- [ ] Las notas a mano de herramientas son cortas (uso real, un cuidado) y su nombre debe existir en el registro; añadir una
      prueba `tool_notes_match_registered_tools` en `tests/generated.rs`.
- [ ] Tests en verde, `scripts/book.sh` sin avisos; commit: `docs(book): parte Características`.

### Tarea 8: Apéndices y seguridad

**Ficheros:** crear `docs/book/src/apendices/{glosario,hojas-de-consulta,enlaces-al-diseno}.md` y
`docs/book/src/seguridad.md`.

- [ ] `glosario.md`: una línea por término (flock, loom, job, tarea A2A, perfil, skill, huella, presupuesto, familia, `hard_stop`).
- [ ] `hojas-de-consulta.md`: órdenes y atajos, claves de `codeloom.toml` y estados A2A en una pantalla cada una (incluye lo generado).
- [ ] `seguridad.md`: los cinco puntos de la spec §7 con enlaces a las páginas de detalle.
- [ ] Tests y `scripts/book.sh`; commit: `docs(book): glosario, hojas de consulta y seguridad`.

### Tarea 9: CI, README y cierre

**Ficheros:** crear `.github/workflows/book.yml`; modificar `README.md`, `CONTRIBUTING.md`, `docs/inventario.md`, la spec y este plan.

- [ ] `book.yml`: job `book` (Linux) con `cargo install mdbook mdbook-mermaid --locked`, `cargo test -p codeloom-book`,
      `scripts/book.sh` y `actions/upload-artifact` con el nombre `codeloom-book-v<versión>`; job `deploy` a Pages con
      `if: false` y un comentario que remita a la fase 4 de la spec.
- [ ] `README.md`: corregir el desfase (MCP y skills sí existen; memoria no), añadir `/agent`, y enlazar el libro. Sigue en inglés.
- [ ] `CONTRIBUTING.md`: regla «clave de configuración, herramienta u orden nueva → actualizar el libro y regenerar con
      `UPDATE_BOOK=1 cargo test -p codeloom-book`».
- [ ] Estado de spec y plan a «hecho», línea en `docs/inventario.md` y mover el par a `hechos/` con enlaces reescritos.
- [ ] Las cuatro comprobaciones, `scripts/book.sh` y la CI en verde; commit: `docs(book): CI del libro, README y cierre`.
