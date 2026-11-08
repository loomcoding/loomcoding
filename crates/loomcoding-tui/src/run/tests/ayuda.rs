//! `/help` desde el compositor y las teclas del recuadro: vistas, Tab, F1, `?`, el rechazo de un
//! argumento desconocido (que conserva el texto escrito) y el compositor intacto con la ayuda abierta.

use super::*;
use crate::commands::Command;
use crate::help_topics::{HelpView, TOPICS};

/// Escribe el texto en el compositor y pulsa Enter.
fn escribir(app: &mut App, texto: &str) -> Option<String> {
    app.composer.set_text(texto);
    handle_key(app, KeyCode::Enter, KeyModifiers::NONE)
}

fn indice_de(tema: &str) -> usize {
    TOPICS
        .iter()
        .position(|(id, _, _)| *id == tema)
        .unwrap_or_else(|| panic!("no hay tema {tema}"))
}

#[test]
fn help_sin_argumento_abre_los_atajos_y_limpia_el_texto() {
    let mut app = App::new();
    assert_eq!(escribir(&mut app, "/help"), None);
    assert!(app.help_open);
    assert_eq!(app.help_view, HelpView::Shortcuts);
    assert!(app.composer.is_empty());
}

#[test]
fn help_con_comando_abre_su_vista_con_o_sin_barra() {
    for texto in ["/help model", "/help MODEL", "/help /model"] {
        let mut app = App::new();
        escribir(&mut app, texto);
        assert!(app.help_open, "{texto}");
        assert_eq!(app.help_view, HelpView::Command(Command::Model), "{texto}");
        assert!(app.composer.is_empty(), "{texto}");
    }
}

#[test]
fn el_tema_de_configuracion_no_se_confunde_con_el_comando_settings() {
    let mut app = App::new();
    escribir(&mut app, "/help config");
    assert_eq!(app.help_view, HelpView::Topic(indice_de("config")));
    let mut app = App::new();
    escribir(&mut app, "/help settings");
    assert_eq!(app.help_view, HelpView::Command(Command::Settings));
}

#[test]
fn help_con_alias_oculto_muestra_su_comando() {
    let mut app = App::new();
    escribir(&mut app, "/help theme");
    assert_eq!(app.help_view, HelpView::Command(Command::Theme));
}

#[test]
fn help_topics_abre_el_indice_y_tab_alterna_con_los_atajos() {
    let mut app = App::new();
    escribir(&mut app, "/help topics");
    assert_eq!(app.help_view, HelpView::Topics);
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.help_view, HelpView::Shortcuts);
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(app.help_view, HelpView::Topics);
    assert!(app.help_open);
}

#[test]
fn help_con_tema_abre_la_vista_del_tema() {
    let mut app = App::new();
    escribir(&mut app, "/help flows");
    assert_eq!(app.help_view, HelpView::Topic(indice_de("flows")));
    assert!(app.composer.is_empty());
}

#[test]
fn help_desconocido_conserva_el_texto_y_avisa() {
    let mut app = App::new();
    assert_eq!(escribir(&mut app, "/help qwerty"), None);
    assert!(!app.help_open);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("No help for 'qwerty' · /help lists the topics")
    );
    assert_eq!(app.composer.text(), "/help qwerty");
}

#[test]
fn help_con_errata_sugiere_el_comando_mas_parecido() {
    let mut app = App::new();
    escribir(&mut app, "/help modl");
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("No help for 'modl' · did you mean /model? · /help lists the topics")
    );
    assert_eq!(app.composer.text(), "/help modl");
}

#[test]
fn f1_y_signo_de_interrogacion_abren_siempre_los_atajos() {
    let mut app = App::new();
    app.help_view = HelpView::Topics;
    handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE);
    assert!(app.help_open);
    assert_eq!(app.help_view, HelpView::Shortcuts);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.help_open);
    app.help_view = HelpView::Command(Command::Model);
    handle_key(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(app.help_open);
    assert_eq!(app.help_view, HelpView::Shortcuts);
}

#[test]
fn con_la_ayuda_abierta_las_letras_no_llegan_al_compositor() {
    let mut app = App::new();
    handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Char('a'), KeyModifiers::NONE);
    assert!(app.composer.is_empty());
    assert!(app.help_open);
}

#[test]
fn el_texto_escrito_sobrevive_a_abrir_y_cerrar_la_ayuda() {
    let mut app = App::new();
    app.composer.set_text("borrador a medias");
    handle_key(&mut app, KeyCode::F(1), KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    handle_key(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.help_open);
    assert_eq!(app.composer.text(), "borrador a medias");
}

#[test]
fn help_con_argumento_mantiene_el_popup_de_comandos() {
    let mut app = App::new();
    app.composer.set_text("/help model");
    assert_eq!(app.slash_query(), Some("/help"));
}
