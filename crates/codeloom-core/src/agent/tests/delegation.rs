use super::*;

async fn turno_con_delegacion(
    modo: Option<crate::config::DelegationMode>,
) -> (Vec<String>, String) {
    let mut herramientas = ToolRegistry::new();
    for nombre in [
        "subagent",
        "subagent_control",
        "subagent_list",
        "artifact_read",
        "development_workflow",
        "noop",
    ] {
        herramientas.register(Arc::new(NamedTool(nombre)));
    }
    let proveedor = proveedor_de("", false, vec![respuesta_con_consumo("hola", 10, 1)]);
    let mut e = entorno_de_turno(proveedor, catalogo_de_ventana(100_000), &[], herramientas).await;
    e.agente = e.agente.with_delegation_mode(modo);
    e.agente
        .run(e.sid, "hola".into(), CancellationToken::new())
        .await
        .unwrap();
    let vistas = e.proveedor.seen.lock().unwrap();
    let nombres = vistas[0].tools.iter().map(|t| t.name.clone()).collect();
    (nombres, vistas[0].system.clone())
}

#[tokio::test]
async fn delegation_off_withdraws_the_subagent_tools_from_the_model() {
    use crate::config::DelegationMode;
    let (con_todo, _) = turno_con_delegacion(None).await;
    assert!(con_todo.iter().any(|n| n == "subagent"), "{con_todo:?}");
    let (sin_delegar, _) = turno_con_delegacion(Some(DelegationMode::Off)).await;
    for retirada in [
        "subagent",
        "subagent_control",
        "subagent_list",
        "artifact_read",
        "development_workflow",
    ] {
        assert!(!sin_delegar.iter().any(|n| n == retirada), "{retirada}");
    }
    assert!(
        sin_delegar.iter().any(|n| n == "noop"),
        "las demás se conservan"
    );
}

#[tokio::test]
async fn delegation_prompt_modes_add_their_rule_and_keep_the_tools() {
    use crate::config::DelegationMode;
    let (_, base) = turno_con_delegacion(None).await;
    assert!(!base.contains("explicitly asks") && !base.contains("proactively"));
    let (herramientas, a_peticion) = turno_con_delegacion(Some(DelegationMode::OnRequest)).await;
    assert!(a_peticion.contains("explicitly asks"), "{a_peticion}");
    assert!(herramientas.iter().any(|n| n == "subagent"));
    let (_, proactiva) = turno_con_delegacion(Some(DelegationMode::Proactive)).await;
    assert!(proactiva.contains("proactively"), "{proactiva}");
}
