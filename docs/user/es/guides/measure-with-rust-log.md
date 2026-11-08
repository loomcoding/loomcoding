# Medir con RUST_LOG y leer el fichero de registro

**Qué es**

`RUST_LOG` hace que Loomcoding escriba sus eventos de traza en un fichero de registro dentro de tu perfil. Si la variable no está definida, no se escribe nada y no se crea ningún fichero.

**Para qué**

- Medir cuánto ahorra la compresión de la salida.
- Leer los contadores del flujo ODD.
- Revisar lo que pasó en una sesión sin leer la terminal.

**Cómo**

1. Define `RUST_LOG` antes de arrancar Loomcoding.
2. Usa Loomcoding como de costumbre.
3. Lee `<perfil>/.loomcoding/logs/loomcoding-daemon.log`.

## Ejemplo

En una shell Unix, define la variable para un solo arranque:

```text
RUST_LOG=output_filter=info loom
```

En PowerShell, defínela para la ventana actual:

```powershell
$env:RUST_LOG = 'output_filter=info'
loom
```

Una línea real del registro tiene este aspecto:

```text
2026-11-07T23:47:59.628314Z  INFO output_filter: salida filtrada regla="cargo" original_bytes=2258 sent_bytes=653
```

## Qué registrar

| Valor de `RUST_LOG` | Qué registra |
|---|---|
| `output_filter=info` | Compresión de la salida: una línea por salida filtrada. |
| `odd=info` | Contadores del flujo ODD. |
| `output_filter=info,odd=info` | Ambos. |
| `info` | Todo lo que tenga nivel `info` o superior. |

## Ficheros de registro

| Proceso | Fichero |
|---|---|
| Cliente (la interfaz de terminal o `--print`) | `<perfil>/.loomcoding/logs/loomcoding-cliente.log` |
| Daemon (ejecuta el agente) | `<perfil>/.loomcoding/logs/loomcoding-daemon.log` |

`<perfil>` es tu carpeta personal, o la carpeta de `LOOMCODING_HOME` cuando esa variable está definida. Los eventos del agente van al registro del daemon, porque el daemon ejecuta el bucle del agente.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `RUST_LOG` | sin definir. No se escribe nada. | Sí |
| Tamaño máximo de cada registro | 5 MiB. Después el fichero rota a `<nombre>.log.1`, que conserva una copia anterior. | No configurable |

El daemon es un proceso aparte que puede seguir vivo entre ejecuciones y conserva el valor con el que arrancó. Ciérralo, o cierra todos los procesos `loom`, para que lea el nuevo valor.

## Privacidad

Los eventos registran nombres de regla, contadores y cifras de bytes. Nunca registran el texto de los comandos, las rutas ni la salida de las herramientas. Un ajuste amplio, como `debug`, puede incluir mensajes de librerías de terceros. Revisa un registro antes de compartirlo.

## Resultado

Cada línea es un evento. Para obtener el ahorro de una regla, suma `original_bytes` y resta `sent_bytes` para el mismo `regla`.

## Si no funciona

| Síntoma | Solución |
|---|---|
| El fichero de registro no existe. | `RUST_LOG` no estaba definida al arrancar Loomcoding, o sigue activo un daemon anterior. Cierra todos los procesos `loom` y arranca de nuevo. |
| El registro del cliente está vacío. | Es normal. Los eventos del agente están en el registro del daemon. |
| El fichero muestra tamaño 0 en Windows. | El tamaño puede mostrar 0 mientras el fichero está abierto. Lee su contenido. |
| No se escribe ningún registro. | Loomcoding ignora un fichero que no puede abrir y sigue funcionando. Revisa los permisos de la carpeta. |

## Siguientes pasos

- [Activar la compresión de la salida](turn-on-output-compression.md)
- [Vigilar un job en segundo plano](watch-a-job.md)
- [Nota técnica: registro](../../../logging.md)
