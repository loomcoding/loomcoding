# Especificación: widgets desplazables de capacidades

Estado: implementada; build, formato y clippy pasan. La suite del workspace
conserva dos fallos fuera de este alcance en el composer y el menú slash.

## Objetivo

Mostrar los nombres y el estado de MCP, LSP y Skills debajo de `Activity` en el
panel derecho del workspace. Las herramientas recientes no deben ocupar todo
el espacio disponible ni ocultar esos estados.

## Comportamiento

- Reducir `Activity` a una lista breve de herramientas recientes.
- Añadir widgets independientes `MCP`, `LSP` y `Skills` debajo de `Activity`.
- MCP lista cada servidor configurado, estado de inicialización y tools
  descubiertas.
- LSP distingue herramienta registrada, `rust-analyzer` detectable y si se
  invocó en esta sesión; no declara una conexión persistente.
- Skills lista capacidades anunciadas por tarjeta y el resultado real de carga
  de skills locales, diferenciando ambas clases; nunca ejecuta `SKILL.md`.
- Cada widget mantiene su propio desplazamiento; rueda sobre el widget y
  `PageUp`/`PageDown` con su foco no cambian el scroll de los demás.
- Sin snapshot, cada widget muestra `pending`/`unknown`, nunca datos inventados.
- En alturas reducidas, cada widget conserva su título y reserva su contenido
  con recorte y desplazamiento local.

## Límites

- No cambia el estado MCP/LSP/Skills ni activa capacidades.
- No inicia consultas LSP ni enumera archivos de skills.
- El desglose informativo de MCP/skills queda dentro de sus widgets y no altera
  el widget de tokens ni el total de uso del provider.
- Conserva el rail actual para el layout Crush; el workspace clásico ofrece
  widgets con scroll independiente.

## Aceptación

1. Los nombres MCP, LSP y skills se ven bajo `Activity` en el panel derecho.
2. Una lista mayor que su widget se desplaza sin mover las otras listas.
3. Clic/foco y rueda actúan sobre el widget correspondiente; `PageUp` y
   `PageDown` desplazan únicamente ese widget.
4. Estados desconocidos, sin configurar, fallidos, declarados y no aplicados
   siguen diferenciándose.
5. El panel de Activity conserva información de herramientas recientes sin
   tapar los widgets de capacidades.
