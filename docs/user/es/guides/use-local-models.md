# Declarar y cambiar servidores de modelos locales

**Qué es**

Un servidor de modelos local se ejecuta en tu propia máquina, por ejemplo `llama-server`, Strata o PolyStrata. Loomcoding lo declara una sola vez, en un registro global, y cualquier proyecto puede usarlo.

**Para qué**

- Usar un modelo que se ejecuta en tu máquina, sin clave de la nube.
- Cambiar entre un proveedor en la nube y un servidor local durante una sesión.
- Añadir otro servidor que hable la API compatible con OpenAI.

**Cómo**

1. Arranca el servidor en el puerto que declara su entrada.
2. Arranca Loomcoding. Comprueba cada servidor una vez, en segundo plano.
3. Escribe `/provider` para elegir un servidor, y después `/model <id>` para elegir su modelo.

## Ejemplo

```toml
# <perfil>/.loomcoding/providers.toml
default = "llama"

[[provider]]
id = "llama"
name = "llama-server"
kind = "openai_compat"
base_url = "http://127.0.0.1:8081/v1"
model = "nombre-de-tu-modelo"
```

Después, en Loomcoding:

```text
> /provider llama
> /model nombre-de-tu-modelo
```

## Dónde vive el registro

`<perfil>` es tu carpeta personal, o la carpeta que indica `LOOMCODING_HOME` cuando esa variable está definida. Loomcoding crea `providers.toml` en el primer arranque, con los tres servidores de abajo. Nunca sobrescribe un fichero que ya existe.

| id | Servidor | Puerto por defecto | Clave |
|---|---|---|---|
| `llama` | llama-server (llama.cpp) | 8081 | ninguna |
| `strata` | Strata | 8080 | variable `STRATA_API_KEY` |
| `polystrata` | PolyStrata | 8082 | ninguna |

La clave nunca se escribe en el fichero. `api_key_env` guarda el nombre de la variable que la contiene. Un servidor en la dirección de bucle (`localhost` o `127.0.0.1`) puede omitir `api_key_env`.

## Ajustes y valores por defecto

| Nombre | Valor por defecto | Requiere reiniciar |
|---|---|---|
| `default` en `providers.toml` | `"llama"` | Sí |
| `[[provider]] base_url` | `http://127.0.0.1:8081/v1` para `llama` | Sí |
| `[[provider]] api_key_env` | sin definir (solo se admite en bucle local) | Sí |
| `[[provider]] context_window` | sin definir | Sí |

| Comando | Qué hace | Requiere reiniciar |
|---|---|---|
| `/provider [id]` | Cambia de proveedor y de modelo. Sin id, abre el selector con el estado de cada servidor. | No |
| `/model [id]` | Cambia el modelo del proveedor actual. Acepta cualquier id, aunque no aparezca en la lista. | No |
| `/settings` | Abre el formulario completo: proveedor, URL, variable de la clave, modelo y esfuerzo. | No |

`/provider` y `/model` guardan la elección en el `loomcoding.toml` del proyecto.

## Estado de los servidores

| Estado | Significado |
|---|---|
| `up (N)` | El servidor respondió con N modelos. |
| `down` | La conexión fue rechazada, se agotó el plazo o la respuesta no es una lista de modelos. |
| `sin acceso (clave)` | El servidor respondió 401 o 403: falta la clave o es incorrecta. |

Un servidor caído conserva su modelo de reserva, así que puedes seleccionarlo antes de arrancarlo.

## Resultado

Cuando funciona, `/provider` muestra cada servidor con su estado. El siguiente prompt va al servidor y al modelo que elegiste.

## Si no funciona

| Síntoma | Solución |
|---|---|
| `/provider` dice «Providers are still loading». | La primera comprobación no ha terminado. Espera un momento. |
| Un servidor encendido aparece como `down`. | Comprueba que el puerto y la ruta `/v1` de `base_url` coinciden con los del servidor. |
| Un servidor aparece como `sin acceso (clave)`. | Define la variable indicada en `api_key_env` antes de arrancar Loomcoding. |
| Un proyecto sigue usando el proveedor antiguo. | Manda su `loomcoding.toml`. Cámbialo con `/provider`. |

Para declarar niveles de esfuerzo de razonamiento en un modelo local, consulta la nota técnica.

## Siguientes pasos

- [Arrancar un servidor de modelos local](../start/local-provider.md)
- [Conectar un proveedor en la nube](../start/cloud-provider.md)
- [Personalizar la interfaz](customize-the-interface.md)
- [Nota técnica: modelos locales](../../../modelos-locales.md)
