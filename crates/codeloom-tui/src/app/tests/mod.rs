//! Tests del estado de la aplicación, repartidos por tema.
//!
//! Cabe señalar que los fixtures compartidos viven en `support`; los que usa un solo tema
//! se quedan en su fichero. Todos ven lo privado del módulo padre.

use super::*;
use batten::state::ToolStatus;
use codeloom_core::job::{JobId, JobProgress, JobResult, JobSnapshot, JobStatus};
use codeloom_core::question::QuestionOption;
use codeloom_core::session::SubagentDescriptor;
use codeloom_core::subagent_phase::SubagentPhase;
use serde_json::json;

mod support;
use support::*;
mod agents_composer_budget;
mod child_tree;
mod composer_target;
mod folded_blocks;
mod notices_and_remote;
mod session;
mod subagent_nodes;
mod theme_and_settings;
mod todos_and_questions;
mod tools_and_activity;
mod usage_meter;
mod usage_records;
mod wake;
