//! # arta-agent (L3)
//!
//! The agent layer: intent-aware commits, the task graph, checkpoints, and
//! rollback by intent or confidence. This is the layer that makes arta
//! different from git.
//!
//! Phase 3 is landing incrementally (see `CLAUDE.md`). Implemented so far:
//!
//! - [`AgentCommit`] — creation, content-addressed storage, and retrieval
//!
//! Still to come: checkpoints, the task graph, and rollback by intent or
//! confidence.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod commit;
mod error;

pub use commit::AgentCommit;
pub use error::{AgentError, Result};

// Re-export the core context types so callers of the agent layer can build a
// commit without also depending on `arta-core` directly.
pub use arta_core::{AgentContext, ToolCall};
