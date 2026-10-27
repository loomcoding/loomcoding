# Tareas A2A sobre agentes vivos

> Estado: aprobada para implementación como R3.

## Objetivo

Hacer que las operaciones A2A existentes ejecuten y consulten tareas en la sesión aislada del agente seleccionado por tenant.

## Contrato

- Usar exclusivamente `SendMessage`, `GetTask`, `CancelTask`, `SendStreamingMessage` y `SubscribeToTask` cuando estén anunciadas por la tarjeta.
- El tenant se resuelve a un agente registrado; cada tarea se asocia al tenant y a una sesión runtime. Un cliente no puede consultar ni cancelar tareas de otro tenant.
- El mensaje A2A se procesa por la cola del worker del agente destino, preservando su provider, cwd, presupuesto, permisos y almacén.
- Solicitudes que requieran permisos se muestran en la TUI destino. Desconexión, timeout o rechazo deniegan la operación con estado/error A2A correspondiente; nunca autoaprueban.
- Cancelar una tarea solo cancela esa ejecución A2A; cerrar una TUI no cancela el agente ni otras tareas.
- Límites de tamaño, concurrencia, duración y resultados son finitos. El proveedor sigue siendo mock en pruebas.

## Aceptación

1. Dos tenants ejecutan tareas en sesiones/configuración distintas.
2. `GetTask` y `CancelTask` de un tenant no revelan tareas de otro.
3. Rechazo/desconexión de aprobación no ejecuta tools protegidas.
4. Estados terminales y streaming usan los tipos/eventos A2A ya implementados.
