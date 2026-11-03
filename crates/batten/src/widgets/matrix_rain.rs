//! Lluvia Matrix transparente, con emisión progresiva y drenaje al detenerla.

use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

use crate::state::transcript::StyleToken;
use crate::widgets::theme::Theme;

/// Juego de glifos de la lluvia, con el ancho de celda que ocupa cada uno.
///
/// El render pinta una celda por columna, así que un juego de ancho 2 (emoji)
/// tiene que avanzar de dos en dos o rompe la rejilla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphSet {
    chars: Vec<char>,
    width: u8,
}

/// Error al construir un [`GlyphSet`] propio.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GlyphError {
    #[error("el juego de glifos está vacío")]
    Empty,
    #[error("los glifos mezclan anchos distintos ({expected} y {found})")]
    MixedWidth { expected: u8, found: u8 },
    #[error("el ancho {0} no está soportado (solo 1 o 2)")]
    UnsupportedWidth(u8),
}

impl GlyphSet {
    /// Construye un juego propio validando que todos los glifos tengan el
    /// mismo ancho de celda.
    pub fn custom(chars: &str) -> Result<Self, GlyphError> {
        let mut iter = chars.chars().filter(|c| !c.is_whitespace());
        let Some(first) = iter.next() else {
            return Err(GlyphError::Empty);
        };
        let width = glyph_width(first);
        if width == 0 || width > 2 {
            return Err(GlyphError::UnsupportedWidth(width));
        }
        for c in iter {
            let w = glyph_width(c);
            if w != width {
                return Err(GlyphError::MixedWidth {
                    expected: width,
                    found: w,
                });
            }
        }
        Ok(Self {
            chars: chars.chars().filter(|c| !c.is_whitespace()).collect(),
            width,
        })
    }

    pub fn katakana() -> Self {
        Self {
            chars: KATAKANA.to_vec(),
            width: 1,
        }
    }

    pub fn emoji() -> Self {
        Self {
            chars: EMOJI.to_vec(),
            width: 2,
        }
    }
    pub fn binary() -> Self {
        Self {
            chars: vec!['0', '1'],
            width: 1,
        }
    }

    pub fn ascii() -> Self {
        Self {
            chars: ASCII.to_vec(),
            width: 1,
        }
    }

    pub fn mixed() -> Self {
        Self {
            chars: MIXED.to_vec(),
            width: 1,
        }
    }

    /// Ancho de celda de cada glifo (1 o 2).
    pub fn width(&self) -> u8 {
        self.width
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Nombre del preset, para mostrarlo en la UI.
    pub fn preset_name(&self) -> &'static str {
        if *self == Self::katakana() {
            "katakana"
        } else if *self == Self::emoji() {
            "emoji"
        } else if *self == Self::binary() {
            "binario"
        } else if *self == Self::ascii() {
            "ascii"
        } else if *self == Self::mixed() {
            "mixto"
        } else {
            "propio"
        }
    }
}

impl Default for GlyphSet {
    fn default() -> Self {
        Self::katakana()
    }
}

fn glyph_width(c: char) -> u8 {
    c.width().unwrap_or(1).min(2) as u8
}

const KATAKANA: &[char] = &[
    'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ', 'ﾁ', 'ﾂ', 'ﾃ',
    'ﾄ', 'ﾅ', 'ﾆ', 'ﾇ', 'ﾈ', 'ﾉ', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ', 'ﾔ', 'ﾕ', 'ﾖ',
    'ﾗ', 'ﾘ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾜ', 'ﾝ', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', '.',
    '=', '*', '+', '-', '<', '>',
];

const EMOJI: &[char] = &[
    '💧', '🌿', '🍃', '✨', '💚', '🟢', '🔋', '🧪', '⚡', '🌱', '🫧', '🌊',
];

const ASCII: &[char] = &[
    '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', '0', '1', '2', '3',
    '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', '@', 'A', 'B', 'C', 'D', 'E', 'F',
    'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y',
    'Z',
];

const MIXED: &[char] = &[
    'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', '0', '1', '2', '3', '4', '5', '6', '7', '8',
    '9', ':', '.', '=', '*', '+', '-', '<', '>', '|', '/', '\\', '~',
];

/// Configuración de la lluvia. `Default` reproduce el comportamiento previo a
/// la configurabilidad, para no alterar el showcase.
#[derive(Clone, Debug, PartialEq)]
pub struct MatrixConfig {
    pub glyphs: GlyphSet,
    /// Velocidad base de caída (celdas por tick).
    pub speed: f32,
    /// Probabilidad de emisión por columna y tick, en tanto por ciento.
    pub density: u8,
    /// Longitud de la cola, en celdas.
    pub length: u16,
    /// Token del tema para la cabeza de la gota. La cola se deriva de él
    /// oscureciéndolo, así que un solo token basta para mantener el tono.
    pub color: StyleToken,
}

impl Default for MatrixConfig {
    fn default() -> Self {
        Self {
            glyphs: GlyphSet::default(),
            speed: 0.25,
            density: 3,
            length: 5,
            // Verde Matrix: el token `success` es el verde del tema.
            color: StyleToken::Success,
        }
    }
}

impl MatrixConfig {
    /// Acota los valores a rangos seguros. Nada sin límite llega al render.
    pub fn clamped(mut self) -> Self {
        self.speed = self.speed.clamp(0.05, 4.0);
        self.density = self.density.min(100);
        self.length = self.length.clamp(1, 200);
        if self.glyphs.is_empty() {
            self.glyphs = GlyphSet::default();
        }
        self
    }
}

#[derive(Clone, Copy)]
struct Drop {
    head: f32,
    speed: f32,
    length: u16,
    seed: u64,
}

/// Porcentaje de columnas sembradas al arrancar un turno. Alto a propósito:
/// la lluvia avisa de que el modelo trabaja, así que debe verse al instante.
const SEED_PERCENT: u64 = 40;

/// Stateful background. Call `advance` on the UI clock and paint it before the
/// foreground widgets. `stop` stops emission; existing drops drain to the edge.
pub struct MatrixRain {
    columns: Vec<Option<Drop>>,
    width: u16,
    height: u16,
    emitting: bool,
    /// Siembra pendiente: se resuelve en el primer `advance` con dimensiones
    /// conocidas, porque `start` puede llegar antes del primer pintado.
    seed_pending: bool,
    random: u64,
    frame: u64,
    config: MatrixConfig,
}

impl Default for MatrixRain {
    fn default() -> Self {
        Self::with_config(MatrixConfig::default())
    }
}

impl MatrixRain {
    pub fn with_config(config: MatrixConfig) -> Self {
        Self {
            columns: Vec::new(),
            width: 0,
            height: 0,
            emitting: false,
            seed_pending: false,
            random: 0x4d41_5452_4958,
            frame: 0,
            config: config.clamped(),
        }
    }

    pub fn config(&self) -> &MatrixConfig {
        &self.config
    }

    /// Cambia la configuración en caliente. Si cambia el ancho de los glifos,
    /// se descartan las gotas vivas para no mezclar rejillas.
    pub fn set_config(&mut self, config: MatrixConfig) {
        let config = config.clamped();
        if config.glyphs.width() != self.config.glyphs.width() {
            self.columns.clear();
            // `render` solo redimensiona cuando cambia `width`; si no lo
            // invalidamos, la rejilla se queda vacía para siempre y la lluvia
            // no vuelve a emitir hasta el siguiente resize del terminal.
            self.width = 0;
            self.height = 0;
        }
        self.config = config;
    }

    pub fn start(&mut self) {
        self.emitting = true;
        // Siembra la pantalla para que el efecto se vea al instante y no
        // dependa de esperar a que una gota caiga desde fuera del área.
        if !self.columns.iter().any(Option::is_some) {
            self.seed_pending = true;
        }
    }

    pub fn stop(&mut self) {
        self.emitting = false;
    }

    pub fn animating(&self) -> bool {
        self.emitting || self.columns.iter().any(Option::is_some)
    }

    /// Separación entre columnas usables: 1 para glifos estrechos, 2 para
    /// glifos anchos (cada gota ocupa dos celdas).
    fn column_step(&self) -> usize {
        usize::from(self.config.glyphs.width())
    }

    /// Advances one 50 ms UI tick. Stopped drops keep moving until their tails
    /// pass the bottom, then the background releases the screen.
    pub fn advance(&mut self) {
        if !self.animating() {
            return;
        }
        self.frame = self.frame.wrapping_add(1);
        for slot in &mut self.columns {
            if let Some(mut drop) = *slot {
                drop.head += drop.speed;
                if drop.head - f32::from(drop.length) > f32::from(self.height) {
                    *slot = None;
                } else {
                    *slot = Some(drop);
                }
            }
        }
        if self.emitting {
            let density = u64::from(self.config.density);
            let base_speed = self.config.speed;
            let base_length = self.config.length;
            let step = self.column_step();
            for column in (0..self.columns.len()).step_by(step) {
                if self.columns[column].is_none() && self.random(100) < density {
                    let length = base_length + self.random(12) as u16;
                    let delay = self.random(u64::from(self.height.max(1))) as f32;
                    self.columns[column] = Some(Drop {
                        head: -(f32::from(length) + delay),
                        speed: base_speed + self.random(65) as f32 / 100.0,
                        length,
                        seed: self.random(u64::MAX),
                    });
                }
            }
        }
    }

    /// Siembra la pantalla al arrancar un turno: reparte gotas ya empezadas por
    /// toda la altura, en vez de esperar a que caigan desde fuera del área.
    fn seed(&mut self) {
        self.seed_pending = false;
        if self.columns.is_empty() || self.height == 0 {
            return;
        }
        let step = self.column_step().max(1);
        let base_speed = self.config.speed;
        let base_length = self.config.length;
        let height = f32::from(self.height);
        // `density` manda: con 0 no se emite nada, tampoco al sembrar.
        let density = u64::from(self.config.density);
        if density == 0 {
            return;
        }
        let seed_percent = SEED_PERCENT.max(density);
        for column in (0..self.columns.len()).step_by(step) {
            if self.random(100) >= seed_percent {
                continue;
            }
            let length = base_length + self.random(12) as u16;
            // Reparte la cabeza por toda la altura: algunas gotas ya dentro de
            // la pantalla y otras entrando, sin bloquear el efecto al arrancar.
            let head = self.random(u64::from(self.height.max(1))) as f32;
            self.columns[column] = Some(Drop {
                head,
                speed: base_speed + self.random(65) as f32 / 100.0,
                length,
                seed: self.random(u64::MAX),
            });
            debug_assert!(head <= height);
        }
    }

    /// Paints only the falling glyphs. Draw foreground widgets afterwards;
    /// leaving empty cells untouched avoids a full-screen flash on start/stop.
    ///
    /// `theme` resuelve el token de color de la config; la cola se deriva de
    /// ese mismo color oscureciéndolo, para conservar el tono.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if area.is_empty() || !self.animating() {
            return;
        }
        if self.width != area.width {
            self.columns.resize(area.width as usize, None);
            self.width = area.width;
        }
        self.height = area.height;
        // Siembra aquí y no en `advance`: hasta el primer pintado no se conoce
        // el ancho, así que `columns` estaría vacío y el sembrado se perdería.
        if self.seed_pending {
            self.seed();
        }
        let glyphs = &self.config.glyphs;
        if glyphs.is_empty() {
            return;
        }
        let glyph_width = u16::from(glyphs.width());
        let head_color = theme.style(self.config.color).fg.unwrap_or(Color::Green);
        let tail = tail_base(head_color);
        let buffer = frame.buffer_mut();
        for (column, drop) in self.columns.iter().enumerate() {
            let Some(drop) = drop else { continue };
            let x = area.x + column as u16;
            // Un glifo de ancho 2 necesita una celda contigua libre.
            if x + glyph_width > area.right() {
                continue;
            }
            let top = (drop.head - f32::from(drop.length)).ceil().max(0.0) as u16;
            let bottom = drop.head.floor().max(0.0).min(f32::from(area.height - 1)) as u16;
            if top > bottom {
                continue;
            }
            for row in top..=bottom {
                let distance = drop.head - f32::from(row);
                let intensity = (1.0 - distance / f32::from(drop.length)).clamp(0.0, 1.0);
                let glyph_index = (glyph_hash(drop.seed, row, self.frame_phase()) as usize)
                    % glyphs.chars().len();
                let glyph = glyphs.chars()[glyph_index].to_string();
                let color = if distance < 1.0 {
                    head_color
                } else {
                    tail_fade(tail, intensity)
                };
                let cell = &mut buffer[(x, area.y + row)];
                cell.set_symbol(&glyph);
                cell.set_fg(color);
                // La celda contigua de un glifo ancho queda vacía para que el
                // terminal no desplace el resto de la fila.
                if glyph_width == 2 {
                    let next = &mut buffer[(x + 1, area.y + row)];
                    next.set_symbol(" ");
                }
            }
        }
    }

    fn frame_phase(&self) -> u64 {
        self.frame
    }

    fn random(&mut self, limit: u64) -> u64 {
        self.random = self
            .random
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        if limit == 0 {
            0
        } else {
            self.random % limit
        }
    }
}

/// Deriva el color base de la cola a partir del de la cabeza, oscureciéndolo
/// para conservar el tono. Así un solo token del tema define toda la gota.
fn tail_base(head: Color) -> Color {
    match head {
        Color::Rgb(r, g, b) => Color::Rgb(
            (f32::from(r) * 0.02) as u8,
            (f32::from(g) * 0.10) as u8,
            (f32::from(b) * 0.04) as u8,
        ),
        other => other,
    }
}

/// Aclara el color de cola según la intensidad, conservando su tono.
fn tail_fade(tail: Color, intensity: f32) -> Color {
    match tail {
        Color::Rgb(r, g, b) => Color::Rgb(
            r,
            (f32::from(g) + intensity * 62.0).min(255.0) as u8,
            (f32::from(b) + intensity * 12.0).min(255.0) as u8,
        ),
        other => other,
    }
}

fn glyph_hash(seed: u64, row: u16, phase: u64) -> u64 {
    seed.wrapping_add(u64::from(row).wrapping_mul(31))
        .wrapping_add(phase / 4)
        .wrapping_mul(2_654_435_761)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn draw(rain: &mut MatrixRain, w: u16, h: u16) -> Terminal<TestBackend> {
        let theme = Theme::default_dark();
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| rain.render(f, f.area(), &theme)).unwrap();
        term
    }

    #[test]
    fn default_config_matches_previous_behaviour() {
        let cfg = MatrixConfig::default();
        assert_eq!(cfg.glyphs, GlyphSet::katakana());
        assert_eq!(cfg.glyphs.width(), 1);
        assert_eq!(cfg.speed, 0.25);
        assert_eq!(cfg.density, 3);
        assert_eq!(cfg.length, 5);
        // Verde Matrix: el token `success` del tema.
        assert_eq!(cfg.color, StyleToken::Success);
    }

    #[test]
    fn tail_is_derived_from_head_token() {
        let head = Color::Rgb(125, 185, 140);
        let tail = tail_base(head);
        // La cola conserva el tono pero es mucho más oscura.
        assert!(matches!(tail, Color::Rgb(_, _, _)));
        if let Color::Rgb(r, g, b) = tail {
            assert!(
                r < 10 && g < 20 && b < 10,
                "cola no oscurecida: {r},{g},{b}"
            );
        }
    }

    #[test]
    fn custom_glyph_set_rejects_mixed_width() {
        // 'a' es ancho 1, '💧' es ancho 2.
        let err = GlyphSet::custom("a💧").unwrap_err();
        assert!(matches!(err, GlyphError::MixedWidth { .. }));
    }

    #[test]
    fn custom_glyph_set_rejects_empty() {
        assert_eq!(GlyphSet::custom("   ").unwrap_err(), GlyphError::Empty);
    }

    #[test]
    fn custom_glyph_set_accepts_uniform_width() {
        let set = GlyphSet::custom("abc").unwrap();
        assert_eq!(set.width(), 1);
        assert_eq!(set.chars(), &['a', 'b', 'c']);
        assert_eq!(set.preset_name(), "propio");
    }

    #[test]
    fn emoji_preset_is_wide() {
        assert_eq!(GlyphSet::emoji().width(), 2);
        assert_eq!(GlyphSet::emoji().preset_name(), "emoji");
    }

    /// Cada preset debe declarar el ancho que sus glifos realmente ocupan.
    /// `unicode-width` da ancho 1 a algunos emoji sin selector de variación
    /// (p. ej. U+1F327), así que esto no se puede dar por supuesto.
    #[test]
    fn every_preset_declares_its_real_width() {
        for set in [
            GlyphSet::katakana(),
            GlyphSet::emoji(),
            GlyphSet::binary(),
            GlyphSet::ascii(),
            GlyphSet::mixed(),
        ] {
            for &c in set.chars() {
                assert_eq!(
                    glyph_width(c),
                    set.width(),
                    "el glifo {c:?} (U+{:04X}) mide {} pero el preset {} declara {}",
                    c as u32,
                    glyph_width(c),
                    set.preset_name(),
                    set.width()
                );
            }
        }
    }

    #[test]
    fn wide_glyphs_do_not_overlap() {
        let mut rain = MatrixRain::with_config(MatrixConfig {
            glyphs: GlyphSet::emoji(),
            density: 100,
            ..MatrixConfig::default()
        });
        rain.start();
        for _ in 0..40 {
            rain.advance();
        }
        let term = draw(&mut rain, 20, 10);
        let buffer = term.backend().buffer();
        // Ninguna celda impar debe tener glifo propio: son la mitad derecha
        // de un emoji y deben quedar en blanco.
        for y in 0..10 {
            for x in (1..20).step_by(2) {
                let symbol = buffer[(x, y)].symbol();
                assert!(
                    symbol == " " || symbol.is_empty(),
                    "celda impar ({x},{y}) tiene glifo {symbol:?}"
                );
            }
        }
    }

    #[test]
    fn start_seeds_visible_drops_immediately() {
        // Regresión: al arrancar un turno la lluvia nace fuera de pantalla y
        // tarda segundos en asomar, así que no se veía como aviso de trabajo.
        let mut rain = MatrixRain::with_config(MatrixConfig {
            density: 100,
            ..MatrixConfig::default()
        });
        rain.start();
        // Un par de ticks basta: el sembrado debe llenar la pantalla ya.
        for _ in 0..2 {
            rain.advance();
        }
        let term = draw(&mut rain, 40, 20);
        let buffer = term.backend().buffer();
        let visible = (0..20)
            .flat_map(|y| (0..40).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let symbol = buffer[(x, y)].symbol();
                !symbol.trim().is_empty() && symbol != " "
            })
            .count();
        assert!(
            visible > 0,
            "tras start() + 2 ticks no hay ningún glifo visible"
        );
    }

    #[test]
    fn density_zero_emits_nothing() {
        let mut rain = MatrixRain::with_config(MatrixConfig {
            density: 0,
            ..MatrixConfig::default()
        });
        rain.start();
        for _ in 0..50 {
            rain.advance();
        }
        assert!(!rain.columns.iter().any(Option::is_some));
    }

    #[test]
    fn speed_and_length_are_clamped() {
        let cfg = MatrixConfig {
            speed: 999.0,
            length: 0,
            density: 250,
            ..MatrixConfig::default()
        }
        .clamped();
        assert_eq!(cfg.speed, 4.0);
        assert_eq!(cfg.length, 1);
        assert_eq!(cfg.density, 100);
    }

    #[test]
    fn empty_glyph_set_falls_back_to_default() {
        let cfg = MatrixConfig {
            glyphs: GlyphSet {
                chars: Vec::new(),
                width: 1,
            },
            ..MatrixConfig::default()
        }
        .clamped();
        assert_eq!(cfg.glyphs, GlyphSet::katakana());
    }

    #[test]
    fn changing_glyph_width_clears_live_drops() {
        let mut rain = MatrixRain::with_config(MatrixConfig {
            density: 100,
            ..MatrixConfig::default()
        });
        rain.start();
        // `columns` se dimensiona en el primer render, así que hay que pintar
        // antes de avanzar para que existan gotas.
        draw(&mut rain, 20, 10);
        for _ in 0..20 {
            rain.advance();
        }
        assert!(rain.columns.iter().any(Option::is_some));
        rain.set_config(MatrixConfig {
            glyphs: GlyphSet::emoji(),
            ..MatrixConfig::default()
        });
        assert!(!rain.columns.iter().any(Option::is_some));
    }

    /// Regresión: al cambiar a glifos de otro ancho `set_config` vaciaba
    /// `columns` pero dejaba `width` intacto, así que `render` no volvía a
    /// redimensionar la rejilla y la lluvia no resucitaba nunca.
    #[test]
    fn changing_glyph_width_keeps_rain_alive() {
        let mut rain = MatrixRain::with_config(MatrixConfig {
            density: 100,
            ..MatrixConfig::default()
        });
        rain.start();
        draw(&mut rain, 20, 10);
        for _ in 0..5 {
            rain.advance();
        }
        assert!(rain.columns.iter().any(Option::is_some));

        rain.set_config(MatrixConfig {
            glyphs: GlyphSet::emoji(),
            density: 100,
            ..MatrixConfig::default()
        });
        assert!(
            !rain.columns.iter().any(Option::is_some),
            "las gotas de la rejilla anterior deben descartarse"
        );

        // El siguiente pintado debe volver a dimensionar la rejilla y emitir.
        draw(&mut rain, 20, 10);
        for _ in 0..2 {
            rain.advance();
        }
        draw(&mut rain, 20, 10);
        assert!(
            rain.columns.iter().any(Option::is_some),
            "la lluvia no revive tras cambiar el ancho de los glifos"
        );
    }

    #[test]
    fn stopped_rain_drains_and_releases() {
        let mut rain = MatrixRain::with_config(MatrixConfig {
            density: 100,
            speed: 4.0,
            ..MatrixConfig::default()
        });
        rain.start();
        for _ in 0..10 {
            rain.advance();
        }
        rain.stop();
        for _ in 0..200 {
            rain.advance();
        }
        assert!(!rain.animating());
    }
}
