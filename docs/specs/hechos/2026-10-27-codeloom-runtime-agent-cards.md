# Tarjetas A2A de los agentes del runtime

> Estado: **implementado** (verificado el 2026-10-30; véase el apartado «Estado» del final y el plan gemelo). Estado original: aprobada como primera tarea de implementación del runtime compartido.
>
> Spec general: `docs/specs/2026-10-26-codeloom-local-server-client-runtime.md`.

## Objetivo

Registrar cada agente vivo del runtime del servidor supervisor junto con su AgentCard A2A y sus metadatos locales de control. La identidad de una ejecución debe distinguirse de otras ejecuciones del mismo proyecto.

## Reglas

- La tarjeta usa exclusivamente campos del modelo A2A existente. La clasificación `internal` vive en el registro local, nunca como campo propio de AgentCard.
- La tarjeta identifica el agente y el proyecto de trabajo. Las credenciales, el transcript, el prompt y las rutas completas no se publican en la tarjeta.
- Un agente sin una interfaz A2A activa se registra internamente, pero no se declara enrutable ni se expone como si pudiera aceptar tareas.
- La interfaz A2A anuncia `tenant` solo si el servidor la soporta; las solicitudes deben seguir el tenant anunciado por esa interfaz.
- El registro y la actualización de tarjetas son deterministas y no dependen de un LLM.

## Aceptación

1. Cada agente del runtime tiene un identificador único durante su vida y una AgentCard que lo distingue de otras ejecuciones del mismo proyecto.
2. El registro conserva la clasificación interna aparte de la tarjeta A2A.
3. Las tarjetas no contienen secretos, conversaciones ni rutas absolutas.
4. Una tarjeta solo anuncia interfaces activas y realmente enrutables.
5. Un fallo de provider/LLM no cambia el registro ni impide abrir agentes.
