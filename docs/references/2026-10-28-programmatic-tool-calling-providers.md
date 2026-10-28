# Referencias oficiales: llamadas programáticas por proveedor

> Consulta: 2026-10-28. Resumen de integración, no copia de las páginas originales.
> Comprobar disponibilidad por modelo y API antes de implementar; las capacidades cambian.

## Anthropic

Fuente principal: [Programmatic tool calling](https://platform.claude.com/docs/es/agents-and-tools/tool-use/programmatic-tool-calling).

- La API permite que el código de `code_execution` llame tools habilitadas mediante
  `allowed_callers`; `caller` relaciona cada tool call con la ejecución de código que la
  originó.
- La versión de `code_execution` declarada debe satisfacer `allowed_callers`. Las respuestas
  programáticas pueden pausar y reanudar el contenedor, enviando los `tool_result` pendientes
  y manteniendo el contenedor y el conjunto de tools en la continuación.
- La documentación indica consultar la Models API para `capabilities.code_execution.supported`;
  la compatibilidad no se deduce solo de que el provider sea Anthropic.
- Tools de MCP Connector no pueden invocarse programáticamente. También hay límites de
  schema y combinaciones no soportadas, por ejemplo `strict: true` y ciertas opciones de
  `tool_choice`.

## OpenAI

Fuente principal: [Programmatic Tool Calling](https://developers.openai.com/api/docs/guides/tools-programmatic-tool-calling).

- Responses API permite a un programa JavaScript orquestar tools declaradas con
  `allowed_callers: ["programmatic"]`; Agents API ofrece el PTC dentro de su propio harness.
- El programa corre en un runtime V8 aislado y hospedado, con `await`, pero sin Node.js,
  instalación de paquetes, acceso directo a red, filesystem general ni subprocess.
- La documentación lista tools compatibles y recomienda comprobar la página del modelo.
  Responses y Agents tienen integración distinta.
- La compatibilidad OpenAI Chat Completions que usa hoy CodeLoom no equivale a Responses API;
  un endpoint OpenAI-compatible no se marca como compatible sin evidencia de su contrato.

## Google Gemini

Fuentes principales: [Code execution](https://ai.google.dev/gemini-api/docs/code-execution),
[Tool combination](https://ai.google.dev/gemini-api/docs/tool-combination) y
[Function calling](https://ai.google.dev/gemini-api/docs/function-calling).

- Code Execution ejecuta Python en un entorno administrado por Google.
- La documentación actual describe combinar herramientas integradas y function calling para
  familias de modelos compatibles. Las llamadas cliente y las herramientas integradas usan
  pasos y circulación de contexto propios de Gemini.
- No es el protocolo `allowed_callers`/`caller` de Anthropic ni el runtime JavaScript PTC de
  OpenAI. Requiere adapter y evaluación propios; disponibilidad y combinaciones dependen del
  modelo/API.

## Consecuencia para CodeLoom

La unidad de soporte es **provider + modelo + API/endpoint + conjunto de tools**, no un
booleano global por provider. Hasta contar con una señal verificable y un adapter capaz de
preservar su flujo, CodeLoom usa el bucle normal de tools. La documentación de cada proveedor
justifica investigar la función, pero no demuestra compatibilidad del runtime actual de
CodeLoom.

## Beneficios a medir

Los proveedores describen ahorro de viajes, procesamiento de resultados dentro del entorno
de ejecución y menor contenido enviado al modelo. Para CodeLoom son hipótesis por validar:

- tokens de entrada/salida y bytes intermedios recibidos por el modelo;
- latencia total, latencia de provider y cantidad de decisiones del modelo;
- éxito/corrección de la tarea, revisado por categoría y modelo.

Menor latencia o consumo no equivale automáticamente a mayor calidad. No extrapolar resultados
entre proveedores ni modelos.
