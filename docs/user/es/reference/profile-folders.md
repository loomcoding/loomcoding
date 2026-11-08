# Carpetas del perfil

**Qué es**

Las carpetas y los ficheros que Loomcoding guarda en el perfil y en el proyecto. El perfil es la carpeta `.loomcoding` dentro de la raíz del perfil. La raíz del perfil es `LOOMCODING_HOME` cuando está definida, y tu carpeta personal del sistema en otro caso. Esta página escribe esa carpeta como `<perfil>/.loomcoding/`.

**Para qué**

- Encontrar la base de datos de sesiones, los registros o las salidas de herramientas guardadas.
- Saber qué ficheros crea Loom por sí mismo, y cuáles escribes tú.
- Borrar o copiar un perfil a propósito.

**Cómo**

1. Define `LOOMCODING_HOME` con una carpeta temporal para usar un perfil aparte.
2. Lee la primera tabla para el perfil, y la segunda para el proyecto.
3. No edites a mano la base de datos ni los ficheros de runtime. Usa los ajustes y los comandos.

## Ejemplo

```text
<perfil>/.loomcoding/
├── providers.toml
├── sessions.db
├── logs/loomcoding-daemon.log
├── tool-output/tool-<llamada>-<n>.txt
├── runtime/server.key
└── agent-trust.json
```

## Carpeta del perfil

| Nombre | Lo crea | Cuándo | Qué guarda |
|---|---|---|---|
| `providers.toml` | Loom | Cuando se lee el perfil por primera vez, si el fichero falta | Servidores de modelos locales y la entrada por defecto para los proyectos nuevos. Consulta [configuración](configuration.md#providers-toml). |
| `sessions.db` | El servidor local | Al arrancar el servidor local | La base de datos (SQLite) de las sesiones y sus mensajes. |
| `logs/loomcoding-daemon.log` | Loom | Solo si `RUST_LOG` está definida | Registro del servidor local. |
| `logs/loomcoding-cliente.log` | Loom | Solo si `RUST_LOG` está definida | Registro del cliente. |
| `tool-output/` | Loom | Cuando la salida de una herramienta es larga, o el filtro conserva el original | El texto íntegro de cada salida de herramienta, en ficheros `tool-<llamada>-<n>.txt`. El modelo recibe el principio y la ruta. |
| `runtime/server.key` | El servidor local | Al arrancar | El secreto que el cliente usa para conectarse al servidor local. En Unix, la carpeta es privada de tu usuario. |
| `runtime/agents/` | Loom | Cuando `[a2a] role` es `consume` o `both` | Carpetas de los pares de agentes remotos, una por arranque del servidor y por proyecto. |
| `agent-trust.json` | Loom | Cuando apruebas un agente | Tus aprobaciones de agentes de proyecto y de catálogo. |
| `mcp.toml` | Tú | Opcional | Servidores MCP. Loom solo lo lee. |
| `runtime.toml` | Tú | Opcional | Ajustes que lee el servidor local. Consulta [configuración](configuration.md#runtime-toml). |
| `models-dev.json` | Loom | Cuando guarda en caché el catálogo de modelos | La caché del catálogo de modelos. Su ubicación no está dentro del perfil; consulta la nota de abajo. |

La caché `models-dev.json` se escribe en `.loomcoding/` dentro de la carpeta que indica `USERPROFILE`, o `HOME` si `USERPROFILE` no está definida. `LOOMCODING_HOME` no la mueve. Por eso, un perfil de prueba no aísla este fichero.

## Carpeta del proyecto

Estos ficheros están en la raíz del proyecto, la carpeta desde la que arrancas Loom.

| Nombre | Lo crea | Cuándo | Qué guarda |
|---|---|---|---|
| `loomcoding.toml` | Loom | En la primera apertura, si el fichero falta | Ajustes del proyecto. Consulta [configuración](configuration.md). |
| `.env` | Tú | Se lee al abrir el agente | Variables para las claves de proveedor y para los comandos que ejecuta Loom. Loom nunca escribe este fichero. Consulta [variables de entorno](environment.md). |
| `AGENTS.md`, `LOOMCODING.md` | Tú | Se leen al abrir el agente | Contexto que se añade al prompt. Cada fichero se corta a 16 000 bytes, con un aviso. |
| `.loomcoding/odd/<función>.md` | El agente | Para trabajos grandes | El documento de tareas de una función. Consulta [ODD y flujos](../concepts/flows-odd.md). |
| `.agents/agents/` | Tú | Opcional | Agentes del proyecto. Es el `project_dir` por defecto. |

Los agentes globales se leen de `.agents/agents/` dentro de tu carpeta personal, no del perfil. Es el `global_dir` por defecto.

## Siguientes pasos

- [Configuración (`loomcoding.toml`)](configuration.md)
- [Variables de entorno](environment.md)
- [Permisos](../concepts/permissions.md)
