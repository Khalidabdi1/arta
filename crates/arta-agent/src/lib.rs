//! # arta-agent (L3)
//!
//! The agent layer: intent-aware commits, the task graph, checkpoints, and
//! rollback by intent or confidence. This is the layer that makes arta
//! different from git.
//!
//! The pieces fit together through [`AgentRepo`], a handle onto an on-disk
//! repository:
//!
//! - [`AgentCommit`] records *why* a change was made — intent, tool calls,
//!   reasoning, and a confidence score — alongside the tree snapshot it points
//!   at. Commits chain through their parent link.
//! - [`Checkpoint`] is an explicit, named rollback point saved ahead of time.
//! - [`TaskNode`] / [`TaskStatus`] model the task graph: agents open a goal,
//!   commit under it, and complete or abandon it. Commits made while a task is
//!   active are attributed to it.
//! - [`AgentRepo`] exposes the operations git cannot: [`commit`](AgentRepo::commit)
//!   with intent, [`checkpoint`](AgentRepo::checkpoint),
//!   [`rollback_to_intent`](AgentRepo::rollback_to_intent), and
//!   [`rollback_to_confidence`](AgentRepo::rollback_to_confidence).
//!
//! Everything here is synchronous, matching the rest of the workspace;
//! concurrent branch operations are Phase 5 hardening.
//!
//! ```no_run
//! use arta_agent::AgentRepo;
//! use arta_core::AgentContext;
//! # fn main() -> Result<(), arta_agent::AgentError> {
//! let repo = AgentRepo::open(".arta")?;
//! let task = repo.open_task("refactor auth", None)?;
//! // `snapshot` is a tree root hash from arta-core::TreeSnapshot.
//! # let snapshot = arta_core::ContentHash::of(b"tree");
//! repo.commit(snapshot, AgentContext::new("split the token module", 0.9))?;
//! repo.checkpoint("before_risky_refactor")?;
//! repo.complete_task(task)?;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod checkpoint;
mod commit;
mod error;
mod repo;
mod task;

pub use checkpoint::Checkpoint;
pub use commit::AgentCommit;
pub use error::{AgentError, Result};
pub use repo::AgentRepo;
pub use task::{TaskNode, TaskStatus};
