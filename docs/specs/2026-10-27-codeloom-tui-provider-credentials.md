# Configuración inicial del proveedor desde la TUI

## Objetivo

Permitir que CodeLoom arranque sin `codeloom.toml` ni API key y que la persona
configure proveedor, modelo y credencial desde `/settings`.

## Alcance

- Si falta `codeloom.toml`, crear una configuración de proyecto mínima para
  OpenRouter con un modelo gratuito, sin incluir secretos.
- Si falta la API key, abrir la TUI con un provider inactivo que explica que hay
  que configurar la credencial en `/settings`; nunca enviar una petición vacía.
- Añadir un campo de API key en `/settings`, ocultar lo escrito y permitir
  aplicar los ajustes al guardarlos.
- En Windows, guardar la API key en Windows Credential Manager, ligada al
  usuario y al nombre `api_key_env`. No escribirla en `codeloom.toml`, `.env`,
  logs, eventos o la interfaz.
- Mantener compatibles las credenciales existentes de variable de entorno y
  `.env` de proyecto. Una clave configurada desde la TUI se pasa al provider
  activo y queda disponible en siguientes arranques.
- En plataformas sin el backend seguro implementado, conservar env/`.env` y
  mostrar un error claro al intentar guardar desde la TUI.

## Aceptación

1. Una carpeta sin `codeloom.toml` abre la TUI y recibe una configuración base.
2. Sin credencial, enviar un prompt devuelve un error local que indica abrir
   `/settings`; no hace una petición HTTP.
3. `/settings` deja introducir una API key sin mostrar su valor y la guarda en
   el almacén de credenciales de Windows.
4. Al guardar, el provider queda activo para el agente actual y la clave vuelve
   a estar disponible al reiniciar.
5. La clave no aparece en el archivo de proyecto ni en mensajes de log/IPC de
   servidor hacia TUI.
6. Pruebas usan almacenamiento temporal/mock y no hacen llamadas de red.

## Fuera de alcance

- Sincronizar claves entre usuarios o máquinas.
- Administrar/revocar credenciales del sistema desde CodeLoom.
- Cambiar el almacenamiento opcional del LLM supervisor de usuario.
