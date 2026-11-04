# Spec — Renombrado a Loomcoding

Estado: propuesto
Fecha: 2026-11-02

## Contexto

El proyecto se llamaba **CodeLoom**, con crates `codeloom-*`, binarios
`loom`/`codeloom`, variables de entorno `CODELOOM_*`, fichero de proyecto
`codeloom.toml` y plugin `codeloom.herdr`.

El nombre CodeLoom dejó de ser exclusivo. Datos verificados el 2026-11-02
(RDAP + DNS + registros de paquetes):

| Recurso | Estado |
|---|---|
| `codeloom.com` | en venta en HugeDomains por **6.695 USD** |
| `codeloom.dev` | registrado, web activa de un tercero |
| `codeloom.io` | registrado, «CodeLoom — Digital Product Studio» |
| `codeloom` en GitHub | cuenta vacía (0 repos, 2017) |
| `codeloom-*` en crates.io | libres, **sin publicar** |

Además, `loomcode.com` y `loomcode.dev` fueron registrados por terceros en
octubre de 2026, y `loom` en crates.io es **tokio-rs/loom**
(0.7.2, 76 M descargas), una crate fundamental del ecosistema async.

Es decir: la marca no tenía dominio disponible, el prefijo de crate estaba
amenazado desde dos frentes y el nombre corto ya estaba tomado en el registro
que más importa para este proyecto.

## Decisión

```
Marca / producto   Loomcoding          (loomcoding.com, github.com/loomcoding)
Crates             loomcoding-core, -cli, -tui, -a2a, -herdr, -mcp, -e2e
Widgets TUI        loomcoding-cloth    (antes `batten`; decidido el 2026-11-05)
Binarios           loom  y  loomcoding
Plugin herdr       loomcoding.herdr
Env vars           LOOMCODING_*
Fichero proyecto   loomcoding.toml
Directorio perfil  ~/.loomcoding/
```

`loomcoding.com` y la organización `github.com/loomcoding` ya están
registrados por el autor. Los ocho nombres `loomcoding*`/`loomcoding-*` están
libres en crates.io, npm y PyPI (verificado con controles `serde`/`react`).

### Por qué `loomcoding` como prefijo de crate y `loom` como binario

Son tres capas distintas y solo una tiene restricción de unicidad global:

| Capa | Restricción | Valor elegido |
|---|---|---|
| Nombre del binario | ninguna | `loom` (corto) y `loomcoding` |
| Nombre de la crate | único global en crates.io | `loomcoding-*` |
| Marca | registro de marca | Loomcoding |

El usuario escribe `loom` en la terminal, pero declara `loomcoding-core` en el
`Cargo.toml`. El nombre corto se conserva sin colisionar con tokio-rs.

### Por qué no `codeloom` ni `loom`

- `codeloom-*`: ata el proyecto a una marca sin dominio, con dos ocupantes
  activos en `.dev`/`.io` y una cuenta de GitHub inactiva.
- `loom-*`: colisiona frontalmente con tokio-rs/loom. Un `cargo add loom-core`
  junto a `loom` en el mismo grafo de dependencias crea confusión permanente.
- `loomcoding-*`: prefijo propio, libre, alineado con dominio y organización.

## Alcance

### Dentro

1. Renombrar los siete paquetes `codeloom-*` del workspace y sus dependencias
   internas, y `batten` → `loomcoding-cloth` (ver más abajo).
2. Renombrar los binarios: `loom` se mantiene, `codeloom` pasa a `loomcoding`.
3. Renombrar el fichero de proyecto `codeloom.toml` → `loomcoding.toml`.
4. Renombrar las variables de entorno `CODELOOM_*` → `LOOMCODING_*`.
5. Renombrar el directorio de perfil `~/.codeloom/` → `~/.loomcoding/`.
6. Renombrar el plugin `codeloom.herdr` → `loomcoding.herdr`.
7. Actualizar identificadores Rust (`codeloom_core` → `loomcoding_core`, etc.).
8. Añadir el aviso de **experimental** en el README.
9. Actualizar specs, planes, AGENTS.md y textos de usuario.

### Las 15 variables de entorno

Inventario exacto medido en el repo:

```
CODELOOM_A2A_KEY                    CODELOOM_PROGRAMMATIC_TOOL_CALLING
CODELOOM_A2A_TOKEN                  CODELOOM_PROJECT_VALUE_
CODELOOM_E2E_KEY                    CODELOOM_PROMPT_CACHE
CODELOOM_HOME                       CODELOOM_SUPERVISOR_KEY
CODELOOM_HOST_SECRET_               CODELOOM_TEST_ENV_VAR
CODELOOM_ISOLATION_TEST_            CODELOOM_TEST_MISSING
CODELOOM_LIVE_KIND                  CODELOOM_TEST_MISSING_
CODELOOM_LIVE_MODEL
```

Las que acaban en `_` son prefijos construidos en tiempo de ejecución (p. ej.
`CODELOOM_HOST_SECRET_<id>`); al renombrar hay que respetar el sufijo dinámico.

### Fuera

- Cambiar la arquitectura, el modelo de mensajes, el `trait Provider` o el
  bus de eventos. Este cambio es **solo de nomenclatura**.
- Publicar en crates.io. El plan deja los nombres reservados pero la
  publicación es una decisión aparte.
- Compatibilidad con `codeloom.toml` o `CODELOOM_*` a largo plazo.

## Aviso de experimental en el README

Requisito explícito del autor. El README debe abrir con una advertencia
visible **antes** de cualquier otra sección, con estas ideas:

- El proyecto es **experimental** y no tiene API estable.
- No hay garantía de compatibilidad entre versiones: configuración, ficheros
  de sesión y formato de eventos pueden cambiar sin aviso.
- No recomendado para uso en producción ni para flujos de trabajo críticos.
- Se aceptan cambios incompatibles hasta que se anuncie una versión estable.

Formato propuesto (GitHub renderiza las alertas nativas):

```markdown
> [!WARNING]
> **Experimental — no estable.** ...
```

Y en el cuerpo del README, donde hoy dice «CodeLoom», pasa a «Loomcoding».

## La crate `batten` pasa a `loomcoding-cloth`

**Decisión (2026-11-05, la persona usuaria):** `batten` se renombra a **`loomcoding-cloth`**,
identificador Rust `loomcoding_cloth`. Es una pieza real del telar (la maza que aprieta la trama), pero
en inglés tiene lecturas incómodas y el nombre ya está publicado por un tercero. *Cloth* (tela) sigue la
analogía del proyecto: los hilos son las líneas de código, el telar es Loomcoding y la **tela** es lo que
sale de él y lo que se ve en pantalla, que es justo lo que pinta esta crate. Libre en crates.io
(verificado el 2026-11-05). Se descartaron `loom-cloth` (rompe el prefijo `loomcoding-*` y repite el choque
con tokio-rs/loom), `swatch`, `motif`, `pattern` (choca con *design pattern*) y `fell`.

Se hace en la misma pasada que el resto, para no repetir una sustitución masiva (2.257 apariciones en 268
ficheros). Lo que sigue es el razonamiento original, que queda como registro:

`batten` es el nombre de la crate de widgets TUI. Se verificó que
**ya está publicada en crates.io por un tercero** (`batten` 0.1.0-beta.4,
autor colinwolff, 45 descargas, 2026-10-22).

Como es una crate **interna** (no se publica, no es API pública), la colisión no
era urgente, pero era una mina para el día de publicar.

Nota de marca: `batten` en inglés tiene dos lecturas incómodas para uso
público — `batten on` significa «medrar a costa de alguien», y *Batten
disease* es una enfermedad neurodegenerativa infantil. Motivo adicional para
no usarlo como marca ni publicarlo.

## Compatibilidad hacia atrás

El proyecto **aún no tiene usuarios externos** y no hay crates publicadas, así
que no se implementa capa de compatibilidad. Decisión deliberada: una capa de
alias duplicaría el inventario de nombres y contradice el objetivo.

Se acepta el coste de que un usuario con un `codeloom.toml` local tenga que
renombrarlo a mano. Se documenta en la sección de migración del README.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Colisión de marca con Loom Inc. | `loomcoding` completo es el elemento distintivo; búsqueda de marca pendiente |
| Colisión con Coder Technologies | «coder» no aparece en el nombre nuevo; riesgo reducido |
| Se olvida algún `codeloom` | El plan cierra con un grep de verificación que debe dar cero |
| Se rompe el e2e de herdr | El plugin id cambia; el test E2E se actualiza en la misma fase |
| Pérdida de historial git | El renombrado es de ficheros y texto, no de historial |

## Preguntas abiertas

1. **Marca registrada.** Falta la búsqueda en USPTO/EUIPO de `loomcoding`
   frente a Loom Inc. No bloquea el renombrado técnico, pero sí el anuncio
   público.
2. **Nombre visible de la org.** Hoy es `Loomcoding`. ¿Se prefiere
   `LoomCoding` o `Loom Coding`?
3. ~~**`batten`**~~: **resuelta el 2026-11-05**, pasa a `loomcoding-cloth`.
4. **Email de commits.** El autor mantiene `valdepeace@gmail.com` por decisión
   explícita. ¿Se añade también un `.mailmap` para el futuro remitente del
   dominio?
5. **Versión.** ¿Se reinicia el versionado a `0.1.0` con el renombrado, o se
   conserva el número actual como `0.1.0` → `0.1.0`?
