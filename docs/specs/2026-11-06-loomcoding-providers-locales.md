# Proveedores locales globales y descubrimiento de modelos — spec

Estado: **aprobada**. Nace de una petición del usuario: tener `llama-server`, Strata y PolyStrata como proveedores
por defecto **a nivel global**, para encenderlos o apagarlos a voluntad sin pelearse con scripts ni indicárselo a
mano, y que Loomcoding **descubra los modelos** de los proveedores disponibles.

## Problema

Hoy el proveedor vive en el `loomcoding.toml` de cada proyecto, y si no existe, `LoomcodingConfig::load_or_create`
lo crea apuntando a OpenRouter. Para probar un modelo local hay que llevar un toml y un script `.ps1` por servidor
(puertos 8081, 8080 y 8082), fijar a mano la variable de la clave y escribir el nombre exacto del modelo. Además,
`build_provider_state` devuelve `UnconfiguredProvider` si no hay clave, de modo que un `llama-server` sin clave ni
siquiera arranca como proveedor.

## Intención

### 1. Registro global de proveedores locales

Un fichero `providers.toml` en `<perfil>/.loomcoding/` (el perfil es `user_home()`, que respeta `LOOMCODING_HOME`),
con una entrada por servidor:

```toml
default = "llama"

[[provider]]
id = "llama"            # nombre corto, el que se teclea
name = "llama-server"   # etiqueta visible
kind = "openai_compat"
base_url = "http://127.0.0.1:8081/v1"
model = "Ornith-1.5-9B"        # modelo de reserva si el servidor no responde
context_window = 65536         # opcional

[[provider]]
id = "strata"
name = "Strata"
kind = "openai_compat"
base_url = "http://127.0.0.1:8080/v1"
api_key_env = "STRATA_API_KEY"  # nunca la clave en claro
model = "qwen3.8-flash-next-coder-iq1_m"
context_window = 65536

[[provider]]
id = "polystrata"
name = "PolyStrata (Flint)"
kind = "openai_compat"
base_url = "http://127.0.0.1:8082/v1"
model = "flint-40b-a2.5b-ud-q4_k_xl"
context_window = 65536
```

- Si el fichero **no existe**, se crea con estas tres entradas (sin pisar nunca uno existente).
- Las claves **solo** por variable de entorno o almacén de credenciales (`credentials::get`), igual que hoy.
  `api_key_env` es opcional: sin él, un servidor de bucle local (`localhost`/loopback) usa una clave ficticia
  (`local`), porque `llama-server` no la pide. Para una URL no local, sin clave sigue siendo «sin configurar».
- Las entradas son proveedores **conocidos**, no proveedores activos: estén encendidos o no, aparecen siempre.

### 2. Valor por defecto al arrancar

- Un proyecto **sin** `loomcoding.toml` se crea tomando el proveedor `default` del registro (kind, base_url,
  api_key_env, modelo), en lugar de OpenRouter. Sin registro legible, queda el comportamiento actual.
- Un `loomcoding.toml` **ya existente** no se toca: la configuración del proyecto manda sobre la global.

### 3. Descubrimiento de modelos

- Al arrancar y al abrir `/model`, `/settings` o el selector de proveedor (con enfriamiento de 5 s), se sondea cada
  proveedor del registro en segundo plano: `GET <base_url>/models` (formato OpenAI; `data[].id`) con tiempo máximo de
  1,5 s y la clave si la hay.
- El resultado alimenta el **catálogo existente**: cada proveedor local se añade como `CatalogProvider` con sus modelos
  descubiertos, así los selectores de `/settings` y `/model` funcionan sin tocar su lógica. Un proveedor que no
  responde conserva su `model` de reserva.
- Estado por proveedor: `up` (respondió, con N modelos), `down` (rechazo de conexión o tiempo agotado) o
  `auth` (401/403: falta o sobra clave). Se muestra en el selector de proveedor junto al nombre.
- **Presupuesto**: una sola petición `GET /models` por proveedor y sondeo, sin cuerpo, nunca una inferencia. Es
  obligatorio porque Strata va justo de recursos: no se le lanza carga.

### 4. Cambiar de proveedor

`/provider [id]`: sin argumento abre el selector de proveedores (con su estado `up`/`down`); con `id` fija el
proveedor y su modelo por el mismo camino de guardado que `/settings` y `/model`.

## Reglas

- Fichero nuevo ≤ 400 líneas; el sondeo vive en `loomcoding-core` (sin `ratatui`), la lectura del fichero y el cableado
  en `loomcoding-cli`, el estado y los selectores en `loomcoding-tui`.
- Sin red real en las pruebas: el sondeo se prueba con `wiremock`.
- Nada sin cota: se leen como máximo 256 KiB de respuesta y 200 modelos por proveedor.

## No objetivos

Arrancar o parar los servidores (los controla el usuario), gestionar descargas de modelos, anunciar `/health` en la
cabecera, y cualquier optimización del rendimiento de Strata (lo trata otro agente).

## Preguntas abiertas

Ninguna. El usuario decidió el 6 de noviembre: el `default` del registro es `llama` y `/provider` se incluye como comando.
