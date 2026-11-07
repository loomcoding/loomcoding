//! Pruebas de la regla `cargo` con salidas reales (R3, R4, R6, R7). Las capturas están en `fixtures/`
//! y se sacaron de un crate de juguete con un `CARGO_TARGET_DIR` temporal fuera del repositorio.

use super::*;
use crate::tools::MAX_TOOL_OUTPUT_BYTES;
use serde_json::json;

const TODO_OK: &str = include_str!("fixtures/cargo_test_todo_ok.txt");
const FALLO_ESCONDIDO: &str = include_str!("fixtures/cargo_test_fallo_escondido.txt");
const PANICKED: &str = include_str!("fixtures/cargo_test_panicked.txt");
const BUILD_AVISOS: &str = include_str!("fixtures/cargo_build_avisos.txt");
const BUILD_JSON: &str = include_str!("fixtures/cargo_build_json.txt");
const CLIPPY_ERRORES: &str = include_str!("fixtures/cargo_clippy_errores.txt");

/// Filtro encendido, como lo deja `[output_filter] enabled = true`.
fn activo() -> FilterConfig {
    FilterConfig::from_toml("[output_filter]\nenabled = true\n").unwrap()
}

/// Volcado temporal que sobrevive al proceso de pruebas.
fn volcado() -> ToolSpill {
    ToolSpill::new(tempfile::tempdir().unwrap().keep())
}

/// Filtra una salida de `bash` con el comando dado.
fn bash_cargo(comando: &str, texto: &str) -> Filtered {
    apply(
        "c1",
        "bash",
        &json!({"command": comando}),
        &ToolOutput::ok(texto),
        &activo(),
        Some(&volcado()),
    )
}

/// Una línea de resultado de test que sale bien: `test nombre ... ok`.
fn es_test_ok(linea: &str) -> bool {
    linea.starts_with("test ") && linea.ends_with(" ... ok")
}

/// Propiedad R3: toda línea con marca de fallo o aviso sobrevive intacta en la salida filtrada.
fn tiene_senal(linea: &str) -> bool {
    let minusculas = linea.to_ascii_lowercase();
    ["error", "failed", "panic", "warning:"]
        .iter()
        .any(|marca| minusculas.contains(marca))
}

#[test]
fn todo_ok_se_reduce_a_lo_que_importa_y_anuncia_la_regla() {
    let salida = bash_cargo("cargo test --test ok", TODO_OK);

    assert_eq!(salida.rule, Some("cargo"), "{}", salida.content);
    assert!(
        !salida.content.contains("test uno ... ok"),
        "{}",
        salida.content
    );
    assert!(
        salida.content.contains("test result: ok. 3 passed"),
        "{}",
        salida.content
    );
    assert!(
        salida.content.contains("running 3 tests"),
        "{}",
        salida.content
    );
    // Cuatro líneas fuera: `Compiling` y los tres `test ... ok`.
    assert_eq!(salida.omitted_lines, 4);
    let total = TODO_OK.lines().count();
    let cabecera = format!("[filtrado: 4 líneas omitidas de {total} — regla cargo;");
    assert!(salida.content.contains(&cabecera), "{}", salida.content);
}

#[test]
fn fallo_escondido_entre_400_ok_sigue_visible_con_su_contexto() {
    let salida = bash_cargo("cargo test --test escondido", FALLO_ESCONDIDO);

    assert_eq!(salida.rule, Some("cargo"));
    assert!(
        salida.content.contains("test falla_escondido ... FAILED"),
        "{}",
        salida.content
    );
    assert!(
        salida.content.contains("suma incorrecta"),
        "{}",
        salida.content
    );
    assert!(salida.content.contains("left: 4"), "{}", salida.content);
    assert!(salida.content.contains("right: 5"), "{}", salida.content);
    assert!(
        salida
            .content
            .contains("test result: FAILED. 400 passed; 1 failed"),
        "{}",
        salida.content
    );
    assert!(
        !salida.content.contains("test ok_a_17 ... ok"),
        "{}",
        salida.content
    );
    assert!(
        !salida.content.contains("test ok_b_199 ... ok"),
        "{}",
        salida.content
    );
    // Los 400 `ok`, la línea `Compiling` y las dos líneas en blanco seguidas que colapsa `general`.
    let lineas: Vec<&str> = FALLO_ESCONDIDO.lines().collect();
    let ok = lineas.iter().filter(|l| es_test_ok(l)).count();
    let compilando = lineas
        .iter()
        .filter(|l| l.trim_start().starts_with("Compiling "))
        .count();
    let blancas_seguidas = lineas
        .windows(2)
        .filter(|p| p[0].trim().is_empty() && p[1].trim().is_empty())
        .count();
    assert_eq!(salida.omitted_lines, ok + compilando + blancas_seguidas);
}

#[test]
fn panico_conserva_el_mensaje_y_el_resumen() {
    let salida = bash_cargo("cargo test --test panico", PANICKED);

    assert_eq!(salida.rule, Some("cargo"));
    assert!(
        salida.content.contains("boom: el estado no es valido"),
        "{}",
        salida.content
    );
    assert!(
        salida.content.contains("panicked at tests"),
        "{}",
        salida.content
    );
    assert!(
        salida.content.contains("test explota ... FAILED"),
        "{}",
        salida.content
    );
    assert!(
        salida
            .content
            .contains("test result: FAILED. 2 passed; 1 failed"),
        "{}",
        salida.content
    );
    assert!(
        !salida.content.contains("test bien ... ok"),
        "{}",
        salida.content
    );
}

#[test]
fn build_con_avisos_conserva_el_aviso_y_su_resumen() {
    let salida = bash_cargo("cargo build", BUILD_AVISOS);

    assert_eq!(salida.rule, Some("cargo"));
    assert!(
        salida
            .content
            .contains("warning: unused variable: `sin_usar`"),
        "{}",
        salida.content
    );
    assert!(
        salida.content.contains("src\\aviso.rs:2:9"),
        "{}",
        salida.content
    );
    assert!(
        salida
            .content
            .contains("warning: `toy` (lib) generated 1 warning"),
        "{}",
        salida.content
    );
    assert!(
        !salida.content.contains("Compiling toy"),
        "{}",
        salida.content
    );
    assert_eq!(salida.omitted_lines, 1);
}

#[test]
fn clippy_con_errores_conserva_todos_los_errores() {
    let salida = bash_cargo("cargo clippy -- -D warnings", CLIPPY_ERRORES);

    assert_eq!(salida.rule, Some("cargo"));
    assert!(
        salida
            .content
            .contains("error: unused variable: `sin_usar`"),
        "{}",
        salida.content
    );
    assert!(
        salida
            .content
            .contains("error: unneeded `return` statement"),
        "{}",
        salida.content
    );
    assert!(
        salida
            .content
            .contains("error: could not compile `toy` (lib) due to 2 previous errors"),
        "{}",
        salida.content
    );
    assert!(
        !salida.content.contains("Checking toy"),
        "{}",
        salida.content
    );
    assert_eq!(salida.omitted_lines, 1);
}

#[test]
fn propiedad_ninguna_linea_con_senal_desaparece_en_ninguna_captura() {
    let capturas = [
        ("cargo test --test ok", TODO_OK),
        ("cargo test --test escondido", FALLO_ESCONDIDO),
        ("cargo test --test panico", PANICKED),
        ("cargo build", BUILD_AVISOS),
        ("cargo build --message-format json", BUILD_JSON),
        ("cargo clippy -- -D warnings", CLIPPY_ERRORES),
    ];
    for (comando, texto) in capturas {
        let salida = bash_cargo(comando, texto);
        for linea in texto.lines().filter(|l| tiene_senal(l)) {
            assert!(
                salida.content.contains(linea),
                "{comando}: se perdió «{linea}»\n{}",
                salida.content
            );
        }
    }
}

#[test]
fn con_fallo_la_lista_blanca_no_pierde_senal() {
    let con_exito = bash_cargo("cargo test --test escondido", FALLO_ESCONDIDO);
    let con_error = apply(
        "c1",
        "bash",
        &json!({"command": "cargo test --test escondido"}),
        &ToolOutput::err(FALLO_ESCONDIDO),
        &activo(),
        Some(&volcado()),
    );

    assert_eq!(con_error.rule, Some("cargo"));
    assert!(con_error
        .content
        .contains("test falla_escondido ... FAILED"));
    assert_eq!(con_error.omitted_lines, con_exito.omitted_lines);
}

#[test]
fn formato_json_pasa_sin_tocar() {
    let original = ToolOutput::ok(BUILD_JSON);
    let salida = bash_cargo("cargo build --message-format json", BUILD_JSON);

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, original.content);
}

#[test]
fn idioma_distinto_del_ingles_pasa_sin_regla_cargo() {
    let texto = "Compilando toy v0.1.0\nAdvertencia: variable sin usar\nTerminado en 0.3s\n";
    let salida = bash_cargo("cargo build", texto);

    assert_ne!(salida.rule, Some("cargo"));
    assert_eq!(salida.content, texto);
}

#[test]
fn el_primer_ejecutable_decide_y_con_tuberias_o_encadenados_no_hay_regla() {
    assert_ne!(
        bash_cargo("cargo test | tail -5", TODO_OK).rule,
        Some("cargo")
    );
    assert_ne!(
        bash_cargo("cargo test && echo ok", TODO_OK).rule,
        Some("cargo")
    );
    assert_ne!(bash_cargo("echo cargo test", TODO_OK).rule, Some("cargo"));
    assert_ne!(bash_cargo("cargo fmt --check", TODO_OK).rule, Some("cargo"));
    assert_eq!(bash_cargo("cargo test 2>&1", TODO_OK).rule, Some("cargo"));
}

#[test]
fn raw_true_devuelve_la_salida_de_cargo_sin_filtrar() {
    let original = ToolOutput::ok(TODO_OK);
    let salida = apply(
        "c1",
        "bash",
        &json!({"command": "cargo test --test ok", "raw": true}),
        &original,
        &activo(),
        Some(&volcado()),
    );

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, TODO_OK);
}

#[test]
fn sin_original_recuperable_la_salida_de_cargo_pasa_intacta() {
    let salida = apply(
        "c1",
        "bash",
        &json!({"command": "cargo test --test ok"}),
        &ToolOutput::ok(TODO_OK),
        &activo(),
        None,
    );

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, TODO_OK);
}

#[test]
fn finished_repetido_se_omite_y_queda_el_ultimo() {
    let texto = "   Compiling a v1\n    Finished `dev` a\n    Finished `test` b\nrunning 1 test\ntest t ... ok\ntest result: ok. 1 passed\n";
    let salida = bash_cargo("cargo test", texto);

    assert_eq!(salida.rule, Some("cargo"));
    assert_eq!(
        salida.content.matches("Finished").count(),
        1,
        "{}",
        salida.content
    );
    assert!(
        salida.content.contains("Finished `test` b"),
        "{}",
        salida.content
    );
}

#[test]
fn linea_no_reconocida_se_conserva() {
    let texto = "   Compiling a v1\nsalida inesperada de un plugin\n    Finished `test` b\nrunning 1 test\ntest t ... ok\ntest result: ok. 1 passed\n";
    let salida = bash_cargo("cargo test", texto);

    assert!(
        salida.content.contains("salida inesperada de un plugin"),
        "{}",
        salida.content
    );
}

#[test]
fn salida_desmesurada_se_acota_y_conserva_el_fallo() {
    let mut texto = String::new();
    for i in 0..2_000 {
        texto.push_str(&format!("test caso_{i} ... ok\n"));
        if i == 1_000 {
            texto.push_str("test caso_roto ... FAILED\n");
        }
    }
    let salida = bash_cargo("cargo test", &texto);
    let para_modelo = contenido_para_modelo(
        &activo(),
        Some(&volcado()),
        "c1",
        Some(("bash", &json!({"command": "cargo test"}))),
        &ToolOutput::ok(texto),
        || "RECURSO".to_owned(),
    );

    assert_eq!(salida.rule, Some("cargo"));
    assert!(salida.content.contains("test caso_roto ... FAILED"));
    assert!(
        para_modelo.len() <= MAX_TOOL_OUTPUT_BYTES,
        "{}",
        para_modelo.len()
    );
    assert!(para_modelo.contains("caso_roto ... FAILED"));
}

#[test]
fn una_linea_de_ruido_con_marca_de_senal_nunca_se_omite() {
    // R3: «error» o «failed» en el nombre de un test o de un crate no convierte la línea en ruido.
    let texto = "   Compiling error-chain v1.0.0\n   Compiling otro v0.1.0\n\
                 test manejo_de_error ... ok\ntest otro ... ok\n\
                 test result: ok. 2 passed; 0 failed; 0 ignored\n";
    let salida = bash_cargo("cargo test", texto);

    assert_eq!(salida.rule, Some("cargo"));
    assert!(salida.content.contains("Compiling error-chain"));
    assert!(salida.content.contains("test manejo_de_error ... ok"));
    assert!(!salida.content.contains("Compiling otro"));
    assert!(!salida.content.contains("test otro ... ok"));
}
