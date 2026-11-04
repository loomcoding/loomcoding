//! Tests: parsing.

use super::support::*;
use super::*;

#[test]
fn parses_a_complete_profile() {
    let (perfil, problemas) = parse_profile("frontend", &perfil_completo()).unwrap();
    assert_eq!(perfil.name, "frontend");
    assert_eq!(
        perfil.description,
        "Especialista en UI, componentes y accesibilidad"
    );
    assert_eq!(perfil.version.as_deref(), Some("1"));
    assert_eq!(
        perfil.persona_role.as_deref(),
        Some("Ingeniero de frontend")
    );
    assert_eq!(
        perfil.persona_goal.as_deref(),
        Some("Implementar y revisar UI del proyecto")
    );
    assert_eq!(
        perfil.persona_backstory.as_deref(),
        Some("Prefiere componentes pequeños, testeables y accesibles")
    );
    assert_eq!(perfil.model.as_deref(), Some("claude-sonnet"));
    assert_eq!(perfil.effort.as_deref(), Some("high"));
    assert_eq!(perfil.max_turns, Some(20));
    assert_eq!(perfil.max_depth, Some(1));
    assert_eq!(
        perfil.body,
        "Eres el agente de frontend. Solo atiendes tareas de UI, componentes y accesibilidad.\nSi la tarea es de otra especialidad, dilo y delega con `delegate`."
    );
    // `transport: a2a-remote` con `a2a.endpoint` se aplica (A8a); `scope` y `delegation`, no.
    assert_eq!(
        perfil.remote_endpoint.as_deref(),
        Some("https://devops.internal/.well-known/agent-card.json")
    );
    assert_eq!(perfil.declared_not_applied.len(), 2);
    for clave in ["scope", "delegation"] {
        assert!(perfil.declared_not_applied.iter().any(|c| c == clave));
        assert!(problemas.contains(&ProfileProblem::DeclaredNotApplied(clave.to_string())));
    }
    for clave in ["transport", "a2a"] {
        assert!(!problemas.contains(&ProfileProblem::DeclaredNotApplied(clave.to_string())));
    }
    assert!(!problemas
        .iter()
        .any(|p| matches!(p, ProfileProblem::UnknownField(_))));
}

#[test]
fn parses_a_minimal_profile() {
    let (perfil, problemas) = parse_profile("frontend", &perfil_minimo()).unwrap();
    assert_eq!(perfil.name, "frontend");
    assert_eq!(perfil.description, "Un agente de prueba");
    assert_eq!(perfil.version, None);
    assert_eq!(perfil.persona_role, None);
    assert_eq!(perfil.persona_goal, None);
    assert_eq!(perfil.persona_backstory, None);
    assert_eq!(perfil.model, None);
    assert_eq!(perfil.provider, None);
    assert_eq!(perfil.effort, None);
    assert_eq!(perfil.max_turns, None);
    assert_eq!(perfil.max_depth, None);
    assert!(perfil.declared_not_applied.is_empty());
    assert_eq!(perfil.body, "Eres el agente.");
    assert!(problemas.is_empty());
}

#[test]
fn local_profile_parses_a_valid_provider_without_a_secret_value() {
    let texto = "---\nname: backend\ndescription: Rust\nmodel: recursive/flint-v1.0:free\nprovider:\n  kind: openai_compat\n  base_url: https://openrouter.ai/api/v1\n  api_key_env: OPENROUTER_API_KEY\n---\n\nBackend.\n";

    let (profile, problems) = parse_profile("backend", texto).unwrap();

    assert_eq!(profile.model.as_deref(), Some("recursive/flint-v1.0:free"));
    assert_eq!(
        profile.provider,
        Some(ProviderConfig {
            kind: crate::config::ProviderKind::OpenAiCompat,
            base_url: "https://openrouter.ai/api/v1".into(),
            api_key_env: "OPENROUTER_API_KEY".into(),
        })
    );
    assert!(problems.is_empty());
}

#[test]
fn local_profile_rejects_an_insecure_provider_url_or_literal_secret() {
    for provider in [
        "kind: openai_compat\n  base_url: http://openrouter.ai/api/v1\n  api_key_env: OPENROUTER_API_KEY",
        "kind: openai_compat\n  base_url: https://openrouter.ai/api/v1\n  api_key_env: sk-or-v1-secret",
    ] {
        let texto = format!(
            "---\nname: backend\ndescription: Rust\nprovider:\n  {provider}\n---\n\nBackend.\n"
        );
        assert!(matches!(
            parse_profile("backend", &texto),
            Err(ProfileProblem::InvalidProvider)
        ));
    }
}

#[test]
fn unknown_fields_warn_but_load() {
    let texto =
        "---\nname: frontend\ndescription: Un agente de prueba\ncolor: azul\n---\n\nCuerpo.\n";
    let (perfil, problemas) = parse_profile("frontend", texto).unwrap();
    assert_eq!(perfil.name, "frontend");
    assert!(problemas.contains(&ProfileProblem::UnknownField("color".to_string())));
}

#[test]
fn body_is_everything_after_the_frontmatter() {
    let texto = "---\nname: frontend\ndescription: x\n---\n\nPrimera línea.\n\n---\nSegunda sección con regla.\n";
    let (perfil, _) = parse_profile("frontend", texto).unwrap();
    assert_eq!(
        perfil.body,
        "Primera línea.\n\n---\nSegunda sección con regla."
    );
}

#[test]
fn crlf_input_is_accepted() {
    let lf = parse_profile("frontend", &perfil_completo()).unwrap().0;
    let crlf = parse_profile("frontend", &perfil_completo().replace('\n', "\r\n"))
        .unwrap()
        .0;
    assert_eq!(crlf, lf);
}
