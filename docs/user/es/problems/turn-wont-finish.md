# ¿Por qué el turno no termina?

**Qué es**

Con ODD activado, Loomcoding no cierra un turno mientras queden tareas abiertas de la funcionalidad. La cabecera muestra la fase, y la lista de tareas muestra las bloqueadas.

**Para qué**

- Saber por qué un turno sigue en marcha o se ha detenido.
- Desactivar la regla cuando estorba.

**Cómo**

1. Lee la cabecera y la lista de tareas.
2. Responde a la pregunta, desbloquea la tarea, o desactiva ODD y reinicia Loom.

## Ejemplo

Desactiva ODD. El cambio se aplica al reiniciar Loom:

```text
> /flow odd off
```

## Mensajes y soluciones

| Síntoma | Causa | Solución |
|---|---|---|
| El agente recibe de vuelta una lista de tareas abiertas | La regla devuelve al agente las tareas abiertas. Lo hace hasta 3 veces por turno, y nunca si la lista no ha cambiado. | Deja que el agente termine las tareas. Para desactivar la regla, consulta la sección siguiente. |
| El turno termina con `Necesita tu decisión` | Todas las tareas abiertas están bloqueadas. Una tarea bloqueada muestra su motivo en la lista como `[bloqueada: motivo]`. | Lee el motivo, respóndelo y envía una petición nueva. |
| Una tarea sigue abierta aunque el trabajo está hecho | Una tarea se marca como hecha solo con evidencia: una llamada real a `bash`, `job_start` o `job_output` de esta sesión. Sin ella, la tarea sigue abierta. | Pide al agente que vuelva a ejecutar la comprobación, y que marque la tarea con esa evidencia. |
| Aparece un recordatorio de una funcionalidad antigua en una petición no relacionada | Tras un reinicio, pueden quedar tareas abiertas de una funcionalidad anterior. El primer turno puede recibir el recordatorio. Los siguientes no insisten. | Desactiva ODD, o cierra las tareas antiguas en `.loomcoding/odd/<funcionalidad>.md`. |
| Quieres que el turno se pare ya | El turno sigue en marcha. | Escribe `/cancel`. Tu cancelación gana siempre. |

## Desactiva ODD

`/flow odd off` guarda este ajuste. Si lo prefieres, edítalo a mano:

```toml
[flows.odd]
enabled = false
```

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `[flows.odd] enabled` en `loomcoding.toml` | `true` | Sí |

## Resultado

- Con ODD desactivado, la cabecera no muestra fase, y el turno puede cerrarse con tareas abiertas.
- Un documento de funcionalidad que falta o no se puede leer nunca bloquea un turno.
- La comprobación de evidencia confirma que la llamada existe. No comprueba que el texto sea cierto.

## Siguientes pasos

- [ODD y flujos (`/flow`)](../concepts/flows-odd.md)
- [Comandos](../reference/commands.md)
- [ODD (documento técnico)](../../../odd.md)
