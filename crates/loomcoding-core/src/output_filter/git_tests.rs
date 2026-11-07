//! Pruebas de la regla `git` con salidas reales (R3, R4, R6, R7). Las capturas están en `fixtures/`
//! y se sacaron de este repositorio; el correo del autor del registro está anonimizado.

use super::*;
use regex::Regex;
use serde_json::json;

const STATUS: &str = include_str!("fixtures/git_status_muchos_ficheros.txt");
const LOG: &str = include_str!("fixtures/git_log_30.txt");
const DIFF_COLOR: &str = include_str!("fixtures/git_diff_color.txt");
const DIFF_COMANDO: &str =
    "git -c color.ui=always diff HEAD~3 HEAD -- crates/loomcoding-core/src/output_filter/cargo.rs";

/// Filtro encendido, como lo deja `[output_filter] enabled = true`.
fn activo() -> FilterConfig {
    FilterConfig::from_toml("[output_filter]\nenabled = true\n").unwrap()
}

/// Volcado temporal que sobrevive al proceso de pruebas.
fn volcado() -> ToolSpill {
    ToolSpill::new(tempfile::tempdir().unwrap().keep())
}

/// Filtra una salida de `bash` con el comando dado.
fn bash_git(comando: &str, texto: &str) -> Filtered {
    apply(
        "c1",
        "bash",
        &json!({"command": comando}),
        &ToolOutput::ok(texto),
        &activo(),
        Some(&volcado()),
    )
}

/// Quita las secuencias de color de la salida de `git -c color.ui=always`.
fn sin_ansi(texto: &str) -> String {
    Regex::new(r"\x1b\[[0-9;]*m")
        .unwrap()
        .replace_all(texto, "")
        .into_owned()
}

/// Propiedad R3: toda línea con marca de fallo o aviso sobrevive intacta en la salida filtrada.
fn tiene_senal(linea: &str) -> bool {
    let minusculas = linea.to_ascii_lowercase();
    ["error", "failed", "panic", "warning:"]
        .iter()
        .any(|marca| minusculas.contains(marca))
}

/// Líneas de pista del formato largo de `git status`: son las que la regla omite.
fn pistas(texto: &str) -> usize {
    texto
        .lines()
        .filter(|linea| linea.trim_start().starts_with("(use \"git "))
        .count()
}

#[test]
fn status_quita_las_pistas_y_conserva_los_ficheros() {
    let salida = bash_git("git status", STATUS);

    assert_eq!(salida.rule, Some("git"), "{}", salida.content);
    assert!(
        !salida.content.contains("(use \"git "),
        "{}",
        salida.content
    );
    assert!(
        salida
            .content
            .contains("modified:   docs/specs/2026-11-06-loomcoding-compresion-de-salida.md"),
        "{}",
        salida.content
    );
    assert!(
        salida
            .content
            .contains("zz-captura/grupo-40/fichero-40.txt"),
        "{}",
        salida.content
    );
    let quitadas = pistas(STATUS);
    assert!(quitadas > 0);
    assert_eq!(salida.omitted_lines, quitadas);
    let total = STATUS.lines().count();
    let cabecera = format!("[filtrado: {quitadas} líneas omitidas de {total} — regla git;");
    assert!(salida.content.contains(&cabecera), "{}", salida.content);
}

#[test]
fn con_fallo_la_lista_blanca_no_cambia_lo_que_se_omite() {
    let con_exito = bash_git("git status", STATUS);
    let con_error = apply(
        "c1",
        "bash",
        &json!({"command": "git status"}),
        &ToolOutput::err(STATUS),
        &activo(),
        Some(&volcado()),
    );

    assert_eq!(con_error.rule, Some("git"));
    assert_eq!(con_error.omitted_lines, con_exito.omitted_lines);
}

#[test]
fn status_en_formato_corto_pasa_sin_tocar() {
    let corto = "?? zz/\n M docs/specs/x.md\nM  src/lib.rs\n";
    let salida = bash_git("git status --short", corto);

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, corto);
}

#[test]
fn una_linea_de_pista_con_marca_de_senal_nunca_se_omite() {
    // R3: la pista es ruido, pero la marca «failed» de su texto la convierte en señal.
    let texto = "On branch compresion\nChanges not staged for commit:\n  (use \"git add <file>...\" to update what will be committed)\n  (use \"git stash\" to set aside the failed merge)\n\tmodified:   src/lib.rs\n\nno changes added to commit (use \"git add\" and/or \"git commit -a\")\n";
    let salida = bash_git("git status", texto);

    assert_eq!(salida.rule, Some("git"), "{}", salida.content);
    assert!(
        salida
            .content
            .contains("(use \"git stash\" to set aside the failed merge)"),
        "{}",
        salida.content
    );
    assert!(
        !salida
            .content
            .contains("(use \"git add <file>...\" to update"),
        "{}",
        salida.content
    );
    assert_eq!(salida.omitted_lines, 1);
}

#[test]
fn log_no_tiene_regla_propia_y_conserva_todos_los_commits() {
    let salida = bash_git("git log -n 30", LOG);

    assert_ne!(salida.rule, Some("git"));
    let commits: Vec<&str> = LOG
        .lines()
        .filter(|linea| linea.starts_with("commit "))
        .collect();
    assert_eq!(commits.len(), 30);
    for commit in commits {
        assert!(salida.content.contains(commit), "falta «{commit}»");
    }
}

#[test]
fn git_diff_solo_pierde_el_ansi_y_no_tiene_regla_propia() {
    let salida = bash_git(DIFF_COMANDO, DIFF_COLOR);

    assert_ne!(salida.rule, Some("git"));
    assert!(!salida.content.contains('\x1b'), "{}", salida.content);
    assert!(
        salida.content.ends_with(&sin_ansi(DIFF_COLOR)),
        "el diff no quedó intacto salvo ANSI:\n{}",
        salida.content
    );
}

#[test]
fn propiedad_ninguna_linea_con_senal_desaparece_en_las_capturas_de_git() {
    let capturas = [
        ("git status", STATUS),
        ("git log -n 30", LOG),
        (DIFF_COMANDO, DIFF_COLOR),
    ];
    for (comando, texto) in capturas {
        let salida = bash_git(comando, texto);
        for linea in texto.lines().filter(|l| tiene_senal(l)) {
            let limpia = sin_ansi(linea);
            assert!(
                salida.content.contains(&limpia),
                "{comando}: se perdió «{limpia}»\n{}",
                salida.content
            );
        }
    }
}

#[test]
fn idioma_distinto_del_ingles_pasa_sin_regla_git() {
    let texto = "Sobre la rama compresion\nCambios para confirmar:\n\tmodificado:   a.rs\n";
    let salida = bash_git("git status", texto);

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, texto);
}

#[test]
fn con_tuberias_o_encadenados_no_hay_regla_git() {
    assert_ne!(bash_git("git status | head -20", STATUS).rule, Some("git"));
    assert_ne!(bash_git("git status && echo ok", STATUS).rule, Some("git"));
    assert_ne!(bash_git("echo git status", STATUS).rule, Some("git"));
    assert_ne!(bash_git("git -C ruta status", STATUS).rule, Some("git"));
    assert_eq!(bash_git("git status 2>&1", STATUS).rule, Some("git"));
}

#[test]
fn raw_true_devuelve_el_status_sin_filtrar() {
    let salida = apply(
        "c1",
        "bash",
        &json!({"command": "git status", "raw": true}),
        &ToolOutput::ok(STATUS),
        &activo(),
        Some(&volcado()),
    );

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, STATUS);
}
