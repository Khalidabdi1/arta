//! # arta-compat (L0)
//!
//! Git compatibility layer. Translates between arta's internal object format
//! and the standard `.git` wire format so that every arta repository is also a
//! valid git repository.
//!
//! Not yet implemented — see Phase 2 in `CLAUDE.md`. This crate is scaffolded
//! so the workspace builds while `arta-core` (Phase 1) lands first.

#![forbid(unsafe_code)]

/// Errors produced by the git compatibility layer.
#[derive(Debug, thiserror::Error)]
pub enum CompatError {
    /// Bridged error from the core object store.
    #[error(transparent)]
    Core(#[from] arta_core::ArtaError),
}
