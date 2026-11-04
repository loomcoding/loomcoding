//! Tests del módulo, por tema.

use super::layout::*;
use super::lines::*;
use super::selection::*;
use super::*;
use crate::state::{Line, LineKind, Transcript, Viewport};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

mod layout;
mod lines;
mod links;
mod scroll;
mod selection;
mod support;
