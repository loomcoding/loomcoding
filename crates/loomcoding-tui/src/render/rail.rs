//! Carril del transcript: el estilo elegido por la persona usuaria (`/ui carril`, `rail` en
//! `ui.toml`), el ancho que reserva y los símbolos de cada fila de un bloque.
//!
//! Todo aquí es puro: no toca el búfer ni el `App`. El pintado, con el color de cada rol, vive en
//! `transcript.rs`, que consulta estas funciones fila a fila.

/// Columnas mínimas del transcript para reservar carril; por debajo, el texto manda.
pub const MIN_WIDTH: u16 = 40;

/// Estilo del carril a la izquierda de cada bloque del transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RailStyle {
    /// Árbol `╭─ │ ╰─`, el de siempre y el valor por defecto.
    #[default]
    Tree,
    /// Punteado `┆` sin esquinas; la cabecera lleva el glifo del rol.
    Dots,
    /// Sin trazo; solo el bloque enfocado lleva un `▌` con el color de su rol.
    Bar,
    /// Caja redonda fina solo del mensaje del usuario (`╭─╮ │ » │ ╰─╯`); el resto va sin trazo,
    /// como en `None`. Mide 3 columnas, como `Tree`, para que el texto no salte entre roles.
    Box,
    /// Sin trazo; el mensaje del usuario lleva fondo tenue y el asistente un `• ` atenuado.
    Tint,
    /// Sin trazo; solo el glifo del rol al abrir el bloque.
    None,
}

impl RailStyle {
    /// Todos los estilos, en el orden en que los recorre `←`/`→` y en que se listan en los avisos.
    pub const ALL: [RailStyle; 6] = [
        RailStyle::Tree,
        RailStyle::Dots,
        RailStyle::Bar,
        RailStyle::Box,
        RailStyle::Tint,
        RailStyle::None,
    ];

    /// Clave estable con la que se escribe en `/ui carril` y en `ui.toml`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Dots => "dots",
            Self::Bar => "bar",
            Self::Box => "box",
            Self::Tint => "tint",
            Self::None => "none",
        }
    }

    /// Resuelve una clave producida por `name`; cualquier otro texto da `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|style| style.name() == name)
    }
}

/// Posición de una fila dentro de su bloque: abre, cierra, ocupa el medio o es la única.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posicion {
    Primera,
    Intermedia,
    Ultima,
    Unica,
}

/// Posición de una fila a partir de si abre y/o cierra su bloque.
pub const fn posicion(primera: bool, ultima: bool) -> Posicion {
    match (primera, ultima) {
        (true, true) => Posicion::Unica,
        (true, false) => Posicion::Primera,
        (false, true) => Posicion::Ultima,
        (false, false) => Posicion::Intermedia,
    }
}

/// Columnas que reserva el carril de `style` en un transcript de `width` columnas. Cero por
/// debajo de `MIN_WIDTH`, para que el texto no pierda ancho en pantallas estrechas.
pub fn rail_width(style: RailStyle, width: u16) -> u16 {
    if width < MIN_WIDTH {
        return 0;
    }
    match style {
        RailStyle::Tree | RailStyle::Dots | RailStyle::Bar | RailStyle::Box => 3,
        RailStyle::None | RailStyle::Tint => 2,
    }
}

/// Símbolos de las dos primeras columnas del carril de una fila. Las columnas que sobran del
/// ancho de `rail_width` se quedan en blanco. `glifo` es el del rol del bloque (`None` si no lo
/// lleva, como las herramientas); solo se usa en la fila de cabecera, salvo en `Tree`, donde
/// ocupa la segunda columna de esa fila. En `Box` no hay trazo fuera del mensaje del usuario: esa
/// caja la pinta `rail_box.rs` aparte, así que aquí todo bloque se trata como en `None`.
pub fn rail_cells(
    style: RailStyle,
    pos: Posicion,
    enfocado: bool,
    glifo: Option<char>,
) -> [char; 2] {
    match style {
        RailStyle::Tree => arbol(pos, enfocado, glifo),
        RailStyle::Dots => punteado(pos, enfocado, glifo),
        RailStyle::Bar => barra(pos, enfocado, glifo),
        RailStyle::None | RailStyle::Tint | RailStyle::Box => solo_glifo(pos, glifo),
    }
}

/// Árbol: `╭─` al abrir, `│` en el cuerpo y `╰─` al cerrar; `──` si el bloque ocupa una fila.
/// Con foco, el trazo es grueso (`┏━ ┃ ┗━`).
fn arbol(pos: Posicion, enfocado: bool, glifo: Option<char>) -> [char; 2] {
    let fino = if enfocado { '━' } else { '─' };
    let cabecera = glifo.unwrap_or(fino);
    match pos {
        Posicion::Unica => [fino, cabecera],
        Posicion::Primera => [if enfocado { '┏' } else { '╭' }, cabecera],
        Posicion::Intermedia => [if enfocado { '┃' } else { '│' }, ' '],
        Posicion::Ultima => [if enfocado { '┗' } else { '╰' }, fino],
    }
}

/// Punteado: `┆` en el cuerpo y en la última fila, sin esquinas. La cabecera lleva el glifo del
/// rol en lugar del punto. Con foco, el punto pasa a `┇`; el color lo pone quien pinta.
fn punteado(pos: Posicion, enfocado: bool, glifo: Option<char>) -> [char; 2] {
    let punto = if enfocado { '┇' } else { '┆' };
    match pos {
        Posicion::Primera | Posicion::Unica => [glifo.unwrap_or(punto), ' '],
        Posicion::Intermedia | Posicion::Ultima => [punto, ' '],
    }
}

/// Barra: ningún trazo; el bloque enfocado lleva un `▌` en la primera columna de todas sus filas
/// y la cabecera conserva el glifo del rol en la segunda.
fn barra(pos: Posicion, enfocado: bool, glifo: Option<char>) -> [char; 2] {
    let marca = if enfocado { '▌' } else { ' ' };
    let cabecera = match pos {
        Posicion::Primera | Posicion::Unica => glifo.unwrap_or(' '),
        Posicion::Intermedia | Posicion::Ultima => ' ',
    };
    [marca, cabecera]
}

/// Sin trazo: solo el glifo del rol, en la cabecera del bloque. Con foco no cambia nada aquí.
fn solo_glifo(pos: Posicion, glifo: Option<char>) -> [char; 2] {
    let cabecera = match pos {
        Posicion::Primera | Posicion::Unica => glifo.unwrap_or(' '),
        Posicion::Intermedia | Posicion::Ultima => ' ',
    };
    [cabecera, ' ']
}
