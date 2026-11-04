//! Tests: validation.

use super::support::*;
use super::*;

#[test]
fn rejects_invalid_names() {
    for nombre in [
        "Frontend",
        "..",
        "a/b",
        "a\\b",
        "",
        "-x",
        "x-",
        "a_b",
        "frontend-ñ",
    ] {
        let resultado = parse_profile(nombre, &texto_con_nombre(nombre));
        assert!(
            matches!(resultado, Err(ProfileProblem::InvalidName(_))),
            "debía rechazar {nombre:?}"
        );
    }
    for valido in ["frontend", "a1", "x-y-2"] {
        assert!(is_valid_agent_name(valido), "debía aceptar {valido:?}");
    }
    assert!(is_valid_agent_name(&"a".repeat(64)));
    assert!(!is_valid_agent_name(&"a".repeat(65)));
}

#[test]
fn rejects_name_that_differs_from_file_stem() {
    let resultado = parse_profile("frontend", &texto_con_nombre("backend"));
    assert_eq!(
        resultado,
        Err(ProfileProblem::NameMismatch {
            file: "frontend".to_string(),
            declared: "backend".to_string(),
        })
    );
}

#[test]
fn rejects_missing_description() {
    let texto = "---\nname: frontend\n---\n\nCuerpo.\n";
    let resultado = parse_profile("frontend", texto);
    assert_eq!(
        resultado.unwrap_err(),
        ProfileProblem::MissingField("description")
    );
}

#[test]
fn rejects_missing_or_unterminated_frontmatter() {
    let sin_delimitadores = "name: frontend\ndescription: x\n";
    let sin_cierre = "---\nname: frontend\ndescription: x\n";
    let vacio = "";
    for texto in [sin_delimitadores, sin_cierre, vacio] {
        let resultado = parse_profile("frontend", texto);
        assert!(
            matches!(resultado, Err(ProfileProblem::BadFrontmatter(_))),
            "debía rechazar {texto:?}"
        );
    }
}
