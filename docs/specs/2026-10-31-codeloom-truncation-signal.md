# CodeLoom — Señal de recorte estructurada en las salidas de herramientas

> Estado: **implementado** (spec escrita junto con el código, no antes; queda dicho).
>
> Fecha: 2026-10-31
>
> Origen: candidata B4 de `docs/references/2026-10-31-agentpatterns-candidatas-a-implementar.md`.
> Relacionado: `2026-10-22-codeloom-decalogo-de-contexto-y-coste.md`.
>
> Patrón afín (agentpatterns.ai, CC BY 4.0):
> [Graceful Tool-Output Truncation](https://agentpatterns.ai/tool-engineering/graceful-tool-output-truncation/).

## 1. Problema

Cuando una salida supera el tope, el modelo recibe cabeza y cola con un marcador **en medio**
(`[... N bytes omitted ...]`). La página citada recoge un fallo real en que el agente tomó el prefijo por el
documento completo. Con el marcador en medio, un modelo que lee con prisa o se queda con el principio no ve la señal.

## 2. Objetivo

Que todo resultado de herramienta recortado **abra** con una cabecera que diga que está recortado, cuántos bytes se
muestran de cuántos y cuántos se omiten, sin superar el tope total.

## 3. No objetivos

- No cambia el tope (`MAX_TOOL_OUTPUT_BYTES`) ni el volcado a disco con su ruta.
- No se toca `limit_bytes` ni `limit_output`, usados también por avisos, resúmenes y adaptadores como segunda red.
- No se mide si la cabecera reduce el error del agente: es una medida preventiva, sin evidencia propia.

## 4. Diseño

- `limit_bytes_with_header` en `tools/mod.rs`: igual que `limit_bytes`, con la cabecera
  `[truncated: showing S of T bytes, head and tail; O bytes omitted from the middle]` al principio. Comparte con
  `limit_bytes` el cálculo de cortes (`cut_points`).
- `contenido_de_herramienta` (el punto por el que pasa todo resultado hacia el modelo) la usa, con una reserva de
  512 bytes (antes 256) para cabecera, marcador y ruta del volcado. Un resultado que cabe se entrega intacto.
- El marcador del medio se conserva: señala dónde falta el texto.

## 5. Criterios de aceptación

1. Un resultado que cabe llega sin cabecera.
2. Un resultado recortado, con o sin volcado, empieza por la cabecera con el total real y los omitidos.
3. El total entregado no supera `MAX_TOOL_OUTPUT_BYTES`, ruta incluida.
4. Con volcado, la ruta sigue al final y el fichero guarda el original íntegro.
5. Las cuatro comprobaciones del proyecto pasan.

## 6. Riesgos

- La cabecera añade unos cien bytes por recorte; despreciable frente al tope.
- Una ruta de volcado de más de unos 370 bytes agotaría la reserva; los tests no lo cubren y las rutas del
  volcado son cortas en la práctica.
