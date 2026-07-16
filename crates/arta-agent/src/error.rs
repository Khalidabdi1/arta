//! Error types for the arta agent layer.

/// Errors produced by the agent layer.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Bridged error from the core object store.
    #[error(transparent)]
    Core(#[from] arta_core::ArtaError),

    /// (De)serialization of an agent-layer object failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An I/O operation against the repository metadata failed.
    #[error("i/o error at {path}: {source}")]
    Io {
        /// The path being operated on when the failure occurred.
        path: std::path::PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A rollback or lookup query matched no commit in history.
    #[error("no commit in history matched query: {0}")]
    NoMatch(String),

    /// A task was referenced by id but does not exist in the graph.
    #[error("task not found: {0}")]
    TaskNotFound(uuid::Uuid),

    /// An operation required a current commit but the repository is empty.
    #[error("repository has no commits yet")]
    Empty,
}

/// Convenience alias for results returned by the agent layer.
pub type Result<T> = std::result::Result<T, AgentError>;

impl AgentError {
    /// Build an [`AgentError::Io`] carrying the path that failed.
    pub(crate) fn io(path: impl Into<std::path::PathBuf>, source: std::io::Error) -> Self {
        AgentError::Io {
            path: path.into(),
            source,
        }
    }
}
