# Activar la compresión de la salida de los comandos

**Qué es**

La compresión de salida quita ruido de la salida de `bash` y de `job_output` antes de que la lea el modelo, por ejemplo códigos de color, líneas repetidas y barras de progreso. Viene desactivada por defecto.

**Para qué**

- Reducir los tokens que cuestan las salidas largas de `cargo test`, `cargo build` o `npm install`.
- Conservar errores, avisos y resúmenes, que nunca se quitan.

**Cómo**

1. En el `loomcoding.toml` del proyecto, añade `[output_filter]` con `enabled = true`.
2. Reinicia Loomcoding.
3. Ejecuta un comando con salida ruidosa. Cuando el filtro quita algo, la salida empieza con una cabecera `[filtrado: …]`.

## Ejemplo

```toml
[output_filter]
enabled = true
```

La cabecera tiene este aspecto. Su texto está en español, y el marcador es la ruta del original guardado:

```text
[filtrado: 2 líneas omitidas de 5 — regla general; el original sigue en <ruta del volcado>]
```

## Reglas

| Regla | Qué quita | Qué conserva |
|---|---|---|
| General (cualquier texto) | Códigos de color y de control, barras de progreso, líneas repetidas (se muestran como `×N`), líneas en blanco. | Todo lo demás. |
| `cargo test`, `build`, `check`, `clippy` | Líneas de compilación en curso, descargas, líneas `Finished` anteriores, líneas `test … ok`. | Errores, avisos, `FAILED`, `panicked` y el resumen. |
| `git status` (formato largo) | Las pistas `(use "git …")` repetidas. | El resto. |
| `npm install`, `pnpm install` | Barras de progreso de pnpm y avisos de deprecación repetidos. | La última línea de progreso. Los avisos repetidos se cuentan. |

Las reglas por comando solo se aplican a comandos simples, sin tuberías, `&&`, `;`, sustituciones ni redirecciones. `2>&1` sí se admite. `git diff` y `git log` solo reciben las reglas generales.

## El original

- La cabecera indica la ruta de un fichero que guarda la salida original completa. Estos ficheros se conservan 7 días.
- El modelo puede pedir la salida sin filtrar con `raw: true` en `bash` o en `job_output`.
- Si no se puede guardar el original, la salida pasa sin cambios.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `[output_filter] enabled` | `false` | Sí |
| `LOOMCODING_NO_FILTER=1` (variable de entorno) | sin definir | Sí. Desactiva el filtro aunque el fichero lo active. |

## Desactivación automática

Una regla con al menos 10 usos y más de un 20 % de nuevas peticiones se desactiva durante el resto del proceso. Recibes un aviso cuando ocurre. Una regla desactivada sigue desactivada hasta que reinicies Loomcoding.

## Resultado

Cuando funciona, las salidas ruidosas empiezan con la cabecera `[filtrado: …]`. Si no hay cabecera, el comando no coincidió con ninguna regla, o su salida no tenía el formato esperado. En ese caso la salida pasa sin cambios.

## Si no funciona

| Síntoma | Solución |
|---|---|
| No aparece ninguna cabecera. | Comprueba que guardaste el fichero y que reiniciaste Loomcoding. Comprueba que `LOOMCODING_NO_FILTER` no está definida. |
| La salida filtrada oculta algo que necesitas. | Pide la salida con `raw: true`. Es posible que la regla sea demasiado estricta; comunica el caso. |
| Una regla dejó de funcionar a mitad de sesión. | Se desactivó automáticamente. Reinicia Loomcoding para volver a activarla. |

## Siguientes pasos

- [Medir con RUST_LOG](measure-with-rust-log.md)
- [Vigilar un job en segundo plano](watch-a-job.md)
- [Contexto y compactación](../concepts/context-and-compaction.md)
- [Nota técnica: filtro de salida](../../../output-filter.md)
