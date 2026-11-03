# Especificación: identidad persistente de sesiones por proyecto

## Objetivo

Que las sesiones de una TUI vuelvan a estar disponibles al reiniciar CodeLoom
en el mismo directorio de proyecto.

## Comportamiento

- La clave de sesión del agente local es estable para el directorio canónico del
  proyecto, no para el ID efímero de cada proceso.
- Se pueden listar y reanudar sesiones existentes con la clave anterior
  `directorio#codeloom-agent:<id>`.
- Los proyectos con directorios distintos mantienen sesiones separadas.

## Aceptación

Después de abrir una nueva instancia en el mismo directorio, aparecen las
sesiones anteriores y siguen siendo rechazadas las sesiones de otros proyectos.
