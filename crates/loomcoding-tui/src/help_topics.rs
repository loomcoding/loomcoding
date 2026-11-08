//! Ayuda dentro de la app: `/help` por temas y por comando.
//!
//! Cabe señalar que la fuente de los ejemplos es la referencia de usuario (`docs/user/{en,es}/reference/commands.md`):
//! las pruebas comprueban que cada ejemplo de aquí aparece textual en esa página, en los dos idiomas.

use crate::commands::{Command, COMMANDS};

/// Vista que muestra el recuadro de ayuda. `Shortcuts` es la de siempre (F1, `?` y `/help` sin argumento).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HelpView {
    #[default]
    Shortcuts,
    Topics,
    Command(Command),
    Topic(usize),
}

/// Ayuda corta de un comando: su línea de uso y su primer ejemplo, tal como aparecen en la referencia.
pub struct CommandHelp {
    pub usage: &'static str,
    pub example: &'static str,
}

/// Ayuda de un comando. Cubre todas las variantes sin comodín: añadir un comando sin ayuda no compila.
/// El ejemplo es el primero de la columna «Example» de la referencia, textual.
pub fn command_help(command: Command) -> CommandHelp {
    let (usage, example) = match command {
        Command::Help => ("/help [<command> | <topic> | topics]", "/help model"),
        Command::New => ("/new", "/new"),
        Command::Sessions => ("/sessions", "/sessions"),
        Command::Settings => ("/settings", "/settings"),
        Command::Model => ("/model [<id>]", "/model <id>"),
        Command::Effort => ("/effort [<level|default>]", "/effort high"),
        Command::Provider => ("/provider [<id>]", "/provider llama"),
        Command::Queue => ("/queue", "/queue"),
        Command::Tools => ("/tools", "/tools"),
        Command::Theme => ("/theme", "/theme"),
        Command::Layout => ("/layout", "/layout"),
        Command::Temas => ("/temas", "/temas"),
        Command::Ui => ("/ui [<row> [value]]", "/ui mouse off"),
        Command::Crush => ("/crush", "/crush"),
        Command::Classic => ("/classic", "/classic"),
        Command::Cancel => ("/cancel", "/cancel"),
        Command::Pause => ("/pause", "/pause"),
        Command::Resume => ("/resume", "/resume"),
        Command::Parent => ("/parent", "/parent"),
        Command::Sidebar => ("/sidebar", "/sidebar"),
        Command::Mouse => ("/mouse", "/mouse"),
        Command::Mascot => ("/mascot", "/mascot"),
        Command::LogoFx => ("/logofx [on|off|<seconds>]", "/logofx 30"),
        Command::Verbose => ("/verbose [on|off]", "/verbose on"),
        Command::Flow => ("/flow [<kind> on|off]", "/flow odd off"),
        Command::Agent => (
            "/agent <name> [--skills a,b] <task>",
            "/agent <name> [--skills a,b] <task>",
        ),
        Command::Focus => ("/focus [<id>]", "/focus <id>"),
        Command::Agents => ("/agents", "/agents"),
        Command::A2a => ("/a2a", "/a2a"),
        Command::RefreshAgents => ("/refresh-agents", "/refresh-agents"),
        Command::Compact => ("/compact", "/compact"),
        Command::Quit => ("/quit", "/quit"),
    };
    CommandHelp { usage, example }
}

/// Página de referencia de los comandos, en inglés; la versión `es` sustituye `/en/` por `/es/`.
pub const COMMANDS_DOC: &str = "docs/user/en/reference/commands.md";

/// Temas de ayuda: (identificador, descripción de una línea, página en inglés del repositorio).
pub const TOPICS: &[(&str, &str, &str)] = &[
    (
        "shortcuts",
        "Keys in the composer, the transcript and the panels",
        "docs/user/en/reference/shortcuts.md",
    ),
    (
        "commands",
        "Every slash command, with its example and when it works",
        "docs/user/en/reference/commands.md",
    ),
    (
        "config",
        "loomcoding.toml: every section and field",
        "docs/user/en/reference/configuration.md",
    ),
    (
        "flows",
        "ODD flow modes and the /flow command",
        "docs/user/en/concepts/flows-odd.md",
    ),
    (
        "a2a",
        "Remote agents over the A2A protocol",
        "docs/user/en/concepts/a2a.md",
    ),
    (
        "jobs",
        "Watching a background job",
        "docs/user/en/guides/watch-a-job.md",
    ),
    (
        "permissions",
        "What the agent may run, and how you approve it",
        "docs/user/en/concepts/permissions.md",
    ),
    (
        "context",
        "The context window and compaction",
        "docs/user/en/concepts/context-and-compaction.md",
    ),
];

/// Comando con o sin barra, sin distinguir mayúsculas. Incluye los alias ocultos del popup, que
/// siguen en `COMMANDS` porque es la tabla única para resolver el texto tecleado.
pub fn find_command(name: &str) -> Option<Command> {
    let buscado = normaliza(name);
    if buscado.is_empty() {
        return None;
    }
    COMMANDS
        .iter()
        .find(|(_, nombre, _)| normaliza(nombre) == buscado)
        .map(|(command, _, _)| *command)
}

/// Nombre con barra del comando más parecido a `name`, o `None` si ninguno está cerca. Primero
/// busca un único comando que empiece por lo escrito; si no hay, el de distancia de edición dentro de `tolerancia`.
pub fn closest_command(name: &str) -> Option<&'static str> {
    let buscado = normaliza(name);
    if buscado.is_empty() {
        return None;
    }
    let nombres: Vec<&'static str> = COMMANDS.iter().map(|(_, nombre, _)| *nombre).collect();
    let prefijos: Vec<&'static str> = nombres
        .iter()
        .copied()
        .filter(|nombre| normaliza(nombre).starts_with(&buscado))
        .collect();
    if let [unico] = prefijos.as_slice() {
        return Some(unico);
    }
    // Empate de distancia: gana el nombre de longitud más parecida (`/quti` es más de `/quit` que de `/ui`).
    let largo = buscado.chars().count();
    nombres
        .iter()
        .map(|nombre| (distancia(&buscado, &normaliza(nombre)), *nombre))
        .filter(|(distancia, _)| *distancia <= tolerancia(largo))
        .min_by_key(|(distancia, nombre)| {
            (
                *distancia,
                normaliza(nombre).chars().count().abs_diff(largo),
            )
        })
        .map(|(_, nombre)| nombre)
}

/// Nombre sin barra y en minúsculas: la forma en que se comparan los comandos.
fn normaliza(texto: &str) -> String {
    texto.trim().trim_start_matches('/').to_ascii_lowercase()
}

/// Distancia de edición entre dos textos, por caracteres, con la transposición de dos letras
/// contiguas contada como un solo cambio (variante de Damerau, «restringida»).
fn distancia(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut tabla = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, fila) in tabla.iter_mut().enumerate() {
        fila[0] = i;
    }
    for (j, celda) in tabla[0].iter_mut().enumerate() {
        *celda = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let coste = usize::from(a[i - 1] != b[j - 1]);
            let mut mejor = (tabla[i - 1][j - 1] + coste)
                .min(tabla[i - 1][j] + 1)
                .min(tabla[i][j - 1] + 1);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                mejor = mejor.min(tabla[i - 2][j - 2] + 1);
            }
            tabla[i][j] = mejor;
        }
    }
    tabla[a.len()][b.len()]
}

/// Cuántas ediciones se toleran: una sola en palabras cortas, para no sugerir cualquier cosa
/// (`bogus` no se parece a `/focus`); dos a partir de seis letras.
fn tolerancia(largo: usize) -> usize {
    if largo <= 5 {
        1
    } else {
        2
    }
}

/// Mensaje de un argumento de `/help` que no es comando ni tema; sugiere `closest` si lo hay.
pub fn no_help_message(name: &str, closest: Option<&str>) -> String {
    match closest {
        Some(near) => {
            format!("No help for '{name}' · did you mean {near}? · /help lists the topics")
        }
        None => format!("No help for '{name}' · /help lists the topics"),
    }
}

#[cfg(test)]
mod tests;
