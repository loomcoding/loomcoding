//! Tests del módulo, por tema.

use super::factory::*;
use super::*;
use crate::config::{A2aConfig, A2aRole};
use crate::remote::http_client;
use crate::security::CardSecurity;
use loomcoding_core::registry::RemoteFetched;
use loomcoding_core::task::TaskState;
use std::collections::BTreeMap;
use std::time::Duration;
use wiremock::matchers::{body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod auth;
mod factory;
mod rpc;
mod stream;
mod support;
