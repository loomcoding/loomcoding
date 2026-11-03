use crate::ipc;
use crate::Cli;
use async_trait::async_trait;
use codeloom_a2a::serve::rpc::{EventStream, HostError, TaskHost, TaskView};
use codeloom_core::error::CoreError;
use codeloom_core::event::LiveEvent;
use codeloom_core::job::{JobId, JobStatus};
use codeloom_core::message::Message;
use codeloom_core::permission::{Decision, PermissionRequest, PermissionResponder};
use codeloom_core::provider::{Provider, ProviderRequest, StreamChunk};
use codeloom_core::question::{Answer, Question};
use codeloom_core::session::{SessionId, SessionStore};
use futures::StreamExt;
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
