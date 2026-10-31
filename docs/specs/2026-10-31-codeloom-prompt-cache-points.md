# CodeLoom — Puntos de caché de prompts en el adaptador de Anthropic

> Estado: **implementado** (2026-10-31); medido en real con Haiku 4.5 por OpenRouter.
>
> Fecha: 2026-10-31
>
> Origen: candidata B1 de `docs/references/2026-10-31-agentpatterns-candidatas-a-implementar.md`.
> Datos propios: `docs/references/2026-10-31-medicion-real-cache-openrouter.md`.
> Relacionado: `2026-10-31-codeloom-cache-usage-ledger.md` (registro de la caché).
>
> Patrones afines (agentpatterns.ai, CC BY 4.0):
> [The Four Terms That Decide What an Agent Task Costs](https://agentpatterns.ai/token-engineering/four-term-agent-cost-model/),
> [Reducing System-Prompt Token Bloat](https://agentpatterns.ai/context-engineering/system-prompt-bloat-reduction/).

## 1. Problema

Medido con Haiku 4.5 por OpenRouter: sin `cache_control` el proveedor no cachea el prefijo repetido. Con un punto de
caché sobre un prefijo de unos 9 700 tokens, la escritura costó un 25 % más y la lectura siguiente cerca de un 90 %
menos. Un agente de código repite el mismo prefijo (herramientas y sistema) en cada paso de cada turno.

## 2. Objetivo

Que el adaptador de Anthropic (`kind = "anthropic"`, incluida la ruta de OpenRouter compatible con Anthropic) marque
puntos de caché, sin cambiar lo que se envía al modelo ni el comportamiento del agente.

## 3. No objetivos

- No afecta a `openai_compat`, `openai_responses` ni `gemini`, que no usan este adaptador. Nótese que la
  configuración por defecto de CodeLoom usa `openai_compat`: este cambio no la toca.
- No usa la caché de 1 hora (TTL de 5 minutos, la predeterminada).
- No se calcula coste con caché: el coste sigue siendo el comunicado por el proveedor.

## 4. Diseño

Una función pura `apply_cache_points(&mut cuerpo)` sobre el cuerpo JSON ya construido, con como mucho **dos**
puntos (el límite del proveedor es cuatro):

1. **Sistema.** El `system` pasa de cadena a un bloque de texto con `cache_control: {"type": "ephemeral"}`. Como el
   proveedor ordena herramientas, sistema y mensajes, este punto cubre herramientas y sistema. Si el sistema está
   vacío y hay herramientas, el punto va en la última herramienta.
2. **Último mensaje.** El último bloque de contenido del último mensaje, si es de tipo `text`, `tool_use`,
   `tool_result` o `image`. Un contenido en cadena se convierte en un bloque de texto. Los bloques de razonamiento no
   admiten el marcador y se omiten.

Con el interruptor apagado el cuerpo es idéntico al actual.

**Interruptor.** `AnthropicProvider::with_prompt_cache(bool)`; por defecto apagado en la biblioteca, de modo que los
usuarios de la biblioteca y las pruebas no cambian. El CLI lo enciende salvo que `CODELOOM_PROMPT_CACHE` valga `0`,
`off` o `false`. Se eligió una variable de entorno y no una clave de `codeloom.toml` para no tocar los más de cuarenta
sitios que construyen `ProviderConfig`; es un compromiso, y después se añadió la clave `[provider] prompt_cache`
(véase `2026-10-31-codeloom-prompt-cache-config.md`), con la variable de entorno por encima.

## 5. Criterios de aceptación

1. Con el interruptor apagado, el cuerpo no cambia.
2. Encendido: el sistema es un bloque con `cache_control` y el mismo texto; el último bloque del último mensaje lleva
   `cache_control`; no hay más de dos marcadores.
3. Sin sistema y con herramientas, la última herramienta lleva el marcador.
4. Un último bloque de razonamiento no se marca.
5. Sin mensajes, solo el punto del sistema.
6. Medición real, dentro del tope de gasto autorizado: por el propio adaptador, la segunda petición con el mismo
   prefijo lee de la caché, y una tercera con un mensaje añadido lee el prefijo anterior.
7. Las cuatro comprobaciones del proyecto pasan.

## 6. Riesgos y preguntas abiertas

- Una sesión de una sola petición paga un 25 % más por la escritura del prefijo y no recupera nada. Los prefijos por
  debajo del mínimo cacheable del modelo se ignoran en silencio y no cuestan más.
- La caché caduca a los 5 minutos sin uso; un turno lento entre peticiones vuelve a pagar la escritura.
- Si el sistema cambia entre turnos (por ejemplo, recarga en caliente del registro de agentes), el prefijo se
  invalida y se reescribe.
- Los datos son de un modelo y un proveedor. No se midió Anthropic directo.
