# Diagnóstico: resultados que desaparecen y comandos que se cortan

Fecha: 2026-11-05. Sesión de dogfooding: CodeLoom trabajando sobre su propio
repositorio.

## Síntomas

1. Las respuestas de `bash` y `grep` aparecen sustituidas por
   `[tool result cleared: N bytes; run the tool again if you need it]` a los pocos
   pasos, incluso siendo de la llamada anterior.
2. Las comillas se corrompen al ejecutar: `echo "dos"` devuelve `\"dos\"`.
3. El turno se cierra tras una tool sin volver a pedir al modelo.

## Causa raíz 1 (principal): la limpieza no protege el turno en curso

`crates/codeloom-core/src/agent/clear.rs` sustituye resultados de herramienta por un
marcador. La protección del trabajo en curso depende de `provider_state`:

```rust
let continuation_calls: HashSet<&str> = history
    .iter()
    .filter(|message| message.provider_state.is_some())      // <-- condición
    .flat_map(|message| message.tool_calls().into_iter().map(|(id, _, _)| id))
    .collect();
...
if index < cutoff
    && content.len() > CLEAR_MIN_BYTES
    && !continuation_calls.contains(call_id.as_str())         // <-- protección
```

El proveedor configurado en `codeloom.toml` es `openai_compat`
(`OpenAiCompatProvider::stream`, `provider/openai.rs:276`), y **ese camino nunca
emite `ProviderState`**: las únicas emisiones están en `openai.rs:758` y `:773`,
dentro de `OpenAiResponsesProvider::stream` (`:657`).

Consecuencia: `continuation_calls` queda **siempre vacío**. La protección del turno
en curso no se activa nunca, y cualquier resultado que no esté entre los `keep` más
recientes se borra aunque pertenezca al paso inmediatamente anterior.

A esto se suman dos umbrales demasiado bajos:

- `KEEP_TOOL_RESULTS = 6` cuenta **resultados de herramienta**, no turnos. Una fase
  normal de este repositorio hace más de seis llamadas seguidas.
- `CLEAR_MIN_BYTES = 256`: cualquier salida útil supera ese tamaño, así que casi
  todo es candidato a borrarse.

Es exactamente lo que ocurrió en esta sesión: cada lectura nueva invalidaba la
anterior, y el modelo entra en un bucle de "repito la llamada porque el resultado
que necesitaba ya no está", que consume pasos y a la postre cierra el turno (síntoma
3).

Test que fija el comportamiento esperado: `clear.rs` debería conservar íntegros los
resultados del turno en curso sin depender de `provider_state`, p. ej. tomando como
frontera el último mensaje `User`.

## Causa raíz 2: comillas dobles escapadas de más

`crates/codeloom-core/src/tools/bash.rs` **no** escapa nada: pasa el comando literal
a `cmd.exe /C` (líneas 41-49, `.arg(command)`). Sin embargo la ejecución reproduce
el doble escapado:

```
> echo uno & echo "dos" & echo tres
uno
\"dos\"
tres
```

Y cmd **conserva** las barras invertidas que recibe (`echo A\B` → `A\B`), así que no
las añade el shell: el comando ya llega a Rust con `\"` dentro. El doble escapado se
produce antes, al reconstruir los argumentos de la tool call. Candidato principal:
el ida y vuelta por `message/codec.rs` (`serde_json::to_string` → `from_str`), que
re-serializa un `input` que ya venía como cadena JSON.

Estado: **mecanismo localizado, punto exacto pendiente de confirmar** con un test
que recorra tool call → ejecución y compare el `input` recibido con el emitido.

## Causa raíz 3: el turno se cierra sin volver al modelo

En `agent/mod.rs` el bucle acota el trabajo con:

- `steps` (`:859`, `:963`) contra `max_steps`;
- `tool_calls_this_turn` contra `max_tool_calls_per_turn` (`:1232`);
- la barrera de escritura de `stop.rs` (`WRITE_TOOLS` incluye `bash` y `subagent`).

Cuando el modelo emite texto sin `tool_calls`, el turno termina: no hay nueva
petición al proveedor. Combinado con la causa 1 (el modelo no puede progresar porque
sus resultados se borran) y la causa 2 (comandos que no hacen lo pedido), el turno
se cierra por falta de progreso, no por un fallo de red ni de hilo.

## Correcciones aplicadas (2026-11-05)

### Hecho: la limpieza ya no depende de `provider_state`

`clear.rs` calcula la frontera del turno con `turn_start_index` y **no poda nada** a
partir de ahí. La protección ya no depende de que el proveedor emita estado: funciona
igual con `openai_compat` y con `openai_responses`.

- `turn_start_index` prefiere el último mensaje `Human`; si no hay, el último `User`.
  Los avisos internos (jobs) son `User` con origen `Internal`, así que **no** abren
  turno nuevo — hay un test que lo fija.
- `KEEP_TOOL_RESULTS = 6` ahora cuenta solo resultados de turnos **cerrados**.
- `CLEAR_MIN_BYTES` sube de 256 a 2 048: por debajo de eso no compensa el marcador.
- `continuation_calls` se conserva: sigue siendo necesario para `openai_responses`,
  donde `provider_state` es `previous_response_id` y el servidor guarda estado.

Tests: 8 en `agent::clear` (turno en curso intacto, turnos cerrados podados, aviso
interno que no mueve la frontera, umbral, error preservado, no mutación).

### No hecho, a propósito: `provider_state` en `openai_compat`

`ProviderState` es `response_id` + `replay_items`: es el identificador de
continuación de la API Responses, que guarda estado en servidor. `openai_compat` no
tiene estado remoto, así que emitir uno sería inventar datos y romper la
continuación. El error no era que faltara el estado, sino que la limpieza **dependía**
de él. Corregido en el lado correcto.

### Hecho: comillas que llegaban escapadas (`\"`)

**El fallo no estaba en `bash.rs` ni en el códec.** `Command::arg` en Windows aplica
las reglas de entrecomillado MSVCRT a la línea de comandos, y convierte cada `"` en
`\"`. Pero `cmd.exe` no parsea con esas reglas: lee la cadena literal, así que recibía
`\"quoted\"` en lugar de `"quoted"`.

Corregido en `tools/bash.rs`: en Windows se entrega la línea cruda con
`raw_arg(format!("/d /s /c \"{command}\""))`. El `/s` hace que cmd quite solo la
primera y la última comilla y deje intacto el resto, incluidas las comillas internas.
En Unix se sigue usando `sh -c` con `arg`.

Tests añadidos (6 en `tools::bash`): comillas simples, comillas anidadas
(`echo "a b" c`), operadores (`&`) y expansión de variables.

Nota: el intérprete que ejecuta estos comandos durante la sesión es el binario ya
compilado, así que el arreglo no se ve en caliente. La validación es la batería de
tests, no la observación en vivo.

### Pendiente: distinguir "terminó" de "no pudo progresar"

`stop.rs` cierra el turno igual en ambos casos. Con la limpieza arreglada el modelo
ya puede progresar, pero conviene un aviso accionable en el segundo caso.

