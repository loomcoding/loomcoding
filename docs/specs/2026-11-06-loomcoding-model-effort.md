# Comandos `/model` y `/effort` — spec

Estado: **propuesta**. Nace de una petición del usuario: dos comandos «muy fundamentales», `/model` para
cambiar el modelo del proveedor seleccionado y `/effort` para cambiar el razonamiento del modelo.

## Problema

Hoy cambiar de modelo o de esfuerzo exige abrir `/settings` (F2), moverse por un formulario de cinco campos
y pulsar «Save». La maquinaria existe y funciona (selectores de proveedor y modelo con catálogo, campo
«Effort» que cicla entre `default` y los niveles del modelo, guardado y reinicio del proveedor mediante
`ProviderSettingsCommand`), pero es demasiado larga para algo que se hace a menudo, sobre todo con modelos
locales que se cambian de un servidor a otro.

## Intención

Dos atajos que **reutilizan el flujo de ajustes existente** (mismo guardado en `loomcoding.toml`, mismo
reinicio del proveedor, mismas reglas) sin abrir el formulario.

### `/model`

- `/model <id>`: fija el modelo del proveedor actual a `<id>` y lo aplica. Acepta cualquier identificador,
  también uno que no esté en el catálogo (los servidores locales usan nombres libres, p. ej. `Ornith-1.5-9B`).
  Aviso: `model: <id>`.
- `/model` sin argumento: abre el **selector de modelos** del proveedor actual (el mismo `SettingsPicker::Model`
  de `/settings`: búsqueda por texto, `↑`/`↓`, `Enter` elige, `Esc` cancela) y, al elegir, aplica. Sin catálogo
  para ese proveedor, avisa: «No model catalog for this provider: use /model <id>».
- Cambiar de modelo reconcilia el esfuerzo: si el nuevo no admite el nivel actual, vuelve a `default` y lo dice
  (regla ya existente de `reconcile_effort`).

### `/effort`

- `/effort <nivel>`: fija el esfuerzo a uno de los niveles del modelo actual o a `default` (sin campo en la
  petición) y lo aplica. Aviso: `effort: <nivel>`.
- `/effort` sin argumento: abre un **selector** con `default` y los niveles, marcando el vigente; `Enter`
  elige y aplica, `Esc` cancela.
- Un nivel desconocido responde con los válidos: `Usage: /effort [default|low|medium|high]` (la lista real).
- Si el modelo no admite niveles (ni el catálogo ni `[provider] efforts` los publican), avisa: «This model has
  no reasoning levels. Declare them in [provider] efforts of loomcoding.toml».
- Los niveles salen de `App::effort_options()` (catálogo y, si no publica niveles, los declarados en
  `[provider] efforts`).

### Aplicación

Ambos comandos terminan en la misma petición que el botón «Save» de `/settings` (`settings_save_requested`),
de modo que el servidor escribe `loomcoding.toml` y reconstruye el proveedor. Mientras hay un turno en
marcha se aplican igual que hoy desde `/settings` (no se inventa otra regla): si `/settings` lo impide,
estos comandos también, con el mismo aviso. El resultado (éxito o error del servidor) se muestra en el
aviso del compositor y el pie refleja el modelo nuevo.

## Popup de `/`

`/model` y `/effort` aparecen en la lista con una descripción corta y aceptan argumentos sin cerrarse el
popup (como `/verbose` y `/logofx`). No son comandos de aspecto: **no** entran en `/ui`.

## No objetivos

Cambiar de proveedor (sigue en `/settings`), editar URL o clave, un selector con vista previa de precios, y
cambiar cómo viaja el esfuerzo al proveedor (eso es otra cosa: hoy `reasoning_effort` solo se envía si hay
`effort` configurado).

## Preguntas abiertas

Ninguna. Decisión tomada como propia: `/effort` sin argumento abre un selector (no cicla a ciegas), porque
con cuatro o cinco niveles ver el vigente es más útil que dar vueltas.
