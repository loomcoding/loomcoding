# ¿Por qué la salida de la herramienta parece filtrada?

**Qué es**

El filtro de salida acorta la salida de `bash` y de `job_output` antes de que el modelo la lea. Viene desactivado por defecto. Cuando actúa, la salida empieza con una cabecera que indica qué se ha quitado.

**Para qué**

- Reconocer la cabecera, y saber dónde está la salida completa.
- Recuperar la salida completa, o desactivar el filtro.

**Cómo**

1. Lee la cabecera. Indica el número de líneas quitadas y el fichero con la salida completa.
2. Pide de nuevo la salida completa con `raw: true`, o desactiva el filtro.

## Ejemplo

La cabecera se imprime en español, tal como la escribe la aplicación:

```text
[filtrado: 2 líneas omitidas de 5 — regla general; el original sigue en <ruta del volcado>]
inicio

fin
```

## Mensajes y soluciones

| Síntoma | Causa | Solución |
|---|---|---|
| Aparece la cabecera, y falta una línea que necesitas | El filtro quitó líneas que su regla considera ruido. | Pide al agente que ejecute el comando de nuevo con `raw: true`, o abre el fichero que indica la cabecera. |
| El modelo sigue pidiendo `raw: true` | La regla quita demasiado para ese comando. Una regla con 10 o más usos y más de un 20 % de peticiones repetidas se desactiva sola para el resto del proceso, y Loom muestra un aviso. | Desactiva el filtro, como se explica abajo, y anota el comando. |
| Quieres que no se filtre nada | El filtro está activado en el proyecto. | Define `enabled = false` y reinicia, o define `LOOMCODING_NO_FILTER` antes de arrancar Loom. |

## Desactiva el filtro

En el `loomcoding.toml` del proyecto, y luego reinicia Loom:

```toml
[output_filter]
enabled = false
```

Para un único arranque, desde PowerShell:

```powershell
$env:LOOMCODING_NO_FILTER = "1"
loom
```

`LOOMCODING_NO_FILTER=1` desactiva el filtro aunque el proyecto lo active.

## Dónde está la salida completa

- La salida completa se guarda en `<perfil>/.loomcoding/tool-output/`, y se conserva 7 días.
- El filtro solo se aplica a `bash` y a `job_output`. Nunca cambia la salida de `read`, `grep` ni `glob`.
- Si Loom no puede guardar la salida completa, la salida pasa sin cambios.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `[output_filter] enabled` en `loomcoding.toml` | `false` | Sí |
| `LOOMCODING_NO_FILTER` (variable de entorno) | Sin definir | Sí. Defínela antes de arrancar Loom. |

## Resultado

- Con el filtro desactivado, la salida no tiene cabecera y no se acorta.
- Con el filtro activado, la cabecera indica qué se ha quitado, y `raw: true` devuelve el texto completo.

## Siguientes pasos

- [Activar la compresión de salida](../guides/turn-on-output-compression.md)
- [Configuración](../reference/configuration.md)
- [Filtro de salida (documento técnico)](../../../output-filter.md)
