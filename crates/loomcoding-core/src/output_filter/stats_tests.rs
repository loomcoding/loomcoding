//! Pruebas de la medición (Tarea 5). Sin reloj: el estado depende solo de la secuencia de llamadas.

use super::stats::{huella_de, FilterStats, UMBRAL_REPETICION_PCT, USOS_MINIMOS};
use super::*;
use serde_json::json;
use std::sync::Arc;

const CARGO: &str = "cargo";

/// Simula `usos` salidas filtradas de `regla`. Tras cada una, las `repeticiones` primeras veces el
/// modelo vuelve a pedir el resultado: con `raw` pide la salida sin filtrar, si no repite el comando.
/// Devuelve todas las reglas que se desactivaron durante la simulación.
fn simula(
    stats: &FilterStats,
    regla: &'static str,
    usos: u64,
    repeticiones: u64,
    raw: bool,
) -> Vec<&'static str> {
    let mut desactivadas = Vec::new();
    for i in 0..usos {
        let huella = i + 1;
        desactivadas.extend(stats.observa_llamada(huella, false));
        desactivadas.extend(stats.registra_uso(regla, huella, 1_000, 200));
        if i < repeticiones {
            desactivadas.extend(stats.observa_llamada(huella, raw));
        }
    }
    desactivadas
}

#[test]
fn diez_usos_con_tres_repeticiones_desactivan_la_regla_y_avisan_una_sola_vez() {
    let stats = FilterStats::default();
    let avisos = simula(&stats, CARGO, USOS_MINIMOS, 3, true);

    assert_eq!(avisos, vec![CARGO], "debe avisar exactamente una vez");
    assert!(stats.desactivada(CARGO));
    // Más re-peticiones después no deben volver a avisar.
    let despues = simula(&stats, CARGO, 5, 5, true);
    assert!(despues.is_empty(), "avisó otra vez: {despues:?}");
}

#[test]
fn repetir_el_mismo_comando_tambien_cuenta_como_repeticion() {
    let stats = FilterStats::default();
    let avisos = simula(&stats, CARGO, USOS_MINIMOS, 3, false);

    assert_eq!(avisos, vec![CARGO]);
    assert!(stats.desactivada(CARGO));
}

#[test]
fn diez_usos_con_una_repeticion_no_desactivan() {
    let stats = FilterStats::default();
    let avisos = simula(&stats, CARGO, USOS_MINIMOS, 1, true);

    assert!(avisos.is_empty(), "{avisos:?}");
    assert!(!stats.desactivada(CARGO));
    assert_eq!(stats.contadores(CARGO).repeticiones, 1);
}

#[test]
fn el_umbral_es_estricto_al_veinte_por_ciento() {
    // Exactamente el 20 % (2 de 10) no basta: hace falta pasar del umbral.
    let stats = FilterStats::default();
    simula(&stats, CARGO, USOS_MINIMOS, 2, true);
    assert_eq!(
        UMBRAL_REPETICION_PCT, 20,
        "la prueba asume el 20 % de la spec"
    );
    assert!(!stats.desactivada(CARGO));
}

#[test]
fn con_menos_de_diez_usos_nunca_se_desactiva() {
    let stats = FilterStats::default();
    let avisos = simula(&stats, CARGO, USOS_MINIMOS - 1, USOS_MINIMOS - 1, true);

    assert!(avisos.is_empty(), "{avisos:?}");
    assert!(!stats.desactivada(CARGO));
    assert_eq!(stats.contadores(CARGO).usos, USOS_MINIMOS - 1);
}

#[test]
fn una_peticion_raw_sin_salida_filtrada_previa_no_cuenta() {
    let stats = FilterStats::default();
    stats.observa_llamada(7, true);

    assert_eq!(stats.contadores(CARGO).repeticiones, 0);
    assert_eq!(stats.contadores("general").repeticiones, 0);
}

#[test]
fn ahorro_por_regla_suma_bytes_originales_y_enviados() {
    let stats = FilterStats::default();
    stats.registra_uso("git", 1, 5_000, 800);
    stats.registra_uso("git", 2, 3_000, 200);

    let c = stats.contadores("git");
    assert_eq!(c.usos, 2);
    assert_eq!(c.bytes_originales, 8_000);
    assert_eq!(c.bytes_enviados, 1_000);
}

#[test]
fn estado_seguro_con_llamadas_concurrentes() {
    let stats = Arc::new(FilterStats::default());
    let hilos: Vec<_> = (0..8)
        .map(|_| {
            let stats = Arc::clone(&stats);
            std::thread::spawn(move || {
                for i in 0..200 {
                    stats.registra_uso("general", i, 10, 4);
                }
            })
        })
        .collect();
    for hilo in hilos {
        hilo.join().unwrap();
    }

    let c = stats.contadores("general");
    assert_eq!(c.usos, 1_600);
    assert_eq!(c.bytes_originales, 16_000);
    assert_eq!(c.bytes_enviados, 6_400);
    assert!(!c.desactivada);
}

#[test]
fn el_contador_no_contiene_el_comando() {
    let stats = FilterStats::default();
    let huella = huella_de(&json!({"command": "cat /home/usuario/secreto.txt"}));
    stats.registra_uso(CARGO, huella, 100, 50);
    stats.observa_llamada(huella, true);

    let volcado = format!("{stats:?} {:?}", stats.contadores(CARGO));
    assert!(!volcado.contains("secreto"), "{volcado}");
    assert!(!volcado.contains("usuario"), "{volcado}");
    assert!(!volcado.contains("/home"), "{volcado}");
}

#[test]
fn una_regla_desactivada_no_se_aplica_ni_siquiera_con_veto_en_filtrar() {
    // R10 de la medición: con la regla vetada, la salida pasa intacta y no se guarda original.
    let salida = filtrar(
        "c1",
        "bash",
        &json!({"command": "cargo test"}),
        &crate::tools::ToolOutput::ok("a\n\n\nb"),
        &FilterConfig::from_toml("[output_filter]\nenabled = true\n").unwrap(),
        None,
        &|regla| regla == "general",
    );
    assert_eq!(salida.rule, None);
    assert_eq!(salida.content, "a\n\n\nb");
}
