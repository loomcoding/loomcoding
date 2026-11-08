# ¿Qué es A2A y cómo lo usa Loomcoding?

**Qué es**

A2A es un protocolo abierto para que los agentes intercambien trabajo en forma de tareas. Loomcoding lo usa como base para hablar con otros agentes, en lugar de un protocolo propio. Cuando Loomcoding añade una regla o un estado propio, esta página lo indica.

**Para qué**

- Ejecutar los subagentes como tareas A2A.
- Usar agentes remotos que otra persona ejecuta.
- Compartir tus agentes locales con otros clientes, si lo activas.

**Cómo**

1. Define el papel de A2A en `loomcoding.toml`. Por defecto es `off`.
2. Aprueba cada agente remoto en `/agents`.
3. Consulta tarjetas, tareas e intercambios con `/a2a`.

## Ejemplo

```toml
[a2a]
role = "consume"   # usa agentes remotos que hayas aprobado
```

## Papeles y estados

| Papel (`[a2a] role`) | Qué hace |
|---|---|
| `off` | Por defecto. Ni cliente A2A ni servidor. |
| `consume` | Delega en agentes remotos que ya has aprobado. |
| `serve` | Expone tus agentes locales a otros clientes. Necesita una activación explícita y autenticación, guardada en `~/.loomcoding/runtime.toml`. |
| `both` | Hace las dos cosas: `consume` y `serve`. |

El protocolo A2A define ocho estados de tarea: `submitted`, `working`, `input-required`, `auth-required`, `completed`, `failed`, `canceled` y `rejected`.

Los subagentes tienen un estado propio más rico, que Loomcoding muestra en la bandada. Cada uno de esos estados se proyecta a un estado de A2A:

| Fase de Loomcoding (propia) | Estado A2A que ven otros agentes |
|---|---|
| Queued, Starting | `submitted` |
| Streaming, Reasoning, ToolRunning, AwaitingPermission, Working, Stalled, Unknown | `working` |
| AwaitingParentInput | `input-required` |
| AwaitingCredentials | `auth-required` |
| SettledOk | `completed` |
| SettledErr, Lost | `failed` |
| Rejected | `rejected` |
| Canceled | `canceled` |

Reglas y fases propias de Loomcoding, que A2A no define:

- Las fases `Stalled` y `Lost`, que vienen de la vigilancia de progreso de los subagentes.
- Los modos de delegación `off`, `on_request` y `proactive`.
- El ajuste de enrutamiento, que elige agente por sí mismo. A2A no dice cómo elegir un agente. El enrutamiento está desactivado por defecto.
- Las fases de ODD de la cabecera; consulta [ODD](flows-odd.md).

## Agentes remotos

Un agente remoto es un fichero de perfil con `transport: a2a-remote` y la dirección del agente. Loomcoding descarga su tarjeta, que describe lo que el agente puede hacer.

1. Abre `/agents` y aprueba el agente remoto. Nada se ejecuta antes de que lo apruebes.
2. Usa `/refresh-agents` para volver a descargar las tarjetas.
3. Un agente remoto puede hacerte una pregunta mientras trabaja. La fase correspondiente es «esperando entrada del padre».

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `[a2a] role` | `off` | Sí |
| `[a2a] request_timeout_secs` | `30` segundos | Sí |
| `[a2a] card_refresh_secs` | `3600` segundos. `0` desactiva el refresco. El mínimo es 60. | Sí |
| `[a2a] max_card_bytes` | `65536` bytes (64 KiB) | Sí |
| `[a2a] max_result_bytes` | `262144` bytes (256 KiB) | Sí |

Loom lee la sección `[a2a]` al crear el agente. Reinicia Loom después de editarla.

Para el estado de cada tarea A2A del proyecto, consulta la [hoja de ruta de A2A](../../../plans/2026-10-22-codeloom-a2a-roadmap.md).

## Siguientes pasos

- [Agentes, subagentes y jobs](agent-subagents-jobs.md)
- [Permisos](permissions.md)
