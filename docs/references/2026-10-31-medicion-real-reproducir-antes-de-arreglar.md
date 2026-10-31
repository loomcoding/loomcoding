# Medición real: ¿reproducir antes de arreglar? (D4, prueba 3)

> Fecha: 2026-10-31. Autorizada por el usuario; gasto ≈ 0,159 USD (acumulado de la sesión ≈ 0,21 de los 0,50 USD).
> Prueba: `crates/codeloom-core/tests/reproduce_first_live.rs` (`#[ignore]`; necesita `OPENROUTER_API_KEY`).
> Origen de la hipótesis: [Premature Completion](https://agentpatterns.ai/patterns/anti-patterns/premature-completion/),
> que cita un caso donde exigir reproducir primero subió un modelo pequeño del 24 % al 77 %.

## Montaje

Bucle real del agente (herramientas `view`, `edit` y `bash`), Haiku 4.5 por OpenRouter. Un proyecto Python mínimo con
**dos fallos reales** en `inventory.py` (una comparación `<=` donde la especificación dice «estrictamente menor» y un
redondeo a céntimos que falta). Dos variantes de la instrucción, tres ejecuciones cada una:

- **Base:** «los tests fallan; arregla `inventory.py` sin tocar los tests».
- **Reproducir primero:** lo mismo más «primero reproduce el fallo ejecutando los tests, luego arregla, luego vuelve a ejecutarlos».

## Resultados

| Variante | Resueltas | Ejecutó tests antes de editar | Llamadas a herramientas | Coste |
|---|---|---|---|---|
| Base | 3 de 3 | 3 de 3 | 27 (9 por ejecución) | 0,0798 $ |
| Reproducir primero | 3 de 3 | 3 de 3 | 27 (9 por ejecución) | 0,0787 $ |

## Lectura

- **No hay diferencia medible.** Haiku 4.5 ya reproduce el fallo antes de arreglarlo sin que se le pida, y resuelve la
  tarea en ambas variantes con el mismo número de pasos. Es un **efecto techo**: la tarea es demasiado fácil para este
  modelo, de modo que la prueba no puede distinguir las variantes.
- **No respalda ni refuta** la cifra de la fuente (24 % a 77 %): aquel caso era otro modelo, otra tarea y un modelo más
  pequeño. Aquí solo se descarta un efecto grande **para este modelo y esta tarea**.
- **Decisión:** no se cambian los prompts de implementación ni de corrección. No hay evidencia propia que lo justifique.
- Límites: un modelo, una tarea, tres repeticiones por variante, temperatura 0. La clasificación «ejecutó tests antes de
  editar» es una heurística (la primera llamada a `bash` cuyo texto contiene «test»).
- Si se quisiera volver a probar, haría falta un modelo más pequeño o una tarea más difícil; con el tope restante
  (≈ 0,29 USD) cabría una segunda tanda corta.
