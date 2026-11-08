# Usar un servidor de modelos local

**Qué es**

Un proveedor local es un servidor de modelos en tu propia máquina, como `llama-server`. Loomcoding declara cada servidor una sola vez, en tu perfil, y cualquier proyecto puede usarlo.

**Para qué**

- Usar un modelo sin enviar prompts a un servicio alojado.
- Cambiar entre servidores locales con `/provider`.

**Cómo**

1. Arranca tu servidor de modelos en el puerto que espera su entrada.
2. Arranca `loom` en tu proyecto. Sondea cada servidor en segundo plano.
3. Ejecuta `/provider` y elige el servidor. Usa `/model` para elegir otro identificador de modelo.

## Ejemplo

Una entrada `[[provider]]` de `providers.toml`, con valores de ejemplo:

```toml
default = "llama"

[[provider]]
id = "llama"
name = "llama-server"
kind = "openai_compat"
base_url = "http://127.0.0.1:8081/v1"
model = "<model-id>"
context_window = 65536
api_key_env = "<NOMBRE_DE_VARIABLE>"   # opcional: el nombre de una variable, nunca la clave
```

Para declarar niveles de razonamiento de un modelo local, añádelos al `loomcoding.toml` del proyecto:

```toml
[provider]
efforts = ["low", "medium", "high"]
```

## Servidores del registro

El primer arranque siembra estas tres entradas. Sus nombres de modelo son marcadores: pon los tuyos.

| id | Servidor | Puerto | Clave |
|---|---|---|---|
| `llama` | `llama-server` (llama.cpp) | 8081 | Ninguna |
| `strata` | Strata | 8080 | Variable `STRATA_API_KEY` |
| `polystrata` | PolyStrata | 8082 | Ninguna |

La clave nunca se escribe en el fichero. Un servidor en una dirección de bucle puede omitir `api_key_env`, y Loomcoding envía entonces un valor de relleno, porque `llama-server` no pide clave. Una dirección remota siempre necesita la variable.

## Estados de los servidores

| Estado | Significado |
|---|---|
| `up (N)` | El servidor respondió con N modelos. |
| `down` | Conexión rechazada, plazo agotado o respuesta que no es una lista de modelos. |
| `sin acceso (clave)` | El servidor respondió 401 o 403: falta la clave o es incorrecta. |

Un servidor `down` conserva su modelo de reserva, así que puedes seleccionarlo antes de arrancarlo. Los sondeos se repiten al abrir `/model`, `/settings` o `/provider`, con un enfriamiento de 5 segundos.

## Comandos

| Comando | Valor por defecto | Requiere reiniciar | Qué hace |
|---|---|---|---|
| `/provider [id]` | No aplica | No | Cambia de proveedor y de modelo. Sin identificador, abre la lista con el estado de cada servidor. |
| `/model [id]` | No aplica | No | Cambia el modelo del proveedor actual. Acepta cualquier identificador, también uno que no aparezca en la lista. |
| `/effort [nivel o default]` | `default` | No | Cambia el esfuerzo de razonamiento, limitado a los niveles que declara el modelo. |
| `/settings` | No aplica | No | Abre el formulario completo: proveedor, URL, variable de clave, modelo y esfuerzo. |

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `default` en `providers.toml` | `llama` | Sí |
| `[provider] efforts` en `loomcoding.toml` | Sin definir: decide el catálogo | Sí tras editarlo a mano |
| `[provider] context_window` en `loomcoding.toml` | Sin definir: decide el catálogo | Sí tras editarlo a mano |
| `[provider] request_timeout_secs` en `loomcoding.toml` | `120` segundos; el máximo es `3600` | Sí tras editarlo a mano |

## Resultado

| Síntoma | Causa y solución |
|---|---|
| `/provider` dice `Providers are still loading` | El sondeo aún no ha respondido. Espera un momento y vuelve a probar. |
| Un servidor que está en marcha aparece como `down` | El puerto o la ruta `/v1` de `base_url` no coinciden con el servidor. Compáralos. |
| El estado muestra `sin acceso (clave)` | Define la variable indicada en `api_key_env` antes de arrancar `loom`. |
| El proyecto sigue usando el proveedor antiguo | Su `loomcoding.toml` manda sobre el registro. Cámbialo con `/provider`. |

Para los detalles técnicos del registro, consulta [el documento de modelos locales](../../../modelos-locales.md).

## Siguientes pasos

- [Enviar tu primer prompt](first-prompt.md)
- [Conectar un proveedor en la nube](cloud-provider.md)
