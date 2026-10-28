# Vista predeterminada de CodeLoom

## Objetivo

Que una instalación nueva abra directamente la vista de conversación más
coherente con el flujo actual del agente: conversación y composer como foco,
tools en orden y contexto de archivos, jobs y subagentes en un rail adaptable.

## Decisión

- El arranque de la TUI de producción seleccionará `Crush` antes de cargar
  preferencias.
- `App::new()` conservará su preset histórico para consumidores de la librería
  y pruebas.
- Una preferencia de tema ya guardada seguirá teniendo prioridad al cargarla.
- El preset `Batten` conservará la vista clásica y seguirá disponible desde el
  selector actual.
- No se crearán widgets ni estados nuevos: el rail y la vista Crush existentes
  ya cubren el contexto necesario.

## Alcance

Solo cambia el look inicial de la TUI de producción cuando no existe
preferencia guardada. No cambia el valor inicial de la librería, el formato de
preferencias, la paleta de otros presets, los atajos, ni el comportamiento del
composer, las tools o los paneles.

## Verificación

Una prueba de `App` comprobará el look de producto seleccionado. La suite de
`codeloom-tui` comprobará que el cambio no rompe el renderizado de ambos looks.
La inspección con `TestBackend` valida contenido y tamaños; no sustituye la
revisión visual en un terminal interactivo.
