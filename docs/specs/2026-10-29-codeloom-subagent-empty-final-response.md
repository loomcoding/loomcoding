# Especificación: respuesta final vacía de un subagente

Estado: implementación y pruebas automatizadas completas; falta probarlo en la
TUI activa después de reiniciar los daemons antiguos.

## Problema

En la prueba, el subagente terminó en `step 8/8`; el padre no recibió hallazgos
y lanzó otro hijo para intentar recuperarlos. El lanzador local acepta cualquier
`TurnOutcome` exitoso, incluso cuando `final_text` está vacío, y persiste un
artefacto vacío con estado `completed`.

## Resultado esperado

- Un hijo local sin texto final no se marca como completado ni publica un artefacto vacío.
- El hijo termina en `failed` con un mensaje claro; si alcanzó `max_steps`, el
  motivo nombra ese límite.
- Un hijo con una respuesta no vacía conserva su comportamiento actual.
- No se aumenta `max_steps` ni se añade una petición extra al modelo.

## Validación

Prueba con proveedor falso que emite una llamada de herramienta en el último paso:
el resultado debe ser `failed`, sin artefactos vacíos y con el límite reflejado
en el mensaje. La prueba de respuesta normal debe seguir completando y entregar
el texto al padre.
