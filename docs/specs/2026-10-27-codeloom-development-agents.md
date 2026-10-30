# Agentes locales para desarrollo

> Estado: **hecho**. Verificado el 2026-10-30 contra el código: los ficheros, comandos y herramientas que describe existen en el repositorio.

## Objetivo

Añadir perfiles de proyecto para repartir trabajo de UI, backend y revisión
independiente entre los subagentes de CodeLoom.

## Alcance

- Crear perfiles locales `frontend`, `backend` y `qa-reviewer` en
  `.agents/agents/`.
- Cada perfil define su especialidad, `model` y un proveedor local opcional.
- Los especialistas usan OpenRouter y el agente principal conserva el proveedor
  local de `codeloom.toml`. La clave se resuelve desde el entorno del proyecto y
  su valor nunca se guarda en el perfil.
- Se ejecutan como subagentes locales; sus tareas usan el modelo de estados A2A
  de CodeLoom, no son endpoints HTTP remotos.
- Los perfiles de proyecto requieren aprobación explícita en `/agents` antes
  de poder delegarles trabajo.
- El supervisor del runtime queda separado: el modelo OpenRouter configurado
  responde consultas de estado y no implementa cambios.

## Fuera de alcance

- Crear servicios HTTP A2A o inventar endpoints remotos para estos perfiles.
- Cambiar el proveedor/modelo principal de `codeloom.toml`.
- Almacenar claves API.
- Añadir campos de perfil que el parser actual no aplique.

## Aceptación

1. El registro carga los tres perfiles desde `.agents/agents/` sin avisos de
   campos ignorados.
2. La TUI los muestra en `/agents` como perfiles de proyecto pendientes de
   aprobación.
3. Sus prompts asignan responsabilidades distintas y coherentes con el
   desarrollo del repositorio.
4. El proveedor del perfil solo se crea después de la aprobación del perfil.
5. Cada especialista usa un ID OpenRouter gratuito con soporte de herramientas.
