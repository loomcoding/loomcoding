# Vigilancia de jobs en segundo plano (`watch`) — spec

Estado: **propuesta**. Nace de una pregunta del usuario sobre cómo manejar las tareas lanzadas y de que otros
agentes ofrecen una herramienta `Monitor` que convierte un proceso en segundo plano en un flujo de avisos.

## Problema

Loomcoding ya tiene jobs: `job_start` arranca un comando, `JobRegistry` guarda su salida acotada y expone
`list`, `read`, `subscribe`, `kill` y `wait`. Pero el modelo solo se entera de dos maneras:

1. **Sondeando** con `job_output`, que gasta turnos y contexto aunque no haya novedades.
2. **Al terminar el job**: `pending_settled_notices` entrega un único aviso `[Job N completed|failed|killed]`.

Para un servidor de desarrollo, un `cargo watch`, una suite de pruebas larga o una compilación, lo útil ocurre
**mientras corre** (un error, `listening on …`, `test result: FAILED`). Sin un mecanismo para eso, el modelo
sondea a ciegas o espera al final.

## Intención

Permitir que un job declare qué líneas de su salida merecen un aviso, y que esos avisos lleguen a la
conversación mientras el job corre, despertando al modelo si está ocioso. Es la contrapartida, dentro de nuestro
modelo de jobs, de la herramienta `Monitor` de otros agentes. **No se añade ninguna librería de planificación**
(se descartó `taskflowrs`, pensada para grafos de tareas paralelas en CPU, no para el ciclo de vida de procesos).

### Parámetro `watch` de `job_start`

```json
{ "command": "cargo watch -x test", "label": "tests",
  "watch": { "pattern": "FAILED|error\\[", "max_events": 20 } }
```

- `pattern`: expresión regular (crate `regex`, que ya depende el core; su motor no retrocede, así que no hay
  explosión exponencial). Máximo 256 bytes y límite de tamaño del autómata. Un patrón inválido hace fallar el
  `job_start` con un error claro, sin arrancar el comando.
- `max_events`: tope de líneas avisadas durante la vida del job (por defecto 20, máximo 100). Al agotarse, el job
  sigue corriendo y se anota un último aviso «watch limit reached».
- Sin `watch`, el comportamiento es el de hoy.

### Qué se considera una línea

La salida se trocea en líneas completas (separador `\n`; un resto sin salto se guarda hasta 4 KiB y se corta).
Cada línea se prueba contra el patrón. Una línea que casa genera un **evento** de al sumo 512 bytes.

### Entrega

- Los eventos se agrupan: como mucho **un aviso cada 5 s por job y hasta 10 líneas por aviso**; las líneas que
  sobran se resumen en «+N más suprimidas». Así una avalancha de errores no inunda el contexto.
- El aviso es un mensaje de la conversación, `[Job N watch] línea1\nlínea2`, entregado por el mismo camino que
  los avisos de fin de job (`flush_subagent_notices`), persistido y publicado en el bus. En el transcript sale
  con el actor «job», distinguible del usuario y del modelo.
- Despertar: si el padre está ocioso, un aviso de vigilancia es una causa de despertar, **bajo el mismo
  presupuesto de turnos automáticos** que los hijos (`MAX_AUTO_TURNS` en la TUI; `MAX_WAKE_TURNS` sin TUI).
  Con el presupuesto agotado, el aviso se entrega igualmente al siguiente turno del usuario, pero no despierta.
- Cuando el job termina, el aviso normal de fin sigue llegando y cierra la vigilancia.

### Observabilidad

`JobSnapshot` gana `watch: Option<WatchInfo { pattern, delivered, suppressed, limit_reached }>`. `job_list`, el
panel de jobs y la tarjeta del agente muestran que el job está vigilado y cuántos avisos ha dado. Es información
propia de Loomcoding: **no son estados A2A** y se rotulan como tales.

## Reglas

- Decálogo de contexto y coste: todo está acotado (patrón 256 B, línea 512 B, resto sin salto 4 KiB, 10 líneas por
  aviso, 1 aviso cada 5 s, `max_events` ≤ 100, `MAX_NOTICE_BYTES` por aviso).
- Núcleo sin `ratatui`. Ficheros nuevos ≤ 400 líneas; el troceo y el emparejado van en un módulo nuevo del core
  (`job_watch.rs`), no en `job.rs`.
- Sin red real en las pruebas. Las comprobaciones de tiempo usan reloj simulado (`tokio::time::pause`).

## No objetivos

Vigilar jobs de otro usuario o de otra sesión, scripts de vigilancia arbitrarios, patrones sobre varias líneas,
reaccionar a la salida con acciones automáticas (solo se avisa al modelo), y cualquier planificador de DAG.

## Preguntas abiertas

1. ¿Un aviso de vigilancia debe despertar al modelo por defecto (propuesto) o solo anotarse hasta el siguiente turno?
2. ¿Hace falta añadir `watch` a un job ya lanzado (`job_watch`)? Propuesta: no en esta primera versión.
