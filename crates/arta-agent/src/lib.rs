//! # arta-agent (L3)
//!
//! The agent layer: intent-aware commits, the task graph, checkpoints, and
//! rollback by intent or confidence. This is the layer that makes arta
//! different from git.
//!
//! Phase 3 lands here incrementally (see `CLAUDE.md`). Implemented so far:
//!
//! - [`AgentCommit`] — an intent-aware commit, content-addressed and stored in
//!   an [`arta_core::BlobStore`] like every other arta object.
//!
//! Still to come: checkpoints, the task graph, and rollback by intent or
//! confidence.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod commit;
mod error;

pub use commit::AgentCommit;
pub use error::{AgentError, Result};
