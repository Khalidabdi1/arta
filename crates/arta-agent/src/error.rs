//! Error type for the agent layer.

/// Errors produced by the agent layer.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Bridged error from the core object store.
    #[error(transparent)]
    Core(#[from] arta_core::ArtaError),

    /// Failed to serialize or deserialize an agent object as JSON.
    #[error("failed to (de)serialize agent object: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// A `Result` specialized to [`AgentError`].
pub type Result<T> = std::result::Result<T, AgentError>;
