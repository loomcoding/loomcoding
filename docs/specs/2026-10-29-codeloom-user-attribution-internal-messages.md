# Especificación: atribución de mensajes internos en el transcript

Estado: corrección solicitada el 2026-10-29.

## Problema

`Role::User` también se usa para contenido interno que no escribió la persona
usuaria. La TUI infería la autoría solo a partir del rol, por lo que tareas,
avisos u otros mensajes internos podían aparecer como `You`. La autoría no puede
inferirse por orden, contenido ni sesión activa.

## Resultado esperado

- Solo prompts enviados por una persona mediante el flujo de entrada se atribuyen
  como `You`.
- Los mensajes `Role::User` generados internamente llevan procedencia `Internal`
  y se muestran como sistema/actividad.
- La procedencia se conserva al persistir y restaurar. Mensajes históricos sin
  procedencia explícita se tratan como internos (fallo cerrado).
- La serialización hacia el modelo conserva los roles y textos actuales.

## Validación

Pruebas de core/TUI cubren procedencia serializada, carga restaurada y en vivo,
tareas y avisos internos, y prompts humanos. Los mensajes internos nunca generan
una línea `You`.
