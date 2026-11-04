//! User-initiated writes. Partial commits are serialized and never check out files.
mod context;
mod index;
mod partial_commit;

pub use partial_commit::{
    partial_commit, CommitError, CommitOutcome, CommitSelection, IndexRecovery,
};

mod snapshot;
pub use snapshot::{snapshot, Snapshot};

static WRITES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
mod rollback;
pub use rollback::{rollback, rollback_patch};

pub mod branch;

pub mod remote;

mod reset;
pub use reset::undo_commit;

mod history_edit;
pub use history_edit::operation_state;

pub mod sequence;
