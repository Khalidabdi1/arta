//! # arta-compat (L0)
//!
//! Git compatibility layer. Translates between arta's internal object format
//! and the standard `.git` wire format so that every arta repository is also a
//! valid git repository.
//!
//! Phase 2 (see `CLAUDE.md`) lands the git object model here:
//!
//! - [`GitOid`] — SHA1 object identity, git's content address
//! - [`GitObject`] — blob / tree / commit, with git's exact framing and parsing
//! - [`LooseObjectStore`] — read/write of zlib-compressed loose objects in
//!   git's `.git/objects/aa/bbbb…` layout
//!
//! Packfile support and full `AgentCommit` translation build on top of these in
//! later phases.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod loose;
mod object;
mod oid;

pub use error::CompatError;
pub use loose::LooseObjectStore;
pub use object::{CommitObject, FileMode, GitObject, GitObjectKind, Signature, TreeRecord};
pub use oid::{GitOid, OID_LEN};
