//! `/logofx`: alterna, fija el intervalo acotado a 5..=600, avisa del uso ante un argumento
//! inválido y persiste en `ui.toml`. Aparece en el menú de comandos y en la ayuda.

use super::*;

/// Escribe el comando en el compositor y pulsa Enter.
fn escribir(app: &mut App, texto: &str) -> Option<String> {
    app.composer.set_text(texto);
    handle_key(app, KeyCode::Enter, KeyModifiers::NONE)
}

#[test]
fn logofx_alterna_y_persiste() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    assert!(app.logo_fx_on);
    for (esperado, aviso) in [
        (false, "logo fx: off"),
        (true, "logo fx: on"),
        (false, "logo fx: off"),
    ] {
        assert_eq!(escribir(&mut app, "/logofx"), None);
        assert_eq!(app.logo_fx_on, esperado);
        assert_eq!(app.slash_feedback.as_deref(), Some(aviso));
        assert_eq!(crate::prefs::load_logo_fx(&path), Some(esperado));
        assert!(app.composer.is_empty());
    }
}

#[test]
fn logofx_acepta_on_y_off_explicitos() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    escribir(&mut app, "/logofx off");
    assert!(!app.logo_fx_on);
    assert_eq!(app.slash_feedback.as_deref(), Some("logo fx: off"));
    assert_eq!(crate::prefs::load_logo_fx(&path), Some(false));
    escribir(&mut app, "/logofx on");
    assert!(app.logo_fx_on);
    assert_eq!(app.slash_feedback.as_deref(), Some("logo fx: on"));
    assert_eq!(crate::prefs::load_logo_fx(&path), Some(true));
}

#[test]
fn logofx_con_segundos_fija_y_acota() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    app.set_prefs_path(path.clone());

    escribir(&mut app, "/logofx 3");
    assert_eq!(app.slash_feedback.as_deref(), Some("logo fx: cada 5 s"));
    assert_eq!(crate::prefs::load_logo_fx_interval(&path), Some(5));

    escribir(&mut app, "/logofx 9999");
    assert_eq!(app.slash_feedback.as_deref(), Some("logo fx: cada 600 s"));
    assert_eq!(crate::prefs::load_logo_fx_interval(&path), Some(600));

    escribir(&mut app, "/logofx 30");
    assert_eq!(app.slash_feedback.as_deref(), Some("logo fx: cada 30 s"));
    assert_eq!(crate::prefs::load_logo_fx_interval(&path), Some(30));
}

#[test]
fn logofx_con_argumento_invalido_avisa_del_uso_sin_cambiar_nada() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    let mut app = App::new();
    app.set_prefs_path(path.clone());
    escribir(&mut app, "/logofx 30");
    let intervalo = crate::prefs::load_logo_fx_interval(&path);

    for invalido in ["/logofx abc", "/logofx -3", "/logofx 1.5"] {
        escribir(&mut app, invalido);
        assert_eq!(
            app.slash_feedback.as_deref(),
            Some("Usage: /logofx [on|off|segundos]"),
            "{invalido}"
        );
        assert!(app.logo_fx_on, "{invalido}");
        assert_eq!(crate::prefs::load_logo_fx(&path), None, "{invalido}");
        assert_eq!(
            crate::prefs::load_logo_fx_interval(&path),
            intervalo,
            "{invalido}"
        );
    }
}

#[test]
fn logofx_figura_en_el_menu_de_comandos() {
    let mut app = App::new();
    app.composer.set_text("/log");
    let nombres: Vec<&str> = app.slash_matches().iter().map(|(_, n, _)| *n).collect();
    assert_eq!(nombres, vec!["/logofx"]);
}
