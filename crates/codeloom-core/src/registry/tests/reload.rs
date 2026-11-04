//! Tests: reload.

use super::support::*;
use super::*;

#[test]
fn new_file_is_added_and_published() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    let registro = registro_con_global_y_proyecto(global.path(), proyecto.path());
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    escribir(global.path(), "frontend.md", &perfil("frontend", "UI"));
    registro.reload_and_publish(&bus);

    assert!(registro.snapshot().get("frontend").is_some());
    match ultimo_evento(&mut rx) {
        Some(LiveEvent::AgentRegistryChanged {
            added,
            removed,
            changed,
            problems,
        }) => {
            assert_eq!(added, vec!["frontend".to_string()]);
            assert!(removed.is_empty());
            assert!(changed.is_empty());
            assert_eq!(problems, 0);
        }
        otro => panic!("se esperaba AgentRegistryChanged, llegó {otro:?}"),
    }
}

#[test]
fn edited_file_is_reported_as_changed() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "frontend.md", &perfil("frontend", "v1"));
    let registro = registro_con_global_y_proyecto(global.path(), proyecto.path());
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    escribir(global.path(), "frontend.md", &perfil("frontend", "v2"));
    registro.reload_and_publish(&bus);

    match ultimo_evento(&mut rx) {
        Some(LiveEvent::AgentRegistryChanged {
            added,
            removed,
            changed,
            ..
        }) => {
            assert!(added.is_empty());
            assert!(removed.is_empty());
            assert_eq!(changed, vec!["frontend".to_string()]);
        }
        otro => panic!("se esperaba AgentRegistryChanged, llegó {otro:?}"),
    }
}

#[test]
fn deleted_file_is_removed() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "frontend.md", &perfil("frontend", "UI"));
    let registro = registro_con_global_y_proyecto(global.path(), proyecto.path());
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    std::fs::remove_file(global.path().join("frontend.md")).unwrap();
    registro.reload_and_publish(&bus);

    assert!(registro.snapshot().get("frontend").is_none());
    match ultimo_evento(&mut rx) {
        Some(LiveEvent::AgentRegistryChanged { removed, .. }) => {
            assert_eq!(removed, vec!["frontend".to_string()]);
        }
        otro => panic!("se esperaba AgentRegistryChanged, llegó {otro:?}"),
    }
}

#[test]
fn broken_edit_keeps_the_last_valid_version_and_reports_the_problem() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "frontend.md", &perfil("frontend", "buena"));
    let registro = registro_con_global_y_proyecto(global.path(), proyecto.path());
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    // Guardado a medias: el frontmatter queda sin cerrar.
    escribir(
        global.path(),
        "frontend.md",
        "---\nname: frontend\ndescription: ",
    );
    registro.reload_and_publish(&bus);

    let instantanea = registro.snapshot();
    let entrada = instantanea
        .get("frontend")
        .expect("la última versión válida debe conservarse");
    assert_eq!(entrada.card.description, "buena");
    assert!(instantanea.problems().iter().any(|p| matches!(
        p,
        LoadProblem::Profile { path, .. } if path == &global.path().join("frontend.md")
    )));
    match ultimo_evento(&mut rx) {
        Some(LiveEvent::AgentRegistryChanged {
            added,
            removed,
            changed,
            problems,
        }) => {
            assert!(added.is_empty() && removed.is_empty() && changed.is_empty());
            assert_eq!(problems, 1);
        }
        otro => panic!("se esperaba AgentRegistryChanged por el problema, llegó {otro:?}"),
    }
}

#[test]
fn unchanged_reload_publishes_nothing() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "frontend.md", &perfil("frontend", "UI"));
    let registro = registro_con_global_y_proyecto(global.path(), proyecto.path());
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    registro.reload_and_publish(&bus);

    assert!(ultimo_evento(&mut rx).is_none());
}
