# Referencia de comandos

**Qué es**

Cada comando con barra que Loomcoding ejecuta en local desde el compositor. Los comandos nunca se envían al modelo. Esta página recoge las 32 entradas de la tabla de comandos del código, con lo que hace cada una, un ejemplo y cuándo funciona.

**Para qué**

- Encontrar el comando que cambia un ajuste o abre un panel.
- Comprobar por qué se rechazó un comando.
- Saber qué comandos están ocultos en el menú de `/`.

**Cómo**

1. Escribe `/` al principio del compositor para abrir el menú, o escribe el nombre completo.
2. Pulsa Enter para ejecutar el comando. El texto se borra al ejecutarlo.
3. Si un comando se rechaza, se muestra el motivo y el texto que escribiste se queda en el compositor.

## Ejemplo

```text
> /model nvidia/nemotron-3.5-super-vl:free
> /effort high
> /flow odd off
```

Los dos primeros comandos se aplican al momento si no hay ningún turno en marcha. El tercero guarda el ajuste del flujo; se aplica al reiniciar Loom.

## Todos los comandos

Las entradas siguen el orden de la tabla del código. «Sin turno en marcha» significa que no se está ejecutando ningún turno.

| Comando | Qué hace | Ejemplo | Disponible cuando |
|---|---|---|---|
| `/help` | Abre el panel de ayuda. Solo, muestra los atajos de teclado (igual que F1). `/help topics` lista los temas. `/help <comando>` muestra el uso y un ejemplo de un comando. `/help <tema>` abre la página de un tema. Tab alterna entre los atajos y los temas. | `/help model`, `/help topics` | Siempre. Un nombre desconocido se rechaza: el texto queda en el compositor, con una sugerencia si hay una cercana. |
| `/new` | Inicia una sesión nueva. | `/new` | Sin turno en marcha. Si no: «Wait for the active turn before switching sessions». |
| `/sessions` | Abre el selector de sesiones. Igual que F3. | `/sessions` | Sin turno en marcha, con el mismo mensaje que `/new`. |
| `/settings` | Abre el panel de ajustes de proveedor y modelo. Igual que F2. | `/settings` | Cuando los ajustes del proveedor están cargados. |
| `/model` | Cambia el modelo. Sin identificador, abre el selector de modelos. | `/model <id>` | Sin turno en marcha y sin guardado en curso. |
| `/effort` | Cambia el esfuerzo de razonamiento. Sin nivel, abre el selector. | `/effort high`, `/effort default` | Sin turno en marcha, y el modelo admite niveles de esfuerzo. |
| `/provider` | Cambia de proveedor. Sin identificador, abre el selector. | `/provider llama` | Sin turno en marcha y sin guardado en curso. |
| `/queue` | Abre el panel de peticiones pendientes. Igual que Ctrl+K. | `/queue` | Siempre. |
| `/tools` | Abre el panel de las últimas llamadas a herramientas. Igual que Ctrl+T. | `/tools` | Siempre. |
| `/theme` | Recorre los temas de color del terminal y muestra su nombre. | `/theme` | Siempre. Alias oculto. |
| `/layout` | Recorre la distribución de pantalla: rail, minimal, cards. | `/layout` | Siempre. Alias oculto. |
| `/temas` | Abre el panel de interfaz. Igual que `/ui` sin argumentos. | `/temas` | Siempre. Alias oculto. |
| `/ui` | Abre el panel de interfaz, o fija una fila: `/ui <fila> [valor]`. | `/ui mouse off` | Siempre. Las filas desconocidas se rechazan. |
| `/crush` | Cambia al aspecto Crush. | `/crush` | Siempre. Alias oculto. |
| `/classic` | Vuelve al aspecto Cloth. | `/classic` | Siempre. Alias oculto. |
| `/cancel` | Cancela el turno activo y pausa la cola. | `/cancel` | Solo mientras corre un turno. Si no: «No active turn to cancel». |
| `/pause` | Pausa las peticiones pendientes. | `/pause` | Siempre. |
| `/resume` | Reanuda las peticiones pendientes. | `/resume` | Siempre. |
| `/parent` | Vuelve a la sesión padre. Igual que Alt+Izquierda. | `/parent` | Sin turno en marcha y con una sesión padre. |
| `/sidebar` | Muestra u oculta la barra lateral del espacio de trabajo. Igual que Ctrl+B. | `/sidebar` | Siempre. Alias oculto. |
| `/mouse` | Activa o desactiva la captura del ratón. Desactivada, puedes seleccionar texto con el ratón. | `/mouse` | Siempre. Alias oculto. Alterna; cualquier argumento se ignora. |
| `/mascot` | Muestra u oculta el muñeco de estado. Desactivado, el estado es una sola línea de texto. | `/mascot` | Siempre. Alias oculto. |
| `/logofx` | Fija los efectos del logotipo: `on`, `off`, o un número de segundos entre ráfagas. | `/logofx 30` | Siempre. |
| `/verbose` | Muestra u oculta los avisos de peticiones del proveedor en la transcripción. Sin argumento, alterna. | `/verbose on` | Siempre. Cambia solo esta sesión; no se guarda. |
| `/flow` | Lista los modos de flujo, o guarda uno como activado o desactivado. | `/flow odd off` | Siempre. Guardar necesita un proyecto abierto. Se aplica al reiniciar. |
| `/agent` | Ejecuta un agente con nombre sobre una tarea. También puedes escribir `@<name>`. | `/agent <name> [--skills a,b] <task>` | Cuando el agente está cargado y es utilizable. |
| `/focus` | Envía lo que escribas a continuación a un subagente. Sin identificador, vuelve a esta sesión. | `/focus <id>` | El identificador debe pertenecer a un subagente vivo. Ctrl+G los recorre. |
| `/agents` | Lista los agentes cargados. | `/agents` | Siempre. |
| `/a2a` | Abre el panel de tarjetas, tareas e intercambios A2A. | `/a2a` | Siempre. |
| `/refresh-agents` | Vuelve a descargar las tarjetas de los agentes remotos. | `/refresh-agents` | Cuando `[a2a] role` es `consume` o `both`. Si no: «Remote agents are not enabled». |
| `/compact` | Resume ahora el historial más antiguo. | `/compact` | Cuando hay una sesión y no hay turno en marcha. Si no, se muestra el motivo. |
| `/quit` | Sale de Loomcoding. Igual que Ctrl+Q. | `/quit` | Siempre. |

## Alias ocultos

Ocho comandos siguen funcionando si escribes su nombre completo, pero el menú de `/` no los lista. El menú solo tiene sitio para las entradas de `/ui`, que cubre los mismos ajustes.

| Comando oculto | Lo cubre en el menú |
|---|---|
| `/theme`, `/layout`, `/crush`, `/classic` | `/ui` (tema, estilo, distribución) |
| `/temas` | `/ui` (panel de interfaz) |
| `/mascot`, `/sidebar`, `/mouse` | `/ui` (muñeco, barra lateral, ratón) |

## Comandos rechazados

Un comando rechazado conserva su texto en el compositor y muestra uno de estos mensajes. Los mensajes aparecen en inglés, como en la aplicación.

| Mensaje | Comandos |
|---|---|
| Wait for the active turn before switching sessions | `/new`, `/sessions` |
| Wait for the active turn before applying settings | `/model`, `/provider`, `/effort` |
| Provider settings are unavailable | `/settings`, `/model`, `/provider`, `/effort` |
| No active turn to cancel | `/cancel` |
| No session to compact; Wait for the active turn before compacting; Already compacting | `/compact` |
| Remote agents are not enabled ([a2a] role) | `/refresh-agents` |
| Una línea de uso que muestra los argumentos admitidos | `/verbose`, `/ui`, `/flow` |
| Unknown row: <nombre>. Rows: theme, style, layout, carril, effects, spinner, mascot, sidebar, mouse | `/ui` |
| Unknown command · /help for commands · Esc to send as text | Cualquier palabra desconocida que empiece por barra |
| No help for '<name>' · did you mean /<name>? · /help lists the topics | `/help`, con un nombre que no es comando ni tema |

## Siguientes pasos

- [Atajos de teclado](shortcuts.md)
- [Configuración (`loomcoding.toml`)](configuration.md)
- [Sesiones](../concepts/sessions.md)
- [ODD y flujos (`/flow`)](../concepts/flows-odd.md)
