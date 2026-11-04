//! Tests del módulo, por tema.

use super::text::*;
use super::*;
use crate::state::{Diff, DiffKind, LineKind, StyleToken, ToolStatus};
use unicode_segmentation::UnicodeSegmentation;

mod edit;
mod generic;
mod spans;
mod summaries;
mod support;
