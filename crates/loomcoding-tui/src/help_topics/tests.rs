//! Pruebas de `help_topics`: cada comando tiene ayuda, sus ejemplos están en la referencia en los
//! dos idiomas, los temas existen en disco y la búsqueda de comandos tolera erratas.

use super::*;
use crate::commands::{Command, COMMANDS};
use std::path::{Path, PathBuf};

/// Ruta absoluta de un fichero del repositorio, relativa a la raíz.
fn raiz(relativa: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relativa)
}

fn referencia(idioma: &str) -> String {
    let ruta = raiz(&format!("docs/user/{idioma}/reference/commands.md"));
    std::fs::read_to_string(&ruta).unwrap_or_else(|error| panic!("{}: {error}", ruta.display()))
}

#[test]
fn cada_comando_tiene_uso_que_empieza_por_su_nombre_y_ejemplo() {
    for (command, name, _) in COMMANDS {
        let help = command_help(*command);
        assert!(
            help.usage.starts_with(name),
            "el uso de {name} debe empezar por su nombre: {:?}",
            help.usage
        );
        assert!(
            help.example.starts_with('/'),
            "el ejemplo de {name} debe empezar por barra: {:?}",
            help.example
        );
    }
}

#[test]
fn el_ejemplo_de_cada_comando_aparece_textual_en_la_referencia_en_y_es() {
    let en = referencia("en");
    let es = referencia("es");
    for (command, name, _) in COMMANDS {
        let ejemplo = command_help(*command).example;
        assert!(
            en.contains(ejemplo),
            "en: falta el ejemplo {ejemplo:?} de {name}"
        );
        assert!(
            es.contains(ejemplo),
            "es: falta el ejemplo {ejemplo:?} de {name}"
        );
    }
}

#[test]
fn la_referencia_de_comandos_existe_en_los_dos_idiomas() {
    assert!(raiz(COMMANDS_DOC).is_file(), "falta {COMMANDS_DOC}");
    assert!(raiz(&COMMANDS_DOC.replacen("/en/", "/es/", 1)).is_file());
}

#[test]
fn cada_tema_tiene_descripcion_y_pagina_en_los_dos_idiomas() {
    for (id, descripcion, ruta) in TOPICS {
        assert!(!descripcion.is_empty(), "{id} sin descripción");
        assert!(
            ruta.contains("/en/"),
            "{id}: la página debe estar en en/: {ruta}"
        );
        assert!(raiz(ruta).is_file(), "{id}: no existe {ruta}");
        let gemela = ruta.replacen("/en/", "/es/", 1);
        assert!(
            raiz(&gemela).is_file(),
            "{id}: no existe la versión es {gemela}"
        );
    }
}

#[test]
fn los_identificadores_de_tema_son_unicos() {
    let mut ids: Vec<&str> = TOPICS.iter().map(|(id, _, _)| *id).collect();
    ids.sort_unstable();
    let total = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), total, "hay temas con el mismo identificador");
}

#[test]
fn find_command_acepta_con_o_sin_barra_sin_mayusculas_y_alias_oculto() {
    assert_eq!(find_command("/model"), Some(Command::Model));
    assert_eq!(find_command("model"), Some(Command::Model));
    assert_eq!(find_command("MODEL"), Some(Command::Model));
    assert_eq!(find_command("theme"), Some(Command::Theme));
    assert_eq!(find_command("nope"), None);
    assert_eq!(find_command(""), None);
}

#[test]
fn closest_command_sugiere_el_mas_parecido() {
    assert_eq!(closest_command("modl"), Some("/model"));
    assert_eq!(closest_command("/quti"), Some("/quit"));
    assert_eq!(closest_command("zzzz"), None);
    // Una palabra corta a dos ediciones de `/focus` no es una errata de `/focus`.
    assert_eq!(closest_command("bogus"), None);
    assert_eq!(closest_command(""), None);
}

#[test]
fn closest_command_prefiere_el_prefijo_unico() {
    assert_eq!(closest_command("refresh"), Some("/refresh-agents"));
}

#[test]
fn el_mensaje_sin_ayuda_nombra_lo_escrito_y_sugiere_si_hay_cercano() {
    assert_eq!(
        no_help_message("bogus", None),
        "No help for 'bogus' · /help lists the topics"
    );
    assert_eq!(
        no_help_message("modl", Some("/model")),
        "No help for 'modl' · did you mean /model? · /help lists the topics"
    );
}
