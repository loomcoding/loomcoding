# Modelos locales en Loomcoding

Cabe señalar que Loomcoding trata los servidores de modelos que corren en la propia máquina
(`llama-server`, Strata, PolyStrata) como proveedores de primera clase: se declaran **una sola vez**,
a nivel de perfil, y sirven para cualquier proyecto, estén encendidos o apagados. Diseño y razones en
`docs/specs/2026-11-06-loomcoding-providers-locales.md`.

## El registro global: `providers.toml`

Vive en el directorio de perfil de Loomcoding, bajo `.loomcoding/` (en Windows, dentro del directorio
personal; la variable `LOOMCODING_HOME` lo sustituye por otro, y es lo que deben usar las pruebas y las
capturas). Si no existe, Loomcoding lo crea en el primer arranque con tres entradas, y **jamás** pisa
uno existente.

| id | Servidor | Puerto | Clave |
|---|---|---|---|
| `llama` | `llama-server` (llama.cpp) | 8081 | ninguna |
| `strata` | Strata | 8080 | variable `STRATA_API_KEY` |
| `polystrata` | PolyStrata | 8082 | ninguna |

Cada entrada admite estos campos:

```toml
default = "llama"            # proveedor con el que nace un proyecto nuevo

[[provider]]
id = "llama"                 # nombre corto que se teclea en /provider
name = "llama-server"        # etiqueta visible
kind = "openai_compat"       # los servidores locales hablan el dialecto de OpenAI
base_url = "http://127.0.0.1:8081/v1"
model = "nombre-del-modelo"  # modelo de reserva si el servidor no responde
context_window = 65536       # opcional
api_key_env = "NOMBRE_VAR"   # opcional: NOMBRE de la variable, nunca la clave
```

Los puertos y los nombres de modelo son valores de ejemplo: ajústalos a lo que sirvas tú. Para añadir
otro servidor basta con otro bloque `[[provider]]`.

## Claves

La clave **nunca** se escribe en el toml. `api_key_env` guarda el *nombre* de una variable de entorno
(o de una credencial del almacén del sistema) que contiene la clave. Un servidor local de bucle
(`localhost` o `127.0.0.1`) puede no tener `api_key_env`: Loomcoding usa entonces una clave ficticia,
porque `llama-server` no la pide. Para cualquier dirección remota la variable sigue siendo obligatoria.

## Qué ocurre al arrancar

1. Un proyecto **sin** `loomcoding.toml` se crea con el proveedor `default` del registro (y no con
   OpenRouter). Un `loomcoding.toml` existente manda siempre sobre el registro.
2. En segundo plano, Loomcoding sondea cada servidor con **una sola** petición `GET <base_url>/models`
   (1,5 s de plazo, respuesta acotada a 256 KiB y 200 modelos). Nunca se lanza una inferencia, de modo que
   el sondeo no carga el servidor.
3. Los modelos descubiertos entran en el catálogo, y cada servidor se rotula con su estado:

| Estado | Significado |
|---|---|
| `up (N)` | respondió con N modelos |
| `down` | conexión rechazada, plazo agotado o respuesta que no es una lista de modelos |
| `sin acceso (clave)` | el servidor respondió 401 o 403: falta o sobra la clave |

Un servidor caído conserva su modelo de reserva, así que puede seleccionarse antes de encenderlo.
Los sondeos se repiten al abrir `/model`, `/settings` o `/provider`, con 5 s de enfriamiento.

## Comandos

| Comando | Qué hace |
|---|---|
| `/provider [id]` | cambia de proveedor (y de modelo) o, sin argumento, abre el selector con el estado de cada servidor |
| `/model [id]` | cambia el modelo del proveedor actual; acepta cualquier identificador, también uno no listado |
| `/effort [nivel\|default]` | cambia el esfuerzo de razonamiento, limitado a los niveles que el modelo declara |
| `/settings` | formulario completo (proveedor, URL, variable de clave, modelo, esfuerzo) |

Los tres primeros guardan en el `loomcoding.toml` del proyecto por el mismo camino que `/settings`.

## Esfuerzo de razonamiento en modelos locales

Loomcoding solo envía `reasoning_effort` si el proyecto declara un `effort`. Un modelo local no figura
en el catálogo público, así que sus niveles se declaran en el proyecto:

```toml
[provider]
efforts = ["low", "medium", "high"]
```

Sin esa lista, `/effort` avisa de que el modelo no tiene niveles declarados.

## Problemas habituales

- **`/provider` dice «Providers are still loading»**: el sondeo aún no ha vuelto; espera un instante.
- **Un servidor sale `down` estando encendido**: comprueba que el puerto y la ruta `/v1` del `base_url`
  coinciden con los del servidor.
- **Sale `sin acceso (clave)`**: define la variable indicada en `api_key_env` antes de lanzar Loomcoding.
- **Un proyecto sigue apuntando al proveedor antiguo**: su `loomcoding.toml` manda; cámbialo con `/provider`.
- **Nunca** arranques el binario real en pruebas o capturas sin `LOOMCODING_HOME` apuntando a un
  directorio temporal (véase `docs/visual-e2e.md`).
