# ¿Por qué no puedo abrir la sesión?

**Qué es**

Loomcoding guarda las sesiones en un servidor local, y el cliente de Loom se conecta a él. Cuando la conexión falla, Loom muestra uno de los mensajes de la tabla de abajo.

**Para qué**

- Relacionar un mensaje con su causa.
- Arrancar limpio después de una actualización, o con un perfil de prueba aparte.

**Cómo**

1. Anota el mensaje que aparece en pantalla.
2. Búscalo en la tabla y aplica la solución.

## Ejemplo

Lista los procesos de Loom que están en marcha. Revísalos antes de parar nada:

```powershell
Get-Process loom, loomcoding -ErrorAction SilentlyContinue | Select-Object Id, ProcessName, StartTime
```

Páralos solo si estás seguro de que pertenecen al perfil que quieres usar:

```powershell
Stop-Process -Name loom, loomcoding -Force
```

Parar un proceso termina el turno que esté ejecutando. Los mensajes ya guardados se conservan en `sessions.db`.

## Mensajes y soluciones

| Mensaje o síntoma | Causa | Solución |
|---|---|---|
| `autenticación local rechazada` | El servidor local en marcha tiene una clave distinta a la de este cliente. Suele ser un servidor de otro perfil que sigue activo. | Cierra todos los procesos de Loom. Arranca Loom de nuevo, siempre con el mismo `LOOMCODING_HOME`. |
| `protocolo local incompatible` | El servidor en marcha es de otra versión, normalmente anterior. | Cierra todos los procesos de Loom antes de arrancar la versión nueva. |
| `servidor Loomcoding incompatible (protocolo N)` | El servidor respondió con otra versión del protocolo. | Cierra todos los procesos de Loom, y arranca de nuevo. |
| `el servidor local no está disponible` | Ningún servidor respondió durante unos 10 segundos después de que Loom intentara arrancar uno. | Cierra cualquier proceso `loom` que siga en marcha, y arranca de nuevo. Para la causa, consulta [Medir con RUST_LOG](../guides/measure-with-rust-log.md). |
| `sessions.db` no se puede abrir, o queda bloqueado | Un servidor de una ejecución `--print`, o de un perfil de prueba, sigue usando la base de datos. | Cierra ese proceso de servidor y arranca de nuevo. |

## Usa un perfil aparte

Loomcoding usa `LOOMCODING_HOME` como carpeta de perfil cuando está definida. Usa el mismo valor en cada terminal que arranque Loom:

```powershell
$env:LOOMCODING_HOME = "<carpeta-vacía>"
loom
```

## Antes de actualizar

Cierra todos los procesos `loom` y `loomcoding` antes de instalar una versión nueva. El servidor antiguo no encuentra la carpeta de perfil nueva.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `LOOMCODING_HOME` (variable de entorno) | Sin definir: tu carpeta personal | Sí. Defínela antes de arrancar Loom. |

## Resultado

- Cuando la sesión se abre con tus datos, la conexión funciona.
- Para quien desarrolla: un servidor en marcha bloquea `loomcoding.exe`, así que `cargo test` puede fallar con `Acceso denegado`. Para antes los servidores.

## Siguientes pasos

- [Sesiones](../concepts/sessions.md)
- [Carpetas de perfil](../reference/profile-folders.md)
- [Variables de entorno](../reference/environment.md)
- [Medir con RUST_LOG](../guides/measure-with-rust-log.md)
