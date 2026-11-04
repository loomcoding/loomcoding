//! Tests del módulo, por tema.

use super::classic::*;
use super::hit::*;
use super::layout::*;
use super::looks::*;
use super::text::*;
use super::*;
use crate::state::Composer;
use ratatui::backend::{Backend, TestBackend};
use ratatui::Terminal;

mod basics;
mod classic;
mod crush;
mod hit;
mod scroll;
mod selection;
mod support;
