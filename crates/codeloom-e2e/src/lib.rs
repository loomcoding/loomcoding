//! Apoyo para pruebas E2E visuales de CodeLoom. Solo se usa como `dev-dependency`.

pub mod mock;
pub mod snapshot;
pub mod term;

pub use mock::{mock_provider, Workspace};
pub use snapshot::{CellSnap, ColorSnap, Snapshot};
pub use term::{cpr_reply, paste_bytes, Key, Term, TermConfig, WaitError};
