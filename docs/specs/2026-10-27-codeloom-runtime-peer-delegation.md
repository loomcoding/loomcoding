# Delegación A2A entre proyectos del runtime local

> Estado: **hecho**. Verificado el 2026-10-30 contra el código: los ficheros, comandos y herramientas que describe existen en el repositorio.

## Objetivo

Permitir que agentes de proyectos abiertos en el runtime local se descubran y deleguen tareas
entre sí mediante A2A.

## Contrato

- El runtime crea perfiles remotos efímeros en un directorio propio por proyecto y generación del
  servidor. La TUI y el agente del proyecto cargan el mismo directorio.
- Cada perfil apunta a la tarjeta directa `/agents/{tenant}/.well-known/agent-card.json`. El
  cliente conserva `tenant` y la URL JSON-RPC anunciados por esa tarjeta.
- El descubrimiento es interno al runtime. No añade una operación de listado JSON-RPC ni campos
  privados a AgentCard. Las tareas usan exclusivamente operaciones A2A ya soportadas.
- Solo se generan peers si el listener runtime está activo y autenticado. La autenticación usa un
  secreto de Credential Manager (o el entorno configurado), nunca TOML, perfiles ni prompts.
- Los peers remotos requieren aprobación en `/agents`. La aprobación queda vinculada al hash de la
  tarjeta y al proyecto solicitante. Las tools protegidas siguen necesitando consentimiento en la
  TUI destino.
- Los perfiles efímeros no contienen rutas completas, prompts ni secretos. Los datos de una
  generación anterior no se cargan en otra.
- El nombre visible de cada peer parte del nombre de su carpeta de proyecto; solo se añade un
  sufijo corto de identidad si hay nombres de carpeta ambiguos.
- Las aprobaciones son por tarjeta/tenant vivo; al reiniciar el runtime, una identidad nueva vuelve
  a requerir aprobación.

## Aceptación

1. Dos proyectos abiertos aparecen mutuamente en `/agents` cuando el runtime sirve A2A y ambos
   proyectos consumen remotos.
2. Tras aprobar el peer, se puede delegar una tarea; el destino la ejecuta en su sesión, cwd,
   provider, permisos y presupuesto.
3. Sin aprobación, desconexión o rechazo, no se ejecutan tools protegidas.
4. Una tarjeta directa por tenant se descarga y valida sin añadir operaciones al protocolo.
5. Con A2A serving apagado, no se crean perfiles de peers ni se abre un listener.
