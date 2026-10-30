# Especificación: fiabilidad interactiva del workflow de desarrollo

Estado: aprobado por la petición del usuario del 2026-10-29 para corregir los
fallos observados durante la prueba interactiva.

## Problema observado

La prueba en Windows mostró que el coordinador siguió llamando al modelo después
de iniciar al planner y este intentó sondear `job_list` dos veces. El dispatch
bloqueó el sondeo correctamente, pero ambos intentos aparecieron como errores
rojos. El planner también consumió sus ocho pasos con comandos POSIX (`ls` y
`pwd`) enviados a `cmd.exe`, sin crear ni informar la spec y el plan.

Una llamada inicial de `start` repitió el perfil `default` para planner e
implementador. Esa selección debe seguir rechazándose porque los tres agentes
han de ser distintos; la explicación de esa condición debe llegar al modelo en
el esquema de la herramienta.

La sesión contiene además dos mensajes `role=user` que el usuario niega haber
enviado. El almacén actual no conserva el origen del cliente. No se reclasifican
ni eliminan esos mensajes en este cambio; se registra esa limitación y no se
atribuye su origen sin evidencia.

## Contrato

- Un `development_workflow start` exitoso termina el turno del padre después de
  persistir el resultado de la herramienta. El siguiente turno del padre lo
  despierta la finalización del hijo.
- Mientras haya un workflow activo, el proveedor solo recibe el coordinador y
  las herramientas de lectura ya permitidas por el dispatch. Las herramientas
  bloqueadas no se anuncian al modelo.
- El planner recibe instrucciones de shell acordes al sistema operativo. En
  Windows debe usar `view`, `ls`, `glob` y `grep` para inspeccionar; si necesita
  shell, debe llamar explícitamente a PowerShell, ya que `bash` ejecuta
  `cmd.exe` en esa plataforma.
- Los roles de planner, implementador y revisor siguen siendo perfiles locales
  distintos. No se relajan las puertas de aprobación ni se inicia implementación
  cuando faltan artefactos verificables.
- Una finalización del planner sin resultado o sin los dos archivos requeridos
  termina en `failed`, sin iniciar implementador.
- Los errores del turno se añaden al transcript para que sigan visibles aunque
  el estado de ejecución vuelva a `ready`.

## Criterios de aceptación

1. Un inicio exitoso no causa otra petición al proveedor en el mismo turno.
2. En el siguiente turno con workflow activo, el esquema de herramientas no
   contiene `job_list`, `job_output`, `subagent_list` ni herramientas de escritura.
3. El prompt de planificación y la descripción de `bash` identifican que en
   Windows el shell es `cmd.exe` y evitan los comandos POSIX observados.
4. Una selección repetida de perfiles continúa dando un error accionable.
5. Los tests con proveedor falso confirman el corte del turno y el filtro de
   herramientas, sin red real.
6. El mensaje de finalización del subagente sigue siendo visible; no se inventan
   paths ni se presentan artefactos ausentes como éxito.

## Fuera de alcance

- Cambiar el transporte local o añadir procedencia a cada mensaje persistido.
- Inferir quién originó los dos mensajes `role=user` de esta sesión: la base de
  datos no contiene esa información.
- Cambiar límites globales de pasos, costes o concurrencia.
