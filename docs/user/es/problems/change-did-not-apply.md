# ¿Por qué no se aplicó mi cambio?

**Qué es**

Algunos ajustes solo se leen al arrancar Loom. Un cambio hecho mientras Loom está en marcha no llega a la sesión en curso hasta que lo reinicias.

**Para qué**

- Saber qué cambios necesitan reinicio.
- Elegir la forma correcta de aplicar cada cambio.

**Cómo**

1. Mira la columna «Requiere reiniciar» de la [referencia de configuración](../reference/configuration.md).
2. Cierra todas las ventanas de `loom` y vuelve a arrancar Loom.

## Ejemplo

Desactivar ODD con un comando guarda el valor. Se aplica al reiniciar Loom:

```text
> /flow odd off
```

El mismo cambio en el fichero del proyecto:

```toml
[flows.odd]
enabled = false
```

## Casos

| Qué cambiaste | Qué pasa hasta reiniciar | Solución |
|---|---|---|
| `loomcoding.toml` a mano | La sesión en curso mantiene el valor anterior. | Reinicia Loom. |
| `[output_filter] enabled` | El filtro sigue como estaba. | Reinicia Loom. |
| `/flow odd on` o `/flow odd off` | El valor guardado se aplica en el siguiente arranque. | Reinicia Loom. |
| `LOOMCODING_NO_FILTER` o `RUST_LOG` | Se leen al arrancar. Una variable definida después de arrancar no hace nada. | Defínela y arranca Loom desde esa terminal. |
| `providers.toml` | Se lee al arrancar. | Reinicia Loom. |
| `/model`, `/provider`, `/effort` | Se aplican ahora. | Nada que hacer. |
| `/verbose` | Cambia solo la sesión, y no escribe el fichero. | Nada que hacer. |
| Formulario de `/settings` | Se aplica al guardar, para las claves que cubre. | Las claves que el formulario no cubre necesitan reinicio. |

## Ajustes y valores por defecto

| Clave | Requiere reiniciar |
|---|---|
| `[provider] request_timeout_secs` | Sí |
| `[flows.odd] enabled` | Sí |
| `[output_filter] enabled` | Sí |
| `[agents] trust_project_agents` | Sí |

## Resultado

- Tras reiniciar, Loom usa el nuevo valor.
- Si Loom no arranca después de editar, el mensaje nombra la clave incorrecta. Por ejemplo, `request_timeout_secs` debe ser un número entero de `1` a `3600`.
- Si el valor sigue sin aplicarse, comprueba que editaste el `loomcoding.toml` de la carpeta desde la que arrancas Loom.

## Siguientes pasos

- [Configuración](../reference/configuration.md)
- [ODD y flujos (`/flow`)](../concepts/flows-odd.md)
- [Carpetas de perfil](../reference/profile-folders.md)
