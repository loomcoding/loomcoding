# Loomcoding — Servidores locales: ventana de contexto declarada y desbordes reconocibles

> Estado: **implementada** (2026-11-04), con un cabo suelto: el criterio 8 (comprobación manual) no se ha
> verificado. Las decisiones de §4 son las que se sometieron a aprobación; lo no decidido consta en §8.
>
> Hallazgos de la implementación: (1) el adaptador OpenAI-compatible emitía `http {estado}` sin cuerpo, de modo que
> la fórmula de desborde jamás podía casar; ahora incluye el cuerpo acotado a 512 caracteres. (2) El adaptador de
> Anthropic tampoco casa con `starts_with("http 400")`, por lo que no se reconocen sus desbordes por esa vía; queda
> fuera de este corte. (3) `..Catalog::default()` no sirve fuera de la crate por el campo privado (E0451): se añadió
> `Catalog::from_providers`. (4) La declaración rige para el par exacto (URL, modelo) del arranque; si se cambia de
> modelo en `/settings`, vuelve a mandar el catálogo hasta reiniciar.
>
> Ampliada el 2026-11-04 con las medidas de una sesión de pruebas sobre dos GPU (§9): aparece un tercer
> defecto (C3, plazo de petición fijo) y se matiza el significado de `context_window` (D3). Las decisiones
> D1, D2, D4 y D5 no cambian.
>
> Relacionado: `docs/specs/2026-10-22-codeloom-decalogo-de-contexto-y-coste.md` (toda cota se declara y
> se prueba) y las specs ya hechas `context-compaction`, `background-compaction` y `context-meter`
> (véase `docs/INDEX.md`).

## 1. Problema

Cabe señalar que Loomcoding ya conversa con servidores locales compatibles con OpenAI
(`kind = "openai_compat"` con `base_url` de bucle local, que `validate_base_url` admite). El 2026-11-04 se
probó contra `llama-server` (llama.cpp; binario CUDA, versión no registrada) con Qwen3.5-9B Q4_K_M y los
indicadores `--jinja -np 2 -fit on --reasoning off`, que dejaron **35 584 tokens por hueco** (`n_ctx_slot`).
Las llamadas a herramientas funcionan (normal, segunda vuelta y en *streaming*: las tres correctas), pero el
contexto presenta dos carencias.

**C1 — Sin ventana conocida no se compacta.** `agent/turn.rs:78-83` obtiene la ventana de
`Catalog::context_window(base_url, modelo)` y el catálogo procede de models.dev. Un servidor local no consta
allí, de modo que el resultado es `None`, y el propio código lo declara: «Sin ventana conocida no se compacta»
(`turn.rs:75-77`). Tampoco arranca la compactación en segundo plano, y el medidor del pie
(`loomcoding-tui/src/app/usage.rs:240`) carece de denominador.

**C2 — El rechazo por exceso no se reconoce.** La red de seguridad (`turn.rs:249-269`) depende de
`compact::is_context_overflow` (`compact.rs:292`), que exige un 400 o un 413 cuyo cuerpo contenga una de siete
fórmulas. Se provocó un desborde a propósito (54 608 tokens en un hueco de 35 584) y el servidor respondió 400
con este cuerpo (reformateado en una línea):

```json
{"error":{"code":400,"message":"request (54608 tokens) exceeds the available context size (35584 tokens), try increasing it","type":"exceed_context_size_error","n_prompt_tokens":54608,"n_ctx":35584}}
```

Ninguna de las siete fórmulas coincide (`exceeds the context` no casa por el «available» intermedio). Se
comprobó reproduciendo la lista en un guion, no ejecutando la función real; el plan lo fija con una prueba.

**Consecuencia:** en cuanto el historial supere la ventana del hueco, el turno falla en lugar de resumir y
reintentar. Con tareas cortas no se nota; con un agente largo, sí.

**C3 — El plazo de la petición es fijo y se agota antes que la ventana.** El adaptador
`OpenAiCompatProvider` fija `request_timeout` en 120 s (`provider/openai/compat.rs:128`) y lo aplica con
`.timeout(...)` a la petición completa, *streaming* incluido (`compat.rs:280`). No se encontró en el código
ninguna vía de configuración que lo modifique. Con un servidor local lento, la petición muere por tiempo
mucho antes de alcanzar la ventana: en una RX 580 con Qwen3.5-4B, un prompt de unos 14 000 tokens tarda 113 s
y uno de 18 000, 111 a 166 s (§9). Ya ocurrió: de cinco subagentes lanzados en paralelo contra ese servidor,
uno terminó con `provider error: OpenAI-compatible response stream timed out`; su petición había tardado unos
119 s (23 s de prefill y 96 s de generación, según el registro del servidor). Que ese subagente fuese
precisamente el afectado es plausible, no demostrado.

Este error **no** lo cubre la red de seguridad de C2: no es un 400 de exceso de contexto, sino un fallo de
transporte que hoy se propaga como error del proveedor.

## 2. Alcance

**Dentro:**
- Reconocer el rechazo por exceso de contexto de llama.cpp (C2).
- Una clave opcional `[provider] context_window` con la que la persona usuaria declara la ventana de su
  servidor, de modo que la compactación preventiva, la de segundo plano y el medidor funcionen (C1).
- Una clave opcional `[provider] request_timeout_secs` con la que se ajusta el plazo de la petición (C3).

**Fuera (no se implementa aquí):**
- Detectar la ventana consultando al servidor. Véase §8, P1.
- Ventanas por perfil de subagente. Véase §8, P2.
- Cambiar `COMPACT_RATIO`, el tope de compactaciones por turno o `max_tokens`.
- Desplazamiento de contexto (*context shift*) del servidor.

## 3. Dónde se consume la ventana hoy

| Consumidor | Ubicación | Efecto de `None` |
|---|---|---|
| Compactación preventiva | `agent/turn.rs:110-136` | no compacta |
| Compactación en segundo plano | `agent/turn.rs:86-109` → `background.rs:287` | no arranca |
| Medidor del pie | `loomcoding-tui/src/app/usage.rs:240-262` | sin ventana |

Los tres llaman a `Catalog::context_window` (`catalog.rs:195`). Hay, pues, **un único punto de decisión**, y
conviene aprovecharlo.

## 4. Decisiones propuestas

**D1 — Dos fórmulas nuevas en `is_context_overflow`:** `exceeds the available context size` y
`exceed_context_size_error`. Proceden de una respuesta real, no de memoria. Se conserva la puerta de estado
(solo 400 y 413) y el resto de la función no cambia.

**D2 — Clave `[provider] context_window = <tokens>`.** Entero estrictamente positivo. Su ausencia deja el
comportamiento actual. Cero, negativo, decimal o cadena son error de configuración con mensaje que nombra la
clave (cero compactaría en cada paso, pues el umbral sería 0).

**D3 — Semántica: tokens por petición que el servidor puede procesar con holgura**, es decir, el contexto
**por hueco** (no el total) **acotado además por lo que cabe en el plazo de la petición** (D6). En
llama-server el techo duro es `n_ctx_slot` (el valor de `-c` dividido entre `-np` cuando el KV no es
unificado), pero **el valor útil puede ser mucho menor**: si el servidor procesa el prompt a `V` tokens/s y el
plazo es `T` segundos, la ventana práctica ronda `V · T` menos lo que se vaya a generar. Con 190 tok/s y
120 s son unos 15 000 tokens aunque el hueco admita 64 000 (§9). Declarar el techo duro cuando es mayor que la
ventana práctica hace que la compactación espere a un 80 % que la petición nunca llegará a alcanzar.
Si se declara el total del servidor, la compactación llegará tarde y quedará la red de D1.

**D4 — Mecanismo: la declaración viaja en el `Catalog`.** Un campo privado, no serializado, y un constructor
`Catalog::with_window_override`; `context_window` lo consulta antes que los datos de models.dev. La clave se
lee con un **cargador aparte** (`ProviderLimitsConfig`), siguiendo el patrón de `ProviderCacheConfig`, porque
`ProviderConfig` se construye en muchos sitios. Razones:
- `turn.rs`, `background.rs` y la TUI no cambian: ya consultan el catálogo.
- No altera `Catalog::providers`, así que `knows_model`, los precios y el selector de modelos no se enteran.
- `load_catalog` (`loomcoding-cli/src/main.rs:204`) es el único lugar que fabrica catálogos para el agente
  (`shared`) y para la TUI (`tx`): basta aplicar allí la declaración, **también cuando no hay caché ni red**.

**D5 — Precedencia.** Lo declarado manda sobre lo publicado por models.dev para el mismo par (URL base
normalizada, modelo). Para cualquier otro par no se altera nada. Si en caliente se cambia de modelo o de URL
(`/settings`), la declaración deja de aplicarse; es deliberado, pues la ventana pertenece a un modelo.

**D6 — Clave `[provider] request_timeout_secs = <segundos>`.** Entero estrictamente positivo con cota
superior declarada (propuesta: `1..=3600`; el decálogo exige que toda cota conste). Su ausencia mantiene los
120 s actuales. Cero, negativo, decimal, cadena o un valor por encima de la cota son error de configuración que
nombra la clave. Se lee con el mismo cargador aparte de D4 (`ProviderLimitsConfig`, ampliado con este campo) y
se aplica al construir el adaptador OpenAI-compatible. Los adaptadores de Anthropic y Gemini conservan su
plazo propio: este alcance es solo el de servidores locales compatibles con OpenAI.

## 5. Criterios de aceptación

1. `is_context_overflow` es verdadera para el cuerpo literal de §1 precedido de `http 400 Bad Request: `, y
   falsa con el prefijo `http 500` o con un 400 que no hable de contexto.
2. Una respuesta 400 real de `OpenAiCompatProvider` con ese cuerpo se reconoce como desborde (prueba con
   `wiremock`, que fija además el formato `http {estado}: {cuerpo}`).
3. Un turno cuyo proveedor rechaza con esa redacción resume y reintenta, como ya hace con la redacción de
   OpenAI.
4. `[provider] context_window` se lee: ausente → `None`; `35584` → `Some(35584)`; `0`, `-1`, `3.5` y `"35k"`
   → error. Guardar `/settings` conserva la clave.
5. `Catalog::context_window` devuelve lo declarado con el catálogo vacío, le gana a lo publicado, no se filtra
   a otro modelo ni a otro proveedor y compara la URL como `prices` (sin espacios, sin barra final, minúsculas).
6. Con ventana declarada, un historial por encima del 80 % provoca una petición de resumen antes de la normal;
   sin declaración y sin catálogo no hay resumen (se fija la regla actual); con una ventana minúscula el número
   de resúmenes no supera `MAX_COMPACTIONS_PER_TURN`.
7. `App::context_window()` devuelve la ventana declarada.
8. Con `LOOMCODING_HOME` aislado y sin red, el medidor del pie muestra la ventana (comprobación manual, §7).
9. `[provider] request_timeout_secs` se lee: ausente → `None` (rige el plazo de 120 s); `600` → `Some(600)`;
   `0`, `-1`, `3.5`, `"10m"` y un valor por encima de la cota → error cuyo mensaje contiene
   `request_timeout_secs`. Guardar `/settings` conserva la clave.
10. Un `OpenAiCompatProvider` construido con un plazo configurado lo aplica de verdad (prueba con `wiremock`
    que retrasa la respuesta por encima y por debajo del plazo configurado), y sin configuración conserva
    los 120 s.

## 6. Uso

```toml
[provider]
kind = "openai_compat"
base_url = "http://127.0.0.1:8091/v1"
api_key_env = "LLAMA_API_KEY"
# Tokens por petición que el servidor procesa CON HOLGURA dentro del plazo (no el techo duro del hueco).
context_window = 35584
# Plazo total de cada petición, en segundos (por defecto, 120).
request_timeout_secs = 300
```

Orientación para elegirlos, con las medidas de §9:
- GPU rápida (RTX 3060 Ti, ~1 800 tok/s de prefill): la ventana práctica coincide casi con el hueco.
- GPU lenta (RX 580, ~130 a 200 tok/s): la ventana práctica con el plazo por defecto está entre 11 000 y 15 000 tokens,
  aunque el hueco admita 64 000. O se declara esa ventana pequeña o se sube `request_timeout_secs` a cambio de
  esperas de varios minutos por petición.
- Regla: `context_window ≈ tok/s de prefill × request_timeout_secs × 0,5`, dejando margen para generar.
  Es una regla de pulgar derivada de pocas medidas, no una fórmula validada.

No se debe copiar el `max_model_len` que algunos servidores publican en `/v1/models`: es la capacidad del
modelo, no la del caché reservado (en una prueba de FreeToken con el mismo modelo constaban 262 144 frente a
1 493 tokens de KV efectivos).

## 7. Riesgos

- **Valor mal declarado.** Demasiado alto: la compactación llega tarde y cubre la red de D1. Demasiado bajo:
  se compacta en exceso, con coste de resumen; lo acota `MAX_COMPACTIONS_PER_TURN`.
- **Estimación sin ancla.** Sin recuento real del proveedor, `estimate_context_tokens` usa bytes entre cuatro.
  En una prueba sintética con el tokenizador de Qwen3.5 y código lleno de cifras salieron unos 2 bytes por
  token (una muestra, no un estudio). Con `usage` del proveedor el recuento se ancla y deja de depender de la
  estimación; la red de D1 cubre el resto.
- **Compactar también cuesta, y mucho más en un servidor lento.** Resumir exige enviar el historial al
  modelo. En la RX 580, resumir 30 000 tokens son unos 3 o 4 minutos de prefill, y esa llamada de resumen
  moriría por el mismo plazo que C3. De ahí que la compactación deba dispararse pronto (con la ventana práctica
  de D3, no con el techo del hueco) y que la de segundo plano sea preferible. No se ha medido cómo se
  comporta la compactación en segundo plano contra un servidor así de lento.
- **Caché de prompt del servidor.** llama-server reutiliza el prefijo común entre peticiones, de modo que los
  turnos siguientes solo pagarían lo nuevo. Las pruebas de §9 la desactivaron adrede (`cache_prompt=false`)
  para medir el peor caso, y no se ha comprobado que Loomcoding la aproveche.
- **El timeout no es un desborde.** Compactar y reintentar ante un `timed out` no está decidido (P5): podría
  convertir un servidor lento en un bucle de resúmenes que también agotan el plazo.
- **Prueba de punta a punta no automatizable.** `load_catalog` descarga de models.dev y los tests no tocan la
  red; el criterio 8 queda como cabo suelto de comprobación manual, como otros del índice.

## 8. Preguntas abiertas

- **P1.** ¿Detectar la ventana al conectar? llama-server expone alguna ruta de propiedades y FreeToken
  `/v1/stats`; no se han verificado sus campos. Sería una mejora posterior, no un sustituto de D2.
- **P2.** Un perfil de subagente con otro modelo en el mismo servidor no hereda la declaración (la clave es
  del par URL + modelo del `loomcoding.toml`). ¿Se admite `context_window` en el perfil?
- **P3.** ¿Se impone un mínimo (por ejemplo, 4 096) además de «mayor que cero»?
- **P4.** ¿Debe avisarse en el pie cuando el servidor no es conocido y no hay `context_window`?
- **P5.** ¿Se trata el `response stream timed out` como desborde recuperable (compactar y reintentar una vez)
  o se deja como error visible? Tiene el riesgo descrito en §7; lo prudente es no tocarlo en este corte y
  limitarse a hacer el plazo configurable (D6).
- **P6.** ¿Aprovecha Loomcoding la caché de prompt de llama-server (`cache_prompt`)? Habría que comprobar
  qué envía el adaptador y medirlo.
- **P7.** La prueba de recuperación de §9 es deliberadamente fácil (un dato literal en texto repetitivo). Falta
  una prueba con preguntas sobre ficheros reales antes de afirmar nada sobre la utilidad de un modelo pequeño
  con contexto largo.

## 9. Evidencia (sesión de pruebas del 2026-11-04)

Todas las cifras son de **una sola pasada** por configuración, con otras sesiones usando la máquina a ratos:
entre ejecuciones el prefill del mismo prompt varió hasta un 50 % (128 frente a 191 tok/s con ~14 000 tokens).
Sirven para órdenes de magnitud y para comparaciones grandes, no para afinar. Servidor: llama.cpp, KV en f16
sin *flash attention* en la RX 580 (Vulkan), `-b 256 -ub 128`, `cache_prompt=false`, razonamiento desactivado.

**RTX 3060 Ti, Qwen3.5-9B Q4_K_M, CUDA, KV q8_0** (medidas anteriores de la misma jornada): `-np 2` deja
35 584 tokens por hueco con 43 a 57 tok/s de generación y 1 750 a 1 890 tok/s de prefill; `-np 1` deja 74 240
con unos 36 tok/s de generación a 70 000 tokens.

**RX 580, Qwen3.5-4B Q4_K_M, KV en VRAM**, un hueco, recuperación de una clave de 4 cifras escondida al 20 %
del texto:

| Contexto | Prompt (tok) | Prefill (tok/s) | Generación (tok/s) | Tiempo | Clave |
|---|---:|---:|---:|---:|---|
| 16k | 4 121 | 127 | 13,5 | 37 s | acierta |
| 16k | 9 035 | 137 | 12,6 | 69 s | acierta |
| 16k | 13 949 | 128 | 12,3 | 113 s | acierta |
| 32k | 8 216 | 203 | 16,3 | 42 s | acierta |
| 32k | 18 044 | 166 | 13,8 | 111 s | acierta |
| 32k | 27 872 | 154 | 12,5 | 184 s | acierta |
| 64k | 16 406 | 173 | 14,0 | 95 s | acierta |
| 64k | 36 041 | 150 | 11,6 | 244 s | acierta |
| 64k | 55 697 | 124 | 9,7 | 454 s | acierta |

Cinco huecos de 16k también arrancan y responden bien (prompts de 4 121 a 13 949 tokens, 191 a 212 tok/s de
prefill), aunque la prueba envía las peticiones de una en una y por tanto no mide concurrencia.

**KV en RAM (`--no-kv-offload`)**, mismo contexto de 64k y mismos prompts que la fila anterior:

| Prompt (tok) | VRAM | RAM | Veces más lento |
|---:|---|---|---:|
| 16 406 | 95 s · gen 14,0 | 169 s · gen 5,5 | 1,8 |
| 36 041 | 244 s · gen 11,6 | 633 s · gen 4,3 | 2,6 |
| 55 697 | 454 s · gen 9,7 | 1 379 s · gen 0,7 | 3,0 |

Con el KV en RAM acierta igualmente la clave, pero la generación se desploma. Conclusión operativa: si el
modelo y el contexto caben en VRAM, no hay motivo para sacar el KV a RAM.

**Cinco subagentes en paralelo contra la RX 580** (Qwen3.5-4B, 5 huecos de 8 192, razonamiento alto), cada uno
leyendo un fichero pequeño: cuatro terminaron con explicaciones cuyas citas de línea se cotejaron con los
ficheros (correctas, con desvíos de 1 o 2 líneas); uno agotó el plazo de 120 s (C3). El perfil de agente con
`model` y `provider` propios (`.agents/agents/*.md`) cargó y resolvió bien el servidor distinto del principal.

**RX 580, Qwen3.5-9B Q4_K_M (5,3 GiB), KV f16 en VRAM**, mismas condiciones y misma prueba de la clave. Con
este modelo la tarjeta queda casi llena (unos 7,4 GiB libres) y el rendimiento es sensiblemente peor que con el 4B:

| Contexto | Prompt (tok) | Prefill (tok/s) | Generación (tok/s) | Tiempo | Clave |
|---|---:|---:|---:|---:|---|
| 8k | 6 998 | 110 | 8,6 | 66 s | acierta |
| 16k | 13 949 | 83 | 10,3 | 172 s | acierta |
| 32k | 27 872 | 102 | 10,8 | 277 s | acierta |
| 2 huecos de 16k, una petición | 13 949 | 80 | 10,3 | 178 s | acierta |
| 2 huecos de 16k, **dos simultáneas** | 18 070 en total | 79 agregados | 5,2 por petición | 228 s | 2 de 2 |
| 48k | 12 311 | 92 | 11,8 | 135 s | acierta |
| 48k | 27 053 | 81 | 10,4 | 337 s | acierta |

La paralelización no multiplica el rendimiento: con dos peticiones simultáneas el prefill agregado (79 tok/s)
es el de una sola, y la generación por petición se reduce a la mitad.

**Incidente:** durante la tercera petición de la configuración de 48k (unos 42 000 tokens de prompt, de unos
nueve minutos), tras más de media hora de carga continua, el equipo se quedó con la pantalla en negro y hubo
que reiniciarlo a la fuerza (`Kernel-Power 41`, sin evento de restablecimiento del controlador gráfico). Es la
segunda vez que la RX 580, que además sirve el monitor, deja el equipo inutilizable con este modelo; la primera
fue repartiendo el modelo entre las dos tarjetas. No se determinó la causa (VRAM al límite, carga sostenida o
controlador). Consecuencia para esta spec: **un servidor local en la tarjeta que mueve el monitor no es un
entorno de pruebas fiable con modelos que la llenan**, y los valores de §6 para la RX 580 deben leerse como
orientativos.

**Limitación de las pruebas:** la clave siempre va al 20 % del texto y rodeada de una frase repetida, sin
distractores. Un acierto aquí no permite afirmar que el modelo razone bien sobre código real a esa longitud (P7).
