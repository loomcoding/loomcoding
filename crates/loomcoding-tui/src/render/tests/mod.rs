//! Tests del lanzador de subagentes, repartidos por tema.
//!
//! Cabe señalar que los ayudantes de prueba compartidos viven en `support`;
//! los que usa un solo tema se quedan en su fichero. Todos ven lo privado del módulo padre.

use super::*;
use crate::app::App;
use loomcoding_core::subagent_phase::SubagentPhase;
use loomcoding_core::task::{TaskState, TaskStatus};
use ratatui::backend::{Backend, TestBackend};
use ratatui::Terminal;

mod support;
use support::*;
mod cabecera_y_pie;
mod disposicion_y_rail;
mod franja_del_muneco;
mod metricas_y_mcp;
mod paneles_y_overlays;
mod transcript_y_arbol;
