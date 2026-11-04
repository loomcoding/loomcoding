//! Tests: approval.

use super::support::*;
use super::*;

#[test]
fn unapproved_agents_are_never_route_candidates() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "rust.md", &perfil("rust", "Experto en Rust"));
    escribir(
        proyecto.path(),
        "front.md",
        &perfil("front", "Interfaz del proyecto"),
    );

    let nombres: Vec<String> = registro_con_global_y_proyecto(global.path(), proyecto.path())
        .snapshot()
        .route_candidates()
        .into_iter()
        .map(|c| c.name)
        .collect();

    assert_eq!(nombres, ["default", "rust"]);
}

#[test]
fn prompt_section_lists_usable_agents_and_flags_the_pending_ones() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "rust.md", &perfil("rust", "Experto en Rust"));
    escribir(
        proyecto.path(),
        "front.md",
        &perfil("front", "Interfaz del proyecto"),
    );

    let seccion = registro_con_global_y_proyecto(global.path(), proyecto.path())
        .snapshot()
        .prompt_section(ConfirmDelegation::Doubt);

    assert!(seccion.contains("`subagent`"), "{seccion}");
    assert!(seccion.contains("do not poll `job_output`"), "{seccion}");
    assert!(seccion.contains("- default (built-in)"), "{seccion}");
    assert!(
        seccion.contains("- rust (global): Experto en Rust"),
        "{seccion}"
    );
    // El de proyecto sin aprobar no se ofrece como disponible, pero se nombra.
    assert!(!seccion.contains("- front"), "{seccion}");
    assert!(
        seccion.contains("awaiting approval: front"),
        "se debe avisar del agente pendiente: {seccion}"
    );
}

#[test]
fn prompt_section_carries_only_the_rule_of_its_mode() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(global.path(), "rust.md", &perfil("rust", "Experto en Rust"));
    let instantanea = registro_con_global_y_proyecto(global.path(), proyecto.path()).snapshot();
    let marcas = [
        (ConfirmDelegation::Doubt, "call `ask_user` first"),
        (ConfirmDelegation::Always, "Do not start any subagent until"),
        (
            ConfirmDelegation::Never,
            "delegate directly. Say in your reply",
        ),
    ];

    for (modo, propia) in marcas {
        let seccion = instantanea.prompt_section(modo);
        assert!(
            seccion.contains(propia),
            "{modo:?} debe llevar su regla: {seccion}"
        );
        for (otro, ajena) in marcas {
            if otro != modo {
                assert!(
                    !seccion.contains(ajena),
                    "{modo:?} no debe llevar la regla de {otro:?}: {seccion}"
                );
            }
        }
    }
}

#[test]
fn approve_and_revoke_flip_a_project_agent_and_a_change_invalidates_it() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    escribir(global.path(), "rust.md", &perfil("rust", "Global"));
    escribir(proyecto.path(), "front.md", &perfil("front", "Interfaz"));
    let registro = registro_con_global_y_proyecto(global.path(), proyecto.path()).with_trust(
        TrustStore::open(datos.path().to_path_buf()).unwrap(),
        proyecto.path().to_path_buf(),
        false,
    );
    assert_eq!(
        registro.snapshot().approval("front"),
        Some(Approval::Pending)
    );

    registro.approve("front").unwrap();
    assert_eq!(
        registro.snapshot().approval("front"),
        Some(Approval::Approved)
    );

    // Un cambio del fichero invalida la aprobación al recargar.
    escribir(proyecto.path(), "front.md", &perfil("front", "Otra cosa"));
    assert_eq!(registro.reload().approval("front"), Some(Approval::Pending));

    registro.approve("front").unwrap();
    registro.revoke("front").unwrap();
    assert_eq!(
        registro.snapshot().approval("front"),
        Some(Approval::Pending)
    );

    assert!(matches!(
        registro.approve("rust"),
        Err(ApprovalError::NoRequerida(_))
    ));
    assert!(matches!(
        registro.approve("nadie"),
        Err(ApprovalError::Desconocido(_))
    ));
}

#[test]
fn approve_without_a_trust_store_is_an_error() {
    let proyecto = tempfile::tempdir().unwrap();
    escribir(proyecto.path(), "front.md", &perfil("front", "Interfaz"));
    let registro = AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(
            proyecto.path().to_path_buf(),
            ProfileLevel::Project,
        )),
    ]);
    assert!(matches!(
        registro.approve("front"),
        Err(ApprovalError::SinAlmacen)
    ));
}

#[test]
fn global_and_builtin_do_not_need_approval() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    escribir(global.path(), "frontend.md", &perfil("frontend", "UI"));
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();

    let instantanea =
        registro_con_confianza(global.path(), proyecto.path(), &almacen, false).snapshot();

    assert_eq!(
        instantanea.approval(AGENTE_POR_DEFECTO),
        Some(Approval::NotRequired)
    );
    assert_eq!(
        instantanea.approval("frontend"),
        Some(Approval::NotRequired)
    );
}

#[test]
fn project_agent_is_pending_until_approved() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    escribir(proyecto.path(), "revisor.md", &perfil("revisor", "Revisa"));
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
    let registro = registro_con_confianza(global.path(), proyecto.path(), &almacen, false);

    let hash = registro
        .snapshot()
        .get("revisor")
        .unwrap()
        .content_hash
        .clone()
        .expect("un agente de fichero lleva huella");
    assert_eq!(
        registro.snapshot().approval("revisor"),
        Some(Approval::Pending)
    );

    almacen.approve(proyecto.path(), "revisor", &hash).unwrap();

    // Sin recargar: la consulta refleja el estado actual del almacén.
    assert_eq!(
        registro.snapshot().approval("revisor"),
        Some(Approval::Approved)
    );
}

#[test]
fn project_agent_without_trust_config_is_pending() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    escribir(proyecto.path(), "revisor.md", &perfil("revisor", "Revisa"));

    let instantanea = registro_con_global_y_proyecto(global.path(), proyecto.path()).snapshot();

    assert_eq!(
        instantanea.approval("revisor"),
        Some(Approval::Pending),
        "sin almacén configurado nada de proyecto se aprueba"
    );
}

#[test]
fn approval_is_invalidated_by_a_content_change() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    escribir(proyecto.path(), "revisor.md", &perfil("revisor", "v1"));
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
    let registro = registro_con_confianza(global.path(), proyecto.path(), &almacen, false);
    let hash_v1 = registro
        .snapshot()
        .get("revisor")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();
    almacen
        .approve(proyecto.path(), "revisor", &hash_v1)
        .unwrap();

    // Un `git pull` que cambia el fichero: la aprobación anterior ya no vale.
    escribir(proyecto.path(), "revisor.md", &perfil("revisor", "v2"));
    registro.reload();

    assert_eq!(
        registro.snapshot().approval("revisor"),
        Some(Approval::Pending)
    );
}

#[test]
fn approval_is_per_project_root() {
    let global = tempfile::tempdir().unwrap();
    let proyecto_a = tempfile::tempdir().unwrap();
    let proyecto_b = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    // Mismo contenido en ambos: la huella coincide, pero la raíz no.
    escribir(
        proyecto_a.path(),
        "revisor.md",
        &perfil("revisor", "Revisa"),
    );
    escribir(
        proyecto_b.path(),
        "revisor.md",
        &perfil("revisor", "Revisa"),
    );
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
    let registro_a = registro_con_confianza(global.path(), proyecto_a.path(), &almacen, false);
    let hash = registro_a
        .snapshot()
        .get("revisor")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();
    almacen
        .approve(proyecto_a.path(), "revisor", &hash)
        .unwrap();

    let registro_b = registro_con_confianza(global.path(), proyecto_b.path(), &almacen, false);

    assert_eq!(
        registro_a.snapshot().approval("revisor"),
        Some(Approval::Approved)
    );
    assert_eq!(
        registro_b.snapshot().approval("revisor"),
        Some(Approval::Pending)
    );
}

#[test]
fn revoke_returns_it_to_pending() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    escribir(proyecto.path(), "revisor.md", &perfil("revisor", "Revisa"));
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
    let registro = registro_con_confianza(global.path(), proyecto.path(), &almacen, false);
    let hash = registro
        .snapshot()
        .get("revisor")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();
    almacen.approve(proyecto.path(), "revisor", &hash).unwrap();

    almacen.revoke(proyecto.path(), "revisor").unwrap();

    assert_eq!(
        registro.snapshot().approval("revisor"),
        Some(Approval::Pending)
    );
}

#[test]
fn trust_all_projects_skips_the_approval() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    escribir(proyecto.path(), "revisor.md", &perfil("revisor", "Revisa"));
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();

    let registro = registro_con_confianza(global.path(), proyecto.path(), &almacen, true);

    assert_eq!(
        registro.snapshot().approval("revisor"),
        Some(Approval::Approved)
    );
}

#[test]
fn approval_of_an_unknown_agent_is_none() {
    let global = tempfile::tempdir().unwrap();
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();

    let instantanea =
        registro_con_confianza(global.path(), proyecto.path(), &almacen, false).snapshot();

    assert_eq!(instantanea.approval("no-existe"), None);
}
