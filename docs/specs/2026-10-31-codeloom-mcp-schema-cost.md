# CodeLoom — Coste de los esquemas MCP al conectar

> Estado: **implementado** (2026-10-31); véase el plan gemelo.
>
> Fecha: 2026-10-31
>
> Origen: candidata A2 de `docs/references/2026-10-31-agentpatterns-candidatas-a-implementar.md`.
> Relacionado: `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`.
>
> Patrón afín (agentpatterns.ai, CC BY 4.0):
> [MCP alwaysLoad: Eager or JIT](https://agentpatterns.ai/tool-engineering/mcp-eager-vs-jit-loading/).

## 1. Estado actual (corrige la candidata)

La candidata A2 se redactó sin tener presente que **ya existe una medición parcial**:

- `ToolRegistry::mcp_usage_components` estima, por petición y servidor, los tokens de nombre, descripción y esquema
  de las herramientas MCP incluidas, y los guarda como `UsageComponent` de tipo `Mcp`.
- El panel de capacidades los muestra por servidor y agente (`server/agente ~N tok/r`).
- `McpServerCapability` ya lleva `tool_count`, pero el panel no lo pinta.

Lo que falta es la medición **al conectar**: hoy el coste solo aparece cuando ya se ha hecho alguna petición, y no hay
ninguna señal de que el conjunto de herramientas visibles pueda ser excesivo.

## 2. Objetivo

Mostrar en el panel de capacidades, para cada servidor MCP, cuántas herramientas expone y cuántos tokens estimados
ocupan sus definiciones, desde que se conecta, y avisar cuando el total supere unos umbrales de partida.

## 3. No objetivos

- No se cambia qué herramientas se envían al modelo ni se difiere su carga (candidatas B2 y B3).
- No se sustituye la estimación por conteo real del proveedor; sigue siendo `longitud / 4`, como el resto de
  estimaciones locales, y se rotula como estimada.
- No se toca el desglose por petición ya existente.

## 4. Diseño

1. Una función pura de núcleo, `tool_schema_tokens(nombre, descripción, esquema)`, que usa `mcp_usage_components`
   y el cálculo nuevo, de modo que ambos cuentan exactamente lo mismo.
2. Un cálculo por servidor sobre las herramientas MCP recién conectadas, hecho una vez en el arranque.
3. `McpServerCapability` gana `schema_tokens: u64` (con valor por defecto 0 al deserializar, para tolerar un demonio
   o cliente de versión distinta).
4. El panel pinta `● id · N tools · ~T tok` y, bajo la lista, un aviso si el total de herramientas MCP supera **30**
   o el total de tokens estimados supera **10.000**.

Los umbrales son **puntos de partida tomados de la página citada**, que menciona caída de selección pasados 30–50
herramientas visibles y 10.000 tokens como umbral de partida para la carga diferida. No están calibrados con datos de
CodeLoom; son constantes con nombre fáciles de cambiar.

## 5. Criterios de aceptación

1. La suma por servidor coincide con la que usa `mcp_usage_components` para las mismas herramientas.
2. Las herramientas sin servidor MCP no cuentan.
3. La línea de servidor muestra herramientas y tokens; un servidor caído muestra su incidencia como hasta ahora.
4. Con el total bajo los umbrales no hay aviso; superando cualquiera de los dos, sí, y nombra cuál.
5. Una captura serializada sin el campo nuevo se deserializa con cero.
6. Las cuatro comprobaciones del proyecto pasan.

## 6. Riesgos y preguntas abiertas

- La estimación `longitud / 4` puede errar bastante con JSON denso; la cifra es orientativa.
- Los umbrales proceden de una fuente externa, no de medición propia.
- Un servidor con herramientas que cambian tras conectar (notificación de lista) quedaría desactualizado hasta
  reiniciar; hoy el runtime tampoco refresca la lista.
