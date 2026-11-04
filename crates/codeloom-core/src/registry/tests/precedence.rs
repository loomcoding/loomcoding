//! Tests: precedence.

use super::support::*;
use super::*;

#[test]
fn builtin_default_is_always_present() {
    let registro = AgentRegistry::new(vec![Box::new(BuiltInSource)]);
    let instantanea = registro.snapshot();

    let defecto = instantanea
        .get(AGENTE_POR_DEFECTO)
        .expect("el agente `default` debe existir siempre");
    assert_eq!(defecto.origin, AgentOrigin::BuiltIn);
    assert_eq!(
        defecto.execution,
        Execution::Local(LocalSpec {
            body: String::new(),
            persona_role: None,
            persona_goal: None,
            persona_backstory: None,
            model: None,
            provider: None,
            scope: crate::registry::profile::AgentScope::default(),
            effort: None,
            max_turns: None,
            max_depth: None,
        })
    );
    assert!(instantanea.problems().is_empty());
}

#[test]
fn project_overrides_global_overrides_builtin() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "frontend.md", &perfil("frontend", "global"));
    escribir(
        global.path(),
        "default.md",
        &perfil("default", "por defecto global"),
    );
    escribir(
        proyecto.path(),
        "frontend.md",
        &perfil("frontend", "proyecto"),
    );

    let instantanea = registro_con_global_y_proyecto(global.path(), proyecto.path()).snapshot();

    let frontend = instantanea.get("frontend").unwrap();
    assert_eq!(frontend.card.description, "proyecto");
    assert_eq!(
        frontend.origin,
        AgentOrigin::File {
            path: proyecto.path().join("frontend.md"),
            level: ProfileLevel::Project,
        }
    );
    let defecto = instantanea.get(AGENTE_POR_DEFECTO).unwrap();
    assert_eq!(
        defecto.origin,
        AgentOrigin::File {
            path: global.path().join("default.md"),
            level: ProfileLevel::Global,
        }
    );
}

#[test]
fn overridden_agent_is_reported_as_shadowed() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "frontend.md", &perfil("frontend", "global"));
    escribir(
        proyecto.path(),
        "frontend.md",
        &perfil("frontend", "proyecto"),
    );

    let instantanea = registro_con_global_y_proyecto(global.path(), proyecto.path()).snapshot();

    let esperado = LoadProblem::Shadowed {
        name: "frontend".to_string(),
        by: AgentOrigin::File {
            path: proyecto.path().join("frontend.md"),
            level: ProfileLevel::Project,
        },
        loser: AgentOrigin::File {
            path: global.path().join("frontend.md"),
            level: ProfileLevel::Global,
        },
    };
    assert!(instantanea.problems().contains(&esperado));
}

#[test]
fn default_can_be_redefined_by_a_file() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(
        global.path(),
        "default.md",
        &perfil("default", "mi defecto"),
    );

    let instantanea = registro_con_global_y_proyecto(global.path(), proyecto.path()).snapshot();

    let defecto = instantanea.get(AGENTE_POR_DEFECTO).unwrap();
    assert_eq!(defecto.card.description, "mi defecto");
    assert!(matches!(defecto.origin, AgentOrigin::File { .. }));
    assert!(instantanea.problems().contains(&LoadProblem::Shadowed {
        name: AGENTE_POR_DEFECTO.to_string(),
        by: AgentOrigin::File {
            path: global.path().join("default.md"),
            level: ProfileLevel::Global,
        },
        loser: AgentOrigin::BuiltIn,
    }));
}

#[test]
fn card_is_synthesized_without_persona_model_or_effort() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(
        global.path(),
        "frontend.md",
        "---\nname: frontend\ndescription: UI\nversion: 2\npersona:\n  role: Ingeniero\nmodel: claude-sonnet\neffort: high\n---\n\nCuerpo.\n",
    );

    let instantanea = registro_con_global_y_proyecto(global.path(), proyecto.path()).snapshot();

    let entrada = instantanea.get("frontend").unwrap();
    assert_eq!(entrada.card, AgentCard::local("frontend", "UI", "2"));
    assert!(entrada.card.skills.is_empty());
    // El modelo y el esfuerzo sí existen, pero solo en la ejecución local.
    match &entrada.execution {
        Execution::Local(spec) => {
            assert_eq!(spec.model.as_deref(), Some("claude-sonnet"));
            assert_eq!(spec.effort.as_deref(), Some("high"));
        }
        Execution::Remote(_) => panic!("un perfil de fichero es local"),
    }
}

#[test]
fn card_is_not_servable() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "frontend.md", &perfil("frontend", "UI"));

    let instantanea = registro_con_global_y_proyecto(global.path(), proyecto.path()).snapshot();

    assert!(!instantanea.get("frontend").unwrap().card.is_servable());
    assert!(!instantanea
        .get(AGENTE_POR_DEFECTO)
        .unwrap()
        .card
        .is_servable());
}

#[test]
fn invalid_profile_is_reported_and_others_still_load() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "roto.md", "sin frontmatter\n");
    escribir(global.path(), "bueno.md", &perfil("bueno", "Carga bien"));

    let instantanea = registro_con_global_y_proyecto(global.path(), proyecto.path()).snapshot();

    assert!(instantanea.get("bueno").is_some());
    assert!(instantanea.get("roto").is_none());
    assert!(instantanea.problems().iter().any(|p| matches!(
        p,
        LoadProblem::Profile { path, problem: ProfileProblem::BadFrontmatter(_) }
            if path == &global.path().join("roto.md")
    )));
}

#[test]
fn snapshot_is_immutable_across_reload() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "frontend.md", &perfil("frontend", "v1"));
    let registro = registro_con_global_y_proyecto(global.path(), proyecto.path());
    let antes = registro.snapshot();

    escribir(global.path(), "frontend.md", &perfil("frontend", "v2"));
    let despues = registro.reload();

    assert_eq!(antes.get("frontend").unwrap().card.description, "v1");
    assert_eq!(despues.get("frontend").unwrap().card.description, "v2");
    assert_eq!(
        registro
            .snapshot()
            .get("frontend")
            .unwrap()
            .card
            .description,
        "v2"
    );
}

#[test]
fn precedence_is_global_then_catalog_then_project() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    let catalogo = catalogo_con(&[("a", "catalogo"), ("b", "catalogo")]);
    escribir(global.path(), "a.md", &perfil("a", "global"));
    escribir(proyecto.path(), "b.md", &perfil("b", "proyecto"));
    let registro = AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(
            global.path().to_path_buf(),
            ProfileLevel::Global,
        )),
        Box::new(FileSource::new(
            catalogo.path().join(".agents").join("agents"),
            ProfileLevel::Catalog,
        )),
        Box::new(FileSource::new(
            proyecto.path().to_path_buf(),
            ProfileLevel::Project,
        )),
    ]);
    let instantanea = registro.snapshot();
    assert_eq!(instantanea.get("a").unwrap().card.description, "catalogo");
    assert_eq!(instantanea.get("b").unwrap().card.description, "proyecto");
    let sombreados = instantanea
        .problems()
        .iter()
        .filter(|p| matches!(p, LoadProblem::Shadowed { .. }))
        .count();
    assert_eq!(sombreados, 2);
}
