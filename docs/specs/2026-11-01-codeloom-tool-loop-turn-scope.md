# Especificación: alcance de la detección de bucles de herramientas

## Objetivo

Evitar que tool calls repetidas de una interacción anterior impidan iniciar una
nueva petición del usuario.

## Comportamiento

- El límite `max_steps` sigue aplicándose al turno que se está ejecutando.
- La detección de tool calls idénticas solo compara llamadas posteriores al
  último mensaje de usuario del historial.
- Un nuevo mensaje del usuario rearma la detección de bucle; una repetición
  dentro de ese nuevo turno todavía puede detenerse.

## Aceptación

Tras un turno detenido por dos tool calls iguales, un nuevo mensaje de usuario
puede iniciar una petición al proveedor. La detección aún detiene repeticiones
dentro del turno actual.
