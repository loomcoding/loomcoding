//! Alias ocultos de `/`: comandos que siguen funcionando al teclearlos enteros, pero que el
//! popup no lista, porque su lugar lo ocupa `/ui`.
//!
//! Cabe señalar que `COMMANDS` no cambia: sigue siendo la tabla única para resolver el texto
//! tecleado. Aquí solo se decide qué sale en el menú.

use crate::commands::{Command, COMMANDS};

/// Comandos que se ejecutan por su nombre completo pero no aparecen en el popup de `/`.
pub const HIDDEN_ALIASES: &[Command] = &[
    Command::Theme,
    Command::Layout,
    Command::Temas,
    Command::Crush,
    Command::Classic,
    Command::Mascot,
    Command::Sidebar,
    Command::Mouse,
];

/// Indica si el comando es un alias oculto del popup.
pub fn is_hidden_alias(command: Command) -> bool {
    HIDDEN_ALIASES.contains(&command)
}

/// Si el texto escrito es exactamente el nombre de un alias oculto, devuelve su comando.
/// Sirve para que Enter sobre `/theme` siga ejecutándolo aunque el popup no lo muestre.
pub fn exact_hidden_alias(text: &str) -> Option<Command> {
    let nombre = text.trim();
    COMMANDS
        .iter()
        .find(|(command, name, _)| *name == nombre && is_hidden_alias(*command))
        .map(|(command, _, _)| *command)
}
