# Fondo «telar» de carga — plan

> **Para quien ejecute:** una tarea cada vez; pruebas primero (rojo visible); los cuatro chequeos
> (`cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`);
> un commit por tarea. Cargo desde PowerShell. Rama `fondo-telar`. Binario real solo con
> `LOOMCODING_HOME` temporal. Ficheros ≤ 400 líneas. Comentarios en español.

**Spec:** `docs/specs/2026-11-08-loomcoding-fondo-telar.md` (léela entera antes de empezar).

## Restricciones globales

- El widget vive en `crates/loomcoding-cloth/src/widgets/loom_scene/` y **solo depende de `ratatui`** y
  del propio `cloth` (reutiliza `GlyphSet::katakana()` y los tonos de `matrix_rain`; haz `pub(crate)` lo
  que haga falta de ahí, sin cambiar su comportamiento).
- `loomcoding-tui` solo selecciona y engancha. `loomcoding-core` no se toca.
- **El render no lee el reloj ni reserva memoria por fotograma.** El avance es `advance()` en el tick
  de 50 ms. Las reservas ocurren solo al cambiar de tamaño.
- Pruebas deterministas: semillas fijas, sin `Instant::now()`, sin red.
- Constantes (no configurables): `APARECER_TICKS = 18`, `DESAPARECER_TICKS = 24`,
  `MAX_CELDAS = 40_000`, `MAX_HILOS = 30`, `PASADA_TICKS = 16` (0,8 s), `CICLO_FILAS = 12`.

## Mapa de ficheros

| Fichero | Responsabilidad |
|---|---|
| `cloth/src/widgets/loom_scene/mod.rs` | `LoomBackground`: API pública y ensamblado |
| `cloth/src/widgets/loom_scene/reveal.rs` | Fundido por celda con orden aleatorio |
| `cloth/src/widgets/loom_scene/layout.rs` | Zonas de la escena según el tamaño |
| `cloth/src/widgets/loom_scene/threads.rs` | Hilos que bajan y convergen |
| `cloth/src/widgets/loom_scene/loom_bar.rs` | Barra, postes, púas, lanzadera y rótulos |
| `cloth/src/widgets/loom_scene/cloth.rs` | Tela en perspectiva y palabra tejida |
| `tui/src/app/background.rs` | `Background` (matrix/loomcoding/off), selección y enganche |
| `tui/src/prefs/background.rs` | Lectura y guardado de `background` en `ui.toml` |

## Tarea 1 — Fundido aleatorio por celda (`reveal.rs`)

**Interfaz:**
```rust
pub struct Reveal { /* w, h, visible: Vec<bool>, nivel_in, nivel_out, dir, semilla_in, semilla_out */ }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Dir { Aparece, Desaparece, Quieto }
impl Reveal {
    pub fn new() -> Self;
    pub fn resize(&mut self, w: u16, h: u16);        // reserva solo si cambia; vacía el estado
    pub fn aparecer(&mut self, semilla: u64);        // dir = Aparece, nivel_in = 0
    pub fn desaparecer(&mut self, semilla: u64);     // dir = Desaparece, nivel_out = 0
    pub fn paso(&mut self);                          // un tick de 50 ms
    pub fn visible(&self, x: u16, y: u16) -> bool;
    pub fn dir(&self) -> Dir;
    pub fn ocupa_pantalla(&self) -> bool;            // alguna celda visible o fundiéndose
}
fn umbral(x: u16, y: u16, semilla: u64) -> f32;      // hash entero → [0, 1)
```
Reglas: en `Aparece`, una celda no visible pasa a visible cuando `umbral(x,y,semilla_in) <= nivel_in`;
`nivel_in` sube `1/APARECER_TICKS` por `paso`. En `Desaparece`, una celda visible pasa a oculta cuando
`umbral(x,y,semilla_out) < nivel_out`; `nivel_out` sube `1/DESAPARECER_TICKS`. Al llegar el nivel a 1,
`dir = Quieto`. Cada llamada a `aparecer`/`desaparecer` reinicia **su** nivel a 0 y no toca las celdas.

- [ ] Pruebas (rojo primero): tras `APARECER_TICKS` pasos todas las celdas son visibles; tras
  `DESAPARECER_TICKS` ninguna; a mitad de aparición, `desaparecer` no cambia el nº de celdas visibles
  en ese instante (sin salto) y el orden de ocultación **difiere** del de aparición con semillas
  distintas; `aparecer` a mitad de la desaparición tampoco salta; mismo resultado con misma semilla;
  `resize` a otro tamaño vacía y no entra en pánico con 0×0; la mitad de las celdas hay visible hacia la
  mitad del fundido (±15 %, por la distribución uniforme del umbral).

## Tarea 2 — Zonas según el tamaño (`layout.rs`)

**Interfaz:**
```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Nivel { Oculto, Compacto, Completo }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Zonas {
    pub nivel: Nivel, pub w: u16, pub h: u16,
    pub hilos_filas: u16,        // filas 0..hilos_filas-1 son de hilos
    pub barra_y: u16,            // primera fila de la barra (ocupa 3)
    pub rotulo_arriba_y: u16,    // CODE GENERATION
    pub tela_y0: u16, pub tela_y1: u16,   // ambas inclusive; 0 filas si Compacto
    pub rotulo_abajo_y: u16,     // última fila
}
pub fn zonas(w: u16, h: u16) -> Zonas;
```
Fórmulas (`h` filas): `barra_y = h * 42 / 100` (Compacto: `h * 70 / 100`); `hilos_filas = barra_y`;
`rotulo_arriba_y = barra_y + 3`; `tela_y0 = barra_y + 4`; `tela_y1 = h - 2`; `rotulo_abajo_y = h - 1`.
Niveles: `Completo` si `w >= 56 && h >= 20`; `Compacto` si `w >= 40 && h >= 10`; si no `Oculto`; también
`Oculto` si `w as u32 * h as u32 > MAX_CELDAS`.

- [ ] Pruebas: 80×24 → `Completo` con `tela_y1 - tela_y0 + 1 >= 7`; 56×20 completo con 7 filas de tela;
  55×20 y 56×19 → `Compacto`; 39×10 y 40×9 → `Oculto`; 300×200 → `Oculto`; 0×0 sin pánico;
  para todo `Completo` las zonas no se solapan (hilos < barra < rótulo < tela < rótulo final).

## Tarea 3 — Hilos (`threads.rs`)

**Interfaz:**
```rust
pub struct Hilos { /* flujos: Vec<Flujo> (≤ MAX_HILOS), semilla */ }
pub struct Celda { pub x: u16, pub y: u16, pub glifo: char, pub tono: Tono, pub intensidad: u8 }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Tono { Verde, Dorado, Azul }
impl Hilos {
    pub fn new(semilla: u64) -> Self;
    pub fn ajustar(&mut self, z: &Zonas);                       // nº de hilos = (w/3).clamp(8, 30)
    pub fn avanzar(&mut self, tick: u64);
    pub fn celdas(&self, z: &Zonas, tick: u64, f: &mut impl FnMut(Celda)); // sin reservar
}
```
Cada hilo `i` baja por `x(y)`: de `x_ini = 1 + (w-3)·i/(n-1)` a `x_fin = w·(0.35 + 0.30·i/(n-1))`, con
suavizado `k = q²(3-2q)`, `q = clamp((u-0.12)/0.88)`, `u = y/hilos_filas`. Cabeza con velocidad
`0.10..0.23` filas/tick y cola de 5..11 celdas; el tono sale de la semilla (≈10 % `Dorado`, ≈12 %
`Azul`). Glifos de `GlyphSet::katakana()`. `intensidad` 0..=255: la cabeza 255 y la cola decrece.

- [ ] Pruebas: todos los hilos terminan cerca de la franja central (`x_fin` entre 35 % y 65 % del
  ancho); ninguna celda se sale de `0..w` ni de `0..hilos_filas`; con `Compacto` los hilos usan
  `hilos_filas` de esa zona; la proporción de tonos con 1000 hilos simulados está en 8–12 % dorado y
  10–14 % azul (semillas fijas); `celdas` no reserva (la prueba usa un contador y comprueba
  `capacity` de los vectores internos antes y después).

## Tarea 4 — Barra, lanzadera y rótulos (`loom_bar.rs`)

**Interfaz:**
```rust
pub fn pintar_barra(z: &Zonas, tick: u64, f: &mut impl FnMut(u16, u16, char, Realce));
pub fn lanzadera_x(z: &Zonas, tick: u64) -> u16;      // onda triangular, periodo 32 ticks
pub const ROTULO_ARRIBA: &str = "CODE GENERATION";
pub const ROTULO_ABAJO: &str = "WEAVING SIGNALS FROM CODE";
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Realce { Marco, Pua, Lanzadera, Rotulo }
```
La barra mide el 44 % del ancho (mínimo 20), centrada, con marco `╔═╗║╚╝` de 3 filas, postes `║` de 3
filas a cada extremo, púas `│` cada 2 celdas por dentro, y la lanzadera `●` que recorre el interior
con ida y vuelta de 32 ticks. Los rótulos van centrados en `rotulo_arriba_y` y `rotulo_abajo_y`; el de
abajo se omite si no cabe en `w`.

- [ ] Pruebas: `lanzadera_x` vale el mínimo en el tick 0, el máximo en el 16 y vuelve al mínimo en el
  32; siempre dentro de la barra; la barra cabe y está centrada en 56, 80 y 120 columnas; los rótulos
  se centran y el de abajo desaparece con `w < 25`; ninguna celda fuera de `w×h`.

## Tarea 5 — Tela y palabra tejida (`cloth.rs`)

**Interfaz:**
```rust
pub fn pintar_tela(z: &Zonas, tick: u64, f: &mut impl FnMut(u16, u16, char, Brillo));
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Brillo { Tenue, Medio, Claro, Palabra }
pub const PALABRA: &str = "LOOMCODING";
pub fn glifo_de_letra(c: char) -> Option<[&'static str; 5]>;   // L O M C D I N G
pub fn fila_de_palabra(columna: usize) -> Option<[bool; 5]>;    // columna de 51 de la palabra
```
Filas de tela `tela_y0..=tela_y1` (`n_filas`); `n_cols = min(52, w - 4)`. Perspectiva: la fila `r`
(0 = arriba) tiene ancho `n_cols · (0.87 + 0.13 · r/(n_filas-1))`, centrada. Fila `r` en la pasada `p`
(`p = tick / PASADA_TICKS`): `serie = (n_filas - 1 + p) - r`; ruido y glifo `[]{}<>0-9ｱ-ｺ` salen de un
hash de `(columna, serie % 12)` (periódico: el bucle enlaza). Las filas con `serie % 12` en `2..=6`
llevan la palabra: fila de fuente `4 - (serie % 12 - 2)` (se lee de abajo arriba como en el HTML), columna
centrada. Fuente de `L O M C D I N G` de 5 filas (anchos 4,4,5,4,4,3,5,4; hueco de 1 entre letras;
**51 columnas** en total). Solo se pinta la palabra si `n_cols >= 51` y `n_filas >= 7`.

- [ ] Pruebas: `fila_de_palabra` reproduce el HTML: las 51 columnas × 5 filas de `LOOMCODING` (comparar
  con una cuadrícula esperada escrita a mano en la prueba); en 80×24 y con `p` = 0..11 hay al menos
  un tick con las 5 filas de la palabra a la vez y ninguno con más de 5; la palabra baja exactamente
  una fila por `PASADA_TICKS`; periodo: el fotograma en `tick` y en `tick + 12·PASADA_TICKS` son
  idénticos; con `w = 56` (`n_cols = 52`) hay palabra y con `w = 50` no; nada fuera de `w×h`.

## Tarea 6 — Ensamblado y API del widget (`mod.rs`)

**Interfaz** (misma forma que `MatrixRain`, para que el enganche sea simétrico):
```rust
pub struct LoomBackground { /* reveal, hilos, tick, emitiendo, semilla */ }
impl LoomBackground {
    pub fn new() -> Self;                              // semilla fija
    pub fn with_seed(semilla: u64) -> Self;
    pub fn start(&mut self);                           // reveal.aparecer(nueva semilla)
    pub fn stop(&mut self);                            // reveal.desaparecer(nueva semilla)
    pub fn is_emitting(&self) -> bool;
    pub fn animating(&self) -> bool;                   // emitiendo || reveal.ocupa_pantalla()
    pub fn advance(&mut self);                         // tick += 1; reveal.paso(); hilos.avanzar
    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme);
    pub fn volcar(&mut self, w: u16, h: u16) -> String; // texto plano de la escena visible (e2e)
}
```
`render`: si `area` es `Oculto`, `!animating()` o el tema no tiene color, no pinta nada. Ajusta
`reveal.resize` y `hilos.ajustar` **solo si cambia el tamaño**. Pinta hilos, barra y tela **solo en
celdas con `reveal.visible`**, y solo en las celdas que pinta (las demás quedan intactas). Tonos: cabeza
con el token `Success`, dorado con `Warning`, azul con `Accent`; las colas y el `Tenue` oscurecen igual
que `matrix_rain` (`tail_base`/`tail_fade`). Cuando `stop` y el fundido acaba, `animating()` es falso.

- [ ] Pruebas: `start` + `APARECER_TICKS` ticks → `volcar(80,24)` contiene `CODE GENERATION`; tras
  `stop` + `DESAPARECER_TICKS` ticks → `animating() == false` y `volcar` vacío; `stop` a mitad de
  aparición no deja celdas fantasma; `render` sobre `TestBackend` no toca celdas ajenas a la escena
  (se rellena el buffer con un marcador y solo cambian celdas de la escena); 40×10 compacto (sin
  tela), 39×9 vacío, 300×200 vacío; `render` con `Oculto` no reserva; **presupuesto**: 100 fotogramas
  de `advance + render` a 200×60 en menos de 3 s en depuración (solo detecta patologías); el
  `volcar` a 80×24 con `with_seed(1)` coincide con el volcado de referencia versionado
  (`tests/fixtures/telar-80x24.txt`, generado una vez y revisado a ojo).

## Tarea 7 — Selección y enganche en la TUI (`tui/src/app/background.rs`, `prefs/background.rs`)

**Interfaz:**
```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)] pub enum Background { #[default] Matrix, Loomcoding, Off }
impl Background {
    pub const ALL: [Background; 3];
    pub const fn name(self) -> &'static str;               // "matrix" | "loomcoding" | "off"
    pub fn from_name(s: &str) -> Option<Background>;
}
// prefs/background.rs
pub fn load_background(path: &Path) -> Option<Background>;
pub fn save_background(path: &Path, b: Background) -> io::Result<()>;
// App
pub background: Background; pub loom_bg: LoomBackground;
impl App { pub fn set_background(&mut self, b: Background);  fn background_start(&mut self); fn background_stop(&mut self); }
```
Enganche (cambios mínimos, sin duplicar lógica): `events.rs:32` → `self.background_start()`;
`events.rs:54` y `:183` → `self.background_stop()`; `run/mod.rs:339` → `app.loom_bg.advance()` junto a
`matrix_rain.advance()`; `wants_clock` suma `app.loom_bg.animating()`; `render/mod.rs:56` pinta
`loom_bg` si `animating()`, y la lluvia como hasta ahora. `background_start` arranca solo el fondo
elegido (`Off` no arranca nada); `background_stop` detiene los dos. `set_background` con un turno en
curso detiene el anterior y arranca el nuevo, persiste con `save_background` y, si falla, deja el aviso
en `slash_feedback` sin impedir el cambio (como `set_mascot`). Carga al arrancar junto a las demás
preferencias (`load_*_prefs`).

- [ ] Pruebas: por defecto `Matrix` (el comportamiento actual no cambia: la prueba existente de la
  lluvia sigue verde); `TurnStarted` con `Loomcoding` pone `loom_bg.is_emitting()` y no la lluvia;
  `TurnEnded` los detiene; con `Off` ninguno arranca; cambiar de `Matrix` a `Loomcoding` en pleno
  turno detiene la lluvia y arranca el telar; el reloj (`wants_clock`) corre mientras el telar se
  desvanece y se para después; ida y vuelta de `ui.toml` (incluido valor desconocido → ignorado);
  un `ui.toml` sin `background` no cambia nada.

## Tarea 8 — Fila «Fondo» en el panel `/ui` y comando

**Cambios:** `UiRow::Background` (`name() == "background"`, `label() == "Fondo"`, ni submenú ni
interruptor) añadida a `UiRow::ALL` **después de `Effects`**; `ui_row_value` devuelve el nombre del
fondo; `ui_row_cycle` lo cicla (`Matrix → Loomcoding → Off`, y hacia atrás); `/ui background <valor>` lo
fija (acepta `matrix`, `loomcoding`, `off`; otro valor → uso con los tres valores). El aviso de fila
desconocida de `/ui` incluye `background`. La fila «Efectos» sigue resumiendo la lluvia y el logo.

- [ ] Pruebas: el panel muestra la fila y su valor en 80×24 y en 40 columnas sin cortar; `←/→` cicla
  y persiste; `/ui background loomcoding` y `/ui background nope` (rechazo, conserva el texto);
  `/ui` sin argumentos sigue abriendo el panel; las pruebas existentes de `ui_panel`/`ui_command`
  se actualizan **solo** en el nº de filas y en la lista de filas del aviso (di cuáles).

## Tarea 9 — Documentación de usuario y ayuda

Actualiza **en inglés y en español** (paridad, párrafos ≤ 4 líneas, guardia de `docs_usuario`):
`reference/configuration.md` (`background` en `ui.toml`: valores, defecto `matrix`, no requiere
reiniciar), `reference/commands.md` (`/ui background`, el aviso de filas), `guides/customize-the-interface.md`
(sección «Cambiar el fondo de carga» con ejemplo primero y la imagen), y la ayuda de la TUI (`help_topics`
si cita las filas). Añade la lista `background` a la prueba de sincronía de configuración
(`CAMPOS_DE_CONFIGURACION`). Imagen: captura SVG del volcado del telar con el generador de
`scripts/capturas/` (entrada nueva en `capturas.psd1`, `ORIGEN.md` regenerado, `-Check` en verde).
Fila en `docs/INDEX.md` (esta spec y este plan) y línea en `docs/CODE-MAP.md`.

- [ ] Pruebas: `cargo test -p loomcoding-cli --test docs_usuario` verde; `generar.ps1 -Check` verde.

## Tarea 10 — E2E en PTY y suavidad del composer

1. `crates/loomcoding-cli/tests/tui_pty_telar.rs` (imita `tui_pty_compresion.rs`): perfil temporal con
   `ui.toml` `background = "loomcoding"`, proveedor `wiremock` lento; captura (a) a mitad de la
   aparición, (b) con la escena completa, (c) a mitad de la desaparición y (d) ya vacía. Volcados en
   `docs/mockups/telar-e2e.md` (anonimizados, sin rutas).
2. **Suavidad:** una prueba que, con el telar a pleno rendimiento en 120×40, escribe 200 caracteres en el
   composer y comprueba que el texto escrito aparece íntegro y que ningún fotograma pierde el cursor
   (misma técnica que las pruebas existentes de composer).
3. Medición manual (se anota en el informe): coste de `render` del fondo a 80×24, 120×40 y 200×60 con
   `RUST_LOG`/temporizador en una versión de `release`.

- [ ] Pruebas: la prueba PTY pasa 3 veces seguidas; los cuatro chequeos del repo en verde.

## Orden y dependencias

`T1` ∥ `T2` → `T3`, `T4`, `T5` (cada una usa `Zonas` de T2) → `T6` (usa T1–T5) → `T7` → `T8` → `T9`
→ `T10`. T1 y T2 pueden ir en paralelo; T3, T4 y T5 también (tocan ficheros distintos).

## Revisión (casos que ninguna tarea cubre sola)

- Un turno que empieza y acaba en menos de 0,9 s: el fundido de aparición se interrumpe y la
  desaparición continúa sin saltos (T1 + T6).
- Redimensionar el terminal con el fondo activo: se reajusta una vez y no queda residuo (T6).
- Cambiar de tema durante el turno: los tonos se recalculan en el siguiente fotograma (T6).
- Un terminal de 40×10 con un overlay abierto: el overlay manda y no hay parpadeo (T10).
- `NO_COLOR=1`: no se pinta nada y el reloj no arranca por el fondo (T7).
