# Aislamiento y ciclo de vida del runtime compartido

> Estado: aprobada para implementación como R5.

## Objetivo

Completar el servidor singleton y asegurar que cada agente mantiene configuración, entorno, sesiones, permisos y tareas aislados aunque compartan proceso y SQLite.

## Contrato

- Un servidor local por perfil; una sola conexión propietaria de SQLite.
- El estado del agente sobrevive a la desconexión de su TUI; sólo la sesión de ese agente puede volver a adjuntarse y los permisos pendientes se deniegan al perder el cliente.
- Cada agente usa cwd canonizado, configuración y `.env` propios, sin mutar variables globales.
- Arranques concurrentes convergen en un servidor. Un servidor incompatible falla explícitamente; nunca se abre una segunda base simultánea.
- A2A escucha solo con config runtime válida, autenticación configurada y bind local; exposición externa requiere proxy/config explícita y autorización.
- El apagado por inactividad considera conexiones, tareas y jobs activos; no mata trabajo en curso.

## Aceptación

1. Dos proyectos simultáneos conservan modelos, claves, sesiones y directorios separados.
2. Una TUI desconectada no cancela el turno; el attach se limita al agente y cwd autorizados.
3. Arranques simultáneos crean/usan un servidor y un SQLite.
4. A2A queda apagado si no se configuró; credenciales inválidas no dejan un listener parcial.
5. El servidor termina tras inactividad configurada y sin trabajo activo.
