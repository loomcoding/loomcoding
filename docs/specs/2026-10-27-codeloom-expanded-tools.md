# Herramientas internas: expansión inicial

> Estado: **hecho**. Verificado el 2026-10-30 contra el código: los ficheros, comandos y herramientas que describe existen en el repositorio.

## Objetivo

Ampliar las herramientas del agente en cinco áreas observadas en Crush y
OpenCode: listado de directorios, creación de ficheros, edición múltiple,
consulta web y Language Server Protocol (LSP).

## Alcance

- `ls`: listar entradas de un directorio del workspace, respetando ignores y
  con máximo de resultados.
- `write`: crear un fichero nuevo; no sobrescribir un existente. Escritura
  exclusiva y permiso explícito.
- `multiedit`: aplicar varios reemplazos exactos a un fichero en una sola
  operación. Validar todos antes de escribir; sin cambios parciales.
- Web: herramientas de búsqueda y lectura de URL pública, con timeout, límites
  de bytes y protección contra destinos locales/privados y redirecciones hacia
  ellos. No ejecutar ni persistir contenido remoto.
- LSP: consultas de diagnósticos, definición, referencias y símbolos para Rust
  usando `rust-analyzer` disponible en `PATH`; el servidor no se descarga.

Todas las respuestas se limitan con `limit_output`. Ninguna herramienta de
lectura lista archivos ignorados. Las herramientas que escriben usan
`ExecutionMode::Exclusive`. Las entradas de rutas quedan dentro del workspace.

## No objetivos

No se incluye descarga de archivos, reemplazo de símbolos, configuración UI
para servidores, instalación de servidores LSP ni ejecución de código remoto.
La búsqueda web no depende de una API con clave; se documentará el proveedor
público elegido y su limitación.

## Criterios

- Las herramientas aparecen en el registro del agente principal.
- Lectura/listado ignoran `.git`, `target`, `node_modules` y reglas gitignore.
- No se sobreescribe un archivo existente con `write`.
- `multiedit` es atómico respecto a validación: cualquier reemplazo ausente o
  ambiguo cancela toda la escritura.
- Las consultas web validan cada destino y cada redirección, y tienen timeout y
  límite de respuesta.
- LSP falla con un error accionable si el servidor configurado no existe o no
  responde; no bloquea el inicio de CodeLoom.
- Tests locales usan mocks/fixtures; no hacen conexiones de red reales.

## Preguntas abiertas / límites de esta primera versión

- Disponibilidad, calidad y límites de la API pública de búsqueda web pueden
  cambiar; se podrá sustituir por un proveedor configurable más adelante.
- La primera implementación LSP solo soporta Rust y el ejecutable
  `rust-analyzer` del `PATH`; no descubre otros servidores ni lenguajes.
- La política de permisos para LSP y web seguirá el sistema `PermissionService`
  existente; no se introduce una configuración de permisos por patrón.
