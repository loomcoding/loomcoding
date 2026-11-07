//! `/odd`: informa del interruptor de ODD y lo guarda en `[odd] enabled` del proyecto. Es de
//! configuración: el cambio aplica al reiniciar, porque la TUI no toca el `OddState` del daemon.

use super::*;

/// Aplicación cuyo proyecto es un directorio temporal con el `loomcoding.toml` dado.
fn app_en_proyecto(toml: Option<&str>) -> (App, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    if let Some(texto) = toml {
        std::fs::write(dir.path().join("loomcoding.toml"), texto).unwrap();
    }
    let app = App::with_context(dir.path().display().to_string(), "modelo");
    (app, dir)
}

#[test]
fn odd_sin_argumento_informa_activo_si_la_config_no_lo_apaga() {
    let (mut app, _dir) = app_en_proyecto(Some("model = \"x\"\n"));
    app.odd_command("").unwrap();
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("ODD: activo (config [odd] enabled)")
    );
}

#[test]
fn odd_sin_argumento_informa_apagado_si_la_config_lo_apaga() {
    let (mut app, _dir) = app_en_proyecto(Some("model = \"x\"\n[odd]\nenabled = false\n"));
    app.odd_command("").unwrap();
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("ODD: apagado (config [odd] enabled)")
    );
}

#[test]
fn odd_off_persiste_la_clave_sin_tocar_el_resto_y_avisa_del_reinicio() {
    let (mut app, dir) = app_en_proyecto(Some("# notas\nmodel = \"x\"\n"));
    app.odd_command("off").unwrap();
    let texto = std::fs::read_to_string(dir.path().join("loomcoding.toml")).unwrap();
    assert!(texto.contains("# notas"), "{texto}");
    assert!(texto.contains("model = \"x\""), "{texto}");
    assert!(texto.contains("[odd]"), "{texto}");
    assert!(texto.contains("enabled = false"), "{texto}");
    let feedback = app.slash_feedback.clone().unwrap_or_default();
    assert!(feedback.contains("aplica al reiniciar Loom"), "{feedback}");
}

#[test]
fn odd_on_vuelve_a_guardar_activo() {
    let (mut app, dir) = app_en_proyecto(Some("model = \"x\"\n[odd]\nenabled = false\n"));
    app.odd_command("on").unwrap();
    let texto = std::fs::read_to_string(dir.path().join("loomcoding.toml")).unwrap();
    assert!(texto.contains("enabled = true"), "{texto}");
    assert!(!texto.contains("enabled = false"), "{texto}");
}

#[test]
fn odd_con_argumento_invalido_da_el_uso() {
    let (mut app, _dir) = app_en_proyecto(Some("model = \"x\"\n"));
    assert_eq!(
        app.odd_command("quizas"),
        Err("Usage: /odd [on|off]".to_string())
    );
}

#[test]
fn odd_on_off_sin_proyecto_se_rechaza_sin_escribir_nada() {
    let mut app = App::new();
    assert!(app.odd_command("off").is_err());
    assert!(app.odd_command("on").is_err());
}
