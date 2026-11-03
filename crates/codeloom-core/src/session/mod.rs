use crate::a2a::Artifact;
use crate::error::Result;
use crate::message::Message;
use crate::subagent_phase::SubagentPhase;
use crate::task::TaskStatus;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod sqlite;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SessionId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub preview: String,
    pub message_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentDescriptor {
    pub parent: SessionId,
    pub child: SessionId,
    pub job_id: String,
    pub label: String,
    /// Estado de la tarea del subagente, según A2A: el estado es el `TaskState` mismo.
    pub status: TaskStatus,
    /// Fase local del subagente. No viaja en `TaskStatus.message`: es texto libre y
    /// `update_subagent_status` puede pisarlo.
    pub phase: SubagentPhase,
    /// Última actividad observada, en milisegundos desde la época. Nunca `Instant`:
    /// el descriptor deriva `Serialize, Deserialize, PartialEq, Eq` y `Instant` no
    /// implementa ninguno de los cuatro.
    pub last_activity_at_ms: u64,
    pub steps: u64,
    pub tool_calls: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    /// Resultado de la tarea, como `Artifact`s de A2A. Vacío mientras no ha terminado.
    pub artifacts: Vec<Artifact>,
    pub notice_delivered: bool,
    /// Nombre del agente con el que arrancó (`None` si la tarea no se lanzó con agente).
    pub agent: Option<String>,
    /// Origen del agente, en texto: `built-in`, `global:<ruta>`, `project:<ruta>` o
    /// `remote:<url>`. Se guarda para saber de dónde salió la definición aunque después cambie.
    pub agent_origin: Option<String>,
    /// `reference_task_ids` de A2A: tareas a las que sigue esta (un seguimiento es una tarea nueva
    /// en el mismo contexto). Los ids son sesiones de hijos del mismo padre. Vacío si no sigue a
    /// ninguna.
    pub reference_task_ids: Vec<SessionId>,
}

impl SubagentDescriptor {
    /// Suma a los contadores saturando en `u64::MAX` en vez de desbordar.
    pub fn add_usage(&mut self, steps: u64, tool_calls: u64, tokens_in: u64, tokens_out: u64) {
        self.steps = self.steps.saturating_add(steps);
        self.tool_calls = self.tool_calls.saturating_add(tool_calls);
        self.tokens_in = self.tokens_in.saturating_add(tokens_in);
        self.tokens_out = self.tokens_out.saturating_add(tokens_out);
    }

    /// Texto del resultado: el de los artefactos que no son de progreso, unidos con `"\n"`. `None` si el
    /// subagente no tiene ninguno.
    pub fn result_text(&self) -> Option<String> {
        if self.artifacts.iter().all(Artifact::is_progress) {
            None
        } else {
            Some(crate::a2a::resultado_texto(&self.artifacts))
        }
    }

    /// Artefactos de progreso que publicó el hijo, en orden de publicación.
    pub fn progress_artifacts(&self) -> impl Iterator<Item = &Artifact> {
        self.artifacts
            .iter()
            .filter(|artefacto| artefacto.is_progress())
    }

    /// Identificador de contexto de A2A: la sesión del padre, en texto.
    pub fn context_id(&self) -> String {
        self.parent.0.to_string()
    }
}

/// Apunte de consumo: los tokens de una respuesta del proveedor y el precio vigente
/// (dólares por millón de tokens) en el momento de apuntarla. Los precios son `None`
/// cuando no se conocían; el apunte conserva su precio aunque el catálogo cambie después.
/// `session` es la sesión que originó el consumo: el padre para su propio turno y el hijo
/// para lo que gasta un subagente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageComponent {
    pub kind: UsageComponentKind,
    pub id: String,
    pub input_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageComponentKind {
    Mcp,
    Skill,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageRecord {
    pub session: SessionId,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
    #[serde(default)]
    pub reported_cost_usd: Option<f64>,
    #[serde(default)]
    pub source: crate::message::UsageSource,
    #[serde(default = "usage_complete_default")]
    pub is_final: bool,
    #[serde(default)]
    pub components: Vec<UsageComponent>,
    /// Tokens de entrada servidos desde la caché del proveedor; cero significa «no informado o sin caché».
    #[serde(default)]
    pub cache_read_tokens: u64,
    /// Tokens de entrada escritos en la caché del proveedor; cero significa «no informado o sin caché».
    #[serde(default)]
    pub cache_write_tokens: u64,
    pub created_at_ms: i64,
}

fn usage_complete_default() -> bool {
    true
}

impl UsageRecord {
    /// Coste en dólares del apunte, o `None` si falta alguno de los dos precios.
    pub fn cost(&self) -> Option<f64> {
        if let Some(cost) = self.reported_cost_usd {
            return Some(cost);
        }
        let input = self.input_price? * self.input_tokens as f64 / 1_000_000.0;
        let output = self.output_price? * self.output_tokens as f64 / 1_000_000.0;
        Some(input + output)
    }
}

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create_session(&self, project: &str) -> Result<SessionId>;
    async fn list_sessions(&self, project: &str) -> Result<Vec<SessionInfo>>;
    async fn session_belongs_to_project(&self, session: SessionId, project: &str) -> Result<bool>;
    async fn append_message(&self, sid: SessionId, msg: &Message) -> Result<()>;
    async fn messages(&self, sid: SessionId) -> Result<Vec<Message>>;
    /// Los últimos `limit` mensajes de la sesión, en orden ascendente.
    ///
    /// Cabe señalar que existe aparte de [`messages`] a propósito: el bucle del agente
    /// llama a esto en cada paso, y cargar el historial entero cada vez era lo que
    /// llevaba al daemon a decenas de gigabytes en sesiones largas. El almacén sigue
    /// conservando todos los mensajes; lo que cambia es cuántos se leen de golpe.
    ///
    /// La implementación por defecto delega en [`messages`] y recorta, para no obligar a
    /// cada almacén a implementarlo. Los almacenes reales deberían sobrescribirlo con una
    /// consulta que ya traiga solo la ventana.
    async fn recent_messages(&self, sid: SessionId, limit: usize) -> Result<Vec<Message>> {
        let mut todos = self.messages(sid).await?;
        if todos.len() > limit {
            todos.drain(..todos.len() - limit);
        }
        Ok(todos)
    }
    async fn save_subagent(&self, descriptor: &SubagentDescriptor) -> Result<()>;
    /// Fija el estado de la tarea del hijo. Un estado terminal no admite transiciones
    /// posteriores: el almacén devuelve error si el actual ya lo es.
    async fn update_subagent_status(
        &self,
        parent: SessionId,
        child: SessionId,
        status: TaskStatus,
    ) -> Result<()>;
    /// Guarda la fase de ejecución y el último latido del hijo, para que se vean tras reabrir el
    /// almacén. No toca a un hijo ya asentado. Por defecto no hace nada.
    async fn update_subagent_runtime(
        &self,
        parent: SessionId,
        child: SessionId,
        phase: SubagentPhase,
        last_activity_at_ms: u64,
    ) -> Result<()> {
        let _ = (parent, child, phase, last_activity_at_ms);
        Ok(())
    }
    async fn list_subagents(&self, parent: SessionId) -> Result<Vec<SubagentDescriptor>>;
    /// Marca el hijo como `Completed` y guarda su resultado como artefactos.
    async fn complete_subagent(
        &self,
        parent: SessionId,
        child: SessionId,
        artifacts: &[Artifact],
    ) -> Result<()>;
    /// Publica un artefacto de progreso en la tarea `child`, que debe seguir en marcha. Con un nombre ya
    /// usado lo reemplaza. Aplica las cotas de `a2a::publicar_progreso` de forma atómica y devuelve el
    /// descriptor nuevo para que quien llama lo publique en el bus.
    async fn publish_subagent_progress(
        &self,
        child: SessionId,
        name: &str,
        description: Option<&str>,
        content: &str,
    ) -> Result<SubagentDescriptor> {
        let _ = (child, name, description, content);
        Err(crate::error::CoreError::Store(
            "subagent progress unavailable".into(),
        ))
    }
    async fn append_subagent_notice(
        &self,
        parent: SessionId,
        child: SessionId,
        message: &Message,
    ) -> Result<bool>;
    /// Guarda un apunte de consumo bajo la sesión indicada.
    async fn record_usage(&self, session: SessionId, record: &UsageRecord) -> Result<()>;
    /// Apuntes de la sesión y de sus hijos (`subagents`), en orden de inserción.
    async fn session_usage(&self, session: SessionId) -> Result<Vec<UsageRecord>>;

    async fn create_development_workflow(
        &self,
        workflow: crate::workflow::DevelopmentWorkflow,
        detail: &str,
    ) -> Result<crate::workflow::DevelopmentWorkflow> {
        let _ = (workflow, detail);
        Err(crate::error::CoreError::Store(
            "development workflow storage unavailable".into(),
        ))
    }
    async fn update_development_workflow(
        &self,
        workflow: crate::workflow::DevelopmentWorkflow,
        actor: &str,
        detail: &str,
    ) -> Result<crate::workflow::DevelopmentWorkflow> {
        let _ = (workflow, actor, detail);
        Err(crate::error::CoreError::Store(
            "development workflow storage unavailable".into(),
        ))
    }
    async fn development_workflow(
        &self,
        id: crate::workflow::WorkflowId,
    ) -> Result<Option<crate::workflow::DevelopmentWorkflow>> {
        let _ = id;
        Err(crate::error::CoreError::Store(
            "development workflow storage unavailable".into(),
        ))
    }
    async fn active_development_workflow(
        &self,
        parent: SessionId,
    ) -> Result<Option<crate::workflow::DevelopmentWorkflow>> {
        let _ = parent;
        Err(crate::error::CoreError::Store(
            "development workflow storage unavailable".into(),
        ))
    }
    async fn latest_development_workflow(
        &self,
        parent: SessionId,
    ) -> Result<Option<crate::workflow::DevelopmentWorkflow>> {
        let _ = parent;
        Err(crate::error::CoreError::Store(
            "development workflow storage unavailable".into(),
        ))
    }
    async fn development_workflow_events(
        &self,
        id: crate::workflow::WorkflowId,
    ) -> Result<Vec<crate::workflow::DevelopmentWorkflowEvent>> {
        let _ = id;
        Err(crate::error::CoreError::Store(
            "development workflow storage unavailable".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_record_cost_none_without_prices() {
        let priced = UsageRecord {
            session: SessionId(1),
            model: "m".into(),
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            input_price: Some(3.0),
            output_price: Some(15.0),
            reported_cost_usd: None,
            source: crate::message::UsageSource::Provider,
            is_final: true,
            components: Vec::new(),
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            created_at_ms: 0,
        };
        let cost = priced.cost().expect("con ambos precios hay coste");
        assert!((cost - 10.5).abs() < 1e-9, "3 + 7.5 = 10.5, y salió {cost}");

        let without_input = UsageRecord {
            input_price: None,
            ..priced.clone()
        };
        assert_eq!(
            without_input.cost(),
            None,
            "sin precio de entrada no hay coste"
        );

        let without_output = UsageRecord {
            output_price: None,
            ..priced
        };
        assert_eq!(
            without_output.cost(),
            None,
            "sin precio de salida no hay coste"
        );
    }

    #[test]
    fn usage_record_prefers_reported_cost_to_catalog_estimate() {
        let record = UsageRecord {
            session: SessionId(1),
            model: "m".into(),
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            input_price: Some(3.0),
            output_price: Some(15.0),
            reported_cost_usd: Some(0.0042),
            source: crate::message::UsageSource::Provider,
            is_final: true,
            components: Vec::new(),
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            created_at_ms: 0,
        };
        assert_eq!(record.cost(), Some(0.0042));
    }

    fn descriptor_sin_artefactos() -> SubagentDescriptor {
        use crate::task::TaskState;
        SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            parent: SessionId(7),
            child: SessionId(8),
            job_id: "subagent-8".into(),
            label: "review".into(),
            status: TaskStatus::new(TaskState::Working),
            artifacts: Vec::new(),
            notice_delivered: false,
            agent: None,
            agent_origin: None,
            reference_task_ids: Vec::new(),
        }
    }

    #[test]
    fn result_text_is_none_without_artifacts() {
        use crate::a2a::Artifact;
        let sin = descriptor_sin_artefactos();
        assert_eq!(sin.result_text(), None);

        let con = SubagentDescriptor {
            phase: SubagentPhase::Queued,
            last_activity_at_ms: 0,
            steps: 0,
            tool_calls: 0,
            tokens_in: 0,
            tokens_out: 0,
            artifacts: vec![
                Artifact::text_artifact("uno"),
                Artifact::text_artifact("dos"),
            ],
            ..sin
        };
        assert_eq!(con.result_text().as_deref(), Some("uno\ndos"));
    }

    #[test]
    fn context_id_is_the_parent_session_id() {
        assert_eq!(descriptor_sin_artefactos().context_id(), "7");
    }
}
