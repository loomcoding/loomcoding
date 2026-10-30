# Especificación: error HTTP de Anthropic y resultado duplicado de subagente

Estado: implementación y pruebas locales completas; falta repetir la llamada con
el proveedor Anthropic configurado por la persona usuaria para confirmar el
mensaje concreto que devuelve su endpoint.

## Problemas

1. El adaptador Anthropic oculta el cuerpo de error y solo informa el estado HTTP,
   por lo que un `400` no permite identificar qué campo o mensaje rechaza el endpoint.
2. Al completar un subagente, su resultado se agrega al historial del padre para
   que el modelo lo use. La TUI vuelve a mostrarlo como un mensaje del usuario,
   aunque el resultado ya se puede leer en el chat del hijo.

## Resultado esperado

- Los errores Anthropic conservan un mensaje de diagnóstico acotado y legible.
- Los mensajes de sistema del historial se serializan en la ubicación aceptada
  por Messages API, sin enviar `role: system` dentro de `messages`.
- El padre conserva el resultado del subagente en el contexto del modelo, pero la
  TUI no duplica ese contenido como mensaje del usuario ni al restaurar sesión.
- Los mensajes reales de la persona usuaria y el panel del hijo siguen visibles.

## Fuera de alcance

- Cambiar credenciales, endpoint o modelo configurado por la persona usuaria.
- Reintentos automáticos o cambios generales en todos los adaptadores.
