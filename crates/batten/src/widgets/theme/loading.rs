//! Carga del tema desde TOML: colores, secciones, adaptación a la profundidad y errores.

use super::*;

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("invalid theme: {0}")]
    Parse(String),
    #[error("invalid color: {0}")]
    Color(String),
}

/// Forma deserializable del `theme.toml`: un mapa plano token → valor de
/// color, más la sección opcional de efectos.
#[derive(Debug, Deserialize)]
pub(super) struct ThemeFile {
    #[serde(default)]
    pub(super) styles: BTreeMap<String, String>,
    #[serde(default)]
    pub(super) effects: Option<EffectsFile>,
}

/// Sección `[effects]` del tema.
#[derive(Debug, Default, Deserialize)]
pub(super) struct EffectsFile {
    #[serde(default)]
    pub(super) matrix: Option<MatrixFile>,
}

/// Sección `[effects.matrix]` del tema. Todos los campos son opcionales: lo que
/// falte cae al `Default` de [`MatrixConfig`].
#[derive(Debug, Default, Deserialize)]
pub(super) struct MatrixFile {
    pub(super) glyphs: Option<String>,
    pub(super) speed: Option<f32>,
    pub(super) density: Option<u8>,
    pub(super) length: Option<u16>,
    pub(super) color: Option<String>,
}

pub(super) fn parse_color(s: &str) -> Result<Color, ThemeError> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let parse =
            |i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ThemeError::Color(s.into()));
        return Ok(Color::Rgb(parse(0)?, parse(2)?, parse(4)?));
    }
    if let Some(idx) = s.strip_prefix("indexed:") {
        return idx
            .parse::<u8>()
            .map(Color::Indexed)
            .map_err(|_| ThemeError::Color(s.into()));
    }
    let c = match s.to_ascii_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "white" => Color::White,
        _ => return Err(ThemeError::Color(s.into())),
    };
    Ok(c)
}

#[cfg(feature = "termprofile")]
pub(super) fn adapt_lower(depth: ColorDepth, color: Color) -> Option<Color> {
    let profile = match depth {
        ColorDepth::Ansi256 => termprofile::TermProfile::Ansi256,
        _ => termprofile::TermProfile::Ansi16,
    };
    profile.adapt_color(color)
}

/// Sin `termprofile` no hay tablas de conversión: se deja el color.
#[cfg(not(feature = "termprofile"))]
pub(super) fn adapt_lower(_depth: ColorDepth, color: Color) -> Option<Color> {
    Some(color)
}

/// Convierte la sección `[effects.matrix]` en una [`MatrixConfig`], cayendo al
/// `Default` en lo que falte. Un glifo o un token inválido es un error de tema,
/// no un fallo silencioso.
pub(super) fn matrix_config_from_file(file: MatrixFile) -> Result<MatrixConfig, ThemeError> {
    let mut config = MatrixConfig::default();
    if let Some(name) = file.glyphs {
        config.glyphs = match name.trim().to_ascii_lowercase().as_str() {
            "katakana" => GlyphSet::katakana(),
            "emoji" => GlyphSet::emoji(),
            "binario" | "binary" => GlyphSet::binary(),
            "ascii" => GlyphSet::ascii(),
            "mixto" | "mixed" => GlyphSet::mixed(),
            other => GlyphSet::custom(other).map_err(|e| ThemeError::Parse(e.to_string()))?,
        };
    }
    if let Some(speed) = file.speed {
        config.speed = speed;
    }
    if let Some(density) = file.density {
        config.density = density;
    }
    if let Some(length) = file.length {
        config.length = length;
    }
    if let Some(name) = file.color {
        config.color = StyleToken::from_key(&name)
            .ok_or_else(|| ThemeError::Parse(format!("token de color desconocido: {name}")))?;
    }
    Ok(config.clamped())
}

pub(super) fn token_to_key(t: StyleToken) -> &'static str {
    t.key()
}

impl Theme {
    /// Adapta un color generado al pintar a la profundidad del tema.
    /// `None` = sin color (usar el del terminal).
    pub fn adapt(&self, color: Color) -> Option<Color> {
        match self.depth {
            ColorDepth::TrueColor => Some(color),
            ColorDepth::NoColor => None,
            ColorDepth::Ansi256 | ColorDepth::Ansi16 => adapt_lower(self.depth, color),
        }
    }

    /// Resuelve un token semántico a su estilo. Si el nombre no coincide con
    /// un campo de `Styles`, devuelve el estilo de extensión correspondiente
    /// o un estilo por defecto.
    pub fn style(&self, token: StyleToken) -> Style {
        self.set_named_style(token).unwrap_or_else(|| {
            let key = String::from(token_to_key(token));
            self.extensions.get(&key).copied().unwrap_or_default()
        })
    }

    /// Construye un tema sobrescribiendo campos nombrados en `[styles]`.
    /// Los nombres no reconocidos se almacenan como extensiones.
    pub fn from_toml(src: &str) -> Result<Self, ThemeError> {
        let file: ThemeFile = toml::from_str(src).map_err(|e| ThemeError::Parse(e.to_string()))?;
        let mut theme = Self::default_dark();
        for (name, value) in file.styles {
            let color = parse_color(&value)?;
            let style = Style::default().fg(color);
            if !theme.apply_override(&name, style) {
                theme.extensions.insert(name.clone(), style);
            }
        }
        if let Some(matrix) = file.effects.and_then(|e| e.matrix) {
            theme.matrix = matrix_config_from_file(matrix)?;
        }
        Ok(theme)
    }

    pub(super) fn set_named_style(&self, token: StyleToken) -> Option<Style> {
        let s = &self.styles;
        Some(match token {
            StyleToken::Default => s.default,
            StyleToken::Muted => s.muted,
            StyleToken::Dim => s.dim,
            StyleToken::Heading => s.heading,
            StyleToken::User => s.user,
            StyleToken::Assistant => s.assistant,
            StyleToken::System => s.system,
            StyleToken::Error => s.error,
            StyleToken::Warning => s.warning,
            StyleToken::Success => s.success,
            StyleToken::ToolName => s.tool_name,
            StyleToken::ToolPath => s.tool_path,
            StyleToken::ToolCommand => s.tool_command,
            StyleToken::ToolCount => s.tool_count,
            StyleToken::ToolPending => s.tool_pending,
            StyleToken::ToolRunning => s.tool_running,
            StyleToken::ToolSuccess => s.tool_success,
            StyleToken::ToolError => s.tool_error,
            StyleToken::ToolCanceled => s.tool_canceled,
            StyleToken::TodoPending => s.todo_pending,
            StyleToken::TodoInProgress => s.todo_in_progress,
            StyleToken::TodoCompleted => s.todo_completed,
            StyleToken::Accent => s.accent,
            StyleToken::AccentFrom => s.accent_from,
            StyleToken::AccentTo => s.accent_to,
            StyleToken::LogoField => s.logo_field,
            StyleToken::Scrollbar => s.scrollbar,
            StyleToken::Border => s.border,
            StyleToken::DiffInsert => s.diff_insert,
            StyleToken::DiffDelete => s.diff_delete,
            StyleToken::DiffHunk => s.diff_hunk,
            StyleToken::Selection => s.selection,
            StyleToken::Code => s.code,
            StyleToken::Strong => s.strong,
            StyleToken::Emphasis => s.emphasis,
        })
    }

    pub(super) fn apply_override(&mut self, name: &str, style: Style) -> bool {
        let s = &mut self.styles;
        match name {
            "default" => s.default = style,
            "muted" => s.muted = style,
            "dim" => s.dim = style,
            "heading" => s.heading = style,
            "user" => s.user = style,
            "assistant" => s.assistant = style,
            "system" => s.system = style,
            "error" => s.error = style,
            "warning" => s.warning = style,
            "success" => s.success = style,
            "tool_name" => s.tool_name = style,
            "tool_path" => s.tool_path = style,
            "tool_command" => s.tool_command = style,
            "tool_count" => s.tool_count = style,
            "tool_pending" => s.tool_pending = style,
            "tool_running" => s.tool_running = style,
            "tool_success" => s.tool_success = style,
            "tool_error" => s.tool_error = style,
            "tool_canceled" => s.tool_canceled = style,
            "todo_pending" => s.todo_pending = style,
            "todo_in_progress" => s.todo_in_progress = style,
            "todo_completed" => s.todo_completed = style,
            "accent" => s.accent = style,
            "accent_from" => s.accent_from = style,
            "accent_to" => s.accent_to = style,
            "logo_field" => s.logo_field = style,
            "scrollbar" => s.scrollbar = style,
            "border" => s.border = style,
            "diff_insert" => s.diff_insert = style,
            "diff_delete" => s.diff_delete = style,
            "diff_hunk" => s.diff_hunk = style,
            "selection" => s.selection = Style::default().bg(style.fg.unwrap_or(Color::Reset)),
            "code" => s.code = style,
            "strong" => s.strong = style,
            "emphasis" => s.emphasis = style,
            _ => return false,
        }
        true
    }
}
