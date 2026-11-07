//! `/flow`: lista los modos y su estado, guarda `[flows.<kind>] enabled` en el proyecto y rechaza
//! un `kind` desconocido. Es de configuración: el cambio aplica al reiniciar.

use super::*;

/// Modo de prueba: `kind` y etiqueta propios, y su interruptor de partida.
fn modo(kind: &str, label: &str, enabled: bool) -> FlowMode {
    FlowMode {
        kind: kind.into(),
        label: label.into(),
        enabled,
    }
}

/// Aplicación cuyo proyecto es un directorio temporal con el `loomcoding.toml` dado.
fn app_en_proyecto(toml: Option<&str>, modos: Vec<FlowMode>) -> (App, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    if let Some(texto) = toml {
        std::fs::write(dir.path().join("loomcoding.toml"), texto).unwrap();
    }
    let app = App::with_context(dir.path().display().to_string(), "modelo").with_flow_modes(modos);
    (app, dir)
}

#[test]
fn flow_sin_argumento_lista_el_estado_de_cada_modo_segun_la_config() {
    let (mut app, _dir) = app_en_proyecto(
        Some("model = \"x\"\n[flows.a]\nenabled = false\n"),
        vec![modo("a", "AAA", true), modo("b", "BBB", true)],
    );
    app.flow_command("").unwrap();
    assert_eq!(
        app.slash_feedback.as_deref(),
        Some("flow a: apagado, b: activo (config [flows.<kind>] enabled) · fase propia de Loom, no estado A2A")
    );
}

#[test]
fn flow_sin_argumento_sin_config_muestra_lo_resuelto_al_arrancar() {
    let (mut app, _dir) = app_en_proyecto(None, vec![modo("a", "AAA", false)]);
    app.flow_command("").unwrap();
    let feedback = app.slash_feedback.clone().unwrap_or_default();
    assert!(feedback.starts_with("flow a: apagado"), "{feedback}");
    assert!(feedback.contains("no estado A2A"), "{feedback}");
}

#[test]
fn flow_off_persiste_la_clave_sin_tocar_el_resto_y_avisa_del_reinicio() {
    let (mut app, dir) = app_en_proyecto(
        Some("# notas\nmodel = \"x\"\n"),
        vec![modo("a", "AAA", true)],
    );
    app.flow_command("a off").unwrap();
    let texto = std::fs::read_to_string(dir.path().join("loomcoding.toml")).unwrap();
    assert!(texto.contains("# notas"), "{texto}");
    assert!(texto.contains("model = \"x\""), "{texto}");
    assert!(texto.contains("[flows.a]"), "{texto}");
    assert!(texto.contains("enabled = false"), "{texto}");
    let feedback = app.slash_feedback.clone().unwrap_or_default();
    assert!(feedback.contains("aplica al reiniciar Loom"), "{feedback}");
}

#[test]
fn flow_on_vuelve_a_guardar_activo() {
    let (mut app, dir) = app_en_proyecto(
        Some("model = \"x\"\n[flows.a]\nenabled = false\n"),
        vec![modo("a", "AAA", false)],
    );
    app.flow_command("a on").unwrap();
    let texto = std::fs::read_to_string(dir.path().join("loomcoding.toml")).unwrap();
    assert!(texto.contains("enabled = true"), "{texto}");
    assert!(!texto.contains("enabled = false"), "{texto}");
}

#[test]
fn flow_con_argumento_invalido_da_el_uso() {
    let (mut app, _dir) = app_en_proyecto(
        Some("model = \"x\"\n"),
        vec![modo("a", "AAA", true), modo("b", "BBB", true)],
    );
    assert_eq!(
        app.flow_command("quizas"),
        Err("Usage: /flow [<kind> on|off] (kinds: a, b)".to_string())
    );
    assert!(app.flow_command("a quizas").is_err());
}

#[test]
fn flow_con_kind_desconocido_da_el_uso_y_no_escribe() {
    let original = "model = \"x\"\n";
    let (mut app, dir) = app_en_proyecto(Some(original), vec![modo("a", "AAA", true)]);
    assert_eq!(
        app.flow_command("zeta on"),
        Err("Usage: /flow [<kind> on|off] (kinds: a)".to_string())
    );
    let texto = std::fs::read_to_string(dir.path().join("loomcoding.toml")).unwrap();
    assert_eq!(texto, original);
}

#[test]
fn flow_on_off_sin_proyecto_se_rechaza_sin_escribir_nada() {
    let mut app = App::new().with_flow_modes(vec![modo("a", "AAA", true)]);
    assert!(app.flow_command("a off").is_err());
    assert!(app.flow_command("a on").is_err());
}
