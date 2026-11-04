# Fallos observados en subagentes (bitácora de campo)

Registro de fallos reales vistos al usar Loomcoding para desarrollar Loomcoding. No es un
spec ni un plan: es materia prima para corregir en lote. Cada entrada lleva la
evidencia (fichero y línea) y una propuesta, no una decisión.

Fecha de apertura: 2026-11-05.

## Resumen

| Fallo | Qué rompe | Severidad |
|---|---|---|
| F-01 | El turno muere al releer un fichero; el mensaje miente | alta |
| F-02 | Un subagente que falla no deja rastro | alta |
| F-03 | El reporte puede llegar truncado y se da por bueno | media |
| F-04 | El subagente afirma trabajo que no está en disco | alta |
| F-05 | El presupuesto de pasos no se adapta a la tarea | media |
| F-06 | El padre no distingue "falló" de "falló sin hacer nada" | media |
| F-07 | Mitigación provisional a mano (parche, no solución) | informativa |

**Tasa de fallo observada: 3 de 5 subagentes, y los 3 fallidos escribieron 0 ficheros.**
Con el código actual, delegar es una moneda al aire. Eso es incompatible con un producto
cuyo modelo de orquestación es precisamente la delegación.

---

## F-01 — El subagente muere en silencio al repetir una tool call, y el mensaje miente

**Severidad:** alta. Es la causa del 100 % de los fallos de esta sesión.

**Síntoma.** El subagente devuelve
`tool error: subagent produced no final response after 45 steps` y **no ha escrito
nada en disco**, aunque su reporte parcial dé a entender que sí.

**Causa raíz.** `crates/loomcoding-core/src/agent/stop.rs:33` corta el turno cuando dos
mensajes del asistente consecutivos tienen la **misma firma de tool calls**:

```rust
if sigs.len() >= 2 && sigs[sigs.len() - 1] == sigs[sigs.len() - 2] {
    return true;
}
```

La firma es `format!("{name}:{input}")` (`stop.rs:8`). Dos `view` del mismo fichero con
el mismo `offset` son la misma firma, así que releer un fichero para releer un trozo
—algo que un agente hace constantemente— **mata el turno**. El bucle sale por `break`
en `agent/mod.rs:867` sin texto final, y `subagent.rs:1084` lo reporta como
"produced no final response after N steps".

**Por qué el mensaje engaña.** `subagent.rs:1079` solo distingue el caso
`steps >= child_max_steps`. Como aquí no se agotó `max_steps` (100), cae en el `else`
y dice "after 45 steps", que suena a presupuesto agotado cuando en realidad fue un
corte por repetición. El operador no puede saber qué pasó.

**Propuesta.**
1. Distinguir el motivo del corte: `should_stop` debe devolver un `enum StopReason`
   (`StepBudget`, `RepeatedToolCalls`, `BudgetExhausted`, `Cancelled`) en vez de `bool`,
   y propagarlo hasta el mensaje de error del subagente.
2. Revisar la regla de repetición: dos lecturas idénticas seguidas son legítimas. O se
   sube el umbral (3-4), o se exceptúan las tools de solo lectura, o se compara contra
   la firma de la ventana entera y no solo las dos últimas.
3. El mensaje debe decir qué tool se repitió y con qué argumentos.

**ESTADO: ARREGLADO el 2026-11-05.** Se aplicaron las propuestas 1 y 2:

- `REPEAT_THRESHOLD` sube de 2 a 3. Dos relecturas idénticas seguidas son legítimas;
  tres ya no aportan información nueva.
- `stop_reason()` devuelve `Option<StopReason>` (`StepBudget` | `RepeatedToolCalls`) y
  `should_stop()` se conserva como envoltorio booleano para no romper llamantes.
- El mensaje de `subagent.rs` ya no miente: distingue presupuesto agotado de corte
  temprano.
- Tests nuevos: `dos_lecturas_iguales_no_cortan`, `repeticion_no_consecutiva_no_corta`,
  `un_mensaje_de_usuario_rearma_la_deteccion`, `stops_on_step_budget`.

Queda pendiente la propuesta 3 (decir qué tool se repitió): requiere propagar la firma
hasta el error, y no bloquea el desarrollo.

**Evidencia de esta sesión.** Los tres subagentes delegados fallaron, y **ninguno de los
tres escribió un solo fichero**:

| Subagente | Encargo | Resultado | Ficheros escritos |
|---|---|---|---|
| orchestrator | 2 planes, 13 fases | `no final response after 45 steps` | 0 |
| backend | Fase 1 (módulo `SubagentPhase`) | `no final response after 6 steps` | 0 |
| docs-writer | `docs/ARCHITECTURE.md` | `reached its step limit (100/100)` | 0 |

El de 6 pasos es el caso más claro: había leído el plan y estaba releyendo para
escribir. Dos lecturas del mismo fichero y turno muerto.

**El test lo consagra.** `stop.rs:58` tiene un test llamado
`stops_on_repeated_tool_calls` que **afirma** que dos `view` del mismo fichero deben
cortar el turno. No es un descuido: el comportamiento está codificado como correcto.
Corregir F-01 implica cambiar ese test, y eso es una decisión de diseño, no un bugfix.

---

## F-02 — Un subagente que falla no deja rastro de lo que hizo

**Severidad:** alta. Impide diagnosticar F-01 y cualquier otro fallo.

**Síntoma.** Cuando un subagente falla, el padre recibe una línea de error. No hay
forma de saber qué ficheros leyó, qué tools llamó, ni en qué paso se atascó.

**Causa.** El error se construye en `subagent.rs:1086` como `CoreError::Tool(reason)`
con un `String`. El historial del hijo se descarta al hacer `drop(child_agent)`
(`subagent.rs:1075`) y no se persiste en ningún sitio consultable.

**Propuesta.** Al fallar, adjuntar al error un resumen estructurado: últimas N tool
calls con nombre y argumentos, pasos consumidos, y motivo del corte. Guardarlo como
artefacto de la tarea para que el padre (y el operador) pueda leerlo.

---

## F-03 — El reporte del subagente puede llegar truncado y el padre lo da por bueno

**Severidad:** media.

**Síntoma.** Un subagente devolvió un reporte que acababa en "Creo el módulo:" y nada
más. El padre (yo) lo interpretó como éxito parcial.

**Causa.** No hay comprobación de que el reporte esté completo ni de que las
afirmaciones del subagente se correspondan con el disco. El padre confía en el texto.

**Propuesta.** El contrato de subagente debería incluir una lista de ficheros tocados
que el padre pueda verificar mecánicamente (existen, tienen el tamaño esperado) antes
de aceptar el reporte. Ver F-04.

---

## F-04 — El subagente afirma haber hecho trabajo que no está en disco

**Severidad:** alta. Es el fallo más peligroso: corrompe la confianza en todo el modelo
de delegación.

**Síntoma.** El subagente de la Fase 1 reportó "Creo el módulo:" y el fichero
`crates/loomcoding-core/src/subagent_phase.rs` **no existía**. `git status` solo mostraba
los cambios del padre.

**Causa.** Nada verifica el resultado. El subagente termina, su texto se propaga, y el
padre no tiene forma de comprobar que el trabajo existe salvo mirando el disco a mano.

**Propuesta.** Verificación obligatoria al cerrar un subagente con tools de escritura:
si el reporte menciona ficheros, comprobar que existen y que `git status` los ve. Si no
coincide, marcar la tarea como fallida en vez de devolver el texto como éxito.

---

## F-05 — El presupuesto de pasos no se adapta a la tarea

**Severidad:** media.

**Síntoma.** El orchestrator recibió un encargo de dos planes completos (13 fases) y
murió a los 45 pasos sin producir nada.

**Causa.** `max_steps` es un número fijo por configuración (`loomcoding.toml:3`, 100).
No hay noción de "esta tarea necesita más" ni de "vas por la mitad, replanifica".

**Propuesta.** O el encargo se trocea antes de delegar (lo que hice después, con
éxito), o el subagente recibe un aviso al acercarse al límite para que entregue un
reporte parcial útil en vez de morir a mitad. Lo segundo es más robusto.

---

## F-06 — El padre no puede distinguir "falló" de "falló y no hizo nada"

**Severidad:** media.

**Síntoma.** Ante un fallo, el padre no sabe si reutilizar el trabajo parcial o
empezar de cero.

**Propuesta.** El resultado de un subagente debería incluir siempre: ficheros
modificados (según `git status`), tests ejecutados y su resultado, y motivo del fallo.
Con eso el padre decide sin adivinar.

---

## F-07 — Mitigación provisional: el padre blinda el encargo a mano

**Severidad:** informativa. Es un parche, no una solución.

Mientras F-01 no esté arreglado, la única forma de que un subagente sobreviva es que el
padre le escriba en el prompt:

- "NUNCA leas el mismo fichero con el mismo `offset` dos veces."
- "Prefiere `grep` a `view`."
- "Escribe los cambios ANTES de verificar."
- Y darle el contexto ya verificado (structs, líneas, firmas) para que no tenga que
  explorar, que es lo que dispara las relecturas.

**Esto no escala.** Depende de que el padre se acuerde, gasta contexto del padre en
duplicar información que el hijo podría descubrir, y no protege de una relectura
legítima. La solución de verdad es F-01.

**Propuesta de producto:** que el propio runtime inyecte esa guía en el prompt de
sistema de todo subagente con tools de lectura, y que la regla de corte por repetición
distinga entre "releo para ver lo mismo" (legítimo) y "estoy en bucle" (cortar).

---

## F-08 — El subagente gasta el presupuesto planificando en voz alta

**Severidad:** alta. Es la variante de F-05 que más veces se ha repetido.

**Síntoma.** El subagente devuelve un reporte que es un monólogo de planificación
("mod.rs: 1–42, header.rs: 43–275, ... Espera — split_footer_meter lo usa el footer
clásico... Reasigno...") y **cero ficheros escritos**. Murió a mitad de decidir.

**Causa.** El encargo pedía una decisión de diseño grande (repartir 4.810 líneas en 6
zonas) y el subagente intentó resolverla entera en el razonamiento antes de tocar disco.
Con `max_steps` fijo, razonar consume el mismo presupuesto que actuar.

**Propuesta.**
1. **Trocear el encargo, no el fichero.** En vez de "parte render.rs en 6 módulos", un
   encargo por zona: "extrae el footer a `render/footer.rs`". Rust permite que
   `render.rs` y `render/` convivan, así que la extracción puede ser incremental y cada
   paso deja el árbol compilando.
2. El prompt debería prohibir explícitamente planificar en el reporte: "no describas el
   reparto, hazlo".
3. Un subagente que lleva N pasos sin escribir nada debería recibir un empujón del
   runtime ("escribe ya lo que tengas"), no morir en silencio.

**Evidencia.** Dos intentos de partir `render.rs`, ambos con 0 ficheros escritos. El
segundo dejó cinco ficheros temporales de índice (`.tmp-symbols*.ps1`) y nada más.

---

## F-10 — `lsp symbols` vuelca 20 KB de JSON y ahoga el contexto del subagente

**Severidad:** alta. Es la causa inmediata de los fallos de esta tanda.

**Síntoma.** Los subagentes mueren sin escribir nada, y sus reportes son monólogos de
planificación. En `~/.loomcoding/tool-output/` hay ficheros de **exactamente 20.030 bytes**,
todos volcados JSON de `lsp symbols`.

**Causa.** `MAX_TOOL_OUTPUT_BYTES = 20_000` (`crates/loomcoding-core/src/tools/mod.rs:62`).
`lsp symbols` devuelve el índice completo de símbolos de un fichero como JSON con rangos,
URIs y tags. Para `config/mod.rs` son >20 KB, así que **siempre** se trunca al máximo: el
subagente recibe 20 KB de JSON inútil (rangos de línea, `"deprecated":false`, `"tags":[]`)
que le come el contexto y no le dice nada accionable.

**Por qué es peor que un truncado normal.** El truncado corta a mitad de un objeto JSON,
así que el subagente no puede ni parsearlo. Y como el contenido es idéntico entre
llamadas, gasta pasos releyendo.

**Propuesta.**
1. `lsp symbols` no debería devolver el JSON crudo del protocolo: debería devolver una
   lista compacta `nombre (línea)` o, mejor, filtrar por `query`.
2. Si se mantiene el JSON, el límite para `lsp` debe ser mucho menor (2-4 KB) y el
   truncado debe ser semántico (cortar por símbolos, no por bytes).
3. El decálogo de contexto exige que nada sin cota llegue al modelo; aquí hay cota, pero
   está tan alta que en la práctica es "sin cota".

**Evidencia.** 8 ficheros de tool-output de 20.029-20.030 bytes entre 09:29 y 10:14,
todos con el mismo volcado de `config/mod.rs`. Coinciden con las tandas de subagentes que
fallaron.

---

## F-11 — 14 GB de RAM: el historial se carga entero y el transcript no tiene cota

**Severidad:** alta. Hace la herramienta inusable en sesiones largas.

**Medición.** `loomcoding.exe` con 25 hilos: **14,76 GB working set, 20,77 GB privada**. No es
una explosión de hilos (25 es normal): es acumulación de memoria.

**Causa 1 — `messages()` sin `LIMIT`.** `crates/loomcoding-core/src/session/sqlite.rs:327`:

```rust
sqlx::query("SELECT payload FROM messages WHERE session_id = ? ORDER BY seq ASC")
```

Carga **todo** el historial de la sesión en un `Vec<Message>` en cada turno. La
compactación reduce lo que se *envía* al modelo, pero no lo que se *lee* de disco: el test
`store_keeps_every_message_after_compaction` (`agent/mod.rs:4342`) confirma que el store
conserva todos los mensajes a propósito. En una sesión de horas con subagentes, eso son
cientos de MB de JSON, y se relee en cada paso del bucle.

**Causa 2 — el transcript no tiene cota de entradas.** `crates/loomcoding-cloth/src/state/transcript.rs:296`:

```rust
pub struct Transcript {
    entries: Vec<Entry>,
    ...
}
```

`push_built` hace `self.entries.push(entry)` sin límite. Y cada `Entry` guarda **dos veces**
el contenido: `source: String` (el texto crudo) y `lines: Vec<Line>` (las líneas derivadas,
cada una con su propio `String`). Es duplicación por diseño, y sin cota de entradas crece
sin techo. `set_fold_limit` solo pliega la *visualización*, no libera memoria.

**Causa 3 — los volcados de herramienta se retienen.** Los 20 KB de `lsp symbols` (F-10) se
guardan en el historial y en el transcript, así que un solo `lsp` mal acotado se multiplica
por cada paso del bucle.

**Propuesta.**
1. `messages()` con ventana: `ORDER BY seq DESC LIMIT N` y dar la vuelta, o cargar solo
   desde el último resumen de compactación. El store puede conservarlo todo; lo que no puede
   es *leerlo* todo en cada turno.
2. Cota de entradas en `Transcript` (por ejemplo 2.000) con descarte de las más antiguas, o
   no guardar `source` cuando ya hay `lines` derivadas.
3. Medir con un benchmark: sesión de 1.000 mensajes, memoria antes y después.

**Sobre hilos.** No es el problema aquí: 25 hilos para 14 GB significa que el trabajo no está
paralelizado, pero añadir hilos no arregla el consumo, lo empeora (cada hilo con su copia).
Primero acotar la memoria; después, si hace falta, paralelizar la ejecución de herramientas
`Parallel`, que ya están marcadas como tales.

---

## F-12 — `grep` devuelve hasta 60 KB y ahoga el contexto del subagente

**Severidad:** alta. Es la causa inmediata del fallo del subagente 310.

**Síntoma.** El subagente murió a los 6 pasos sin escribir nada. Su tool-output de las
11:22 pesa **23.099 bytes** y contiene el resultado de un `grep` con el patrón `1000`.

**Causa.** `crates/loomcoding-core/src/tools/grep.rs`:

```rust
const MAX_GREP_MATCHES: usize = 200;      // línea 9
const MAX_GREP_LINE_CHARS: usize = 300;   // línea 11
```

200 × 300 = **60 KB de techo**. Un patrón trivial como `1000` (que el subagente usó para
buscar la cota que le pedí) casa con todo el repo y devuelve 23 KB de líneas de código
ajenas a la tarea. Eso le come el contexto y no le dice nada accionable.

**Por qué es peor que F-10.** F-10 (`lsp symbols`) siempre devuelve lo mismo y se detecta
fácil. `grep` es la herramienta que un agente usa *constantemente* para orientarse, así que
el riesgo se multiplica por cada paso.

**Propuesta.**
1. Bajar el techo: 50 coincidencias × 200 caracteres = 10 KB como máximo.
2. El aviso final ya existe (`[200 of 250 matches shown; narrow the pattern]`): mantenerlo,
   pero que sea lo primero que se lea, no lo último.
3. Cuando hay muchas coincidencias, devolver **solo el recuento por fichero** en vez de las
   líneas: `crates/foo.rs: 37 coincidencias`. Eso orienta sin inundar.
4. Considerar un modo `files_only` por defecto para patrones cortos (< 4 caracteres).

**Evidencia.** `tool-call_c08097e5d7d64589b03928dc-...-0.txt`, 23.099 bytes, 11:22:29, resultado de `grep "1000"` sobre todo el repo.

---

## Notas de entorno (no son fallos del producto, pero cuestan pasos)

- `findstr /C:"texto con espacios"` en cmd falla con `No se puede abrir result"`.
- `git log --pretty=format:"%h %ad %s"` en cmd da `ambiguous argument '%ad'`.
- `%ERRORLEVEL%` no se expande en la misma línea del comando: hay que usar `&` y una
  llamada aparte, o PowerShell.
- `cargo build` choca con un daemon en marcha que tenga tomado `target/`; conviene
  `--target-dir .verify-target`.
- Las salidas de herramienta grandes se truncan en el contexto del agente, lo que
  obliga a releer en trozos y **dispara F-01**.
