# Plan — renombrado a Loomcoding

Spec: `docs/specs/2026-11-02-loomcoding-renombrado.md`

## Estado

Pendiente. Ninguna fase ejecutada.

## Restricciones globales

Los cuatro checks deben pasar en cada commit:

```bash
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

Sin dependencias nuevas. Sin red en tests. El renombrado es **solo de
nomenclatura**: si un test falla por comportamiento y no por nombre, es un bug
introducido en este cambio.

Regla de oro: **el orden de las fases importa**. No se puede renombrar los
paquetes antes que los directorios, ni los `use` antes que los `Cargo.toml`.

## Inventario de partida

Medido el 2026-11-02 con `grep` sobre el repo (excluyendo `target/`):

| Elemento | Cantidad |
|---|---|
| Ficheros que mencionan `codeloom` | 375 |
| Ocurrencias totales | 3.937 |
| Crates del workspace | 8 (7 `codeloom-*` + `batten`) |
| Crates a renombrar | `codeloom-core`, `-cli`, `-tui`, `-a2a`, `-herdr`, `-mcp`, `-e2e` |
| + crate interna | `batten` (no se renombra) |
| Variables de entorno | 15 |
| Fichero de proyecto | `codeloom.toml` (110 apariciones) |
| Directorio de perfil | `~/.codeloom/` (18 apariciones) |
| Plugin herdr | `codeloom.herdr` (10 apariciones) |
| Binarios | `loom`, `codeloom` |

Variables de entorno a renombrar (15), prefijo `CODELOOM_` → `LOOMCODING_`.
La lista exacta se obtiene con:

```bash
grep -rhoE 'CODELOOM_[A-Z0-9_]+' --include='*.rs' --include='*.toml' --include='*.md' . \
  | sort -u
```

## Fases

### Fase 1 — workspace y directorios

1. `Cargo.toml` (raíz)
   - `members`: `crates/codeloom-*` → `crates/loomcoding-*`.
   - `[workspace.dependencies]`: claves y `path` de los seis paquetes.
2. Renombrar los directorios con `git mv` para conservar el historial:
   ```bash
   git mv crates/codeloom-core  crates/loomcoding-core
   git mv crates/codeloom-cli   crates/loomcoding-cli
   git mv crates/codeloom-tui   crates/loomcoding-tui
   git mv crates/codeloom-a2a   crates/loomcoding-a2a
   git mv crates/codeloom-herdr crates/loomcoding-herdr
   git mv crates/codeloom-mcp   crates/loomcoding-mcp
   git mv crates/codeloom-e2e   crates/loomcoding-e2e
   ```
   `crates/batten` **no se mueve**.
3. En cada `crates/loomcoding-*/Cargo.toml`:
   - `[package] name`
   - dependencias internas (`path`, claves)
   - `[[bin]] name`: `loom` se mantiene; `codeloom` → `loomcoding`.

**Verificación:** `cargo metadata --no-deps` lista los siete paquetes
renombrados más `batten`, sin errores de resolución de workspace.

### Fase 2 — identificadores Rust

1. En todo `crates/**/*.rs`:
   - `codeloom_core` → `loomcoding_core`
   - `codeloom_cli` → `loomcoding_cli`
   - `codeloom_tui` → `loomcoding_tui`
   - `codeloom_a2a` → `loomcoding_a2a`
   - `codeloom_herdr` → `loomcoding_herdr`
   - `codeloom_mcp` → `loomcoding_mcp`
   - `codeloom_e2e` → `loomcoding_e2e`
   - Tipos: `CodeLoomConfig` → `LoomcodingConfig`, y cualquier `CodeLoom*`.
2. `crates/loomcoding-cli/src/home.rs`
   - `HOME_ENV` pasa a `"LOOMCODING_HOME"`.
   - Comentario de módulo actualizado.
   - Tests: siguen pasando sin cambios (prueban `resolve`, no el literal).

**Verificación:** `cargo build` compila.

### Fase 3 — variables de entorno y perfil

1. Sustituir las 15 variables `CODELOOM_*` → `LOOMCODING_*` en código, docs y
   tests. Origen principal: `crates/loomcoding-cli/src/home.rs` y los tests E2E.
2. `~/.codeloom/` → `~/.loomcoding/` (18 apariciones verificadas):
   - `crates/loomcoding-cli/src/ipc.rs:130` (`.join(".codeloom")`)
   - `crates/loomcoding-cli/src/main.rs:335` y `:618`
   - `crates/loomcoding-cli/src/server.rs:1921` y `:3072`
   - `crates/loomcoding-cli/tests/tui_pty.rs:89`
   - `crates/loomcoding-core/src/catalog.rs:330` (doc) y `:337` (`.join(".codeloom")`)
   - `crates/loomcoding-e2e/src/mock.rs:104` (doc), `:122`, `:156`
   - `crates/loomcoding-core/src/session/sqlite.rs:2094` (ruta de ejemplo)
   - `crates/loomcoding-tui/src/app.rs:6860` (ruta de ejemplo)
3. `~/.codeloom/mcp.toml` → `~/.loomcoding/mcp.toml` (mensaje de error en
   `server.rs:737`).
4. `.codeloom-supervisor` → `.loomcoding-supervisor` (`server.rs:1952`, `:2430`).
5. Claves de metadatos A2A `metadata.codeloom.kind` →
   `metadata.loomcoding.kind`: `crates/loomcoding-core/src/a2a.rs:160`,
   `crates/loomcoding-core/src/tools/artifact_publish.rs:4`. **Ojo:** esto es
   protocolo visible entre procesos; cambiarlo rompe la interoperabilidad con
   un daemon viejo. Se cambia porque no hay despliegues.

**Verificación:** `cargo test` pasa y ningún test usa `CODELOOM_HOME`.

### Fase 4 — fichero de proyecto

1. `git mv codeloom.toml loomcoding.toml`.
2. `crates/loomcoding-cli/src/server.rs`: `cwd.join("codeloom.toml")` →
   `cwd.join("loomcoding.toml")` en las tres apariciones del código (`:653`,
   `:2329`, `:3065`).
3. `crates/loomcoding-cli/src/main.rs:47`: comentario que menciona
   `codeloom.toml`.
4. El resto de las 110 apariciones de `codeloom.toml` son docs, comentarios y
   ejemplos; se actualizan aquí las de `crates/**` y en la fase 7 las de
   `docs/**`:
   - `crates/loomcoding-core/src/config/mod.rs` (`:107`, `:229`, `:424`, `:465`)
   - `crates/loomcoding-core/src/budget.rs` (`:1`, `:15`)
   - `crates/loomcoding-core/src/registry/mod.rs:301`
   - `crates/loomcoding-core/src/agent/compact.rs:30`
   - `crates/loomcoding-a2a/src/config.rs` (`:1`, `:225`)
   - `crates/loomcoding-e2e/src/mock.rs` (`:115`, `:151`)
   - `crates/loomcoding-tui/src/app.rs` (`:1211`, `:1398`)
   - `crates/batten/src/widgets/funcionalidad_table.rs:215` (ejemplo de UI)
   - `crates/loomcoding-core/tests/anthropic_live.rs:21`
5. El contenido del fichero no cambia (mismas claves).

**Verificación:** `cargo test -p loomcoding-cli` pasa; el arranque con
`LOOMCODING_HOME` temporal + `loomcoding.toml` carga la config.

### Fase 5 — plugin herdr

1. `herdr-plugin.toml`: `id`/`name` `codeloom.herdr` → `loomcoding.herdr`.
2. Resto de apariciones de `codeloom.herdr` en código y docs.
3. Test E2E de herdr: actualizar el identificador esperado.

**Verificación:** `cargo test -p loomcoding-herdr` y el E2E del plugin.

### Fase 6 — README con aviso de experimental

1. `README.md`: insertar la alerta **antes** de todo (excepto el logo):

   ```markdown
   > [!WARNING]
   > **Experimental — no estable.** Loomcoding está en desarrollo activo y no
   > ofrece API estable. La configuración (`loomcoding.toml`), las variables
   > de entorno, el formato de sesión y el protocolo de eventos pueden cambiar
   > sin aviso entre versiones. No lo uses en producción ni en flujos de
   > trabajo críticos. Se aceptan cambios incompatibles hasta que se anuncie
   > una versión estable.
   ```

2. Sustituir «CodeLoom» por «Loomcoding» y el logo/alt.
3. Renombrar `docs/assets/codeloom-logo.png` → `loomcoding-logo.png` y
   actualizar la referencia.
4. Sección de migración breve: `codeloom.toml` → `loomcoding.toml`,
   `CODELOOM_*` → `LOOMCODING_*`, `~/.codeloom/` → `~/.loomcoding/`.

**Verificación:** el README renderiza la alerta y no queda ninguna mención a
CodeLoom como nombre del producto.

### Fase 7 — docs, planes y AGENTS.md

1. `AGENTS.md`: nombre del producto, crates, binarios, plugin, ejemplos de
   comandos (`cargo test -p loomcoding-core`).
2. `docs/specs/**` y `docs/plans/**`: actualizar referencias al nombre y a las
   rutas de crate en los documentos **vigentes**. Los documentos históricos
   (`docs/plans/hechos/`) **no se tocan**: son registro de lo que pasó.
3. Este spec y este plan se quedan como están (son el registro del renombrado).

**Verificación:** ningún documento vigente menciona `codeloom` salvo los
históricos y este par.

### Fase 8 — verificación de cierre

1. Grep de control, debe dar **cero** fuera de históricos y este par:

   ```bash
   grep -riE 'codeloom' --include='*.rs' --include='*.toml' --include='*.md' \
     --include='*.json' --include='*.yml' --include='*.sh' . \
     | grep -v 'docs/plans/hechos/' \
     | grep -v 'loomcoding-renombrado'
   ```

2. Los cuatro checks globales.
3. Prueba manual aislada (nunca sin `LOOMCODING_HOME`):

   ```bash
   mkdir -p /tmp/lc-profile
   LOOMCODING_HOME=/tmp/lc-profile cargo run -p loomcoding-cli --bin loom
   ```

4. Ejecutar el e2e de herdr con el perfil aislado.

## Orden de commits

Cada fase es un commit que compila y pasa los cuatro checks. No se hace un
único commit gigante: si algo falla, hay que poder bisecar.

1. workspace y directorios
2. identificadores Rust
3. env vars y perfil
4. fichero de proyecto
5. plugin herdr
6. README experimental
7. docs y AGENTS.md

## Riesgos operativos

- **`git mv` en Windows**: verificar que git detecta los renombrados. Si no,
  el historial de cada fichero se pierde en el diff.
- **`.gitignore`**: puede contener rutas `target/` por crate; revisar.
- **CI**: si hay workflows con `-p codeloom-*`, se rompen en la fase 1.
- **Ficheros temporales del repo**: hay `build.log`, `tmp_*.txt`,
  `scripts_tmp_diag.ps1` y `target-peer-label/`. Limpiarlos antes de empezar
  para que el grep de cierre sea fiable, o añadirlos a `.gitignore`.
