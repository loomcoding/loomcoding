# Registro de `tracing` y medición (`RUST_LOG`)

> **Estado:** implementado. Spec: `docs/specs/2026-11-07-loomcoding-registro-tracing.md`. Plan:
> `docs/plans/2026-11-07-loomcoding-registro-tracing.md`.

## Para qué sirve

Loomcoding emite eventos `tracing` en varios puntos, sobre todo para **medir** cosas antes de decidir si merecen
quedar activadas (la compresión de salida, el desarrollo orgánico). Hasta esta función nadie recogía esos eventos y
las cifras no se podían leer. Ahora, si lo pides, se escriben en un fichero. Es observabilidad de depuración y de
medición: no sustituye al transcript, que sigue siendo el diario visible de la conversación.

## Encenderlo

Define `RUST_LOG` antes de arrancar. Usa la sintaxis de `EnvFilter` de `tracing-subscriber`:

```
RUST_LOG=output_filter=info loom          # solo la compresión de salida
RUST_LOG=odd=info loom                    # solo la medición de ODD
RUST_LOG=output_filter=info,odd=info loom # ambas
RUST_LOG=info loom                        # todo lo que emita nivel info o superior
```

- **Sin `RUST_LOG` (o vacía) no se instala nada** y no se crea ningún fichero ni directorio: el comportamiento y el coste
  son los de siempre.
- Se lee al arrancar el proceso. Si cambias la variable, reinicia `loom`. El daemon es un proceso aparte que puede
  seguir vivo entre ejecuciones y conserva el filtro con el que nació: ciérralo (o cierra todos los `loom`) para que
  el cambio le llegue.

## Dónde se escribe

Un fichero por proceso, dentro del directorio de datos del perfil:

| Proceso | Fichero |
|---|---|
| Cliente (la TUI o `--print`) | `<perfil>/.loomcoding/logs/loomcoding-cliente.log` |
| Daemon (quien ejecuta el motor) | `<perfil>/.loomcoding/logs/loomcoding-daemon.log` |

`<perfil>` es el directorio personal, o el valor de `LOOMCODING_HOME` si está definido. **Los eventos del agente
(`output_filter`, `odd`) salen en el log del daemon**, porque es el daemon quien ejecuta el bucle; el del cliente suele
quedar vacío.

Garantías:

- **Nunca** se escribe en stdout ni en stderr: la TUI es dueña de la terminal y un solo byte la rompería.
- **Acotado:** al pasar de 5 MiB el fichero rota a `<nombre>.log.1` (una sola copia anterior). No crece sin límite.
- **Nunca impide arrancar:** si no se puede abrir el fichero (permisos, disco), se ignora en silencio.
- Una línea por evento, legible y apta para `grep`: marca de tiempo, nivel, destino y campos.

Ejemplo real (compresión de salida con un modelo local):

```
2026-11-07T23:47:59.628314Z  INFO output_filter: salida filtrada regla="cargo" original_bytes=2258 sent_bytes=653
```

En Windows, el tamaño de un fichero abierto puede mostrarse como 0 hasta que se cierra el descriptor; lee el contenido,
no te fíes del tamaño del listado.

## Qué destinos existen hoy

| Destino (`target`) | Quién lo emite | Campos | Para qué |
|---|---|---|---|
| `output_filter` | `output_filter/stats.rs` (`registra_uso`) | `regla`, `original_bytes`, `sent_bytes` | Medir el ahorro de la compresión (véase `docs/output-filter.md`). |
| `odd` | `loomcoding-workflows/src/odd/state.rs` | `evento`, `total` | Contadores de medición del desarrollo orgánico (véase `docs/odd.md`). |
| (sin destino propio) | `registro_local.rs`, `sondeo_local.rs` | `error` | Avisos de configuración y sondeo de proveedores locales (`warn`). |

## Privacidad

Lo que sale al fichero es lo que cada punto emite, y los puntos de medición se diseñaron para **no** incluir datos
personales: nombres de regla, contadores y bytes; nunca el texto de un comando, rutas ni salidas de herramientas. Aun así,
`RUST_LOG=debug` o un `RUST_LOG` amplio puede arrastrar mensajes de librerías de terceros: es una decisión explícita de
quien depura. No compartas un registro sin revisarlo.

## Añadir un evento nuevo

1. Emite con `tracing::info!(target: "mi_destino", campo = valor, "mensaje breve")`. Un destino por función medible.
2. **Solo cifras y nombres de categoría.** Si necesitas distinguir llamadas, usa una huella numérica en memoria (véase
   `huella_de` en `output_filter/stats.rs`), nunca el texto.
3. Añade el destino a la tabla de arriba y a la guía de la función que mide.
4. No instales suscriptores propios: el único se instala en `crates/loomcoding-cli/src/registro_tracing.rs`.

## Para quien toque el código

- Instalación y rotación: `crates/loomcoding-cli/src/registro_tracing.rs` (`instala`, `instala_con`, `Escritor`,
  `MAX_LOG_BYTES`). Lo llama `main.rs`, con el rol `cliente` o `daemon` según `--server`.
- Pruebas: en el mismo módulo (sin `RUST_LOG` no se crea nada; el filtro por destino; la rotación; un directorio no
  escribible no falla) y el E2E `crates/loomcoding-cli/tests/tui_pty_compresion.rs`, que comprueba que el registro
  contiene la línea de `output_filter` (volcado en `docs/mockups/compresion-e2e.md`).
- Dependencia: `tracing-subscriber` con `env-filter`, ya declarada en el `Cargo.toml` del espacio de trabajo. La rotación
  es propia y mínima (sin `tracing-appender`).
