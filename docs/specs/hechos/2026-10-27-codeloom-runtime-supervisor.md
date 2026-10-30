# Agente supervisor del runtime

> Estado: **implementado** (verificado el 2026-10-30; véase el apartado «Estado» del final y el plan gemelo). Estado original: aprobada para implementación como R4.

## Objetivo

Dar al servidor un supervisor que mantenga conocimiento determinista del registro y estado del runtime y pueda usar un LLM opcional para razonamiento y coordinación.

## Contrato

- Configuración opcional en `~/.codeloom/runtime.toml`, sección `[supervisor]`; provider/modelo y la variable de credencial son propios del perfil. La variable se lee del entorno del proceso, nunca del `.env` de un proyecto.
- La primera versión recibe solicitudes explícitas por la AgentCard A2A del supervisor, recibe una instantánea acotada del registro y no dispone de herramientas para alterar agentes ni decisiones de seguridad.
- Registro, altas/bajas, actividad, límites y ciclo de vida funcionan sin LLM.
- Provider/modelo y credencial son configuración propia del perfil de usuario; no se heredan del primer proyecto, `.env` ni los agentes.
- Sin configuración o ante error/timeout del LLM, se omite la decisión asistida, se registra el problema y continúa el flujo clásico. No hay reintentos infinitos.
- La indisponibilidad del LLM nunca bloquea startup, workers, permisos, cancelación o apagado. Nunca convierte una decisión de seguridad en aprobación.
- Las llamadas del supervisor son explícitas, acotadas por tiempo/tokens y se guardan en una sesión/coste propios. No hay llamadas periódicas sin necesidad concreta.

## Aceptación

1. Runtime operativo sin config/credencial del supervisor.
2. Error y timeout del provider conservan las operaciones deterministas y el estado de agentes.
3. `.env` de proyecto no se consulta para credenciales del supervisor.
4. La aprobación humana sigue siendo obligatoria con o sin LLM.

Ejemplo de configuración opcional:

```toml
[supervisor]
model = "modelo-elegido"

[supervisor.provider]
kind = "openai_compat"
base_url = "https://api.example/v1"
api_key_env = "CODELOOM_SUPERVISOR_KEY"
```
