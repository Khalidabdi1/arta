//! Intent-aware commits.
//!
//! An [`AgentCommit`] is arta's analogue of a git commit, but it records *why*
//! a change was made — the agent's intent, tool calls, reasoning, and
//! confidence — alongside *what* changed (a snapshot hash and a parent link).
//! The "why" payload is an [`AgentContext`] from `arta-core`, so the same
//! structured metadata that the compat layer embeds in a git commit body lives
//! directly inside the commit here.
//!
//! Commits are content-addressed: serializing an [`AgentCommit`] and storing it
//! yields a [`ContentHash`] that names it. Because a commit carries its
//! parent's hash, the hashes form a chain that the [`AgentRepo`](crate::AgentRepo)
//! walks to produce history.

use arta_core::{AgentContext, BlobStore, ContentHash, ToolCall};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Result;

/// A single intent-aware commit.
///
/// The change itself is the `snapshot` (a `TreeSnapshot` root hash from
/// `arta-core`); `parent` links to the prior commit, if any. The `context`
/// carries intent, reasoning, tool calls, and confidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCommit {
    /// The working-tree snapshot this commit records.
    pub snapshot: ContentHash,
    /// The parent commit, or `None` for the first commit in a history.
    pub parent: Option<ContentHash>,
    /// The agent's intent, reasoning, tool calls, and confidence.
    pub context: AgentContext,
    /// The task this commit was made under, if any.
    pub task_id: Option<Uuid>,
    /// When the commit was created.
    pub timestamp: DateTime<Utc>,
}

impl AgentCommit {
    /// Create a commit recording `snapshot` under `context`, chained onto
    /// `parent`.
    ///
    /// The timestamp is taken from the system clock at construction time.
    pub fn new(context: AgentContext, snapshot: ContentHash, parent: Option<ContentHash>) -> Self {
        AgentCommit {
            snapshot,
            parent,
            context,
            task_id: None,
            timestamp: Utc::now(),
        }
    }

    /// Attach a task id, consuming and returning `self` for chaining.
    pub fn under_task(mut self, task_id: Uuid) -> Self {
        self.task_id = Some(task_id);
        self
    }

    /// The commit's intent string.
    pub fn intent(&self) -> &str {
        &self.context.intent
    }

    /// The commit's confidence score, in `0.0..=1.0`.
    pub fn confidence(&self) -> f32 {
        self.context.confidence
    }

    /// The tool calls recorded with this commit.
    pub fn tool_calls(&self) -> &[ToolCall] {
        &self.context.tool_calls
    }

    /// Serialize to the canonical JSON byte form used for storage.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self).map_err(arta_core::ArtaError::from)?)
    }

    /// Deserialize from the JSON byte form produced by [`AgentCommit::to_json`].
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes).map_err(arta_core::ArtaError::from)?)
    }

    /// Store this commit in `store`, returning its content hash.
    pub fn store(&self, store: &BlobStore) -> Result<ContentHash> {
        Ok(store.put(&self.to_json()?)?)
    }

    /// Load a commit from `store` by its hash.
    pub fn load(store: &BlobStore, hash: &ContentHash) -> Result<Self> {
        AgentCommit::from_json(&store.get(hash)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, BlobStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(dir.path().join("objects")).unwrap();
        (dir, store)
    }

    #[test]
    fn store_and_load_round_trips() {
        let (_dir, store) = store();
        let ctx = AgentContext::new("add backoff", 0.7);
        let snap = ContentHash::of(b"tree-1");
        let commit = AgentCommit::new(ctx, snap, None);
        let hash = commit.store(&store).unwrap();
        assert_eq!(AgentCommit::load(&store, &hash).unwrap(), commit);
    }

    #[test]
    fn accessors_read_through_to_context() {
        let ctx = AgentContext::new("do a thing", 0.42);
        let commit = AgentCommit::new(ctx, ContentHash::of(b"t"), None);
        assert_eq!(commit.intent(), "do a thing");
        assert_eq!(commit.confidence(), 0.42);
        assert!(commit.tool_calls().is_empty());
    }

    #[test]
    fn task_id_is_optional_and_settable() {
        let ctx = AgentContext::new("x", 1.0);
        let commit = AgentCommit::new(ctx, ContentHash::of(b"t"), None);
        assert!(commit.task_id.is_none());
        let id = Uuid::from_u128(7);
        assert_eq!(commit.under_task(id).task_id, Some(id));
    }
}
