//! # arta-agent (L3)
//!
//! The agent layer: intent-aware commits, the task graph, checkpoints, and
//! rollback by intent or confidence. This is the layer that makes arta
//! different from git.
//!
//! Not yet implemented — see Phase 3 in `CLAUDE.md`. This crate is scaffolded
//! so the workspace builds while `arta-core` (Phase 1) lands first.

#![forbid(unsafe_code)]

/// Errors produced by the agent layer.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Bridged error from the core object store.
    #[error(transparent)]
    Core(#[from] arta_core::ArtaError),
}
