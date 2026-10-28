# Observatorio A2A en la TUI

## Objetivo

Dar una vista de lectura para inspeccionar las piezas reales de A2A que CodeLoom
conoce: tarjetas de agentes, tareas y actividad entrante y saliente.

## Alcance

- `/a2a` abre un overlay con tres paneles: tarjetas, tareas y actividad reciente.
- Las tarjetas muestran los campos declarados por `AgentCard` y, por separado,
  la aprobación y disponibilidad local del perfil remoto.
- Las tareas muestran identificadores, contexto, estado, referencias y metadatos
  de artefactos disponibles en `SubagentDescriptor`.
- La actividad registra llamadas JSON-RPC entrantes y salientes (`SendMessage`, `GetTask`,
  `CancelTask`, `SubscribeToTask`) y eventos SSE (`TaskStatusUpdateEvent`,
  `TaskArtifactUpdateEvent`) con dirección, agente, IDs, estado, resultado y
  duración cuando se conoce.
- Los cambios observados del host local se etiquetan como tales, no como una
  captura de operaciones JSON-RPC recibidas.
- Las entradas de actividad están limitadas a 128. No se guardan cuerpos JSON,
  texto de mensajes, contenido de artefactos ni credenciales.
- La vista es de solo lectura. Navegación por panel con Tab, selección con ↑/↓ y
  desplazamiento del detalle con PgUp/PgDn; Esc cierra.
- La actividad conserva las últimas 128 observaciones; las listas del detalle
  tienen un tope visual por sección para mantener acotado el render.

## Límites semánticos

- La tarjeta describe lo que declara el agente; no garantiza que cada capacidad
  funcione en ese momento.
- `SubagentDescriptor` expone el identificador de tarea de CodeLoom. La traza
  remota muestra el ID del peer cuando se conoce; no se confunden ambos.
- Los avisos existentes de tareas A2A entrantes siguen siendo eventos de estado
  del host. No se presentan como un registro completo de todas las peticiones
  entrantes ni como captura del tráfico HTTP.
- El esquema de autenticación puede mostrarse por nombre, nunca el valor del
  secreto ni el nombre de la variable de entorno.

## Verificación

Probar la proyección segura de las tarjetas y tareas, la cota de actividad, las
operaciones y eventos de flujo, la navegación y el render en terminal ancha,
estrecha y sin datos. Los tests de buffer no sustituyen una revisión visual en
terminal real.
