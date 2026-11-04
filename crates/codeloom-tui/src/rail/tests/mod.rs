//! Tests del módulo, por tema.

use super::format::*;
use super::metrics::*;
use super::subagents::*;
use super::text::*;
use super::*;
use codeloom_core::session::{SessionId, SubagentDescriptor, UsageRecord};
use codeloom_core::subagent_phase::SubagentPhase;
use codeloom_core::task::{TaskState, TaskStatus};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

mod cost;
mod layout;
mod metrics;
mod selection;
mod states;
mod support;
