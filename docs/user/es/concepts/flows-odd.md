# ¿Qué es ODD, y cómo activo o desactivo los flujos con /flow?

**Qué es**

ODD (desarrollo orgánico) es un flujo ligero para trabajar en funciones nuevas. Viene integrado en Loomcoding y está activo por defecto. Solo se nota cuando el trabajo es grande. ODD es una etiqueta propia de Loomcoding. No es un estado de A2A.

**Para qué**

- Guardar las tareas de una función en un documento de tu proyecto.
- Ver la fase actual en la cabecera.
- Evitar que un turno se cierre mientras quedan tareas abiertas.

**Cómo**

1. Pide una función.
2. Para un trabajo grande, el agente crea `.loomcoding/odd/<función>.md` en el proyecto, con las tareas como casillas.
3. Escribe `/flow` para ver los modos de flujo. Para desactivar ODD, escribe `/flow odd off` y reinicia Loom.

## Ejemplo

```text
> /flow
> /flow odd off
```

El cambio se guarda en `loomcoding.toml` y se aplica al reiniciar Loom.

## Cómo funciona

La cabecera muestra la fase del trabajo:

| Cabecera | La pone |
|---|---|
| `ODD · exploring` | Leer ficheros |
| `ODD · implementing (2/5)` | Escribir o editar ficheros, con progreso |
| `ODD · checking` | Ejecutar un comando conocido de prueba, compilación, clippy o fmt |
| `ODD · deciding` | Hacerte una pregunta |
| `ODD · planning` | Actualizar el documento de la función |

Mientras corre un `gauntlet` o un `development_workflow`, la cabecera muestra la fase de ese flujo.

Una tarea se marca como hecha solo con evidencia: una llamada real a `bash`, `job_start` o `job_output` de esta sesión. Loomcoding comprueba que la llamada existe. No comprueba que el texto de la evidencia sea cierto.

Si el agente intenta terminar el turno con tareas abiertas, Loomcoding le devuelve la lista. Esto ocurre como máximo 3 veces por turno, y nunca si la lista no ha cambiado. Una tarea bloqueada muestra su motivo. Si todas las tareas abiertas están bloqueadas, el turno termina con «Necesita tu decisión».

Un documento de función que falta o no se puede leer nunca bloquea un turno. Los subagentes no ven la herramienta de tareas de ODD ni la regla de cierre.

Para contar los eventos de ODD en el registro, define `RUST_LOG=odd=info`. Consulta la [guía de registro](../../../logging.md).

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `[flows.odd] enabled` | `true` | Sí |

`/flow` lista los modos y su estado. `/flow odd on` y `/flow odd off` guardan este ajuste. El cambio se aplica solo al reiniciar Loom.

## Límites conocidos

- La comprobación de evidencia confirma que la llamada existe, no que su texto sea cierto.
- Tras un reinicio, si quedan tareas de una función anterior, el primer turno puede recibir un recordatorio aunque tu petición no tenga relación. Después no insiste.
- La fase se guarda en memoria. Tras un reinicio aparece como «idle» hasta que se ejecuta la primera herramienta.
- `/flow odd on` y `/flow odd off` no actúan sobre la sesión en curso.

## Siguientes pasos

- [Agentes, subagentes y jobs](agent-subagents-jobs.md)
- [Permisos](permissions.md)
