//! Tests del bucle de la TUI, repartidos por tema.
//!
//! Cabe señalar que los fixtures compartidos viven en `support`; los que usa un solo tema
//! se quedan en su fichero. Todos ven lo privado del módulo padre.

use super::*;
use loomcoding_core::subagent_phase::SubagentPhase;
use ratatui::layout::Rect;

mod support;
use support::*;
mod agent_command;
mod block_navigation;
mod commands_ui;
mod dialogs;
mod keys_basic;
mod mouse_and_selection;
mod rail_and_focus;
mod selection_and_quit;
mod settings;
mod slash_commands;
mod tool_inspector_and_queue;
mod wake;
