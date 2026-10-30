# Plan: observatorio A2A

> Estado: **hecho**. Verificado el 2026-10-30 contra el código: los ficheros, comandos y herramientas que describe existen en el repositorio.

## Sección

Tarjetas, tareas y actividad A2A visible desde la TUI.

## Trabajo

1. Definir un evento local y seguro para resumir operaciones A2A.
2. Emitir observaciones para JSON-RPC de cliente y servidor y actualizaciones SSE.
3. Mantener hasta 128 observaciones en `App` y abrir el observatorio con `/a2a`.
4. Renderizar tarjetas, tareas y eventos reales con navegación por panel y detalle desplazable.
5. Añadir pruebas para datos, privacidad, cota, navegación y tamaños.

## Cierre

- Las tarjetas usan campos del `AgentCard`; los datos locales se rotulan aparte.
- La vista distingue el ID de tarea de CodeLoom del ID asignado por un remoto.
- Ningún prompt, mensaje, parte de artefacto, token o secreto entra en la traza.
- JSON-RPC y SSE se reflejan con los nombres de operación/evento A2A; los cambios
  internos del host se distinguen de las llamadas de protocolo.
- La traza y el render permanecen acotados y manejan listas vacías y terminales
  pequeñas.
- Formato, pruebas de crates afectados y build del CLI pasan; terminal real queda
  anotada si no se puede inspeccionar en esta ejecución.
