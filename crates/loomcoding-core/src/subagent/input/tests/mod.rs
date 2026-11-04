//! Tests del buzón de entrada de los subagentes.

use super::*;
use crate::event::{EventBus, LiveEvent};
use crate::permission::{Decision, PermissionRequest, PermissionResponder, PermissionService};
use crate::question::QuestionOption;
use crate::session::SessionStore;
use crate::subagent::{resumen_de_permiso, TaskTracker};
use crate::subagent_phase::SubagentPhase;
use crate::task::TaskState;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

mod broker;
mod responder;
mod support;
