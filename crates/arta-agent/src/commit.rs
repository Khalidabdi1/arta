//! Intent-aware commits.
//!
//! An [`AgentCommit`] is arta's analogue of a git commit, but it records *why*
//! a change was made — the agent's intent, tool calls, reasoning, and
//! confidence — not just the tree it points at. Commits form a chain through
//! their `parent` link; walking that chain is how the [`crate::AgentRepo`]
//! answers rollback queries.

use arta_core::{AgentContext, BlobStore, ContentHash, ToolCall};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

/// A single intent-aware commit.
///
/// A commit binds a working-tree [`snapshot`](AgentCommit::snapshot) to the
/// agent context that produced it. It is content-addressed like every other
/// arta object: storing a commit yields the [`ContentHash`] that names it, and
/// the same commit bytes always hash to the same address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCommit {
    /// The tree snapshot this commit records (a root hash from `arta-core`).
    pub snapshot: ContentHash,
    /// The commit this one builds on, or `None` for the first commit.
    pub parent: Option<ContentHash>,
    /// A short natural-language statement of what the change is meant to do.
    pub intent: String,
    /// The task this commit was made under, if any (see [`crate::TaskNode`]).
    pub task_id: Option<Uuid>,
    /// The tool calls the agent issued while producing the change.
    pub tool_calls: Vec<ToolCall>,
    /// Optional longer-form reasoning chain.
    pub reasoning: Option<String>,
    /// The agent's confidence: `0.0` = guessing, `1.0` = certain.
    pub confidence: f32,
    /// When the commit was created.
    pub timestamp: DateTime<Utc>,
}

impl AgentCommit {
    /// Build a commit from a snapshot, its parent, and the agent context that
    /// produced it. The intent, tool calls, reasoning, and confidence are taken
    /// from `context`; the timestamp is set to now.
    pub fn new(
        snapshot: ContentHash,
        parent: Option<ContentHash>,
        task_id: Option<Uuid>,
        context: AgentContext,
    ) -> Self {
        AgentCommit {
            snapshot,
            parent,
            intent: context.intent,
            task_id,
            tool_calls: context.tool_calls,
            reasoning: context.reasoning,
            confidence: context.confidence,
            timestamp: Utc::now(),
        }
    }

    /// Serialize to the canonical JSON byte form used for storage.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self).map_err(arta_core::ArtaError::from)?)
    }

    /// Deserialize from the JSON byte form produced by [`AgentCommit::to_json`].
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes).map_err(arta_core::ArtaError::from)?)
    }

    /// Store this commit in `store`, returning the content hash that names it.
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
    fn commit_round_trips_through_the_store() {
        let (_dir, store) = store();
        let ctx = AgentContext::new("first change", 0.9).with_reasoning("because");
        let commit = AgentCommit::new(ContentHash::of(b"tree"), None, None, ctx);
        let hash = commit.store(&store).unwrap();
        assert_eq!(AgentCommit::load(&store, &hash).unwrap(), commit);
    }

    #[test]
    fn context_fields_carry_onto_the_commit() {
        let (_dir, store) = store();
        let ctx = AgentContext::new("do the thing", 0.42).with_tool_call(ToolCall {
            name: "edit_file".into(),
            arguments: serde_json::json!({ "path": "a.rs" }),
        });
        let commit = AgentCommit::new(ContentHash::of(b"tree"), None, None, ctx);
        assert_eq!(commit.intent, "do the thing");
        assert_eq!(commit.confidence, 0.42);
        assert_eq!(commit.tool_calls.len(), 1);
        // Storing is content-addressed and idempotent.
        assert_eq!(commit.store(&store).unwrap(), commit.store(&store).unwrap());
    }
}
