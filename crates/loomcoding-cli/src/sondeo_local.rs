//! Sondeo de los proveedores locales del registro y su publicación en el catálogo de la interfaz.
//!
//! El catálogo de models.dev llega por `load_catalog`; este módulo añade los servidores locales a
//! cada publicación y relanza el sondeo en segundo plano. Nunca se espera a un servidor: el arranque
//! y el compositor siguen su curso, y el envío de un refresco es no bloqueante. Los refrescos
//! (al abrir `/model`, `/settings` o el selector de proveedor) se enfrían durante cinco segundos.
//! Cada servidor recibe solo `GET <base_url>/models`, jamás una inferencia.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};
use std::time::{Duration, Instant};

use loomcoding_core::catalog::{Catalog, CatalogProvider};
use loomcoding_core::providers_catalog::proveedor_local;
use loomcoding_core::providers_probe::probe;
use loomcoding_core::providers_registry::LocalProvider;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// Margen mínimo entre dos sondeos provocados por la interfaz.
pub const ENFRIAMIENTO: Duration = Duration::from_secs(5);

/// Estado que el publicador recuerda entre publicaciones.
#[derive(Default)]
struct Estado {
    /// Último catálogo base (models.dev o caché), sin proveedores locales.
    base: Catalog,
    /// Proveedores locales del último sondeo, ya traducidos al catálogo.
    locales: Vec<CatalogProvider>,
    /// Instante en el que empezó el último sondeo, para el enfriamiento.
    ultimo_sondeo: Option<Instant>,
}

/// Publica el catálogo en la interfaz y en el compartido del agente, siempre con los proveedores
/// locales del último sondeo fusionados.
pub struct Publicador {
    tx: UnboundedSender<Arc<Catalog>>,
    compartido: Arc<RwLock<Catalog>>,
    declarada: crate::DeclaredWindow,
    registro: Vec<LocalProvider>,
    estado: Mutex<Estado>,
}

impl Publicador {
    /// Crea el publicador. Con un `registro` vacío nunca se sondea y la publicación queda igual.
    pub fn new(
        tx: UnboundedSender<Arc<Catalog>>,
        compartido: Arc<RwLock<Catalog>>,
        declarada: crate::DeclaredWindow,
        registro: Vec<LocalProvider>,
    ) -> Arc<Self> {
        Arc::new(Self {
            tx,
            compartido,
            declarada,
            registro,
            estado: Mutex::new(Estado::default()),
        })
    }

    /// `true` si la persona usuaria declaró una ventana de contexto en `loomcoding.toml`.
    pub fn tiene_ventana_declarada(&self) -> bool {
        self.declarada.is_some()
    }

    /// Publica un catálogo base (models.dev o caché) con los proveedores locales del último sondeo.
    pub fn publicar_base(&self, base: Catalog) {
        let mut estado = self.estado();
        estado.base = base;
        self.emitir(&estado);
    }

    /// Sondea todos los proveedores del registro en paralelo y republica con el resultado.
    pub async fn sondear_y_publicar(&self) {
        self.estado().ultimo_sondeo = Some(Instant::now());
        let locales = sondear_todos(&self.registro).await;
        let mut estado = self.estado();
        estado.locales = locales;
        self.emitir(&estado);
    }

    /// Refresco pedido por la interfaz: no sondea si el último sondeo es más reciente que el
    /// enfriamiento.
    pub async fn refrescar(&self) {
        let toca = {
            let estado = self.estado();
            debe_sondear(estado.ultimo_sondeo, Instant::now(), ENFRIAMIENTO)
        };
        if toca {
            self.sondear_y_publicar().await;
        }
    }

    fn estado(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Fusiona base y locales, aplica la ventana declarada y publica en el compartido y en el canal.
    fn emitir(&self, estado: &Estado) {
        let catalogo = crate::with_declared_window(
            estado
                .base
                .clone()
                .with_local_providers(estado.locales.clone()),
            &self.declarada,
        );
        if let Ok(mut guardia) = self.compartido.write() {
            *guardia = catalogo.clone();
        }
        let _ = self.tx.send(Arc::new(catalogo));
    }
}

/// `true` si no hubo sondeo previo o si el último empezó hace al menos `margen`.
pub fn debe_sondear(ultimo: Option<Instant>, ahora: Instant, margen: Duration) -> bool {
    match ultimo {
        None => true,
        Some(ultimo) => ahora.saturating_duration_since(ultimo) >= margen,
    }
}

/// Arranca el sondeo: uno inicial en segundo plano y, después, un refresco por cada petición que
/// llegue por el canal devuelto. Varias peticiones seguidas valen por una y el enfriamiento
/// descarta las que llegan demasiado pronto. El envío nunca espera al sondeo.
pub fn arrancar_sondeo(publicador: Arc<Publicador>) -> UnboundedSender<()> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    tokio::spawn(atender_refrescos(publicador, rx));
    tx
}

async fn atender_refrescos(publicador: Arc<Publicador>, mut rx: UnboundedReceiver<()>) {
    if publicador.registro.is_empty() {
        return;
    }
    publicador.sondear_y_publicar().await;
    while rx.recv().await.is_some() {
        // Una ráfaga de peticiones cuenta como una sola.
        while rx.try_recv().is_ok() {}
        publicador.refrescar().await;
    }
}

/// Sondea cada proveedor en su propia tarea y devuelve los que tienen tipo conocido, en orden.
async fn sondear_todos(registro: &[LocalProvider]) -> Vec<CatalogProvider> {
    let tareas: Vec<_> = registro
        .iter()
        .map(|provider| {
            let provider = provider.clone();
            tokio::spawn(async move {
                let clave = clave_de(provider.api_key_env.clone()).await;
                let estado = probe(&provider, clave.as_deref()).await;
                proveedor_local(&provider, &estado)
            })
        })
        .collect();
    let mut locales = Vec::with_capacity(tareas.len());
    for tarea in tareas {
        match tarea.await {
            Ok(Some(provider)) => locales.push(provider),
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "el sondeo de un proveedor local falló"),
        }
    }
    locales
}

/// Clave del proveedor, leída fuera del ejecutor: variable de entorno o almacén de credenciales.
/// Sin `api_key_env` no hay clave y el sondeo va sin cabecera de autorización.
async fn clave_de(nombre: Option<String>) -> Option<String> {
    let nombre = nombre
        .map(|nombre| nombre.trim().to_string())
        .filter(|nombre| !nombre.is_empty())?;
    tokio::task::spawn_blocking(move || crate::resolve_credential(&nombre))
        .await
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use loomcoding_core::config::ProviderKind;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn local(id: &str, base_url: &str) -> LocalProvider {
        LocalProvider {
            id: id.into(),
            name: format!("{id}-server"),
            kind: "openai_compat".into(),
            base_url: base_url.into(),
            api_key_env: None,
            model: format!("{id}-reserva"),
            context_window: Some(65_536),
        }
    }

    fn publicador(
        registro: Vec<LocalProvider>,
    ) -> (
        Arc<Publicador>,
        UnboundedReceiver<Arc<Catalog>>,
        Arc<RwLock<Catalog>>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let compartido = Arc::new(RwLock::new(Catalog::default()));
        let publicador = Publicador::new(tx, compartido.clone(), None, registro);
        (publicador, rx, compartido)
    }

    async fn servidor_con_modelos(ids: &[&str]) -> MockServer {
        let servidor = MockServer::start().await;
        let data: Vec<_> = ids.iter().map(|id| json!({ "id": id })).collect();
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": data })))
            .mount(&servidor)
            .await;
        servidor
    }

    /// Base URL de un puerto que acaba de quedar libre: la conexión será rechazada.
    fn puerto_cerrado() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        format!("http://127.0.0.1:{port}/v1")
    }

    /// Última publicación pendiente en el canal.
    fn ultimo(rx: &mut UnboundedReceiver<Arc<Catalog>>) -> Arc<Catalog> {
        let mut ultimo = None;
        while let Ok(catalogo) = rx.try_recv() {
            ultimo = Some(catalogo);
        }
        ultimo.expect("se esperaba una publicación")
    }

    #[tokio::test]
    async fn los_modelos_descubiertos_llegan_al_catalogo_y_al_compartido() {
        let servidor = servidor_con_modelos(&["uno", "dos"]).await;
        let (publicador, mut rx, compartido) =
            publicador(vec![local("llama", &format!("{}/v1", servidor.uri()))]);

        publicador.sondear_y_publicar().await;

        let catalogo = ultimo(&mut rx);
        let llama = catalogo.provider("llama").expect("llama");
        assert_eq!(llama.name, "llama-server · up (2)");
        assert_eq!(llama.kind, ProviderKind::OpenAiCompat);
        let ids: Vec<&str> = llama.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["uno", "dos"]);
        assert_eq!(compartido.read().unwrap().providers.len(), 1);
    }

    #[tokio::test]
    async fn un_servidor_caido_conserva_su_modelo_de_reserva_y_dice_down() {
        let (publicador, mut rx, _) = publicador(vec![local("llama", &puerto_cerrado())]);

        publicador.sondear_y_publicar().await;

        let catalogo = ultimo(&mut rx);
        let llama = catalogo.provider("llama").expect("llama");
        assert_eq!(llama.name, "llama-server · down");
        assert_eq!(llama.models.len(), 1);
        assert_eq!(llama.models[0].id, "llama-reserva");
    }

    #[tokio::test]
    async fn el_local_sustituye_al_de_models_dev_con_el_mismo_id() {
        let servidor = servidor_con_modelos(&["uno"]).await;
        let (publicador, mut rx, _) =
            publicador(vec![local("llama", &format!("{}/v1", servidor.uri()))]);
        let remoto = CatalogProvider {
            id: "llama".into(),
            name: "Llama de models.dev".into(),
            kind: ProviderKind::OpenAiCompat,
            base_url: "https://llama.test/v1".into(),
            api_key_env: String::new(),
            models: vec![],
        };
        publicador.publicar_base(Catalog::from_providers(vec![remoto]));
        ultimo(&mut rx);

        publicador.sondear_y_publicar().await;

        let catalogo = ultimo(&mut rx);
        let llamas: Vec<&CatalogProvider> = catalogo
            .providers
            .iter()
            .filter(|p| p.id == "llama")
            .collect();
        assert_eq!(llamas.len(), 1, "el id repetido se sustituye");
        assert_eq!(llamas[0].name, "llama-server · up (1)");
    }

    #[tokio::test]
    async fn el_refresco_dentro_del_enfriamiento_no_vuelve_a_sondear() {
        let servidor = servidor_con_modelos(&["uno"]).await;
        let (publicador, _rx, _) =
            publicador(vec![local("llama", &format!("{}/v1", servidor.uri()))]);

        publicador.sondear_y_publicar().await;
        publicador.refrescar().await;
        publicador.refrescar().await;

        let peticiones = servidor.received_requests().await.unwrap();
        assert_eq!(peticiones.len(), 1, "solo el sondeo inicial");
        assert_eq!(peticiones[0].method.as_str(), "GET");
    }

    #[tokio::test]
    async fn las_rafagas_del_canal_provocan_un_unico_sondeo() {
        let servidor = servidor_con_modelos(&["uno"]).await;
        let (publicador, mut rx, _) =
            publicador(vec![local("llama", &format!("{}/v1", servidor.uri()))]);

        let refresco = arrancar_sondeo(publicador);
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("el sondeo inicial publica")
            .expect("el canal sigue abierto");
        for _ in 0..3 {
            refresco.send(()).expect("el sondeo sigue vivo");
        }
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert_eq!(servidor.received_requests().await.unwrap().len(), 1);
    }

    #[test]
    fn el_enfriamiento_deja_pasar_solo_tras_el_margen() {
        let t0 = Instant::now();
        assert!(debe_sondear(None, t0, ENFRIAMIENTO));
        assert!(!debe_sondear(
            Some(t0),
            t0 + Duration::from_secs(4),
            ENFRIAMIENTO
        ));
        assert!(debe_sondear(
            Some(t0),
            t0 + Duration::from_secs(5),
            ENFRIAMIENTO
        ));
        assert_eq!(ENFRIAMIENTO, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn sin_registro_la_publicacion_base_no_cambia() {
        let (publicador, mut rx, _) = publicador(vec![]);
        let base = Catalog::from_providers(vec![CatalogProvider {
            id: "x".into(),
            name: "X".into(),
            kind: ProviderKind::Anthropic,
            base_url: "https://x.test".into(),
            api_key_env: String::new(),
            models: vec![],
        }]);

        publicador.publicar_base(base.clone());

        assert_eq!(*ultimo(&mut rx), base);
    }
}
