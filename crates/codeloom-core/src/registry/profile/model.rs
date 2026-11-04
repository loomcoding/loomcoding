//! Tipos del perfil de agente: ámbito, edición, perfil, avisos y errores de guardado.

use super::*;

/// Permisos declarados localmente por un perfil. Se muestran en la UI; el runtime todavía no
/// los aplica como allowlist.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentScope {
    #[serde(default)]
    pub mcp: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProfileEdit {
    pub description: String,
    pub version: Option<String>,
    pub card_skills: Vec<AgentSkill>,
    pub scope: AgentScope,
}

#[derive(Debug, thiserror::Error)]
pub enum ProfileSaveError {
    #[error("perfil cambiado desde que se abrió el editor")]
    Changed,
    #[error("error al leer o guardar el perfil: {0}")]
    Io(#[from] std::io::Error),
    #[error("perfil no válido: {0}")]
    Invalid(#[from] ProfileProblem),
}

/// Perfil ya validado de un agente local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProfile {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub persona_role: Option<String>,
    pub persona_goal: Option<String>,
    pub persona_backstory: Option<String>,
    /// Cuerpo markdown del fichero; es el prompt de sistema del agente.
    pub body: String,
    pub model: Option<String>,
    /// Proveedor privado del perfil local; nunca se serializa en `AgentCard`.
    pub provider: Option<ProviderConfig>,
    /// Skills públicas de `AgentCard`; distintas de `scope.skills`.
    pub card_skills: Vec<AgentSkill>,
    /// Configuración local CodeLoom. Se muestra, pero no aplica permisos todavía.
    pub scope: AgentScope,
    pub effort: Option<String>,
    pub max_turns: Option<u32>,
    pub max_depth: Option<u32>,
    /// Claves presentes que se leen pero no se aplican todavía: `scope`, `delegation`,
    /// `transport` y `a2a`.
    pub declared_not_applied: Vec<String>,
    /// Punto de entrada del agente remoto (`transport: a2a-remote` y `a2a.endpoint`). Con él el
    /// perfil es un puntero a un remoto, no un agente local: persona, modelo y cuerpo no se usan.
    pub remote_endpoint: Option<String>,
    /// `a2a.credentials` de un perfil remoto: nombre de esquema de la tarjeta → nombre de la
    /// variable de entorno que guarda su credencial (A8d). Nunca el secreto.
    pub remote_credentials: BTreeMap<String, String>,
}

/// Problema de un perfil. Los avisos viajan junto al perfil; los errores impiden cargarlo.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProfileProblem {
    #[error("nombre de agente no válido: `{0}`")]
    InvalidName(String),
    #[error("el `name` `{declared}` no coincide con el nombre del fichero `{file}`")]
    NameMismatch { file: String, declared: String },
    #[error("falta el campo obligatorio `{0}`")]
    MissingField(&'static str),
    #[error("frontmatter no válido: {0}")]
    BadFrontmatter(String),
    #[error("campo desconocido `{0}` (se ignora)")]
    UnknownField(String),
    #[error("campo `{0}` declarado y no aplicado todavía")]
    DeclaredNotApplied(String),
    #[error("proveedor de perfil no válido")]
    InvalidProvider,
}
