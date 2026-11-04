//! Piezas comunes de los tests.

use super::*;

pub(super) fn escribir(dir: &Path, nombre: &str, texto: &str) {
    std::fs::write(dir.join(nombre), texto).unwrap();
}

pub(super) fn perfil(nombre: &str, descripcion: &str) -> String {
    format!("---\nname: {nombre}\ndescription: {descripcion}\n---\n\nCuerpo de {nombre}.\n")
}

pub(super) fn registro_con_global_y_proyecto(global: &Path, proyecto: &Path) -> AgentRegistry {
    AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(global.to_path_buf(), ProfileLevel::Global)),
        Box::new(FileSource::new(
            proyecto.to_path_buf(),
            ProfileLevel::Project,
        )),
    ])
}

/// Bus con un suscriptor y el evento que llegue, si llega alguno.
pub(super) fn ultimo_evento(
    rx: &mut tokio::sync::broadcast::Receiver<LiveEvent>,
) -> Option<LiveEvent> {
    rx.try_recv().ok()
}

/// Registro con confianza configurada sobre `almacen` para la raíz `proyecto`.
pub(super) fn registro_con_confianza(
    global: &Path,
    proyecto: &Path,
    almacen: &TrustStore,
    confiar_todo: bool,
) -> AgentRegistry {
    registro_con_global_y_proyecto(global, proyecto).with_trust(
        almacen.clone(),
        proyecto.to_path_buf(),
        confiar_todo,
    )
}

/// Raíz de catálogo con un perfil por cada `(nombre, descripcion)`, en `<raíz>/.agents/agents`.
pub(super) fn catalogo_con(agentes: &[(&str, &str)]) -> tempfile::TempDir {
    let raiz = tempfile::tempdir().unwrap();
    let dir = raiz.path().join(".agents").join("agents");
    std::fs::create_dir_all(&dir).unwrap();
    for (nombre, descripcion) in agentes {
        escribir(&dir, &format!("{nombre}.md"), &perfil(nombre, descripcion));
    }
    raiz
}

pub(super) fn registro_con_catalogo(
    catalogo: &Path,
    proyecto: &Path,
    almacen: &TrustStore,
    confiar_todo: bool,
) -> AgentRegistry {
    AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(
            catalogo.join(".agents").join("agents"),
            ProfileLevel::Catalog,
        )),
    ])
    .with_trust(almacen.clone(), proyecto.to_path_buf(), confiar_todo)
}

pub(super) fn perfil_remoto(nombre: &str, descripcion: &str, endpoint: &str) -> String {
    format!(
        "---\nname: {nombre}\ndescription: {descripcion}\ntransport: a2a-remote\na2a:\n  endpoint: {endpoint}\n---\n"
    )
}

pub(super) fn descargada(hash: &str) -> RemoteFetched {
    RemoteFetched {
        card_hash: hash.to_string(),
        jsonrpc_url: "https://agente.example/a2a/v1".into(),
        tenant: None,
        requires_auth: false,
        security: None,
        auth_problem: None,
        streaming: false,
    }
}

pub(super) fn tarjeta(descripcion: &str) -> AgentCard {
    AgentCard::local("lejano", descripcion, "1.0.0")
}

/// Registro con un remoto en el nivel global y la confianza configurada.
pub(super) fn registro_con_remoto(
    global: &Path,
    datos: &Path,
    confiar_proyectos: bool,
) -> (AgentRegistry, EventBus) {
    escribir(
        global,
        "lejano.md",
        &perfil_remoto(
            "lejano",
            "Escrita por quien da de alta",
            "https://agente.example",
        ),
    );
    let registro = AgentRegistry::new(vec![
        Box::new(BuiltInSource),
        Box::new(FileSource::new(global.to_path_buf(), ProfileLevel::Global)),
    ])
    .with_trust(
        TrustStore::open(datos.to_path_buf()).unwrap(),
        global.to_path_buf(),
        confiar_proyectos,
    );
    (registro, EventBus::new(16))
}
