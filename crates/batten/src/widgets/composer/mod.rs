//! Widget para el composer de entrada.

use crate::state::{Composer, ComposerLayout, StyleToken, VisualRow};
use crate::widgets::{ComposerStyle, Theme};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui::Frame;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

mod classic;
mod hit;
mod layout;
mod looks;
mod text;

pub use classic::{render, render_focused, render_focused_with_model};
pub use hit::hit_index;
use layout::*;
pub use layout::{height_for_look, scroll_cursor_by_rows};
pub use looks::{render_crush, render_for_look, render_martty, render_mindfork};
use text::*;

#[cfg(test)]
mod tests;
