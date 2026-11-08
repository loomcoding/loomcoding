# Variables de entorno

**Qué es**

Las variables de entorno que Loomcoding lee, con su valor por defecto y su efecto. También recoge los nombres que eliges tú, como la variable que guarda la clave de un proveedor, y las que pone herdr.

**Para qué**

- Ejecutar Loom con un perfil aparte, para pruebas o para empezar limpio.
- Activar el fichero de registro, o desactivar el filtro de salida.
- Saber de dónde se lee la clave de un proveedor.

**Cómo**

1. Define la variable en la shell antes de arrancar `loom`, o en el fichero `.env` del proyecto para las claves de proveedor.
2. Usa el nombre exactamente como aparece aquí. Los nombres distinguen mayúsculas de minúsculas.
3. Reinicia Loom tras cambiar una variable que se lee al arrancar. La tabla indica cuándo se lee cada una.

## Ejemplo

```powershell
$env:LOOMCODING_HOME = "<carpeta temporal>"
$env:RUST_LOG = "odd=info"
loom
```

Así arranca Loom con un perfil aparte, y escribe los eventos de ODD en el fichero de registro de la carpeta `logs/` de ese perfil.

## Variables de Loom

| Variable | Valor por defecto | Qué hace |
|---|---|---|
| `LOOMCODING_HOME` | la carpeta personal del sistema | Raíz del perfil. Loom guarda sus ficheros en `<valor>/.loomcoding/`. Usa una carpeta temporal para ejecutar con un perfil aparte. |
| `RUST_LOG` | sin valor | Si está definida y no vacía, Loom escribe un fichero de registro en `logs/` del perfil. El valor es un filtro, por ejemplo `odd=info`. El fichero rota al llegar a 5 MiB. |
| `LOOMCODING_NO_FILTER` | sin valor | `1` desactiva el filtro de salida en todo el proceso, aunque `[output_filter] enabled` sea true. |
| `LOOMCODING_MAX_TOOL_CALLS_PER_RESPONSE` | 128 | Número máximo de llamadas a herramientas en una respuesta del modelo. Solo se usa un entero positivo; los demás valores se ignoran. |
| `LOOMCODING_MAX_TOOL_CALLS_PER_TURN` | 256 | Número máximo de llamadas a herramientas en un turno. Misma regla que la anterior. |
| `LOOMCODING_PROMPT_CACHE` | sin valor | `0`, `off` o `false` desactiva la caché de prompts en el adaptador de Anthropic. Cualquier otro valor la activa. Sin valor, decide `[provider] prompt_cache`, y por defecto está activada. |
| `LOOMCODING_PROGRAMMATIC_TOOL_CALLING` | desactivada | `1`, `true` o `yes` activa la vía de llamadas programáticas a herramientas del bucle del agente. Cualquier otro valor la mantiene desactivada. |
| `LOOMCODING_NO_CLIPBOARD` | sin valor | Si está definida, con cualquier valor, Loom no lee imágenes del portapapeles. Pegar texto sigue funcionando. |
| `NO_COLOR` | sin valor | Si está definida, con cualquier valor, se desactivan el muñeco braille y los efectos del logotipo. |
| `USERPROFILE`, `HOME` | los valores del sistema | Solo se usan para localizar la caché del catálogo de modelos, `models-dev.json`. `LOOMCODING_HOME` no mueve ese fichero. |

## Claves de proveedor y credenciales

El nombre de la variable de una clave de proveedor es el valor de `api_key_env` en `loomcoding.toml` o en `providers.toml`. Ese nombre lo eliges tú. Estos nombres aparecen por defecto en los ficheros y en la pantalla de ajustes:

| Nombre | Dónde aparece por defecto |
|---|---|
| `OPENROUTER_API_KEY` | El fichero de proyecto de respaldo, que se usa cuando `providers.toml` no se puede usar. |
| `STRATA_API_KEY` | La entrada `strata` de `providers.toml`. |
| `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY` | Los preajustes de proveedor de la pantalla de ajustes. |

Loom busca una clave en este orden, y usa la primera que no esté vacía:

1. El entorno del proceso, con el nombre de `api_key_env`.
2. El fichero `.env` de la raíz del proyecto, con el mismo nombre.
3. El almacén de credenciales del sistema operativo, con el mismo nombre.

Para una dirección local (localhost o una IP de bucle) sin clave, Loom usa una clave de relleno, de modo que un servidor local que no pide clave funciona sin ella. Loom no escribe claves en ningún fichero.

## Variables que tú nombras

| Ajuste | Lo que guarda la variable |
|---|---|
| `api_key_env` en `[provider]` o en `providers.toml` | La clave del proveedor. Consulta la sección anterior. |
| `env` en `[a2a.serve.auth]` | El secreto que los clientes deben enviar al servidor A2A local. |
| `env` en la autenticación del perfil de un agente remoto | El secreto de un agente A2A remoto. |
| Valores `env` de un servidor MCP en `mcp.toml` | Las variables del anfitrión que se pasan al servidor. El mapa va del nombre que ve el servidor al nombre en el anfitrión. |

## Variables que pone herdr

herdr las define cuando ejecuta Loom como plugin. Tú no las defines. El plugin `loomcoding-herdr` las lee.

| Variable | Lo que contiene |
|---|---|
| `HERDR_ENV` | `1` cuando Loom se ejecuta dentro de herdr. |
| `HERDR_PLUGIN_CONTEXT_JSON` | El contexto del espacio de trabajo, en JSON. |
| `HERDR_PANE_ID` | El identificador del panel. |
| `HERDR_PLUGIN_ID` | El identificador del plugin. |
| `HERDR_BIN_PATH` | La ruta del programa herdr. Por defecto: `herdr`. |

## Variables que ven los comandos que ejecuta Loom

La herramienta `bash` no pasa todo el entorno a los comandos que ejecuta. Les pasa una lista fija de variables del sistema, entre ellas `PATH`, `SYSTEMROOT`, `TEMP`, `HOME`, `USERPROFILE`, `LANG`, `CARGO_HOME` y `RUSTUP_HOME`. También les pasa las variables del fichero `.env` del proyecto. Las demás variables de tu shell no se pasan.

## Siguientes pasos

- [Configuración (`loomcoding.toml`)](configuration.md)
- [Carpetas del perfil](profile-folders.md)
- [Comandos](commands.md)
