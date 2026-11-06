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
mod effort_command;
mod keys_basic;
mod logo_fx_clock;
mod logo_fx_command;
mod mascot_clock;
mod model_command;
mod mouse_and_selection;
mod rail_and_focus;
mod selection_and_quit;
mod settings;
mod sidebar_keys;
mod signo_de_interrogacion;
mod slash_commands;
mod tool_inspector_and_queue;
mod ui_command;
mod wake;
