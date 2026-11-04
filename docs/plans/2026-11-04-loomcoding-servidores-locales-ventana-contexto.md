# Plan — Servidores locales: ventana de contexto declarada y desbordes reconocibles

> Estado: **propuesta** (2026-11-04). Pendiente de que se apruebe la spec; sin código aún.

Spec: `docs/specs/2026-11-04-loomcoding-servidores-locales-ventana-contexto.md` (decisiones D1 a D5, criterios 1 a 8).

Cabe señalar que cada tarea sigue el procedimiento del repositorio: prueba en rojo primero, implementación
mínima, y los cuatro checks al terminar (`cargo build`, `cargo test`, `cargo fmt --check`,
`cargo clippy -- -D warnings`). El árbol de trabajo trae cambios ajenos (`lib.rs`, `gauntlet.rs`, del plan
del gauntlet): no se tocan ni se añaden a los commits de este plan.

## Restricciones globales

- Cuerpo literal de llama.cpp (prueba y spec §1): `{"error":{"code":400,"message":"request (54608 tokens) exceeds the available context size (35584 tokens), try increasing it","type":"exceed_context_size_error","n_prompt_tokens":54608,"n_ctx":35584}}`.
- Fórmulas nuevas (D1): `exceeds the available context size` y `exceed_context_size_error`. La puerta de estado (solo 400 y 413) no cambia.
- Clave `[provider] context_window`: entero estrictamente positivo; cero, negativo, decimal o cadena son `CoreError::Config` cuyo mensaje nombra la clave (D2).
- Semántica: tokens por petición (por hueco), no el total (D3).
- La declaración vive en `Catalog` en un campo privado, no serializado; no altera `Catalog::providers` (D4).
- Precedencia: lo declarado gana a models.dev solo para el mismo par (URL normalizada, modelo) (D5).
- Comentarios y documentación en español pijo; identificadores y mensajes de error intactos.

## Foco de revisión

- Un 400 que habla de contexto pero llega con `http 500`: no es desborde (tarea 1).
- URL declarada con barra final o mayúsculas: debe casar igual que `prices` (tarea 3).
- Catálogo descargado después de la declaración: no debe borrarla (tarea 4).
- `/settings` guarda el fichero: la clave `context_window` no desaparece (tarea 2).
- Ventana minúscula declarada: no hay bucle de resúmenes, manda `MAX_COMPACTIONS_PER_TURN` (tarea 4).

## Estructura de ficheros

| Fichero | Responsabilidad |
|---|---|
| `crates/loomcoding-core/src/agent/compact.rs` | `is_context_overflow` con las dos fórmulas nuevas |
| `crates/loomcoding-core/src/config/mod.rs` | `ProviderLimitsConfig` (cargador de `context_window`) |
| `crates/loomcoding-core/src/catalog.rs` | declaración en `Catalog` y precedencia en `context_window` |
| `crates/loomcoding-cli/src/main.rs` | `load_catalog` aplica la declaración |
| `crates/loomcoding-core/src/agent/tests/` | pruebas de turno con redacción de llama.cpp y ventana declarada |
| `docs/`, `CHANGELOG.md` | estado, índice y manual de uso |

## Tarea 1 — Reconocer el desborde de llama.cpp (D1; criterios 1 a 3)

**Ficheros:** modifica `compact.rs:292-308`; pruebas en el módulo de tests de `compact.rs` (cerca de la línea 385),
`agent/tests/support.rs` (≈131-137) y `agent/tests/compaction_in_turn.rs`; una prueba `wiremock` junto a las del
adaptador `OpenAiCompatProvider`.

**Interfaces:** consume `is_context_overflow(&CoreError) -> bool`. Produce `PlanProvider` con texto de desborde
configurable (campo nuevo, por defecto el texto actual de OpenAI) para las pruebas de turno.

- [ ] **Paso 1: pruebas en rojo.**
  - `desborde_de_llama_cpp_se_reconoce`: `CoreError::Provider("http 400 Bad Request: " + cuerpo literal)` → `true`.
  - `desborde_de_llama_cpp_con_500_no_se_reconoce`: mismo cuerpo con prefijo `http 500` → `false`.
  - `error_400_sin_contexto_no_se_reconoce`: `http 400 Bad Request: {"error":"bad json"}` → `false`.
  - Prueba `wiremock` `respuesta_400_de_llama_cpp_llega_como_desborde`: el servidor simulado responde 400 con el
    cuerpo literal; el error devuelto por el adaptador cumple `is_context_overflow`. Fija además el formato
    `http {estado}: {cuerpo}`; si el adaptador formatea distinto, se corrige la prueba al formato real y se anota
    en la spec antes de seguir.
  - Prueba de turno en `compaction_in_turn.rs`: `PlanProvider` rechaza la primera petición con la redacción de
    llama.cpp; el turno pide un resumen, reintenta y termina bien (réplica de la prueba existente con la redacción de OpenAI).
- [ ] **Paso 2:** ejecutar `cargo test -p loomcoding-core compact` y `cargo test -p loomcoding-core compaction_in_turn`; esperado: fallan las pruebas nuevas.
- [ ] **Paso 3:** ampliar `FORMULAS` a `[&str; 9]` con las dos cadenas de las restricciones globales; actualizar el comentario de la función («una de ellas procede de una respuesta real de llama-server, 2026-11-04»).
- [ ] **Paso 4:** repetir los comandos del paso 2; esperado: verde. Cuatro checks.
- [ ] **Paso 5:** commit `feat(core): reconocer el desborde de contexto de llama.cpp`.

## Tarea 2 — Cargador de `[provider] context_window` (D2, D3; criterio 4)

**Ficheros:** modifica `config/mod.rs` (junto a `ProviderCacheConfig`, ≈289-318); pruebas en su módulo (patrón ≈679-694).

**Interfaces:** produce
`pub struct ProviderLimitsConfig { pub context_window: Option<u64> }` con
`from_toml(&str) -> Result<ProviderLimitsConfig>` y `load(&Path) -> Result<ProviderLimitsConfig>`.

- [ ] **Paso 1: pruebas en rojo.** `limites_ausentes_dan_none`; `context_window_35584_se_lee`;
  `context_window_invalida_es_error` para `0`, `-1`, `3.5` y `"35k"`, comprobando que el mensaje contiene `context_window`;
  `guardar_ajustes_conserva_context_window` (guardar con la ruta de `toml_edit` ≈415 un cambio de modelo y releer la clave).
- [ ] **Paso 2:** `cargo test -p loomcoding-core config`; esperado: no compila o falla.
- [ ] **Paso 3:** implementar siguiendo `ProviderCacheConfig` (cargador aparte, resto de claves ignorado). Deserializar como `Option<i64>` y convertir, de modo que `0` y los negativos den el error propio y no el genérico de serde; decimales y cadenas fallan en la deserialización, con el mensaje envuelto para que nombre la clave.
- [ ] **Paso 4:** esperado verde; si la prueba de guardado falla, el guardado reescribe la sección y hay que preservar la clave (corregir en `toml_edit`, no en la prueba). Cuatro checks.
- [ ] **Paso 5:** commit `feat(core): leer [provider] context_window`.

## Tarea 3 — Declaración en el catálogo (D4, D5; criterio 5)

**Ficheros:** modifica `catalog.rs` (struct `Catalog`, `context_window` en 195, constructor nuevo); pruebas en su módulo.

**Interfaces:** consume `normalize_base_url`. Produce
`Catalog::with_window_override(self, base_url: &str, model: &str, window: u64) -> Catalog` y un campo privado
`window_override: Option<(String, String, u64)>` con `#[serde(skip)]`. `Catalog::default()` y la
deserialización siguen funcionando sin él.

- [ ] **Paso 1: pruebas en rojo.** `declarada_con_catalogo_vacio_devuelve_la_ventana`;
  `declarada_gana_a_la_publicada` (catálogo con ventana distinta para el mismo par);
  `declarada_no_se_filtra_a_otro_modelo_ni_a_otro_proveedor`;
  `url_declarada_se_compara_como_prices` (`" HTTP://127.0.0.1:8091/v1/ "` contra `http://127.0.0.1:8091/v1`);
  `declaracion_no_se_serializa` (la ida y vuelta a JSON del catálogo no la conserva);
  `declaracion_no_altera_knows_model` (sigue siendo `None` con catálogo vacío).
- [ ] **Paso 2:** `cargo test -p loomcoding-core catalog`; esperado: no compila.
- [ ] **Paso 3:** implementar; `context_window` consulta primero la declaración (mismo par normalizado) y después cae al código actual. El modelo se compara exacto, como en el resto del fichero.
- [ ] **Paso 4:** esperado verde. Cuatro checks.
- [ ] **Paso 5:** commit `feat(core): ventana de contexto declarada en el catálogo`.

## Tarea 4 — Conectar con `load_catalog` y probar los consumidores (D4; criterios 6 y 7)

**Ficheros:** modifica `crates/loomcoding-cli/src/main.rs:204` y sus dos llamadores (`server/client.rs:199`,
`server/agent_build.rs:205`); pruebas en `agent/tests/` y en `loomcoding-tui` (`app/usage.rs`).

**Interfaces:** consume `ProviderLimitsConfig::load`, `Catalog::with_window_override` y `LoomcodingConfig`
(`base_url` y `model`). `load_catalog` recibe además `declared: Option<(String, String, u64)>` (URL base,
modelo, ventana); se calcula una sola vez en cada llamador a partir de la configuración ya cargada.

- [ ] **Paso 1: pruebas en rojo.**
  - `agent/tests`: con `catalogo_de_ventana` reemplazado por un catálogo vacío más `with_window_override("http://x","m",ventana)`, un historial por encima del 80 % (`historial_grande`) produce una petición de resumen (`es_peticion_de_resumen`) antes de la normal.
  - Misma prueba sin declaración y sin catálogo: **no** hay resumen (fija la regla actual).
  - Ventana declarada minúscula (por ejemplo 64): el número de resúmenes no supera `MAX_COMPACTIONS_PER_TURN`.
  - TUI: `App::context_window()` devuelve la ventana declarada con catálogo vacío.
  - CLI: prueba de la función pura que construye `declared` desde un TOML (ausente → `None`; presente → par correcto).
- [ ] **Paso 2:** ejecutar las pruebas anteriores por nombre; esperado: fallan.
- [ ] **Paso 3:** implementar. En `load_catalog`: (a) antes de leer la caché, si hay declaración, publicar `Catalog::default().with_window_override(..)` para que funcione sin caché y sin red; (b) cada catálogo leído o descargado se publica ya con `with_window_override` aplicado. Si en `client.rs:199` el directorio de trabajo no permite resolver `loomcoding.toml`, tomar la ruta del llamador que ya la tiene, sin releer a ciegas. Un error al leer la clave se muestra como el resto de errores de configuración al arrancar, no se ignora en silencio.
- [ ] **Paso 4:** esperado verde. Cuatro checks.
- [ ] **Paso 5:** commit `feat(cli): aplicar la ventana declarada al catálogo del agente y de la interfaz`.

## Tarea 5 — Documentación y comprobación manual (criterio 8)

**Ficheros:** `docs/INDEX.md`, cabeceras de spec y plan, `CHANGELOG.md`, el manual de configuración que documente `[provider]` (localizar con `Grep` de `prompt_cache`), `docs/INVARIANTS.md` solo si procede.

- [ ] **Paso 1:** añadir en `docs/INDEX.md`, tabla «Tanda del 2026-11-01 al 2026-11-05», la fila `| servidores-locales-ventana-contexto (11-04) | propuesta | …|` (formato `nombre (MM-DD)`, requerido por el guardarraíl de cobertura del índice); al ejecutarse, cambiar a `hecho con un cabo suelto` (criterio 8 manual).
- [ ] **Paso 2:** documentar `context_window` junto a `prompt_cache`, con el ejemplo de la spec §6 y la advertencia sobre `max_model_len`; entrada en `CHANGELOG.md`.
- [ ] **Paso 3:** `cargo test -p loomcoding-e2e` (guardarraíles) y cuatro checks; esperado verde.
- [ ] **Paso 4:** comprobación manual: `G:\temp\loom-qwen\lanzar-loom.cmd` con `context_window = 35584` en el `loomcoding.toml` de esa carpeta; confirmar que el medidor del pie muestra la ventana y, con una tarea que lea muchos ficheros, que aparece un resumen en lugar de un error. Anotar el resultado en la spec.
- [ ] **Paso 5:** commit `docs: ventana de contexto declarada para servidores locales`.

## Cobertura de la spec

| Spec | Tarea |
|---|---|
| D1, criterios 1 a 3 | 1 |
| D2, D3, criterio 4 | 2 |
| D4, D5, criterio 5 | 3 |
| criterios 6 y 7 | 4 |
| criterio 8, índice y manual | 5 |

Las preguntas abiertas P1 a P4 de la spec quedan fuera del plan; si se aprueba P3 (mínimo de 4 096), es una línea en la tarea 2.

## Verificación

```text
cargo build
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```
