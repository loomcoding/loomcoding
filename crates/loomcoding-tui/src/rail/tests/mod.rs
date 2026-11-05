//! Tests del módulo, por tema.

use super::format::*;
use super::metrics::*;
use super::subagents::*;
use super::text::*;
use super::*;
use loomcoding_core::session::{SessionId, SubagentDescriptor, UsageRecord};
use loomcoding_core::subagent_phase::SubagentPhase;
use loomcoding_core::task::{TaskState, TaskStatus};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

mod capacidades;
mod cost;
mod layout;
mod limpio;
mod metrics;
mod selection;
mod states;
mod support;
