//! # arta-agent (L3)
//!
//! The agent layer: intent-aware commits, the task graph, checkpoints, and
//! rollback by intent or confidence. This is the layer that makes arta
//! different from git.
//!
//! Phase 3 lands here incrementally (see `CLAUDE.md`). Implemented so far:
//!
//! - [`AgentCommit`] — creation and content-addressed storage of an
//!   intent-aware commit.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod commit;
mod error;

pub use commit::AgentCommit;
pub use error::AgentError;
