//! El hijo pide al proveedor lo mismo que el padre: temperatura y tope de salida.

use super::*;
use crate::config::LimitsConfig;
use crate::provider::{Capabilities, Provider, ProviderRequest, ProviderStream};

/// Proveedor que anota la temperatura y el tope de salida de cada petición.
struct ProveedorDeMuestreo(Arc<std::sync::Mutex<Vec<(f32, u32)>>>);

#[async_trait::async_trait]
impl Provider for ProveedorDeMuestreo {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            tool_use: true,
            reasoning: false,
            streaming: true,
        }
    }
    async fn stream(
        &self,
        req: ProviderRequest,
        _cancel: CancellationToken,
    ) -> Result<ProviderStream> {
        self.0
            .lock()
            .unwrap()
            .push((req.temperature, req.max_tokens));
        Ok(Box::pin(futures::stream::iter(vec![
            StreamChunk::TextDelta("listo".into()),
            StreamChunk::Done(FinishReason::Stop),
        ])))
    }
}

#[tokio::test]
async fn a_child_inherits_the_parents_temperature_and_output_cap() {
    let dir = tempfile::tempdir().unwrap();
    let (store, parent) = tienda_de_prueba(dir.path(), "limites.db").await;
    let jobs = Arc::new(LocalJobRegistry::default());
    let vistas = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider: Arc<dyn Provider> = Arc::new(ProveedorDeMuestreo(vistas.clone()));
    let lanzador = InProcessSubagentLauncher::new(
        Arc::new(RwLock::new(provider)),
        store.clone(),
        Arc::new(ToolRegistry::new()),
        Arc::new(PermissionService::allow_all()),
        Arc::new(RwLock::new(config_de_prueba())),
        dir.path().to_path_buf(),
        jobs.clone(),
        EventBus::new(64),
    )
    .with_limits(LimitsConfig {
        temperature: 0.7,
        max_output_tokens: 1234,
        ..LimitsConfig::default()
    });
    let (child, job) = lanzador
        .start(
            parent,
            "obrero".into(),
            "tarea".into(),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    jobs.wait(parent, &job).await.unwrap();
    espera_estado_del_hijo(&store, parent, child, TaskState::Completed).await;
    assert_eq!(*vistas.lock().unwrap(), vec![(0.7, 1234)]);
}
