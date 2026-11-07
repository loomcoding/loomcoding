//! Sección de prompt de ODD: texto acotado y estático, que nombra la herramienta y el cierre.

use crate::odd::prompt::{OddPrompt, TEXTO};
use crate::odd::state::OddState;
use loomcoding_core::agent::PromptContributor;
use loomcoding_core::tools::ToolRegistry;

#[test]
fn el_texto_cabe_en_su_tope() {
    assert!(TEXTO.len() <= 2_000, "el texto ocupa {} bytes", TEXTO.len());
}

#[test]
fn el_texto_nombra_la_herramienta_y_la_decision() {
    assert!(TEXTO.contains("odd_track"));
    assert!(TEXTO.contains("Necesita tu decisión"));
}

#[test]
fn apagado_no_aporta_seccion() {
    let estado = OddState::new();
    estado.set_enabled(false);
    assert_eq!(OddPrompt::new(estado).section(), None);
}

#[test]
fn encendido_aporta_siempre_el_mismo_texto() {
    let prompt = OddPrompt::new(OddState::new());
    let primera = prompt.section();
    assert_eq!(primera.as_deref(), Some(TEXTO));
    assert_eq!(prompt.section(), primera);
}

#[test]
fn solo_odd_track_no_se_ofrece_a_los_hijos() {
    let mut registro = ToolRegistry::new();
    let _estado = crate::register(&mut registro);
    let herramienta = registro.get("odd_track").expect("odd_track registrada");
    assert!(herramienta.parent_only());
    for spec in registro.specs() {
        if spec.name != "odd_track" {
            let otra = registro.get(&spec.name).expect("herramienta listada");
            assert!(
                !otra.parent_only(),
                "{} no debe ser solo del padre",
                spec.name
            );
        }
    }
}
