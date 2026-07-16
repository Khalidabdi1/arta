//! Intent-aware commits.
//!
//! An [`AgentCommit`] is arta's answer to a git commit: it points at a
//! content-addressed [`TreeSnapshot`](arta_core::TreeSnapshot) and its parent
//! commit, but it also records *why* the change was made — the agent's intent,
//! tool calls, reasoning, and confidence — plus the task it belongs to.
//!
//! Commits are stored as JSON blobs in the shared [`BlobStore`] and addressed
//! by their [`ContentHash`], exactly like every other arta object.

use arta_core::{AgentContext, BlobStore, ContentHash, ToolCall};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

/// A single intent-aware commit in the agent history.
///
/// The parent link makes commits a singly linked chain (history is walked by
/// following `parent`), while `task_id` associates the commit with a node in
/// the task graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCommit {
    /// The root hash of the working-tree snapshot this commit records.
    pub snapshot: ContentHash,
    /// The commit this one builds on, or `None` for the first commit.
    pub parent: Option<ContentHash>,
    /// A short natural-language statement of what the change is meant to do.
    pub intent: String,
    /// The task this commit belongs to, if it was made inside one.
    pub task_id: Option<Uuid>,
    /// The tool calls the agent issued while producing the change.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// Optional longer-form reasoning chain.
    #[serde(default)]
    pub reasoning: Option<String>,
    /// The agent's confidence: `0.0` = guessing, `1.0` = certain.
    pub confidence: f32,
    /// When the commit was created.
    pub timestamp: DateTime<Utc>,
}

impl AgentCommit {
    /// Build a commit from a snapshot, its parent, and the agent context that
    /// produced it. The timestamp is stamped at construction time.
    pub fn new(
        snapshot: ContentHash,
        parent: Option<ContentHash>,
        context: AgentContext,
        task_id: Option<Uuid>,
    ) -> Self {
        AgentCommit {
            snapshot,
            parent,
            intent: context.intent,
            task_id,
            tool_calls: context.tool_calls,
            reasoning: context.reasoning,
            confidence: context.confidence.clamp(0.0, 1.0),
            timestamp: Utc::now(),
        }
    }

    /// Reconstruct the [`AgentContext`] embedded in this commit.
    pub fn context(&self) -> AgentContext {
        AgentContext {
            intent: self.intent.clone(),
            tool_calls: self.tool_calls.clone(),
            reasoning: self.reasoning.clone(),
            confidence: self.confidence,
        }
    }

    /// Serialize to the canonical JSON byte form used for storage.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }

    /// Deserialize from the JSON byte form produced by [`AgentCommit::to_json`].
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
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
    fn commit_round_trips_through_store() {
        let (_dir, store) = store();
        let snapshot = ContentHash::of(b"tree");
        let ctx = AgentContext::new("add retry to token refresh", 0.9)
            .with_reasoning("refresh path had no backoff");
        let commit = AgentCommit::new(snapshot, None, ctx, None);
        let hash = commit.store(&store).unwrap();
        assert_eq!(AgentCommit::load(&store, &hash).unwrap(), commit);
    }

    #[test]
    fn context_round_trips_into_and_out_of_commit() {
        let ctx = AgentContext::new("do a thing", 0.5).with_tool_call(ToolCall {
            name: "edit_file".into(),
            arguments: serde_json::json!({ "path": "x.rs" }),
        });
        let commit = AgentCommit::new(ContentHash::of(b"t"), None, ctx.clone(), None);
        assert_eq!(commit.context(), ctx);
    }

    #[test]
    fn confidence_is_clamped_on_construction() {
        let ctx = AgentContext {
            intent: "x".into(),
            tool_calls: vec![],
            reasoning: None,
            confidence: 4.0,
        };
        let commit = AgentCommit::new(ContentHash::of(b"t"), None, ctx, None);
        assert_eq!(commit.confidence, 1.0);
    }
}
