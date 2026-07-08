//! Error types for the git compatibility layer.

use std::path::PathBuf;

/// Errors produced by the git compatibility layer.
#[derive(Debug, thiserror::Error)]
pub enum CompatError {
    /// Bridged error from the core object store.
    #[error(transparent)]
    Core(#[from] arta_core::ArtaError),

    /// An I/O operation against the git object store failed.
    #[error("i/o error at {path}: {source}")]
    Io {
        /// The path being operated on when the failure occurred.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A hex string could not be decoded into a [`crate::GitOid`].
    #[error("invalid git object id: {0}")]
    InvalidOid(String),

    /// An object was requested by oid but is not present in the store.
    #[error("git object not found: {0}")]
    NotFound(String),

    /// A git object's bytes could not be parsed into the expected structure.
    #[error("malformed git object: {0}")]
    Malformed(String),

    /// An object type that this layer does not (yet) handle was encountered.
    #[error("unsupported git object type: {0}")]
    UnsupportedObject(String),

    /// A stored object's bytes did not hash to the oid it was fetched under.
    #[error("corrupt git object: expected {expected}, computed {actual}")]
    Corrupt {
        /// The oid the object was requested under.
        expected: String,
        /// The oid actually computed from the object's bytes.
        actual: String,
    },
}

impl CompatError {
    /// Build a [`CompatError::Io`] carrying the path that failed.
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        CompatError::Io {
            path: path.into(),
            source,
        }
    }
}
