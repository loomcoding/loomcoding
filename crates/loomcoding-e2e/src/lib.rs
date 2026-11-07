//! Apoyo para pruebas E2E visuales de Loomcoding. Solo se usa como `dev-dependency`.

pub mod daemon;
pub mod mock;
pub mod snapshot;
pub mod term;

pub use daemon::kill_daemons_of;
pub use mock::{
    mock_provider, mock_provider_child_asks, mock_provider_empty_subagent, mock_provider_odd,
    mock_provider_watch, Workspace, ODD_CIERRE_FINAL, ODD_ID_BASH, ODD_INTENTO_DE_CIERRE,
};
pub use snapshot::{CellSnap, ColorSnap, Snapshot};
pub use term::{cpr_reply, paste_bytes, Key, Term, TermConfig, WaitError};
