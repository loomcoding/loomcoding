//! Pruebas de la capa de compresión. Cada una cita la regla R1–R10 de la spec que cubre.

use super::*;
use crate::tools::MAX_TOOL_OUTPUT_BYTES;
use serde_json::json;
use std::time::Instant;

/// Filtro encendido, como lo deja `[output_filter] enabled = true`.
fn activo() -> FilterConfig {
    FilterConfig::from_toml("[output_filter]\nenabled = true\n").unwrap()
}

fn bash_con(cfg: &FilterConfig, spill: Option<&ToolSpill>, out: &ToolOutput) -> Filtered {
    apply("c1", "bash", &json!({"command": "x"}), out, cfg, spill)
}

/// Volcado en un directorio temporal que se conserva mientras dure el proceso de pruebas.
fn volcado() -> ToolSpill {
    ToolSpill::new(tempfile::tempdir().unwrap().keep())
}

fn bash(texto: &str) -> Filtered {
    bash_con(&activo(), Some(&volcado()), &ToolOutput::ok(texto))
}

#[test]
fn r1_la_cabecera_aparece_siempre_que_se_omite_algo() {
    let dir = tempfile::tempdir().unwrap();
    let spill = ToolSpill::new(dir.path());
    // Cinco líneas: tres en blanco consecutivas se reducen a una, así que se omiten dos.
    let salida = bash_con(&activo(), Some(&spill), &ToolOutput::ok("a\n\n\n\nb"));

    assert_eq!(salida.rule, Some("general"));
    assert_eq!(salida.omitted_lines, 2);
    let ruta = salida.original.clone().expect("hay original");
    let cabecera = format!(
        "[filtrado: 2 líneas omitidas de 5 — regla general; el original sigue en {}]",
        ruta.display()
    );
    assert!(salida.content.starts_with(&cabecera), "{}", salida.content);
    assert_eq!(salida.content, format!("{cabecera}\na\n\nb"));
}

#[test]
fn r2_el_original_integro_queda_en_el_volcado() {
    let dir = tempfile::tempdir().unwrap();
    let spill = ToolSpill::new(dir.path());
    let original = "uno\n\n\n\ndos\n";
    let salida = bash_con(&activo(), Some(&spill), &ToolOutput::ok(original));

    let ruta = salida.original.expect("hay original");
    assert_eq!(std::fs::read_to_string(ruta).unwrap(), original);
}

#[test]
fn r2_raw_true_devuelve_la_salida_sin_filtrar() {
    let original = ToolOutput::ok("a\n\n\n\nb\n");
    let salida = apply(
        "c1",
        "bash",
        &json!({"command": "x", "raw": true}),
        &original,
        &activo(),
        None,
    );
    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, original.content);
    assert_eq!(salida.original, None);
}

#[test]
fn r4_con_fallo_se_aplican_solo_las_reglas_generales() {
    let ruido = "\x1b[31merror: fallo\x1b[0m\n\n\n\nfin\n";
    let spill = volcado();
    let con_exito = bash_con(&activo(), Some(&spill), &ToolOutput::ok(ruido));
    let con_error = bash_con(&activo(), Some(&spill), &ToolOutput::err(ruido));

    assert_eq!(con_error.rule, Some("general"));
    // La cabecera lleva la ruta del original, distinta en cada llamada: se compara el cuerpo.
    let cuerpo = |f: &Filtered| {
        f.content
            .split_once('\n')
            .map(|(_, resto)| resto.to_owned())
    };
    assert_eq!(cuerpo(&con_error), cuerpo(&con_exito));
    assert!(con_error.content.contains("error: fallo"));
    assert!(!con_error.content.contains('\u{1b}'));
}

#[test]
fn r5_el_colapso_dice_cuantas_repeticiones_habia() {
    let salida = bash("x\nx\nx\n");

    assert!(salida.content.contains("x ×3\n"), "{}", salida.content);
    assert_eq!(salida.omitted_lines, 2);
    assert!(
        salida.content.contains("2 líneas omitidas de 3"),
        "{}",
        salida.content
    );
}

#[test]
fn r6_una_salida_sin_nada_que_compactar_pasa_intacta() {
    let original = ToolOutput::ok("mytool: listo\nwarning: conservar\nresultado 42\n");
    let salida = bash_con(&activo(), None, &original);

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, original.content);
}

#[test]
fn r8_una_entrada_de_5_mib_pasa_sin_filtrar_y_rapido() {
    let grande = "a\n\n".repeat(5 * 1024 * 1024 / 3);
    assert!(grande.len() > 5 * 1024 * 1024 - 4096);
    let inicio = Instant::now();
    let salida = bash_con(&activo(), None, &ToolOutput::ok(grande.clone()));

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, grande);
    assert!(inicio.elapsed().as_secs() < 1, "tardó demasiado");
}

#[test]
fn r10_con_enabled_false_no_se_toca_nada() {
    let original = ToolOutput::ok("a\n\n\n\nb\n");
    let salida = bash_con(&FilterConfig::default(), None, &original);

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, original.content);
}

#[test]
fn r10_la_variable_de_entorno_apaga_el_filtro_aunque_la_seccion_lo_encienda() {
    assert!(!activo().sin_entorno(Some("1")).enabled);
    assert!(activo().sin_entorno(None).enabled);
    assert!(activo().sin_entorno(Some("0")).enabled);
}

#[test]
fn utf8_invalido_no_provoca_panico_y_se_filtra() {
    let texto = String::from_utf8_lossy(&[0xff, b'a', b'\n', b'\n', b'\n', b'b']).into_owned();
    let salida = bash(&texto);

    assert_eq!(salida.rule, Some("general"));
    assert!(salida.content.contains("\u{FFFD}a"), "{}", salida.content);
}

#[test]
fn sin_salto_final_la_salida_no_gana_uno() {
    let salida = bash("a\n\n\nb");

    assert_eq!(salida.rule, Some("general"));
    assert!(salida.content.ends_with("\na\n\nb"), "{}", salida.content);
    assert!(!salida.content.ends_with('\n'));
}

#[test]
fn salida_vacia_pasa_sin_cambios() {
    let salida = bash("");
    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, "");
}

#[test]
fn crlf_sin_nada_que_compactar_pasa_intacto() {
    let original = ToolOutput::ok("a\r\nb\r\n");
    let salida = bash_con(&activo(), None, &original);

    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, original.content);
}

#[test]
fn ansi_se_quita_y_el_texto_queda() {
    let salida = bash("\x1b[31merror: fallo\x1b[0m\n");

    assert!(
        salida.content.contains("error: fallo"),
        "{}",
        salida.content
    );
    assert!(!salida.content.contains('\u{1b}'));
}

#[test]
fn progreso_con_retorno_de_carro_conserva_el_ultimo_estado() {
    let salida = bash("Compiling a\rCompiling b\rFinished\n");

    assert!(salida.content.contains("Finished"), "{}", salida.content);
    assert!(
        !salida.content.contains("Compiling a"),
        "{}",
        salida.content
    );
    assert_eq!(salida.omitted_lines, 2);
}

#[test]
fn propiedad_ninguna_linea_con_senal_desaparece() {
    // Señal conocida, ruido y repeticiones mezclados; una línea con progreso y error incluida.
    let senales = [
        "error[E0308]: mismatched types",
        "test x ... FAILED",
        "thread 'main' panicked at src/lib.rs:3",
        "warning: unused variable",
        "error: a\rCompiling 2/3",
    ];
    let mut lineas: Vec<&str> = Vec::new();
    lineas.extend(senales);
    lineas.extend(["ok", "ok", "ok", "", "", "Compiling foo", "Compiling foo"]);
    lineas.extend(senales);
    lineas.extend(["error: repetido", "error: repetido", "error: repetido"]);
    let entrada = lineas.join("\n");

    let salida = bash(&entrada);

    for senal in senales {
        assert!(
            salida.content.contains(senal),
            "perdida: {senal}\n{}",
            salida.content
        );
    }
}

#[test]
fn job_output_filtra_solo_el_texto_y_conserva_la_forma() {
    let dir = tempfile::tempdir().unwrap();
    let spill = ToolSpill::new(dir.path());
    let json = json!({
        "id": "j1",
        "status": "done",
        "text": "a\n\n\nb",
        "next": 6,
        "truncated": false
    })
    .to_string();
    let salida = apply(
        "c1",
        "job_output",
        &json!({"id": "j1"}),
        &ToolOutput::ok(json),
        &activo(),
        Some(&spill),
    );

    assert_eq!(salida.rule, Some("general"));
    let valor: serde_json::Value = serde_json::from_str(&salida.content).unwrap();
    assert_eq!(valor["next"], json!(6));
    assert_eq!(valor["id"], json!("j1"));
    let texto = valor["text"].as_str().unwrap();
    assert!(
        texto.starts_with("[filtrado: 1 líneas omitidas de 4"),
        "{texto}"
    );
}

#[test]
fn job_output_con_raw_pasa_sin_tocar() {
    let json = json!({"id": "j1", "text": "a\n\n\nb", "next": 6}).to_string();
    let salida = apply(
        "c1",
        "job_output",
        &json!({"id": "j1", "raw": true}),
        &ToolOutput::ok(json.clone()),
        &activo(),
        None,
    );
    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, json);
}

#[test]
fn herramientas_fuera_de_la_lista_pasan_intactas() {
    let original = ToolOutput::ok("a\n\n\n\nb\n");
    let salida = apply(
        "c1",
        "read",
        &json!({"path": "x"}),
        &original,
        &activo(),
        None,
    );
    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, original.content);
}

#[test]
fn contenido_para_modelo_sin_filtro_usa_el_recurso_de_siempre() {
    let contenido = contenido_para_modelo(
        &FilterConfig::default(),
        None,
        "c1",
        Some(("bash", &json!({"command": "x"}))),
        &ToolOutput::ok("a\n\n\nb"),
        || "RECURSO".to_owned(),
    );
    assert_eq!(contenido, "RECURSO");
}

#[test]
fn contenido_para_modelo_recorta_lo_filtrado_y_conserva_la_cabecera() {
    let dir = tempfile::tempdir().unwrap();
    let spill = ToolSpill::new(dir.path());
    let voluminosa = "linea\n\n\n".repeat(10_000);
    let contenido = contenido_para_modelo(
        &activo(),
        Some(&spill),
        "c1",
        Some(("bash", &json!({"command": "x"}))),
        &ToolOutput::ok(voluminosa),
        || "RECURSO".to_owned(),
    );

    assert!(
        contenido.len() <= MAX_TOOL_OUTPUT_BYTES,
        "{}",
        contenido.len()
    );
    assert!(
        contenido.contains("[filtrado: "),
        "sin cabecera de filtrado"
    );
}

#[test]
fn la_config_lee_la_seccion_y_el_valor_por_defecto_es_apagado() {
    assert!(!FilterConfig::from_toml("model = \"x\"\n").unwrap().enabled);
    assert!(
        FilterConfig::from_toml("[output_filter]\nenabled = true\n")
            .unwrap()
            .enabled
    );
    assert!(FilterConfig::from_toml("[output_filter]\nenabled = \"si\"\n").is_err());
}

#[test]
fn el_esquema_de_bash_y_job_output_declara_raw() {
    use crate::tools::{bash::BashTool, job_output::JobOutputTool, Tool};
    for esquema in [BashTool.schema(), JobOutputTool.schema()] {
        assert_eq!(esquema["properties"]["raw"]["type"], json!("boolean"));
    }
}

#[test]
fn sin_original_recuperable_la_salida_pasa_intacta() {
    // R2: si no hay dónde guardar el original, no se omite nada.
    let entrada = "a\n\n\n\nb";
    let salida = bash_con(&activo(), None, &ToolOutput::ok(entrada));
    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, entrada);
    assert_eq!(salida.omitted_lines, 0);
}
