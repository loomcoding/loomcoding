//! Perfil de un agente: el fichero `<nombre>.md`, con frontmatter YAML y cuerpo markdown.
//!
//! Formato de la sección 2 de `docs/specs/2026-10-15-loom-agent-profile-a2a.md`, con las
//! decisiones de `docs/specs/hechos/2026-10-22-codeloom-a2a-agent-registry-design.md`. Hoy se
//! interpretan `name`, `description`, `version`, `persona`, `model`, `provider`, `effort`,
//! `max_turns` y `max_depth`. `transport: a2a-remote` con `a2a.endpoint` declara un agente remoto (A8a) y se
//! aplica. `scope`, `delegation` y cualquier otro `transport` o `a2a` se leen, se declaran como no
//! aplicados y no se ejecutan. Cualquier otra clave se ignora con aviso: un campo nuevo no
//! rompe la carga.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::card::AgentSkill;
use crate::config::ProviderConfig;
use serde::{Deserialize, Serialize};
use serde_yaml_bw::{Mapping, Value};

mod fields;
mod model;
mod parse;
mod remote;
mod save;

pub use fields::is_valid_agent_name;
use fields::*;
pub use model::{AgentProfile, AgentProfileEdit, AgentScope, ProfileProblem, ProfileSaveError};
pub use parse::parse_profile;
use parse::*;
use remote::*;
pub use save::save_profile_edit;

#[cfg(test)]
mod tests;
