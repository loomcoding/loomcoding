//! `/help` y el Tab del recuadro de ayuda: eligen la vista que se muestra. Solo se calcula al
//! enviar el comando o al pulsar Tab; el compositor no interviene.

use super::App;
use crate::help_topics::{closest_command, find_command, no_help_message, HelpView, TOPICS};

impl App {
    /// Abre el recuadro en los atajos. Lo usan F1, `?` y `/help` sin argumento.
    pub fn open_shortcuts_help(&mut self) {
        self.help_view = HelpView::Shortcuts;
        self.help_open = true;
    }

    /// Abre la ayuda en la vista que pide `arg` (lo que sigue a `/help`): vacío, `topics`, un comando
    /// (con o sin barra) o un tema. Un argumento desconocido devuelve el aviso y no abre nada.
    pub fn help_command(&mut self, arg: &str) -> Result<(), String> {
        let arg = arg.trim();
        let vista = match arg {
            "" => HelpView::Shortcuts,
            "topics" => HelpView::Topics,
            _ => match find_command(arg) {
                Some(command) => HelpView::Command(command),
                None => match TOPICS
                    .iter()
                    .position(|(id, _, _)| id.eq_ignore_ascii_case(arg))
                {
                    Some(indice) => HelpView::Topic(indice),
                    None => return Err(no_help_message(arg, closest_command(arg))),
                },
            },
        };
        self.help_view = vista;
        self.help_open = true;
        Ok(())
    }

    /// Tab en el recuadro: el índice de temas pasa a los atajos, y cualquier otra vista pasa al índice.
    pub fn help_next_view(&mut self) {
        self.help_view = match self.help_view {
            HelpView::Topics => HelpView::Shortcuts,
            _ => HelpView::Topics,
        };
    }
}
