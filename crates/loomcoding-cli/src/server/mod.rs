use crate::ipc;
use crate::Cli;
use async_trait::async_trait;
use futures::StreamExt;
use loomcoding_a2a::serve::rpc::{EventStream, HostError, TaskHost, TaskView};
use loomcoding_core::error::CoreError;
use loomcoding_core::event::LiveEvent;
use loomcoding_core::job::{JobId, JobStatus};
use loomcoding_core::message::Message;
use loomcoding_core::permission::{Decision, PermissionRequest, PermissionResponder};
use loomcoding_core::provider::{Provider, ProviderRequest, StreamChunk};
use loomcoding_core::question::{Answer, Question};
use loomcoding_core::session::{SessionId, SessionStore};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
#[cfg(unix)]
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex, RwLock, Semaphore};
use tokio_util::sync::CancellationToken;

mod protocol;
pub use protocol::*;

mod daemon;
pub use daemon::*;

mod client;
pub use client::*;

mod manager;
use manager::*;

mod worker;
use worker::*;

mod snapshot;
use snapshot::*;

mod agent_build;
use agent_build::*;

mod agent_ui;
use agent_ui::*;

mod runtime_tasks;
use runtime_tasks::*;

#[cfg(test)]
mod runtime_tests;

#[cfg(test)]
mod tests;
