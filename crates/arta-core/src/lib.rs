//! # arta-core
//!
//! The L2 core of arta: a content-addressable object store with first-class
//! support for agent context. This crate provides the primitives the higher
//! layers (agent, compat, cli) build on:
//!
//! - [`ContentHash`] — BLAKE3 content addressing (arta's replacement for SHA1)
//! - [`BlobStore`] — an on-disk, deduplicating store of raw objects
//! - [`TreeSnapshot`] — a recursively content-addressed directory snapshot
//! - [`AgentContext`] — structured intent/reasoning/confidence metadata
//!
//! Everything here is synchronous and has no ambient global state; a store is
//! just a handle to a directory. This keeps the core `no_std`-friendly in
//! spirit and amenable to a future WASM build target.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod context;
mod error;
mod hash;
mod snapshot;
mod store;

pub use context::{AgentContext, ToolCall};
pub use error::{ArtaError, Result};
pub use hash::{ContentHash, HASH_LEN};
pub use snapshot::{EntryTarget, TreeEntry, TreeSnapshot};
pub use store::BlobStore;
