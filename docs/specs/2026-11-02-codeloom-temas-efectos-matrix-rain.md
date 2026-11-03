# Temas → Efectos → Matrix rain: configuración bajo un árbol

- **Estado**: propuesta
- **Fecha**: 2026-11-02
- **Crates afectados**: `batten` (widget + tema), `codeloom-tui` (ventana, comandos, prefs)
- **Relacionado**: `docs/specs/2026-10-23-batten-showcase-theme-matrix-design.md`

## Problema

Hoy no hay forma de configurar la lluvia desde la aplicación:

1. **La lluvia ya está conectada, pero no es configurable.** `MatrixRain`
   (`crates/batten/src/widgets/matrix_rain.rs`) se instancia en `App`
   (`app.rs:857`), se pinta en `render.rs:2520` y se arranca al empezar un turno
   (`run.rs:1652`). Lo que falta es poder ajustarla sin recompilar.
2. **Los glifos son una constante.** `const GLYPHS: &[char]` (línea 7) con 63
   katakana, dígitos y símbolos. No se puede cambiar sin recompilar.
3. **Los colores están hardcodeados** en `render`: `Color::Rgb(125, 185, 140)`
   para la cabeza y un verde calculado para la cola. No leen del tema.
4. **No hay subcomandos.** El matcher de comandos (`app.rs:1347`) solo mira la
   **primera palabra**; `COMMANDS` es una tabla plana de 21 entradas. Un
   `/temas efectos matrix` no parsea hoy.
5. **No hay dónde persistir** la configuración de efectos: `prefs.rs` solo
   guarda tema y ratón.

## Objetivo

Que la lluvia se configure desde la aplicación, bajo un orden estable
`temas → efectos → matrix rain`, con una ventana propia para cada nivel, y que
la configuración sobreviva al reinicio.

## Diseño

### 1. Navegación: ventana con árbol

Se descarta extender el parser de comandos con subcomandos de texto: los glifos
son decenas de caracteres y no se escriben a mano en una línea de comando.

`/temas` abre una **ventana con árbol** de dos secciones:

```
┌ Temas ──────────────────────────────┐
│  ▸ Efectos                          │
│      Matrix rain                    │
│  ▸ Tema                             │
│      batten · crush · martty · ...  │
└─────────────────────────────────────┘
```

- `↑`/`↓` mueven la selección, `Enter` entra, `Esc` vuelve un nivel y cierra
  en la raíz.
- El nivel `Tema` lista `ThemePreset::ALL` (7 presets) y aplica en vivo al
  confirmar, igual que el ciclo actual de `/theme`.
- El nivel `Efectos` está preparado para más efectos, pero en esta entrega solo
  contiene `Matrix rain`.

Se reutiliza el patrón de ventana ya existente: `open_settings`
(`app.rs:1845`) y `draw_provider_settings` (`render.rs:1200`). **No** se usa
`settings_panel.rs`, que es un mockup estático sin estado.

### 2. Formulario de Matrix rain

Dentro de `Efectos → Matrix rain`:

| Campo | Tipo | Valores |
|---|---|---|
| Activado | bool | on/off |
| Glifos | preset + editor | katakana · emoji · binario · ascii · mixto · propio |
| Velocidad | rango | 0.25 – 1.00 (paso 0.05) |
| Densidad | rango | 1 – 10 (probabilidad de emisión) |
| Longitud | rango | 5 – 20 celdas |
| Color cabeza | color | token del tema |
| Color cola | color | token del tema |

Cada cambio se previsualiza en vivo sobre el fondo de la propia ventana.

### 3. Glifos y el problema del ancho

**Restricción dura**: el render pinta una celda por columna
(`buffer[(x, area.y + row)]`). Un emoji ocupa **dos celdas** en terminal, así
que escribirlo tal cual rompe la rejilla y desplaza el resto de la fila.

Solución: el juego de glifos se declara con su **ancho** (`1` o `2`). Si el
juego es de ancho 2, el render avanza de dos en dos columnas y deja la celda
contigua vacía. El campo `width` es parte de la config, no una suposición.

Presets:

- `katakana` — el actual (63 glifos, ancho 1)
- `emoji` — un juego corto de ancho 2
- `binario` — `0`/`1` (ancho 1)
- `ascii` — imprimibles (ancho 1)
- `mixto` — katakana + dígitos + símbolos (ancho 1)
- `propio` — cadena del usuario, ancho declarado

El editor de glifos es un campo de texto; se valida que todos los caracteres
tengan el mismo ancho y se rechaza el juego si no.

**Hallazgo de la Fase 1**: `unicode-width` da ancho **1** a algunos emoji que
visualmente son anchos, porque necesitan selector de variación (U+FE0F). Por
ejemplo `🌧` (U+1F327) mide 1, mientras que `💧` (U+1F4A7) mide 2. Por eso el
ancho **no se puede suponer por el rango Unicode**: se mide con
`UnicodeWidthChar::width` y hay un test (`every_preset_declares_its_real_width`)
que verifica que cada preset declara el ancho que sus glifos realmente ocupan.

### 4. Persistencia

Dos capas, con el tema como base y las preferencias como override:

- **`theme.toml`** gana una sección `[effects.matrix]` con los valores por
  defecto del tema. `ThemeFile` (`theme.rs:272`) hoy solo tiene `styles`; se le
  añade `effects` como campo opcional.
- **`prefs.rs`** guarda lo que el usuario cambia en la ventana, con el mismo
  patrón que `load_theme`/`save_theme`.

Al arrancar: se carga el tema, se aplica `[effects.matrix]`, y encima el
override de prefs si existe.

```toml
[effects.matrix]
glyphs = "katakana"   # katakana | emoji | binario | ascii | mixto | <propio>
speed = 0.25
density = 3
length = 5
color = "success"     # token del tema; la cola se deriva de él
```

**Los colores son tokens del tema, no hex.** Un solo token (`color`) define la
gota completa: la cabeza usa el color del token y la cola se deriva
oscureciéndolo, conservando el tono. Así, al cambiar de preset de tema la
lluvia sigue en armonía. El default es `success`, que es el verde del tema — el
verde Matrix.

### 5. Conexión con la TUI real

Ya existe y no cambia: `MatrixRain` vive en `App` (`app.rs:857`), se pinta como
fondo en `render.rs:2520` y se arranca al empezar un turno (`run.rs:1652`). Lo
único que añade esta spec es que su `MatrixConfig` venga del tema y de prefs en
vez de `Default`.

`animating()` ya devuelve `false` cuando no hay gotas, así que no cuesta nada
cuando está apagada.

## Fuera de alcance

- Otros efectos (fuego, nieve, plasma). El árbol queda preparado, pero no se
  implementan.
- Animaciones en el *showcase*: sigue usando su propia copia.
- Configuración por proyecto: la lluvia es global, no por workspace.

## Preguntas abiertas

1. ¿La lluvia debe poder activarse como fondo permanente, o solo como efecto
   temporal que se apaga solo? Afecta a si `enabled` es un bool o un temporizador.
2. ¿El editor de glifos acepta pegar una cadena larga, o se limita a un número
   fijo de caracteres por el coste de repintado?
3. ¿Los colores de la lluvia deben ser tokens del tema (`accent`, `success`) o
   colores libres en hex? Los tokens mantienen coherencia entre presets; el hex
   da libertad pero rompe la armonía al cambiar de tema.

   **Resuelta**: tokens del tema. Un solo token `color`; la cola se deriva de
   él. Default `success` (verde Matrix).
