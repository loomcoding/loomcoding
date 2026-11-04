//! Fábrica de clientes: resuelve seguridad, entorno y bus de eventos de cada remoto.

use super::*;

/// Lee una variable de entorno: el entorno del proceso, o el de una prueba.
pub(super) type LectorDeEntorno = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Fábrica de clientes de los remotos del registro: la que se entrega al lanzador. Solo existe con
/// el rol `consume` (o `both`); con A2A apagado no se construye y los remotos no se ejecutan.
pub struct A2aClientFactory {
    pub(super) http: Client,
    pub(super) config: A2aConfig,
    pub(super) max_result_bytes: usize,
    pub(super) env: LectorDeEntorno,
    pub(super) observer: Option<EventBus>,
}

impl A2aClientFactory {
    /// Lee las credenciales de otra fuente que no sea el entorno del proceso (pruebas).
    pub fn with_env(
        mut self,
        env: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        self.env = Arc::new(env);
        self
    }

    /// Publica observaciones seguras de las operaciones remotas en el bus local.
    pub fn with_event_bus(mut self, bus: EventBus) -> Self {
        self.observer = Some(bus);
        self
    }

    /// Cliente HTTP para un plan: el compartido, o uno propio si el plan exige una identidad TLS.
    fn http_for(&self, plan: &AuthPlan) -> Result<Client, String> {
        match plan.client_identity() {
            None => Ok(self.http.clone()),
            Some(identidad) => http_client_with_identity(&self.config, identidad.pem())
                .map_err(|error| error.to_string()),
        }
    }

    /// Plan de autenticación de un remoto con tarjeta descargada, o el motivo de que no lo haya.
    fn plan_for(&self, spec: &RemoteSpec) -> Option<Result<AuthPlan, String>> {
        let descargada = spec.fetched.as_ref()?;
        Some(AuthPlan::for_card(
            descargada.security.as_deref(),
            descargada.requires_auth,
            &spec.credentials,
            &*self.env,
        ))
    }

    /// `None` si el rol no es de consumo: no se construye ningún cliente HTTP.
    pub fn from_config(config: &A2aConfig) -> Result<Option<Self>, RemoteError> {
        Ok(http_client(config)?.map(|http| A2aClientFactory {
            http,
            config: config.clone(),
            max_result_bytes: config.max_result_bytes,
            env: Arc::new(|variable| std::env::var(variable).ok()),
            observer: None,
        }))
    }
}

impl RemoteClientFactory for A2aClientFactory {
    /// Cliente de la interfaz que la tarjeta descargada declaró; sin tarjeta, ninguno.
    fn client_for(&self, name: &str, spec: &RemoteSpec) -> Option<Arc<dyn RemoteTaskClient>> {
        let descargada = spec.fetched.as_ref()?;
        // Sin un plan de autenticación válido no hay cliente: jamás se habla sin credencial a un
        // remoto que la pide.
        let plan = self.plan_for(spec)?.ok()?;
        // Un remoto con mTLS usa un cliente HTTP propio con su identidad; si no se puede construir,
        // no hay cliente: jamás se cae a hablarle sin ella.
        let http = self.http_for(&plan).ok()?;
        Some(Arc::new(
            JsonRpcClient::new(
                http,
                descargada.jsonrpc_url.clone(),
                descargada.tenant.clone(),
                self.max_result_bytes,
            )
            .with_auth(plan)
            .with_streaming(descargada.streaming)
            .with_observer(name, self.observer.clone()),
        ))
    }

    fn unavailable_reason(&self, _name: &str, spec: &RemoteSpec) -> Option<String> {
        match self.plan_for(spec)? {
            Err(motivo) => Some(motivo),
            Ok(plan) => self.http_for(&plan).err(),
        }
    }
}
