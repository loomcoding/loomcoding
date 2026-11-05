# Loomcoding — Ajustes del modelo local (temperatura, esfuerzos declarables, salida repetida)

> Estado: **propuesta** (2026-11-05). Plan: `docs/plans/2026-11-05-loomcoding-ajustes-modelo-local.md`.

## 1. Problema

Pilotando un modelo local de 9B (Ornith) con el Gauntlet se observó que la oficina carece de tres mandos:

1. **Temperatura fija a 0,0** en cada petición (`agent/turn.rs`, `agent/compaction.rs`, `agent/handoff.rs`).
   A temperatura cero un modelo pequeño que cae en un bucle de texto repite hasta agotar el presupuesto.
2. **Esfuerzo no declarable.** `effort` ya admite cualquier cadena y viaja como `reasoning_effort`, pero la
   TUI solo ofrece los niveles del catálogo: un modelo local fuera del catálogo no tiene selector.
3. **Bucle de texto sin detector.** Solo `RepeatedToolCalls` corta repeticiones, y únicamente de herramientas
   que escriben. Un modelo que repite el mismo párrafo en cada paso, sin llamar a nada, consume los pasos.

## 2. Diseño

1. **`[limits] temperature`** (número, por defecto `0.0`, rango `0.0..=2.0`; fuera de rango es error de
   configuración). Vive en `LimitsConfig` porque ese es el único estado de petición que ya recibe `AgentLoop`
   (`with_limits`). Se usa en turno, compactación y relevo. El clasificador del enrutador conserva `0.0`
   (decisión determinista). Los subagentes locales **heredan los límites del padre** (hoy construyen su
   `AgentLoop` con los de por defecto, lo que también ignoraba `max_output_tokens`).
2. **`[provider] efforts = ["none", "low", ...]`** (lista opcional de cadenas no vacías, máximo 16). Alimenta
   `effort_options()` de la TUI cuando el catálogo no conoce el modelo; «default» (no enviar nada) siempre
   se ofrece. El valor elegido se envía tal cual.
3. **`StopReason::RepeatedOutput`**: el turno se corta cuando las **tres** últimas respuestas del asistente
   del turno actual tienen el mismo texto (normalizado: espacios colapsados, mínimo 40 caracteres) y ninguna
   llama a herramientas que escriban. Se avisa en el transcript («salida repetida») y el motivo se informa
   al modelo/usuario con `label()`. Un mensaje de usuario rearma la detección.

## 3. Criterios de aceptación

1. Una configuración con `temperature = 0.7` produce peticiones con `0.7` en el turno, la compactación y los
   subagentes; `3.0` o `-1` falla al cargar nombrando la clave.
2. `efforts` declarados aparecen como opciones de esfuerzo sin catálogo; vacío o ausente no cambia nada.
3. Tres respuestas idénticas seguidas detienen el turno con `RepeatedOutput`; dos no; textos cortos no.
4. Los cuatro chequeos en verde en cada commit.

## Preguntas abiertas

- ¿Umbral de 3 repeticiones? Se propone 3 (menos que las 6 de herramientas: aquí no hay trabajo útil que perder).
