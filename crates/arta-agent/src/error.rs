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
}

/// Convenience alias for results returned by arta agent APIs.
pub type Result<T> = std::result::Result<T, AgentError>;
