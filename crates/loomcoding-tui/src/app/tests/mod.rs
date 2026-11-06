//! Tests del estado de la aplicación, repartidos por tema.
//!
//! Cabe señalar que los fixtures compartidos viven en `support`; los que usa un solo tema
//! se quedan en su fichero. Todos ven lo privado del módulo padre.

use super::*;
use loomcoding_cloth::state::ToolStatus;
use loomcoding_core::job::{JobId, JobProgress, JobResult, JobSnapshot, JobStatus};
use loomcoding_core::question::QuestionOption;
use loomcoding_core::session::SubagentDescriptor;
use loomcoding_core::subagent_phase::SubagentPhase;
use serde_json::json;

mod support;
use support::*;
mod agents_composer_budget;
mod child_tree;
mod composer_target;
mod folded_blocks;
mod logo_fx;
mod menu_izquierdo;
mod notices_and_remote;
mod session;
mod session_diff_state;
mod subagent_nodes;
mod theme_and_settings;
mod todos_and_questions;
mod tools_and_activity;
mod turn_end;
mod usage_meter;
mod usage_records;
mod wake;
