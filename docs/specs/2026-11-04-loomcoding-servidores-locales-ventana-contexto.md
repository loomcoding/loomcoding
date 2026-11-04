# Loomcoding — Servidores locales: ventana de contexto declarada y desbordes reconocibles

> Estado: **propuesta** (2026-11-04). Sin código aún. Las decisiones de §4 son las que se someten a
> aprobación; lo no decidido consta en §8.
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

## 2. Alcance

**Dentro:**
- Reconocer el rechazo por exceso de contexto de llama.cpp (C2).
- Una clave opcional `[provider] context_window` con la que la persona usuaria declara la ventana de su
  servidor, de modo que la compactación preventiva, la de segundo plano y el medidor funcionen (C1).

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

**D3 — Semántica: tokens que el servidor admite por petición**, es decir, el contexto **por hueco**, no el
total. En llama-server es `n_ctx_slot` (el valor de `-c` dividido entre `-np` cuando el KV no es unificado).
Si se declara el total, la compactación preventiva llegará tarde y quedará la red de D1.

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

## 6. Uso

```toml
[provider]
kind = "openai_compat"
base_url = "http://127.0.0.1:8091/v1"
api_key_env = "LLAMA_API_KEY"
context_window = 35584   # contexto POR HUECO: en llama-server, `n_ctx_slot` del log de arranque
```

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
- **Prueba de punta a punta no automatizable.** `load_catalog` descarga de models.dev y los tests no tocan la
  red; el criterio 8 queda como cabo suelto de comprobación manual, como otros del índice.

## 8. Preguntas abiertas

- **P1.** ¿Detectar la ventana al conectar? llama-server expone alguna ruta de propiedades y FreeToken
  `/v1/stats`; no se han verificado sus campos. Sería una mejora posterior, no un sustituto de D2.
- **P2.** Un perfil de subagente con otro modelo en el mismo servidor no hereda la declaración (la clave es
  del par URL + modelo del `loomcoding.toml`). ¿Se admite `context_window` en el perfil?
- **P3.** ¿Se impone un mínimo (por ejemplo, 4 096) además de «mayor que cero»?
- **P4.** ¿Debe avisarse en el pie cuando el servidor no es conocido y no hay `context_window`?
