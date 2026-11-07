//! `/flow` desde el compositor: un argumento válido se aplica y limpia el texto; uno inválido o un
//! `kind` desconocido conserva el texto escrito y muestra el uso, como `/effort`.

use super::*;
use loomcoding_core::flow::FlowMode;

/// Escribe el comando en el compositor y pulsa Enter.
fn escribir(app: &mut App, texto: &str) -> Option<String> {
    app.composer.set_text(texto);
    handle_key(app, KeyCode::Enter, KeyModifiers::NONE)
}

/// Aplicación de un proyecto temporal con un único modo `a`.
fn app_con_modo_a(dir: &tempfile::TempDir) -> App {
    App::with_context(dir.path().display().to_string(), "modelo").with_flow_modes(vec![FlowMode {
        kind: "a".into(),
        label: "AAA".into(),
        enabled: true,
    }])
}

#[test]
fn flow_con_argumento_invalido_conserva_el_texto_y_da_el_uso() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_con_modo_a(&dir);
    assert_eq!(escribir(&mut app, "/flow quizas"), None);
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("Usage: /flow [<kind> on|off] (kinds: a)")
    );
    assert_eq!(app.composer.text(), "/flow quizas");
}

#[test]
fn flow_con_kind_desconocido_conserva_el_texto() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_con_modo_a(&dir);
    assert_eq!(escribir(&mut app, "/flow zeta off"), None);
    assert_eq!(app.composer.text(), "/flow zeta off");
}

#[test]
fn flow_off_desde_el_compositor_guarda_la_clave_y_limpia_el_texto() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("loomcoding.toml"), "model = \"x\"\n").unwrap();
    let mut app = app_con_modo_a(&dir);
    assert_eq!(escribir(&mut app, "/flow a off"), None);
    let feedback = app.slash_feedback.clone().unwrap_or_default();
    assert!(feedback.contains("aplica al reiniciar Loom"), "{feedback}");
    assert!(app.composer.is_empty());
    let texto = std::fs::read_to_string(dir.path().join("loomcoding.toml")).unwrap();
    assert!(texto.contains("enabled = false"), "{texto}");
}
