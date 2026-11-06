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

/// Ejes de composer, herramientas y chrome de cada paquete. Son la única
/// fuente de verdad: `Look::classic`, `Look::crush` y `with_bundle` las leen.
const CLASSIC_AXES: (ComposerStyle, ToolStyle, ChromeStyle) = (
    ComposerStyle::Classic,
    ToolStyle::Pinned,
    ChromeStyle::Panels,
);
const FLAT_AXES: (ComposerStyle, ToolStyle, ChromeStyle) =
    (ComposerStyle::Crush, ToolStyle::Inline, ChromeStyle::Flat);

/// Paquete de estilo: fija a la vez composer, herramientas y chrome; el layout
/// queda fuera, porque se elige aparte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleBundle {
    /// Caja con líneas y herramientas fijadas (`Look::classic`).
    Classic,
    /// Sin líneas y herramientas en el transcript (`Look::crush`).
    Flat,
}

impl StyleBundle {
    /// Todos los paquetes, en el orden de rotación.
    pub const ALL: [StyleBundle; 2] = [StyleBundle::Classic, StyleBundle::Flat];

    /// Nombre estable con el que se escribe el paquete en `ui.toml` y en `/ui`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Flat => "flat",
        }
    }

    /// Resuelve un nombre producido por `name`; cualquier otro texto produce `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|bundle| bundle.name() == name)
    }

    /// Paquete siguiente en la rotación. Ciclo de dos pasos.
    pub const fn next(self) -> Self {
        match self {
            Self::Classic => Self::Flat,
            Self::Flat => Self::Classic,
        }
    }

    const fn axes(self) -> (ComposerStyle, ToolStyle, ChromeStyle) {
        match self {
            Self::Classic => CLASSIC_AXES,
            Self::Flat => FLAT_AXES,
        }
    }
}

impl Look {
    pub const fn classic() -> Self {
        let (composer, tool, chrome) = CLASSIC_AXES;
        Self {
            composer,
            tool,
            layout: LayoutStyle::Workspace,
            chrome,
        }
    }

    pub const fn crush() -> Self {
        let (composer, tool, chrome) = FLAT_AXES;
        Self {
            composer,
            tool,
            layout: LayoutStyle::ContextRail,
            chrome,
        }
    }

    /// Paquete al que equivale el look, si coincide exactamente en composer,
    /// herramientas y chrome. El layout no cuenta. Un look mixto devuelve `None`.
    pub fn bundle(&self) -> Option<StyleBundle> {
        let axes = (self.composer, self.tool, self.chrome);
        StyleBundle::ALL
            .into_iter()
            .find(|bundle| bundle.axes() == axes)
    }

    /// Copia del look con composer, herramientas y chrome del paquete. El layout no se toca.
    pub const fn with_bundle(self, bundle: StyleBundle) -> Self {
        let (composer, tool, chrome) = bundle.axes();
        Self {
            composer,
            tool,
            layout: self.layout,
            chrome,
        }
    }
}
