# ¿Por qué un servidor local aparece como down o sin acceso?

**Qué es**

La lista de `/provider` muestra un estado junto a cada servidor local. Esta página explica los dos estados que indican que el servidor no puede responder: `down` y `sin acceso (clave)`.

**Para qué**

- Averiguar por qué un servidor que está en marcha aparece como `down`.
- Arreglar una clave que necesita un servidor.

**Cómo**

1. Ejecuta `/provider` y lee el estado del servidor.
2. Aplica la solución de ese estado en la tabla de abajo.

## Ejemplo

Pregunta al servidor lo mismo que Loomcoding pregunta al sondearlo, `GET <base_url>/models`. Usa el puerto de tu `base_url`:

```powershell
curl.exe -i http://127.0.0.1:8081/v1/models
```

Una respuesta `200` con una lista de modelos significa que Loomcoding puede leer el servidor.

## Estados y soluciones

| Estado | Causa | Solución |
|---|---|---|
| `down` | La conexión fue rechazada, el sondeo agotó 1,5 segundos, o la respuesta no es una lista de modelos. | Arranca el servidor en el puerto de `base_url`, y comprueba que `base_url` termina en `/v1`. Ejecuta el ejemplo de arriba. |
| `sin acceso (clave)` | El servidor respondió `401` o `403`: falta la clave o es incorrecta. | Define la variable indicada en `api_key_env` antes de arrancar Loom, y reinicia Loom. |
| `/provider` dice `Providers are still loading` | Los sondeos aún no han respondido. | Espera un momento y abre `/provider` de nuevo. |
| El proyecto sigue usando el proveedor antiguo | El `loomcoding.toml` del proyecto tiene prioridad sobre el registro. | Ejecuta `/provider` y elige el servidor. |

## Servidores por defecto

| id | Servidor | Puerto | Clave |
|---|---|---|---|
| `llama` | `llama-server` (llama.cpp) | 8081 | Ninguna |
| `strata` | Strata | 8080 | `STRATA_API_KEY` |
| `polystrata` | PolyStrata | 8082 | Ninguna |

Un servidor en una dirección de bucle (`127.0.0.1` o `localhost`) puede no tener `api_key_env`. Una dirección remota siempre necesita la variable.

## Define la clave para una terminal

Define la variable en la misma terminal en la que arrancas Loom. Sustituye el marcador por tu clave:

```powershell
$env:STRATA_API_KEY = "<tu-clave>"
loom
```

También puedes escribir la variable en el fichero `.env` de la carpeta del proyecto. La clave nunca va a `providers.toml` ni a `loomcoding.toml`.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `default` en `providers.toml` | `llama` | Sí |

## Resultado

- Cuando el estado muestra `up (N)`, el servidor está listo y puedes elegir sus modelos.
- Un servidor `down` conserva su modelo de reserva, así que puedes elegirlo antes de arrancarlo.

## Siguientes pasos

- [Ejecutar un servidor de modelos local](../start/local-provider.md)
- [Usar modelos locales](../guides/use-local-models.md)
- [Variables de entorno](../reference/environment.md)
