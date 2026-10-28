# Plan: vista predeterminada de CodeLoom

## Sección

Vista principal: conversación, composer, actividad y rail de contexto.

## Trabajo

1. Seleccionar Crush en el arranque de la TUI antes de cargar preferencias.
2. Mantener intacta la carga de preferencias y el cambio al preset Batten.
3. Añadir una prueba del estado inicial.
4. Ejecutar las pruebas de `codeloom-tui` y el formato Rust.

## Cierre

- La vista nueva conserva el foco visual en conversación y composer.
- A partir de 80 columnas, el rail existente enseña el contexto de actividad;
  por debajo de ese ancho, el chat ocupa la pantalla completa.
- Una preferencia guardada sigue ganando al valor inicial.
- La prueba cubre el preset inicial y las pruebas existentes cubren ambos looks.
- La validación en terminal real queda anotada por separado si no está
  disponible en esta ejecución.
