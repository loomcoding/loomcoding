# Diagnóstico de fallos en subagentes locales

## Objetivo

Conservar y mostrar el error que hace fallar un subagente local para que la
persona pueda distinguir errores de provider, herramientas y ejecución.

## Alcance

- Al asentarse como fallido un job local, guardar su detalle acotado en el
  mensaje del estado A2A si ningún camino ya guardó un mensaje más específico.
- Hacer que `subagent_list` devuelva el estado real y ese mensaje opcional.
- Mantener intactos los mensajes que ya haya guardado un agente A2A remoto.
- Cubrir el flujo con una prueba usando un provider de prueba, sin red.

## Fuera de alcance

- Recuperar detalles de fallos antiguos cuyo `status_message` ya es `NULL`.
- Cambiar el supervisor: su contrato no le da acceso a tareas locales de una
  sesión de proyecto.
- Mostrar historiales completos, tool outputs o prompts del hijo.

## Aceptación

1. Un fallo del provider local queda en el `TaskStatus` fallido, limitado por el
   tope existente de avisos.
2. `subagent_list` muestra `failed` y el texto del error en campos separados.
3. Los mensajes específicos ya persistidos conservan prioridad.
