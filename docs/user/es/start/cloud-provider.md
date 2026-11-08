# Conectar un proveedor en la nube

**Qué es**

Un proveedor en la nube es un modelo alojado al que se llega por HTTPS con una clave de API. Esta página lo configura desde `/settings`, usando OpenRouter como ejemplo.

**Para qué**

- Usar un modelo alojado cuando no hay ningún servidor local en marcha.
- Guardar el proveedor en el proyecto, para que la siguiente sesión arranque con él.

**Cómo**

1. Arranca `loom` en tu proyecto y escribe `/settings`.
2. Indica el tipo de proveedor, la URL base, el nombre de la variable de la clave y el modelo.
3. Escribe la clave en el campo de clave y guarda. O define la variable fuera de Loomcoding.

## Ejemplo

La forma del `loomcoding.toml` del proyecto tras guardar, con marcadores:

```toml
model = "<model-id>"
max_steps = 8

[provider]
kind = "openai_compat"
base_url = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
```

Si quieres guardar la clave en un fichero, copia el [`.env.example`](../../../../.env.example) del repositorio a `.env` en la carpeta de tu proyecto, y escribe la clave después de `OPENROUTER_API_KEY=`.

## De dónde sale la clave

Loomcoding busca la clave en este orden y usa la primera que encuentra:

1. La clave introducida en `/settings`, para ese guardado.
2. La variable indicada en `api_key_env`, en el entorno del proceso `loom`.
3. La misma variable, en el fichero `.env` de la carpeta del proyecto.
4. La entrada del Administrador de credenciales de Windows que guarda `/settings`. Solo en Windows.

La clave nunca va a `loomcoding.toml`. El fichero guarda solo el nombre de la variable. Una dirección local (`localhost`, `127.0.0.1` o `::1`) no necesita clave.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `[provider] kind` | Obligatorio, sin valor por defecto. Uno de `openai_compat`, `openai_responses`, `anthropic`, `gemini` | No, tras `/settings` |
| `[provider] base_url` | Obligatorio, sin valor por defecto. HTTPS, o HTTP para una dirección local | No, tras `/settings` |
| `[provider] api_key_env` | Vacío: ninguna variable | No, tras `/settings` |
| `model` | Obligatorio, sin valor por defecto | No, tras `/model` o `/settings` |
| `effort` | Sin definir: el nivel por defecto | No, tras `/effort` |

## Resultado

Cuando el guardado funciona, el siguiente prompt va al proveedor. Si falla, el mensaje indica qué campo corregir:

| Mensaje | Qué hacer |
|---|---|
| `Model cannot be empty` | Escribe un identificador de modelo. |
| `Base URL is invalid` | Comprueba que la URL esté completa. |
| `Use HTTPS, or HTTP for a localhost endpoint` | Usa `https://`. El HTTP plano solo vale para una dirección local. |
| `Keep credentials out of the Base URL; use the API key environment variable` | Quita el usuario, la contraseña y cualquier cadena de consulta de la URL. |
| `API key variable must be a valid environment variable name` | Usa letras, cifras y guiones bajos, y no empieces por una cifra. |
| `secure API key saving is only available on Windows; use the configured environment variable` | En otros sistemas, define la variable en su lugar. |
| `No API key configured. Open /settings and save your provider key.` | No se encontró clave en ninguno de los cuatro lugares anteriores. |

## Siguientes pasos

- [Enviar tu primer prompt](first-prompt.md)
- [Usar un servidor de modelos local](local-provider.md)
