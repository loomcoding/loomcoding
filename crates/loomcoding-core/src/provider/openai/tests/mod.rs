//! Tests del módulo, por tema.

use super::compat::*;
use super::responses::*;
use super::*;
use crate::message::{ContentPart, Message, Role};
use crate::provider::ToolSpec;
use futures::StreamExt;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod effort;
mod http;
mod mapping;
mod responses;
mod support;
mod wire;
