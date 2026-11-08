# ¿Por qué el modelo no responde, o se detiene pronto?

**Qué es**

Esta página cubre las respuestas que nunca llegan, los errores del proveedor y los turnos que terminan después de una herramienta. El ajuste de tiempo de espera solo se aplica al adaptador `openai_compat`.

**Para qué**

- Leer el mensaje que muestra Loomcoding.
- Subir el tiempo de espera de la petición cuando un modelo grande necesita más tiempo.

**Cómo**

1. Busca el mensaje en la primera tabla y lee su causa.
2. Aplica la solución. Si la solución edita `loomcoding.toml`, reinicia Loom.

## Ejemplo

Una petición que tarda más que el tiempo de espera termina con este mensaje:

```text
OpenAI-compatible request timed out before response headers
```

Sube el tiempo de espera en el `loomcoding.toml` del proyecto:

```toml
[provider]
request_timeout_secs = 600
```

## Mensajes y soluciones

| Mensaje | Causa | Solución |
|---|---|---|
| `OpenAI-compatible request timed out before response headers` | El servidor no envió el inicio de la respuesta dentro de `request_timeout_secs`. | Sube `request_timeout_secs`, o comprueba que el servidor no esté saturado. |
| `OpenAI-compatible request could not connect before response headers` | No hay conexión con `base_url`: el servidor está apagado, o el puerto o la ruta `/v1` son incorrectos. | Consulta [¿Por qué un servidor local aparece como down o sin acceso?](local-server-down.md). |
| `OpenAI-compatible request failed before response headers` | Otro error antes de que empezara la respuesta. | Comprueba `base_url` y la red, y envía de nuevo la petición. |
| `OpenAI-compatible response stream timed out` | La respuesta empezó y luego se detuvo más tiempo que el límite. | Sube `request_timeout_secs`. |
| `OpenAI-compatible response stream failed` | La conexión se cortó durante la respuesta. | Envía de nuevo la petición. |
| `http <status>: <texto>` | El proveedor rechazó la petición. El texto después del código indica el motivo. | `401` o `403`: revisa la clave en [Conectar un proveedor en la nube](../start/cloud-provider.md). Un mensaje sobre la longitud del contexto: empieza una sesión limpia con `/new`. |

Los adaptadores de Anthropic y Gemini usan un tiempo de espera fijo de 120 segundos. `request_timeout_secs` no lo cambia.

## Turnos que se detienen pronto

| Lo que ves | Causa | Solución |
|---|---|---|
| `[tool result cleared: N bytes; run the tool again if you need it]` en lugar de un resultado anterior | Se quitaron resultados antiguos para ahorrar contexto. La corrección está en el código desde el 2026-11-05: el turno en curso conserva sus resultados, y los resultados de hasta 2.048 bytes nunca se quitan. Consulta el [diagnóstico](../../../status/2026-11-05-diagnostico-llamadas-cortadas.md). | Compila Loom desde el código fuente actual (consulta [Instalar desde el código fuente](../start/install.md)). Si sigue ocurriendo, empieza una sesión limpia con `/new` y envía de nuevo la petición. |
| El turno termina después de un resultado de herramienta, sin más pasos | El modelo respondió con texto y sin llamada a herramienta, así que el turno se cierra. | Envía una petición nueva. Si hay tareas ODD abiertas, consulta [¿Por qué el turno no termina?](turn-wont-finish.md). |

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `[provider] request_timeout_secs` en `loomcoding.toml` | `120` segundos; de `1` a `3600` | Sí |

## Resultado

- Tras subir el tiempo de espera, una respuesta lenta se completa si llega dentro del nuevo límite.
- Un valor fuera de `1` a `3600` impide que Loom arranque. El mensaje nombra `request_timeout_secs`.

## Siguientes pasos

- [¿Por qué un servidor local aparece como down o sin acceso?](local-server-down.md)
- [Conectar un proveedor en la nube](../start/cloud-provider.md)
- [Configuración](../reference/configuration.md)
- [Medir con RUST_LOG](../guides/measure-with-rust-log.md)
