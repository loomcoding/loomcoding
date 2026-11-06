//! Pruebas de las preferencias de los efectos del logo (`logo_fx` y `logo_fx_interval`).
//! Viven aparte de `prefs.rs` para que ese fichero no siga creciendo.

use super::*;

/// Cuenta las líneas que asignan exactamente la clave `name`.
fn asignaciones(path: &Path, name: &str) -> usize {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| is_assignment_of(line, name))
        .count()
}

#[test]
fn logo_fx_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    assert_eq!(load_logo_fx(&path), None);
    for on in [false, true, false] {
        save_logo_fx(&path, on).unwrap();
        assert_eq!(load_logo_fx(&path), Some(on), "logo_fx {on}");
    }
    assert_eq!(asignaciones(&path, "logo_fx"), 1);
}

#[test]
fn logo_fx_interval_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    assert_eq!(load_logo_fx_interval(&path), None);
    for secs in [20u64, 5, 600, 45] {
        save_logo_fx_interval(&path, secs).unwrap();
        assert_eq!(load_logo_fx_interval(&path), Some(secs), "{secs} s");
    }
    assert_eq!(asignaciones(&path, "logo_fx_interval"), 1);
}

#[test]
fn logo_fx_interval_se_acota_al_cargar() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    std::fs::write(&path, "logo_fx_interval = 3\n").unwrap();
    assert_eq!(load_logo_fx_interval(&path), Some(5));
    std::fs::write(&path, "logo_fx_interval = 9999\n").unwrap();
    assert_eq!(load_logo_fx_interval(&path), Some(600));
}

#[test]
fn valores_invalidos_se_ignoran() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    for contenido in [
        "logo_fx = \"si\"\n",
        "logo_fx = 1\n",
        "logo_fx = True\n",
        "logo_fx =\n",
    ] {
        std::fs::write(&path, contenido).unwrap();
        assert_eq!(load_logo_fx(&path), None, "contenido {contenido:?}");
    }
    for contenido in [
        "logo_fx_interval = \"veinte\"\n",
        "logo_fx_interval = -3\n",
        "logo_fx_interval = 45.5\n",
        "logo_fx_interval =\n",
    ] {
        std::fs::write(&path, contenido).unwrap();
        assert_eq!(
            load_logo_fx_interval(&path),
            None,
            "contenido {contenido:?}"
        );
    }
}

#[test]
fn guardar_una_clave_no_toca_las_otras() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    save_theme(&path, ThemePreset::Pawan).unwrap();
    save_mouse(&path, false).unwrap();
    save_mascot(&path, true).unwrap();
    save_logo_fx(&path, false).unwrap();
    save_logo_fx_interval(&path, 30).unwrap();

    assert_eq!(load_theme(&path), Some(ThemePreset::Pawan));
    assert_eq!(load_mouse(&path), Some(false));
    assert_eq!(load_mascot(&path), Some(true));
    assert_eq!(load_logo_fx(&path), Some(false));
    assert_eq!(load_logo_fx_interval(&path), Some(30));

    save_logo_fx(&path, true).unwrap();
    assert_eq!(load_logo_fx_interval(&path), Some(30));
    assert_eq!(load_mouse(&path), Some(false));
    assert_eq!(asignaciones(&path, "logo_fx"), 1);
    assert_eq!(asignaciones(&path, "logo_fx_interval"), 1);
}
