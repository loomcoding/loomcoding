# Registro de cambios

Los cambios relevantes se anotan aquí por versión. Antes de la primera release, los cambios
pendientes se agrupan en `Unreleased`; las versiones publicadas usarán etiquetas Git y notas de
release.

## Unreleased

### Compresión de salida de herramientas (`output_filter`)

- Capa determinista que reduce la salida de `bash` y `job_output` antes de que llegue al modelo: reglas generales (ANSI,
  barras de progreso, líneas repetidas, líneas en blanco) y, para `cargo test|build|check|clippy`, `git status` y
  `npm|pnpm install`, una lista blanca de señal. Cada salida filtrada lleva una cabecera con la ruta del original;
  `raw: true` devuelve la salida sin filtrar.
- **Apagado por defecto.** `[output_filter] enabled = true` lo enciende; `LOOMCODING_NO_FILTER=1` lo apaga siempre. Guía
  de uso y de medición: `docs/output-filter.md`.
- Medición: contadores por regla (usos, re-peticiones, bytes), y desactivación automática de una regla con al menos 10
  usos y más del 20 % de re-peticiones, con un único aviso (`RuntimeNotice`). Sin datos personales.
- La dedup de lecturas en el historial queda aplazada hasta medir.
- **Cable:** nueva variante `LiveEvent::OutputFiltered { rule, original_bytes, sent_bytes }`, solo local (no se reenvía
  desde los subagentes). Un cliente antiguo conectado a un daemon nuevo no la entenderá (el binario es el mismo para
  ambos hoy). La TUI la ignora por ahora.
- Medición por `tracing` con destino `output_filter`: con `RUST_LOG=output_filter=info` sale al registro del daemon
  (véase la entrada de registro de `tracing` más abajo).

### Registro de `tracing` a fichero

- Si `RUST_LOG` está definida y no vacía, el cliente y el daemon instalan un suscriptor que escribe a
  `<perfil>/.loomcoding/logs/loomcoding-<rol>.log` (`cliente` o `daemon`). Sin `RUST_LOG` no se instala nada ni se crea
  fichero. Nunca se escribe en stdout ni en stderr.
- Cota de 5 MiB por fichero, con una sola copia anterior (`.1`). Un fallo de E/S nunca impide arrancar.
- Sin dependencias nuevas (`tracing-subscriber` con `env-filter`, ya en el espacio de trabajo). Spec:
  `docs/specs/2026-11-07-loomcoding-registro-tracing.md`.

### Desarrollo orgánico (ODD)

- Nuevo crate `loomcoding-workflows` con ODD, un modo ligero **activo por defecto** (`[flows.odd] enabled = true`, `/flow` para
  verlo o guardarlo; aplica al reiniciar). Es un flujo propio de Loomcoding: no cambia ningún estado ni tarea A2A.
- Herramienta `odd_track` (solo el agente principal): documento de feature en `.loomcoding/odd/<feature>.md` solo para
  trabajo grande, con casillas que exigen evidencia (una llamada real de `bash`, `job_start` o `job_output` de los últimos
  500 mensajes de la sesión).
- El cierre del turno se bloquea mientras queden tareas sin hacer: hasta 3 reinyecciones por turno, ninguna si no hay
  progreso, plazo de 2 s, y la cancelación gana siempre. «Necesita tu decisión» o un documento ilegible lo levantan.
- La cabecera muestra la fase inferida de las herramientas y el avance (`ODD · implementing (2/5)`), y la lista de tareas
  de la TUI es una proyección del documento. `gauntlet` y `development_workflow` también aparecen en la cabecera.
- Puntos de extensión genéricos en el núcleo, solo para el agente principal: `TurnCloseGate`, `ToolObserver`,
  `PromptContributor` y `Tool::parent_only`. Los subagentes locales y las tareas A2A servidas no los heredan (prueba
  automática).
- **Cable:** nuevo variante `LiveEvent::FlowChanged` (con `FlowSummary.persistent`, `false` si falta); un cliente antiguo conectado a un daemon nuevo no lo entenderá
  (el binario es el mismo para ambos hoy).
- Medición sin datos personales por `tracing` (destino `odd`).

### Vigilancia de jobs (`watch`)

- `job_start` admite `watch: {pattern, max_events?}`: las líneas de la salida que casan con el patrón (regex) llegan a la
  conversación como avisos `[Job <id> watch] …`, acotados (hasta 100 avisos, 10 líneas por aviso y un aviso cada 5 s).
- Nueva herramienta `job_watch` para añadir, cambiar o quitar la vigilancia de un job que ya está en marcha.
- Un aviso despierta al modelo por defecto (turno «job watch»), dentro del mismo presupuesto de turnos automáticos que
  los avisos de subagentes.
- La TUI muestra «watching: N alerts», con «+M suppressed» y «limit reached» cuando procede, en la sección Jobs del rail
  y en la actividad del job. Es información de Loomcoding, no un estado A2A.

### Proveedores locales

- Registro global `providers.toml` (llama-server en 8081 por defecto, Strata en 8080, PolyStrata en 8082): un proyecto
  nuevo nace con el proveedor `default` en vez de OpenRouter, y un servidor local sin clave ya no queda «sin configurar».
- Descubrimiento de modelos: un único `GET /models` por servidor en segundo plano; los modelos aparecen en `/model` y
  `/settings` con el estado del servidor («up (N)», «down», «sin acceso»). Nuevo comando `/provider [id]`.

### Interfaz

- Nueva fila «Carril» en `/ui` (`/ui carril <valor>`, clave `rail` de `ui.toml`) con seis estilos para el trazo del
  transcript: `tree` (el de siempre, por defecto), `dots`, `bar` (como Crush), `box` (la caja de MastraCode, solo para los
  mensajes del usuario), `tint` (fondo tenue, como Pi y Codex) y `none`. Por debajo de 40 columnas no se reserva carril.
- Nuevo comando `/ui [fila] [valor]`: abre el panel «Interfaz» o fija un ajuste de aspecto directamente. Los antiguos
  `/theme`, `/layout`, `/temas`, `/crush`, `/classic`, `/mascot`, `/sidebar` y `/mouse` siguen funcionando, pero dejan de
  saturar el popup de `/`; la ayuda se condensa en 21 líneas.
- Nuevos comandos `/model [id]` y `/effort [nivel|default]`: cambian el modelo del proveedor y el esfuerzo de
  razonamiento sin abrir `/settings`, por su mismo camino de guardado. Sin argumento abren un selector; el modelo
  acepta cualquier identificador y el esfuerzo se limita a los niveles del modelo (catálogo o `[provider] efforts`).
- `/temas` abre ahora el panel «Interfaz», con tema, estilo (`classic`/`flat`), distribución, efectos, spinner,
  muñeco, menú lateral y ratón en una sola lista; `←`/`→` cambian el valor sin entrar. Nueva clave `style` en
  `ui.toml`, que sobrevive al cambio de tema igual que `layout`.

- Menú izquierdo del layout `Workspace`: logo «LOOM», bloques sin mayúsculas ni contadores a cero
  (`Actividad`, `Sesión`) y bloque `Ficheros` con el diff de `git` desplegable por fichero tocado en la
  sesión (`↑`/`↓` y `Enter` con el foco en el menú; `Shift+Tab` desde el compositor).
- Un efecto al azar sobre el logo cada cierto tiempo (lluvia binaria, reflejo, linterna, tipográfico,
  glitch, disolución, pulso), sin reloj en reposo. Nuevas claves `logo_fx` y `logo_fx_interval` en
  `ui.toml` y comando `/logofx [on|off|segundos]`.
- Barra de desplazamiento propia: el pulgar llega siempre al final y su aspecto sigue al marco del tema
  (flechas y pista punteada con paneles; línea fina sin cajas).
- Muñeco de estado: moonwalk al esperar y brazos en alto al terminar.

### Servidores locales

- Nuevas claves opcionales `[provider] context_window` y `request_timeout_secs` en `loomcoding.toml`
  (la segunda, de 1 a 3600 s; por defecto 120 s en el adaptador OpenAI-compatible).
- Reconocido el rechazo por exceso de contexto de llama.cpp, que ahora provoca un resumen y un reintento.
- Los errores HTTP del adaptador OpenAI-compatible incluyen el cuerpo de la respuesta, acotado a 512 caracteres.

### Renombrado

- CodeLoom pasa a llamarse **Loomcoding**; la crate `batten`, a `loomcoding-cloth`. Cambian los
  paquetes, el binario `codeloom` (ahora `loomcoding`), `loomcoding.toml`, `LOOMCODING_*`,
  `~/.loomcoding/` y el plugin `loomcoding.herdr`. Sin capa de compatibilidad: véase la sección
  de migración del README.

### Preparación del proyecto

- Añadidas pautas de contribución, seguridad, conducta y licencias.
- Preparada CI para los checks del workspace en Linux y Windows.
- Ignorados los ficheros `.env.*` además de `.env`.
