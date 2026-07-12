//! Error types for the arta agent layer.

use std::path::PathBuf;

use uuid::Uuid;

/// Errors produced by the agent layer.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Bridged error from the core object store.
    #[error(transparent)]
    Core(#[from] arta_core::ArtaError),

    /// An I/O operation against the repository's mutable metadata (HEAD, refs,
    /// task and checkpoint files) failed.
    #[error("i/o error at {path}: {source}")]
    Io {
        /// The path that was being operated on when the failure occurred.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// An operation required a current commit but the repository has none yet.
    #[error("repository has no commits yet")]
    EmptyHistory,

    /// A rollback query matched no commit in the current history.
    #[error("no commit in history matches the query: {0}")]
    NoMatch(String),

    /// A task was referenced by id but does not exist in the repository.
    #[error("no such task: {0}")]
    NoSuchTask(Uuid),

    /// An operation that requires an active task was attempted with none open.
    #[error("no task is currently active")]
    NoActiveTask,
}

/// Convenience alias for results returned by the agent layer.
pub type Result<T> = std::result::Result<T, AgentError>;

impl AgentError {
    /// Build an [`AgentError::Io`] carrying the path that failed.
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        AgentError::Io {
            path: path.into(),
            source,
        }
    }
}
