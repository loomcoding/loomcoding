# Usar un agente A2A remoto

**Qué es**

Un agente remoto se ejecuta en otro servidor y habla el protocolo A2A. Loomcoding descarga su tarjeta, que describe lo que hace el agente, y le delega trabajo solo después de que lo apruebes.

**Para qué**

- Delegar una parte de una tarea en un agente que ejecuta otra persona.
- Revisar lo que sabe hacer un agente remoto antes de confiar en él.

**Cómo**

1. Pon `role = "consume"` en la sección `[a2a]` del `loomcoding.toml` del proyecto, y después reinicia Loomcoding.
2. Crea un perfil `.md` con `transport: a2a-remote` y `a2a.endpoint`.
3. Abre `/agents`, revisa la tarjeta y pulsa `a` para aprobarlo.

## Ejemplo

```toml
# loomcoding.toml del proyecto
[a2a]
role = "consume"
```

```md
---
name: devops
description: Responde preguntas sobre el pipeline de despliegue.
transport: a2a-remote
a2a:
  endpoint: https://agents.example.invalid/.well-known/agent-card.json
---
```

Guarda el perfil en `~/.agents/agents/` para todos los proyectos, o en `.agents/agents/` para un solo proyecto.

## Credenciales

Si el agente remoto necesita una clave, asocia su esquema de seguridad con el **nombre** de una variable de entorno. Nunca escribas el secreto en el fichero.

```md
a2a:
  endpoint: https://agents.example.invalid/.well-known/agent-card.json
  credentials:
    apiKey: DEVOPS_API_KEY
```

El perfil no se carga si el valor parece un secreto en lugar del nombre de una variable.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `[a2a] role` | `off` | Sí |
| `[a2a] request_timeout_secs` | `30` segundos | Sí |
| `[a2a] card_refresh_secs` | `3600` segundos. `0` desactiva la actualización. El mínimo es 60. | Sí |
| `[a2a] max_card_bytes` | `65536` (64 KiB) | Sí |
| `[a2a] max_result_bytes` | `262144` (256 KiB) | Sí |
| `[a2a] ca_file` | sin definir. Fichero PEM con autoridades de certificación adicionales. | Sí |
| `[a2a] trusted_jwks` | sin definir. Fichero JWKS para las firmas de las tarjetas. | Sí |

| Comando | Qué hace | Requiere reiniciar |
|---|---|---|
| `/agents` | Lista los agentes. Pulsa `a` para aprobar y `r` para revocar. | No |
| `/refresh-agents` | Vuelve a descargar las tarjetas de los agentes remotos. | No |
| `/a2a` | Muestra tarjetas, tareas e intercambios. | No |

## Resultado

Tras aprobarlo, `/agents` muestra el agente como aprobado, y el agente puede recibir trabajo. Un agente remoto puede hacerte una pregunta mientras trabaja. La fase correspondiente es `awaiting parent input`.

La aprobación está ligada al punto de entrada y a la tarjeta. Si el agente remoto cambia su tarjeta, vuelve a estar pendiente hasta que lo apruebes otra vez.

## Si no funciona

| Síntoma | Solución |
|---|---|
| Aparece el mensaje «Remote agents are not enabled». | Pon `role = "consume"` y reinicia Loomcoding. |
| El agente está pendiente y dice que su tarjeta no está descargada. | Ejecuta `/refresh-agents`. Un agente sin tarjeta no se puede aprobar. |
| El perfil no se carga por el endpoint. | El endpoint debe usar HTTPS. El HTTP simple solo se admite en direcciones de bucle local. |
| Un agente aprobado volvió a estar pendiente. | Cambió su tarjeta. Revísala y apruébalo otra vez. |

## Siguientes pasos

- [Qué es A2A](../concepts/a2a.md)
- [Agentes, subagentes y jobs](../concepts/agent-subagents-jobs.md)
- [Crear un agente de perfil](create-a-profile-agent.md)
- [Permisos](../concepts/permissions.md)
