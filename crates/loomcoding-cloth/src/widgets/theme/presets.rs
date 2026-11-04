//! Presets del tema y su tabla de colores.

use super::*;

/// Paletas de referencia disponibles en Cloth y su showcase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreset {
    Cloth,
    Martty,
    Mindfork,
    Pawan,
    Crush,
    MastraCode,
    Loomcoding,
}

impl ThemePreset {
    pub const ALL: [Self; 7] = [
        Self::Cloth,
        Self::Martty,
        Self::Mindfork,
        Self::Pawan,
        Self::Crush,
        Self::MastraCode,
        Self::Loomcoding,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Cloth => "Cloth",
            Self::Martty => "Martty",
            Self::Mindfork => "Mindfork",
            Self::Pawan => "Pawan",
            Self::Crush => "Crush · Charmtone",
            Self::MastraCode => "MastraCode",
            Self::Loomcoding => "Loomcoding",
        }
    }
}

impl Theme {
    /// Paleta de terminal inspirada en un proyecto de referencia.
    /// Los componentes y el layout siguen siendo los de Cloth.
    pub fn preset(preset: ThemePreset) -> Self {
        use ThemePreset::*;

        let mut theme = Self::default_dark();
        let (bg, fg, muted, accent, user, assistant, warning, error, success, border) = match preset
        {
            Cloth => return theme,
            // Martty's DEFAULT_DARK DeepSeek token map.
            Martty => (
                Color::Rgb(15, 17, 21),
                Color::Rgb(237, 243, 254),
                Color::Rgb(151, 157, 166),
                Color::Rgb(86, 134, 254),
                Color::Rgb(103, 158, 254),
                Color::Rgb(78, 209, 126),
                Color::Rgb(247, 173, 49),
                Color::Rgb(242, 90, 90),
                Color::Rgb(78, 209, 126),
                Color::Rgb(44, 44, 46),
            ),
            // Mindfork's dark palette roles over CANVAS_DARK.
            Mindfork => (
                Color::Rgb(15, 17, 21),
                Color::Rgb(201, 204, 210),
                Color::Rgb(133, 139, 146),
                Color::Rgb(220, 175, 97),
                Color::Rgb(121, 169, 219),
                Color::Rgb(111, 192, 130),
                Color::Rgb(220, 175, 97),
                Color::Rgb(223, 105, 92),
                Color::Rgb(111, 192, 130),
                Color::Rgb(54, 58, 66),
            ),
            // Pawan's built-in DEFAULT Ratatui palette.
            Pawan => (
                Color::Rgb(0, 0, 0),
                Color::Rgb(255, 255, 255),
                Color::Rgb(136, 136, 136),
                Color::Rgb(0, 204, 204),
                Color::Rgb(0, 204, 204),
                Color::Rgb(230, 230, 230),
                Color::Rgb(255, 184, 108),
                Color::Rgb(255, 85, 85),
                Color::Rgb(80, 250, 123),
                Color::Rgb(68, 68, 68),
            ),
            // Crush's Charmtone Pantera tokens from charmtoneOpts.
            Crush => (
                Color::Rgb(32, 31, 38),
                Color::Rgb(241, 239, 239),
                Color::Rgb(191, 188, 200),
                Color::Rgb(107, 80, 255),
                Color::Rgb(107, 80, 255),
                Color::Rgb(241, 239, 239),
                Color::Rgb(245, 239, 52),
                Color::Rgb(235, 66, 104),
                Color::Rgb(0, 255, 178),
                Color::Rgb(45, 44, 53),
            ),
            // MastraCode's darkSurface and darkTheme source-map values.
            MastraCode => (
                Color::Rgb(2, 2, 2),
                Color::Rgb(250, 250, 250),
                Color::Rgb(140, 140, 148),
                Color::Rgb(98, 246, 157),
                Color::Rgb(250, 250, 250),
                Color::Rgb(217, 217, 220),
                Color::Rgb(245, 158, 11),
                Color::Rgb(239, 68, 68),
                Color::Rgb(98, 246, 157),
                Color::Rgb(63, 63, 70),
            ),
            // Paleta propia de Loomcoding. El acento (194, 90, 39) y el borde (92, 99, 112)
            // salen del logo (`widgets/logo.rs`); el resto de colores es decisión propia.
            Loomcoding => (
                Color::Rgb(20, 21, 26),
                Color::Rgb(222, 224, 228),
                Color::Rgb(139, 145, 156),
                Color::Rgb(194, 90, 39),
                Color::Rgb(226, 132, 78),
                Color::Rgb(222, 224, 228),
                Color::Rgb(229, 192, 123),
                Color::Rgb(224, 108, 117),
                Color::Rgb(152, 195, 121),
                Color::Rgb(92, 99, 112),
            ),
        };

        theme.styles.default = Style::default().fg(fg).bg(bg);
        theme.styles.muted = Style::default().fg(muted);
        theme.styles.dim = Style::default().fg(muted);
        theme.styles.heading = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        theme.styles.user = Style::default().fg(user);
        theme.styles.assistant = Style::default().fg(assistant);
        theme.styles.system = Style::default().fg(muted);
        theme.styles.error = Style::default().fg(error);
        theme.styles.warning = Style::default().fg(warning);
        theme.styles.success = Style::default().fg(success);
        theme.styles.tool_name = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        theme.styles.tool_pending = Style::default().fg(muted);
        theme.styles.tool_running = Style::default().fg(accent);
        theme.styles.tool_success = Style::default().fg(success);
        theme.styles.tool_error = Style::default().fg(error);
        theme.styles.tool_canceled = Style::default().fg(muted);
        theme.styles.todo_pending = Style::default().fg(muted);
        theme.styles.todo_in_progress = Style::default().fg(accent);
        theme.styles.todo_completed = Style::default().fg(success);
        theme.styles.accent = Style::default().fg(accent);
        theme.styles.accent_from = Style::default().fg(accent);
        theme.styles.accent_to = Style::default().fg(user);
        theme.styles.border = Style::default().fg(border);
        theme.styles.scrollbar = Style::default().fg(border);
        theme.styles.logo_field = Style::default().fg(muted);
        theme.styles.selection = Style::default().bg(border);
        theme.styles.diff_insert = Style::default().fg(success);
        theme.styles.diff_delete = Style::default().fg(error);
        theme.styles.diff_hunk = Style::default().fg(accent);
        theme.styles.code = Style::default().fg(accent);
        // Tokens de pieza: por defecto derivados de los existentes del preset.
        // Deben ir después de `accent`, `default` y `dim`, que se fijan arriba.
        theme.styles.tool_path = theme.styles.accent;
        theme.styles.tool_command = theme.styles.default;
        theme.styles.tool_count = theme.styles.dim;
        theme.glyphs.tool_pending = '●';
        if preset == Crush {
            theme.look = Look::crush();
            // Crush rompe el derivado de la ruta con su verde menta (`success`,
            // 0,255,178), que no compite con el violeta de `accent`. El recuento
            // conserva el `dim` derivado y el comando, el `default`.
            theme.styles.tool_path = theme.styles.success;
        }
        theme
    }

    pub fn default_dark() -> Self {
        Self {
            styles: Styles::default(),
            glyphs: Glyphs::default(),
            extensions: BTreeMap::new(),
            depth: ColorDepth::TrueColor,
            hyperlinks: true,
            look: Look::classic(),
            matrix: MatrixConfig::default(),
        }
    }

    /// Tema fijo y determinista para tests.
    pub fn test() -> Self {
        Self::default_dark()
    }
}
