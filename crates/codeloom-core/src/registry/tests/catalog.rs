//! Tests: catalog.

use super::support::*;
use super::*;

#[test]
fn a_catalog_agent_is_pending_even_when_project_agents_are_trusted() {
    let catalogo = catalogo_con(&[("revisor", "Revisa")]);
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
    let registro = registro_con_catalogo(catalogo.path(), proyecto.path(), &almacen, true);
    assert_eq!(
        registro.snapshot().approval("revisor"),
        Some(Approval::Pending)
    );
}

#[test]
fn a_catalog_approval_is_keyed_by_the_catalog_root_not_by_the_project() {
    let catalogo = catalogo_con(&[("revisor", "Revisa")]);
    let proyecto_a = tempfile::tempdir().unwrap();
    let proyecto_b = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
    let registro_a = registro_con_catalogo(catalogo.path(), proyecto_a.path(), &almacen, false);
    registro_a.approve("revisor").unwrap();
    assert_eq!(
        registro_a.snapshot().approval("revisor"),
        Some(Approval::Approved)
    );
    // Otro proyecto, mismo catálogo: la aprobación sigue valiendo.
    let registro_b = registro_con_catalogo(catalogo.path(), proyecto_b.path(), &almacen, false);
    assert_eq!(
        registro_b.snapshot().approval("revisor"),
        Some(Approval::Approved)
    );
    registro_b.revoke("revisor").unwrap();
    assert_eq!(
        registro_a.reload().approval("revisor"),
        Some(Approval::Pending)
    );
}

#[test]
fn a_changed_catalog_profile_loses_its_approval() {
    let catalogo = catalogo_con(&[("revisor", "Revisa")]);
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
    let registro = registro_con_catalogo(catalogo.path(), proyecto.path(), &almacen, false);
    registro.approve("revisor").unwrap();
    let dir = catalogo.path().join(".agents").join("agents");
    escribir(&dir, "revisor.md", &perfil("revisor", "Otra cosa"));
    assert_eq!(
        registro.reload().approval("revisor"),
        Some(Approval::Pending)
    );
}

#[test]
fn only_approved_catalog_agents_reach_the_prompt_labelled_as_catalog() {
    let catalogo = catalogo_con(&[("revisor", "Revisa"), ("otro", "Sin aprobar")]);
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
    let registro = registro_con_catalogo(catalogo.path(), proyecto.path(), &almacen, false);
    registro.approve("revisor").unwrap();
    let seccion = registro
        .snapshot()
        .prompt_section(ConfirmDelegation::default());
    assert!(seccion.contains("- revisor (catalog): Revisa"), "{seccion}");
    assert!(!seccion.contains("- otro"), "{seccion}");
}

#[test]
fn the_catalog_origin_text_names_the_level() {
    let catalogo = catalogo_con(&[("revisor", "Revisa")]);
    let proyecto = tempfile::tempdir().unwrap();
    let datos = tempfile::tempdir().unwrap();
    let almacen = TrustStore::open(datos.path().to_path_buf()).unwrap();
    let registro = registro_con_catalogo(catalogo.path(), proyecto.path(), &almacen, false);
    let instantanea = registro.snapshot();
    let texto = instantanea.get("revisor").unwrap().origin.as_text();
    assert!(texto.starts_with("catalog:"), "{texto}");
}
