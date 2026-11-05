//! El look: profundidad de color y estilos de composer, herramientas, layout y chrome.

/// Colores que admite el terminal. Lo fija `Theme::for_profile`; los
/// widgets lo usan para los colores que generan al pintar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorDepth {
    #[default]
    TrueColor,
    Ansi256,
    Ansi16,
    NoColor,
}

/// Variante de presentación del composer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerStyle {
    /// Caja redondeada con título y pie (comportamiento histórico).
    Classic,
    /// Sin marco, prompt `> ` y `::: ` por línea, con wrap y scroll vertical.
    Crush,
}

/// Dónde se presentan las llamadas a herramientas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStyle {
    /// Tarjeta fija junto al composer (solo la última).
    Pinned,
    /// Entradas dentro del transcript, en orden cronológico.
    Inline,
}

/// Disposición de la pantalla: áreas laterales y cabecera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutStyle {
    /// Sidebar izquierdo y transcript a la derecha (disposición histórica).
    Workspace,
    /// Transcript a la izquierda y un rail de contexto a la derecha.
    ContextRail,
    /// Solo el transcript, a todo el ancho; sin rail ni panel lateral.
    Minimal,
    /// Solo el transcript a todo el ancho; las tarjetas se añadirán en otra tarea.
    Cards,
}

impl LayoutStyle {
    /// Nombre estable con el que se escribe la disposición en `ui.toml` y en `/layout`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::ContextRail => "rail",
            Self::Minimal => "minimal",
            Self::Cards => "cards",
        }
    }

    /// Resuelve un nombre producido por `name`; cualquier otro texto produce `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::Workspace,
            Self::ContextRail,
            Self::Minimal,
            Self::Cards,
        ]
        .into_iter()
        .find(|layout| layout.name() == name)
    }

    /// Disposición siguiente en la rotación de `/layout`. Cabe señalar que
    /// `Workspace` no forma parte del ciclo: pasa directamente a `ContextRail`.
    pub const fn next(self) -> Self {
        match self {
            Self::Workspace | Self::Cards => Self::ContextRail,
            Self::ContextRail => Self::Minimal,
            Self::Minimal => Self::Cards,
        }
    }
}

/// Marco visual de los paneles y de la barra de estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeStyle {
    /// Paneles con borde y cabecera de dos filas (comportamiento histórico).
    Panels,
    /// Sin cajas: cabecera de una línea y separadores ligeros.
    Flat,
}

/// Presentación de cada elemento; un preset es paleta más `Look`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    pub composer: ComposerStyle,
    pub tool: ToolStyle,
    pub layout: LayoutStyle,
    pub chrome: ChromeStyle,
}

impl Look {
    pub const fn classic() -> Self {
        Self {
            composer: ComposerStyle::Classic,
            tool: ToolStyle::Pinned,
            layout: LayoutStyle::Workspace,
            chrome: ChromeStyle::Panels,
        }
    }

    pub const fn crush() -> Self {
        Self {
            composer: ComposerStyle::Crush,
            tool: ToolStyle::Inline,
            layout: LayoutStyle::ContextRail,
            chrome: ChromeStyle::Flat,
        }
    }
}
