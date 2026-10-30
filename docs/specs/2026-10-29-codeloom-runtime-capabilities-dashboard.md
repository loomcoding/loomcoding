# Especificación: dashboard de capacidades runtime

Estado: implementación solicitada; el dashboard debe reflejar el runtime real.

## Objetivo

Coordinar en la TUI los datos reales de uso, agentes/workflows y capacidades de
herramientas. La interfaz no debe presentar como cargado, conectado o disponible
algo que el runtime no haya confirmado.

## Contenido

- **Uso**: reutilizar los contadores ya separados por petición/turno/sesión/familia
  y la fuente `provider`/`estimate`/sin dato de la especificación de precisión.
- **MCP**: por servidor configurado, presentar conectado/fallido, cantidad real
  de tools descubiertas y un motivo de error saneado. No exponer comandos,
  argumentos, variables de entorno ni valores secretos.
- **LSP**: distinguir `lsp` registrado en el agente, presencia detectable de
  `rust-analyzer` y ejecución de una consulta. El servidor LSP es efímero por
  llamada; nunca se mostrará como conexión persistente.
- **Skills**: mostrar las skills declaradas en tarjetas de agentes como
  capacidades anunciadas y las listas `scope.skills` como declaradas/no aplicadas.
  La versión actual no carga skills locales `SKILL.md`; la UI debe indicar que
  ese runtime no existe, sin presentar esas skills como ejecutables.
- **Coordinación**: estados de uso, herramientas y agentes llegan del mismo
  runtime local que construye el agente. Los cambios de registro actualizan la
  sección correspondiente sin inventar valores cuando aún no hay snapshot.

## Presentación

Añadir un resumen corto y siempre visible de `MCP`, `LSP` y `skills` a la
cabecera del workspace. El rail/context dashboard conserva los detalles por
servidor y skill cuando está visible. Mantener los contadores de uso actuales y
no sustituirlos por cifras agregadas ambiguas. Los recuentos se derivan de
snapshots recibidos; ausencia de configuración, fallo y estado no comprobado son
estados distintos.

## Límites

- Solo observar estado; no añadir/quitar servidores, iniciar consultas LSP ni
  aplicar scopes desde el dashboard.
- No implementar el motor de skills, MCP hot-reload ni conexión LSP persistente.
- No transmitir secretos, comandos completos de MCP ni datos de entorno a la TUI.
- Mantener el core independiente de Ratatui/Crossterm y conservar los límites
  del protocolo IPC.

## Criterios de aceptación

1. El rail muestra valores reales de MCP configurado/conectado/fallido y tools
   descubiertas; configuración ausente se muestra como no configurada.
2. LSP diferencia herramienta registrada, `rust-analyzer` detectable y
   herramienta aún no invocada; nunca afirma conexión persistente.
3. Skills distinguen capacidades de tarjetas cargadas, scopes declarados y
   ausencia del runtime local `SKILL.md`.
4. Recargar el registro actualiza las skills mostradas; no se mezclan estados
   de otro proyecto/agente.
5. Sin snapshot no se pintan ceros ni datos de ejemplo. Los contadores de tokens
   conservan su fuente y ámbito.
6. La vista acotada recorta filas sin bloquear el chat, alterar los contadores o
   exponer secretos.
