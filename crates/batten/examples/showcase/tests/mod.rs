//! Tests del módulo, por tema.

use super::*;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Terminal;

mod matrix;
mod mouse;
mod render;
mod support;
