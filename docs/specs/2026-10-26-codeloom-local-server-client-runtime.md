# Servidor local CodeLoom y clientes TUI

> Estado: aprobado para implementación el 2026-10-26.
>
> Fecha: 2026-10-26

## Objetivo

CodeLoom es dueño de su runtime compartido. El servidor local aloja un agente supervisor de runtime: conoce el conjunto de agentes registrados y su estado, los vigila, controla el runtime y atiende solicitudes de coordinación. No toma las tareas de trabajo de los agentes.

La primera ejecución de `loom.exe` inicia el servidor local y abre su TUI como cliente; las siguientes ejecuciones se conectan a ese mismo servidor y registran o seleccionan un agente asociado al directorio desde el que se lanzó. El registro se representa con AgentCards A2A. El transporte entre agentes y las operaciones de agentes usan A2A; las decisiones internas del supervisor y el IPC entre TUI y servidor no se presentan como operaciones A2A.

Herdr puede inspirar el patrón daemon + IPC, pero CodeLoom funciona sin Herdr instalado, ejecutándose o configurado.

## Modelo

```text
loom.exe (proyecto A) ─┐                 ┌─ TUI A
loom.exe (proyecto B) ─┼─ IPC local ─────┤  servidor CodeLoom
loom.exe (proyecto C) ─┘                 │  agentes, sesiones, tools y eventos
                                          └─ registro común de agentes
```

- Un servidor por perfil local de usuario; todos los clientes CodeLoom de ese perfil usan el mismo endpoint. El servidor supervisor mantiene el registro de AgentCards y estado de los agentes de runtime. Cada invocación interactiva registra un agente con ID propio; `--attach <id>` permite volver a conectarse a uno existente.
- El servidor posee los `AgentLoop`, providers, ejecución de tools, sesiones, permisos, trabajos y estado de agentes. El proceso cliente se ocupa de terminal, renderizado, teclado y diálogos; reenvía comandos y recibe eventos.
- El agente supervisor puede usar provider y modelo propios, configurados a nivel del perfil de usuario/runtime. No hereda la configuración ni el `.env` del primer proyecto que inicia el servidor, ni la de los agentes registrados. El registro y la vigilancia de estado son deterministas y no llaman al LLM; el supervisor lo consume solo para razonamiento o coordinación que lo requiera. Sus llamadas y coste se registran en una sesión propia.
- El LLM del supervisor nunca es requisito para iniciar, mantener o usar el servidor ni para que los agentes trabajen. Si falta configuración, el provider falla o vence el tiempo de espera, el runtime sigue con el flujo clásico determinista; se omite solo la decisión asistida por LLM y se informa del fallo. No se esperan reintentos indefinidos.
- El fallback no rebaja permisos ni controles de seguridad: si una operación necesita aprobación humana o autorización explícita, no se autoaprueba por indisponibilidad del LLM.
- Cada agente conserva su propio `cwd`, configuración de proyecto, sesión, permisos, presupuesto y estado. Compartir servidor no mezcla contextos ni historial.
- El servidor abre y escribe SQLite. Las TUI no abren directamente la misma base de datos: el `SqliteStore` actual asume un solo proceso por base.
- Los cambios de estado y resultados llegan a los clientes por eventos del servidor, no por polling.
- El descubrimiento y la comunicación entre agentes usan las tarjetas y operaciones A2A implementadas por CodeLoom. No se añaden herramientas `local_agents`/`ask_local_agent` como protocolo alternativo ni métodos JSON-RPC propios.
- El registro de tarjetas es una estructura interna del runtime. Para una interfaz A2A compartida, cada tarjeta anuncia su `tenant` estándar y el servidor enruta las operaciones A2A soportadas con ese valor; no se añade una operación A2A de listado de agentes.
- Los agentes del runtime son internos: su dirección A2A se limita al runtime local autorizado y no se publica fuera de él por defecto. A2A no define un campo estándar `internal`; esa clasificación pertenece al registro/política local y no se inventa un campo en AgentCard. La tarjeta contiene únicamente campos A2A válidos y capacidades que el agente realmente sirve.

## Arranque y conexión

1. `loom.exe` calcula la ruta del endpoint local del perfil de usuario.
2. Si el servidor responde y negocia un protocolo compatible, el proceso se conecta como cliente y solicita un agente para su directorio.
3. Si no hay servidor, intenta adquirir el bloqueo de instancia y arrancarlo. Si otro proceso gana la carrera, se conecta al servidor que acaba de aparecer.
4. Si la conexión existente es incompatible, el cliente muestra una instrucción clara; no inicia un segundo servidor sobre el mismo almacén.
5. La primera TUI se conecta al mismo runtime que las posteriores; el servidor no depende del proceso TUI que lo arrancó para conservar a los demás clientes.

El IPC solo escucha localmente. Unix usa un socket en el directorio de datos del perfil, directorio con permisos `0700` y socket `0600`. Windows usa named pipe con `PIPE_REJECT_REMOTE_CLIENTS` y DACL explícita para el SID del usuario y `LocalSystem`; no se acepta la DACL por defecto. El protocolo usa versión y longitud-prefijo JSON con máximo de 64 KiB por marco. No abre puertos de red.

La CLI inicia un proceso oculto `loom --server` si no logra conectar. La creación del socket/primera instancia del pipe arbitra la carrera; el perdedor conecta al servidor que ganó. El servidor sigue vivo si hay clientes, turnos o jobs activos, o si sirve A2A; sale tras cinco minutos sin clientes ni trabajo activo. `--print` se ejecuta como solicitud del servidor y devuelve el resultado sin abrir TUI ni mantener un agente interactivo.

## Separación de componentes existentes

- `codeloom-core` sigue siendo el motor reusable, sin dependencia de Ratatui ni de Herdr. El servidor lo aloja.
- `codeloom-tui` pasa a ser cliente de eventos y comandos del runtime; ya no crea un `AgentLoop` independiente por proceso TUI.
- `codeloom-cli` conserva los binarios `loom` y `codeloom` y añade la selección/conexión al servidor propio.
- `codeloom-herdr` continúa opcional. Puede reportar estado o integrarse con el host, pero no arranca, localiza ni autentica el servidor CodeLoom.
- El servidor A2A existente es la implementación de protocolo A2A para agentes. El servidor supervisor del runtime es el dueño del estado y puede alojar/dirigir esas interfaces; no se crean operaciones A2A propias para UI, registro, control interno ni TUI.

## Aislamiento y seguridad

- El límite de confianza es el perfil del sistema operativo: el IPC solo admite procesos del mismo usuario. No defiende contra otro proceso malicioso ejecutado con la misma cuenta.
- Resolver permisos en el servidor, junto al agente que ejecuta la tool. La TUI recibe la solicitud y devuelve la decisión para ese agente.
- Asociar cada comando entrante a un agente/cliente autorizado; nunca aceptar rutas o identificadores de agente arbitrarios sin validación.
- Cargar configuración y entorno por agente sin mutar variables de proceso compartidas. Una clave de un proyecto no puede reemplazar la de otro agente.
- El agente solicitante solo recibe la petición que envió y el resultado aprobado y acotado. Los prompts, transcripts y herramientas del destino no se comparten.
- Una caída de una TUI desconecta solo ese cliente. No cancela agentes ni altera sesiones de otros clientes.

## Fuera de alcance

- Mensajería o tareas entre perfiles locales de usuario.
- Servidor remoto, nube, SSH o conexión entre perfiles de usuario.
- Cambios o dependencia obligatoria de Herdr.
- Migrar o reemplazar el formato actual de sesiones SQLite.

## Criterios de aceptación

1. La primera ejecución inicia un único servidor y abre una TUI conectada.
2. Ejecuciones posteriores en directorios distintos conectan al mismo servidor, no crean otro proceso de runtime y abren TUI independientes.
3. Cada cliente crea/selecciona un agente con directorio, configuración, historial, permisos y presupuesto aislados.
4. El supervisor conoce las AgentCards y el estado del conjunto. El descubrimiento entre agentes se limita a lo que exponen las tarjetas y operaciones A2A; datos internos como prompts, transcript, código y rutas completas no se filtran por defecto.
5. El cierre de una TUI no detiene otras ni cancela sus turnos.
6. Dos arranques simultáneos no crean dos servidores ni escriben simultáneamente el SQLite.
7. Servidor y clientes intercambian eventos con límites y versión de protocolo; un cliente incompatible falla de forma explícita.
8. Las llamadas entre agentes usan exclusivamente operaciones A2A soportadas; la política local aplica aprobación, permisos y presupuesto sin alterar el protocolo.
9. Una petición entre agentes requiere aprobación en la TUI destino, respeta sus permisos/presupuesto y limita petición y resultado mediante los límites del runtime y del protocolo.
10. Los tests de IPC, aislamiento, consentimiento y carrera no requieren red ni provider real.

## Decisiones que debe cerrar el plan

- El tiempo exacto de inactividad previo al apagado puede ajustarse tras medir el uso; la primera implementación usa cinco minutos.
- Los mensajes con aprobación se cancelan si la TUI destino se desconecta antes de responder.
- Definir la ruta/formato de configuración a nivel de usuario para el provider/modelo del supervisor y su manejo de credenciales antes de implementarlo. No usar `.env` de proyecto como fallback.
- El agente supervisor y su LLM son opcionales para el avance del runtime; una indisponibilidad del modelo vuelve al comportamiento clásico y nunca detiene agentes.
