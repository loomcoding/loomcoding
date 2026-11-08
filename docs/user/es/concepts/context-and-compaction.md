# ¿Qué pasa cuando se llena el contexto y cómo lo compacto?

**Qué es**

Un modelo lee una cantidad limitada de texto de una vez: su ventana de contexto. Loomcoding estima cuánto de esa ventana ocupa el historial. Cuando el historial se acerca al límite, resume la parte más antigua.

**Para qué**

- Mantener sesiones largas sin perder la última parte de la conversación.
- Enviar menos texto al modelo, lo que reduce el coste.
- Compactar a mano antes de un paso grande.

**Cómo**

1. Mira el medidor de contexto de la interfaz.
2. Escribe `/compact` para resumir ahora el historial antiguo. Solo funciona cuando el agente está inactivo.
3. Si no lo haces, Loomcoding compacta por sí mismo cuando el historial llega al 80 % de la ventana.

## Ejemplo

```text
> /compact
```

Para preparar el resumen en segundo plano, actívalo en el fichero del proyecto:

```toml
[compaction]
background = true
```

## Cómo funciona

| Concepto | Valor |
|---|---|
| Umbral de compactación automática | 80 % de la ventana de contexto del modelo (fijo) |
| Historial reciente que se conserva íntegro | Los últimos 10.000 tokens estimados |
| Tamaño de un resumen | Como máximo 16.000 bytes, sin contar su cabecera |
| Compactaciones automáticas en un turno | Como máximo 3 |

Compactar nunca borra la sesión. El historial completo sigue en la base de datos. Lo único que se acorta es el texto que se envía al modelo.

Con `background = true`, un resumen listo y al día se usa sin llamar al modelo. Si un resumen todavía se está escribiendo, Loom espera un poco. Si no está listo a tiempo, compacta de la forma normal.

La salida de las herramientas tiene su propio límite. Una salida larga conserva su principio y su final, con una nota sobre la parte omitida y la ruta del texto completo. Un filtro de salida opcional quita ruido de los resultados de `bash` y `job_output`. Está desactivado por defecto; consulta la [guía del filtro de salida](../../../output-filter.md).

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `[compaction] background` | `false` | Sí |
| `[output_filter] enabled` | `false` | Sí |
| `LOOMCODING_NO_FILTER` (variable de entorno) | Sin valor. El valor `1` desactiva el filtro siempre. | Sí. Defínela antes de arrancar Loom. |

## Resultado

Tras `/compact`, la sesión conserva todos sus mensajes y las siguientes respuestas usan el historial más corto. Si `/compact` no hace nada, es probable que el agente esté ocupado. Espera a que termine el turno e inténtalo de nuevo.

## Siguientes pasos

- [Sesiones](sessions.md)
- [Permisos](permissions.md)
- [Agentes, subagentes y jobs](agent-subagents-jobs.md)
