//! Tests: remote.

use super::support::*;
use super::*;

#[test]
fn a_remote_profile_carries_its_endpoint_and_is_not_reported_as_not_applied() {
    let texto = perfil_remoto("transport: a2a-remote\na2a:\n  endpoint: https://agente.example\n");

    let (perfil, avisos) = parse_profile("lejano", &texto).unwrap();

    assert_eq!(
        perfil.remote_endpoint.as_deref(),
        Some("https://agente.example")
    );
    assert!(perfil.declared_not_applied.is_empty(), "{perfil:?}");
    assert!(avisos.is_empty(), "{avisos:?}");
}

#[test]
fn a_remote_profile_without_an_endpoint_does_not_load() {
    for extra in [
        "transport: a2a-remote\n",
        "transport: a2a-remote\na2a:\n  endpoint: \"  \"\n",
        "transport: a2a-remote\na2a:\n  otra: 1\n",
    ] {
        let resultado = parse_profile("lejano", &perfil_remoto(extra));
        assert_eq!(
            resultado,
            Err(ProfileProblem::MissingField("a2a.endpoint")),
            "{extra}"
        );
    }
}

#[test]
fn a_remote_profile_reads_its_credentials() {
    let texto = perfil_remoto(
        "transport: a2a-remote\na2a:\n  endpoint: https://agente.example\n  credentials:\n    claveApi: AGENTE_KEY\n    otro: _TOKEN2\n",
    );

    let (perfil, avisos) = parse_profile("lejano", &texto).unwrap();

    assert!(avisos.is_empty(), "{avisos:?}");
    assert_eq!(
        perfil
            .remote_credentials
            .get("claveApi")
            .map(String::as_str),
        Some("AGENTE_KEY")
    );
    assert_eq!(perfil.remote_credentials.len(), 2);
}

#[test]
fn a_credential_that_is_not_an_env_var_name_does_not_load_and_is_not_echoed() {
    for valor in ["sk-abc123 secreto", "con espacio", "1EMPIEZA", "a.b"] {
        let texto = perfil_remoto(&format!(
            "transport: a2a-remote\na2a:\n  endpoint: https://agente.example\n  credentials:\n    claveApi: \"{valor}\"\n"
        ));

        let error = parse_profile("lejano", &texto).unwrap_err();

        assert!(
            matches!(error, ProfileProblem::BadFrontmatter(_)),
            "{valor}: {error:?}"
        );
        assert!(!error.to_string().contains(valor), "{error}");
    }
}

#[test]
fn credentials_must_be_a_map_and_are_capped() {
    let texto = perfil_remoto(
        "transport: a2a-remote\na2a:\n  endpoint: https://agente.example\n  credentials: [uno]\n",
    );
    assert!(parse_profile("lejano", &texto).is_err());

    let muchas: String = (0..30).map(|i| format!("    e{i}: VAR{i}\n")).collect();
    let texto = perfil_remoto(&format!(
        "transport: a2a-remote\na2a:\n  endpoint: https://agente.example\n  credentials:\n{muchas}"
    ));
    assert!(parse_profile("lejano", &texto).is_err());
}

#[test]
fn a_remote_profile_without_credentials_has_none() {
    let texto = perfil_remoto("transport: a2a-remote\na2a:\n  endpoint: https://agente.example\n");

    let (perfil, _) = parse_profile("lejano", &texto).unwrap();

    assert!(perfil.remote_credentials.is_empty());
}

#[test]
fn a_remote_endpoint_over_plain_http_outside_loopback_does_not_load() {
    let texto = perfil_remoto("transport: a2a-remote\na2a:\n  endpoint: http://agente.example\n");

    let resultado = parse_profile("lejano", &texto);

    assert!(
        matches!(resultado, Err(ProfileProblem::BadFrontmatter(_))),
        "{resultado:?}"
    );
}

#[test]
fn another_transport_stays_declared_and_not_applied() {
    let texto = perfil_remoto("transport: in-process\n");

    let (perfil, _) = parse_profile("lejano", &texto).unwrap();

    assert_eq!(perfil.remote_endpoint, None);
    assert!(perfil
        .declared_not_applied
        .contains(&"transport".to_string()));
}
