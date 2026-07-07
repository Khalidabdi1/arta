//! Error types for the arta core object store.

use std::path::PathBuf;

/// Errors produced by the arta core layer.
#[derive(Debug, thiserror::Error)]
pub enum ArtaError {
    /// An I/O operation against the object store or working tree failed.
    #[error("i/o error at {path}: {source}")]
    Io {
        /// The path that was being operated on when the failure occurred.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A hex string could not be decoded into a [`crate::ContentHash`].
    #[error("invalid content hash: {0}")]
    InvalidHash(String),

    /// An object was requested by hash but does not exist in the store.
    #[error("object not found: {0}")]
    NotFound(String),

    /// (De)serialization of an agent context object failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Convenience alias for results returned by arta core APIs.
pub type Result<T> = std::result::Result<T, ArtaError>;

impl ArtaError {
    /// Build an [`ArtaError::Io`] carrying the path that failed.
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        ArtaError::Io {
            path: path.into(),
            source,
        }
    }
}
