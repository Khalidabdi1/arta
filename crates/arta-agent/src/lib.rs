//! # arta-agent (L3)
//!
//! The agent layer: intent-aware commits, the task graph, checkpoints, and
//! rollback by intent or confidence. This is the layer that makes arta
//! different from git.
//!
//! Everything here builds on `arta-core`'s content-addressable [`BlobStore`]:
//! commits, checkpoints, and task nodes are ordinary content-addressed objects.
//! An [`AgentRepo`] ties them together with a moveable `HEAD`, so an agent can
//! commit intent, snapshot rollback points, and later ask to "go back to the
//! last point I was confident about" — queries that plain git cannot express.
//!
//! ```no_run
//! use arta_agent::AgentRepo;
//! use arta_core::AgentContext;
//!
//! # fn main() -> Result<(), arta_agent::AgentError> {
//! let mut repo = AgentRepo::init("/path/to/repo")?;
//! // a TreeSnapshot root hash from arta-core:
//! let snapshot = arta_core::ContentHash::of(b"tree");
//! let ctx = AgentContext::new("wire up token refresh", 0.9);
//! let commit = repo.commit(ctx, snapshot)?;
//! repo.checkpoint("before risky refactor")?;
//! // ... more commits ...
//! repo.rollback_to_confidence(0.8)?; // back to the last sure-footed commit
//! # let _ = commit;
//! # Ok(())
//! # }
//! ```
//!
//! [`BlobStore`]: arta_core::BlobStore

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod checkpoint;
mod commit;
mod repo;
mod task;

pub use checkpoint::Checkpoint;
pub use commit::AgentCommit;
pub use repo::AgentRepo;
pub use task::{TaskNode, TaskStatus};

/// Errors produced by the agent layer.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Bridged error from the core object store.
    #[error(transparent)]
    Core(#[from] arta_core::ArtaError),

    /// Reading or writing the repository's mutable state (`HEAD`, tasks) failed.
    #[error("i/o error at {path}: {source}")]
    Io {
        /// The path being operated on when the failure occurred.
        path: std::path::PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// An operation required a current `HEAD` but the repository has no commits
    /// yet.
    #[error("repository has no commits yet")]
    EmptyHistory,

    /// A rollback query matched no commit in the reachable history.
    #[error("no commit matched the query: {0}")]
    NoMatch(String),

    /// A task was referenced by id but is not present in the repository.
    #[error("no such task: {0}")]
    UnknownTask(uuid::Uuid),
}

/// Convenience alias for results returned by agent-layer APIs.
pub type Result<T> = std::result::Result<T, AgentError>;
