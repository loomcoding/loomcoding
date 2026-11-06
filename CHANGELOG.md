# Registro de cambios

Los cambios relevantes se anotan aquí por versión. Antes de la primera release, los cambios
pendientes se agrupan en `Unreleased`; las versiones publicadas usarán etiquetas Git y notas de
release.

## Unreleased

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
