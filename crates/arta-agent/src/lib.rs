//! # arta-agent (L3)
//!
//! The agent layer: intent-aware commits, the task graph, checkpoints, and
//! rollback by intent or confidence. This is the layer that makes arta
//! different from git.
//!
//! It builds on [`arta_core`]'s content-addressed object store. An
//! [`AgentRepo`] owns a [`BlobStore`](arta_core::BlobStore) plus a small amount
//! of mutable state (`HEAD`, checkpoints, the task graph), persisted under an
//! `.arta` directory.
//!
//! ## What this layer provides
//!
//! - [`AgentCommit`] — a commit that records intent, tool calls, reasoning, and
//!   a confidence score alongside the snapshot it captures.
//! - [`Checkpoint`] — a named rollback point pinning a commit and its snapshot.
//! - [`TaskNode`] / [`TaskStatus`] — nodes in the task graph agents work in.
//! - [`AgentRepo`] — the entry point tying these together, with
//!   [`rollback_to_intent`](AgentRepo::rollback_to_intent) and
//!   [`rollback_to_confidence`](AgentRepo::rollback_to_confidence).
//!
//! ```no_run
//! use arta_agent::AgentRepo;
//! use arta_core::{AgentContext, TreeSnapshot};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut repo = AgentRepo::open(".")?;
//! let (_tree, snapshot) = TreeSnapshot::snapshot_dir(repo.store(), Path::new("src"))?;
//! let task = repo.open_task("refactor auth", None)?;
//! repo.commit(AgentContext::new("extract token refresh", 0.9), snapshot)?;
//! repo.checkpoint("before_risky_refactor")?;
//! repo.complete_task(task)?;
//! # Ok(()) }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod checkpoint;
mod commit;
mod error;
mod intent;
mod repo;
mod task;

pub use checkpoint::Checkpoint;
pub use commit::AgentCommit;
pub use error::{AgentError, Result};
pub use intent::{match_strength, MatchStrength};
pub use repo::AgentRepo;
pub use task::{TaskNode, TaskStatus};
