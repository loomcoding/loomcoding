//! `/odd` desde el compositor: un argumento válido se aplica y limpia el texto; uno inválido
//! conserva el texto escrito y muestra el uso, como `/effort`.

use super::*;

/// Escribe el comando en el compositor y pulsa Enter.
fn escribir(app: &mut App, texto: &str) -> Option<String> {
    app.composer.set_text(texto);
    handle_key(app, KeyCode::Enter, KeyModifiers::NONE)
}

#[test]
fn odd_con_argumento_invalido_conserva_el_texto_y_da_el_uso() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::with_context(dir.path().display().to_string(), "modelo");
    assert_eq!(escribir(&mut app, "/odd quizas"), None);
    assert_eq!(app.slash_feedback.as_deref(), Some("Usage: /odd [on|off]"));
    assert_eq!(app.composer.text(), "/odd quizas");
}

#[test]
fn odd_off_desde_el_compositor_guarda_la_clave_y_limpia_el_texto() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("loomcoding.toml"), "model = \"x\"\n").unwrap();
    let mut app = App::with_context(dir.path().display().to_string(), "modelo");
    assert_eq!(escribir(&mut app, "/odd off"), None);
    let feedback = app.slash_feedback.clone().unwrap_or_default();
    assert!(feedback.contains("aplica al reiniciar Loom"), "{feedback}");
    assert!(app.composer.is_empty());
    let texto = std::fs::read_to_string(dir.path().join("loomcoding.toml")).unwrap();
    assert!(texto.contains("enabled = false"), "{texto}");
}
