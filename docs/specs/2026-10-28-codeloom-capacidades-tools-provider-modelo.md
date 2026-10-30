# Capacidades de tools por provider, modelo y endpoint

> Estado: **pendiente**. Sin implementar: en el código solo existe una caché puntual de capacidad en el adapter de Anthropic (`capability_cache`), no el tipo de estado/evidencia por provider y modelo que propone.
>
> Fecha: 2026-10-28
>
> Relacionado: `2026-10-28-codeloom-programmatic-tool-calling.md`,
> `2026-10-28-codeloom-provider-timeouts.md` y los adapters de
> `crates/codeloom-core/src/provider/`.

## Objetivo

Antes de enviar tools, CodeLoom debe comprobar qué puede saber sobre la
combinación concreta de provider, modelo, endpoint y función solicitada. Así
puede evitar incompatibilidades conocidas y explicar con evidencia por qué una
petición no usó una capacidad.

## Qué debe comprobar

1. **Protocolo del adapter:** endpoint/wire format que se va a llamar y si el
   adapter sabe serializar y leer tools para ese protocolo. Una ruta Chat
   Completions, Responses, Anthropic Messages o Gemini no se considera
   intercambiable por tener JSON compatible.
2. **Tool calling directo:** si ese modelo y ruta admiten definiciones de
   funciones, respuestas de tool call y el ciclo de devolver resultados. Debe
   distinguirse de parallel calling y del modo `required`.
3. **Funciones concretas solicitadas:** compatibilidad del schema que se va a
   enviar, número de tools y parámetros especiales que el adapter use. No
   anunciar compatibilidad si el adapter no representa correctamente la
   respuesta.
4. **Streaming con tools:** si el endpoint y adapter pueden recibir deltas de
   texto y de argumentos de tool en una misma respuesta. La capacidad general
   de streaming no prueba esta combinación.
5. **Capacidades programáticas por separado:** PTC, code execution, tool
   search/deferred tools y herramientas gestionadas por el proveedor son
   capacidades independientes. Function calling directo o paralelo no prueba
   ninguna de ellas. En particular, la capacidad PTC de Anthropic no es lo
   mismo que server-side code execution.
6. **Ruta real de OpenRouter:** comprobar `supported_parameters` para el model
   ID y, cuando se fije o conozca el proveedor de destino, las capacidades del
   endpoint de ese proveedor. La unión de parámetros del catálogo solo prueba
   que alguna ruta los acepta; no que la ruta finalmente elegida los admita.

## Evidencia y estados

Cada capacidad se informa como una de estas cuatro opciones:

- `supported`: adapter y evidencia disponible confirman la función para el
  destino consultado.
- `unsupported`: el adapter no implementa la función o una fuente fiable dice
  que el modelo/ruta no la admite.
- `unknown`: falta metadata, no hay permiso de consulta o el proveedor enruta a
  endpoints con capacidades distintas.
- `rejected`: una petición real fue rechazada; se conserva el estado HTTP, la
  capacidad solicitada y el momento, sin guardar ni mostrar el cuerpo crudo.

La evidencia identifica adapter/protocolo, provider/base URL, model ID,
capacidad, fuente (`adapter`, metadata oficial, catálogo o respuesta real) y
fecha de observación. No se almacenan claves, cuerpos de petición, prompts ni
cuerpos de error.

La metadata se cachea por destino y modelo con caducidad. Un cambio de modelo,
base URL, protocolo o proveedor de enrutamiento invalida la entrada. Una
respuesta `rejected` invalida la afirmación positiva para esa combinación; no
se convierte en una lista global de modelos incompatibles.

## Política de ejecución

- Si el adapter no implementa una capacidad o la metadata confirma que no está
  disponible, no se envía esa forma de petición.
- `unknown` permite el tool calling directo solo si el adapter soporta el
  protocolo; CodeLoom debe mostrar que no pudo verificar la capacidad. No se
  presenta como `supported`.
- PTC solo se activa por opt-in. Con `unknown`, el adapter puede hacer un único
  intento de la petición PTC si el usuario lo habilitó. Solo un rechazo
  inequívoco de compatibilidad permite fallback a llamadas directas.
- No se hace una llamada de prueba oculta: podría generar coste, latencia o
  efectos. La comprobación previa usa metadata documentada o información local
  del adapter.
- `HTTP 400` por sí solo no significa “no admite tools”. Se informa como
  petición rechazada, salvo que el adapter pueda reconocer de forma inequívoca
  la función incompatible. Auth, permisos, límites, cuotas, parámetros
  inválidos y errores de transporte no activan fallback automático.
- Una respuesta válida sin tool call significa que el modelo decidió no llamar
  una tool; no demuestra incompatibilidad. Se distingue de una tool call
  malformada y de un stream interrumpido.
- El registro de capacidad no cambia permisos, allowlists, `ExecutionMode` ni
  validación de argumentos de CodeLoom.

## Aplicación a adapters actuales

| Adapter | Verificación previa | No inferir |
|---|---|---|
| OpenAI-compatible / Chat Completions | Compatibilidad wire conocida por el adapter; metadata del proveedor/ruta cuando haya una fuente documentada. | Que todo modelo compatible con Chat Completions admita tools, streaming de argumentos o PTC. |
| OpenAI Responses | Capacidad del adapter para Responses y metadata oficial disponible del modelo; rechazo de PTC solo ante señal explícita. | Que soporte en Chat Completions implique Responses PTC o viceversa. |
| Anthropic Messages | Modelo exacto vía Models API; revisar por separado `server_tools.code_execution` y `code_execution` para tool use ordinario y PTC. | Que soporte code execution implique PTC, o que la metadata Anthropic aplique a una ruta OpenRouter. |
| Gemini | Endpoint, modelo y función que expone el adapter; usar las tablas oficiales de function calling/code execution cuando identifiquen ese modelo. | Que function calling implique ejecución desde código o que Gemini API y un proxy OpenAI-compatible compartan el contrato. |

## Interfaz mínima

En esta iteración el runtime solo necesita una consulta de capacidades por
petición y notices concisos con estado y fuente. No se requiere un catálogo UI,
una lista mantenida de IDs de modelos, ni un servicio genérico de probing. Los
providers que no exponen metadata conservan `unknown` y el adapter aplica la
política anterior.

## Aceptación

1. El mismo model ID puede dar resultados distintos para provider, endpoint o
   protocolo distintos.
2. Metadata ausente produce `unknown`, no falsa afirmación de soporte.
3. Para OpenRouter, el catálogo agregado no se toma como garantía de un endpoint
   elegido.
4. Tool calling, streaming, parallel tools, PTC y herramientas gestionadas se
   evalúan independientemente.
5. Rechazo de autenticación, cuota, transporte o schema no se clasifica como
   incompatibilidad de tools ni provoca fallback automático.
6. No se realiza tráfico de prueba ni se exponen secretos o cuerpos crudos.
7. Pruebas con fixtures verifican parseo de metadata, cache por destino,
   invalidación y clasificación de rechazos, sin red real.

## Fuentes primarias consultadas el 2026-10-28

- [OpenAI Function calling](https://developers.openai.com/api/docs/guides/function-calling)
- [Anthropic Models API](https://platform.claude.com/docs/en/api/models)
- [Anthropic Models overview](https://platform.claude.com/docs/en/models/overview)
- [Gemini Function calling](https://ai.google.dev/gemini-api/docs/function-calling)
- [OpenRouter model catalog API](https://openrouter.ai/docs/api/api-reference/models/get-models)
- [OpenRouter tool calling](https://openrouter.ai/blog/tutorials/tool-calling/)
