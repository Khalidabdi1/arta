//! Intent-aware commits.
//!
//! An [`AgentCommit`] is arta's equivalent of a git commit, but it records
//! *why* a change was made in addition to *what* changed. Alongside the
//! [`ContentHash`] of a [`TreeSnapshot`] and an optional parent, it carries the
//! agent's intent, the tool calls it issued, its reasoning chain, a confidence
//! score, and — optionally — the task it belongs to.
//!
//! Commits are content-addressed just like every other arta object: a commit
//! is serialized to canonical JSON and stored in a [`BlobStore`], addressed by
//! the BLAKE3 hash of those bytes. Because the timestamp is part of the payload
//! two otherwise-identical commits still hash differently, which is what lets a
//! linear history exist at all.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use arta_core::{AgentContext, BlobStore, ContentHash, ToolCall};

use crate::error::Result;

/// A commit that records the agent's intent behind a change.
///
/// The `snapshot` field points at the [`TreeSnapshot`](arta_core::TreeSnapshot)
/// captured for this commit; `parent` points at the previous [`AgentCommit`],
/// or is `None` for the first commit on a branch. The remaining fields mirror
/// [`AgentContext`] and describe why the change was made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCommit {
    /// The tree snapshot this commit records.
    pub snapshot: ContentHash,
    /// The parent commit, or `None` for a branch's first commit.
    pub parent: Option<ContentHash>,
    /// A short natural-language statement of what the change is meant to do.
    pub intent: String,
    /// The task this commit belongs to, if it was made inside an open task.
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
    /// Create a commit for `snapshot` from an [`AgentContext`], stamping it with
    /// the current time.
    ///
    /// Use [`AgentCommit::with_parent`] and [`AgentCommit::in_task`] to attach a
    /// parent commit and a task, respectively.
    pub fn new(snapshot: ContentHash, context: AgentContext) -> Self {
        AgentCommit {
            snapshot,
            parent: None,
            intent: context.intent,
            task_id: None,
            tool_calls: context.tool_calls,
            reasoning: context.reasoning,
            confidence: context.confidence,
            timestamp: Utc::now(),
        }
    }

    /// Set the parent commit, consuming and returning `self` for chaining.
    pub fn with_parent(mut self, parent: ContentHash) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Associate this commit with a task, consuming and returning `self`.
    pub fn in_task(mut self, task_id: Uuid) -> Self {
        self.task_id = Some(task_id);
        self
    }

    /// Override the commit timestamp, consuming and returning `self`.
    ///
    /// Primarily useful for reconstructing a commit deterministically (for
    /// example in tests) rather than stamping it with the wall clock.
    pub fn at(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Reconstruct the [`AgentContext`] describing this commit's intent.
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
    ///
    /// The returned [`ContentHash`] is the commit's identity and is what a
    /// child commit records as its `parent`.
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

    use chrono::TimeZone;

    fn store() -> (tempfile::TempDir, BlobStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(dir.path().join("objects")).unwrap();
        (dir, store)
    }

    fn snapshot_hash(seed: &[u8]) -> ContentHash {
        ContentHash::of(seed)
    }

    #[test]
    fn store_and_load_round_trips() {
        let (_dir, store) = store();
        let ctx = AgentContext::new("initialize the repo", 0.9);
        let commit = AgentCommit::new(snapshot_hash(b"tree-a"), ctx);
        let hash = commit.store(&store).unwrap();
        assert_eq!(AgentCommit::load(&store, &hash).unwrap(), commit);
    }

    #[test]
    fn json_round_trips_with_all_fields() {
        let ctx = AgentContext::new("add backoff to token refresh", 0.75)
            .with_reasoning("the refresh path retried without delay")
            .with_tool_call(ToolCall {
                name: "edit_file".into(),
                arguments: serde_json::json!({ "path": "auth.rs" }),
            });
        let commit = AgentCommit::new(snapshot_hash(b"tree-b"), ctx)
            .with_parent(snapshot_hash(b"parent"))
            .in_task(Uuid::from_u128(42));
        let bytes = commit.to_json().unwrap();
        assert_eq!(AgentCommit::from_json(&bytes).unwrap(), commit);
    }

    #[test]
    fn parent_and_task_default_to_none() {
        let commit = AgentCommit::new(snapshot_hash(b"tree-c"), AgentContext::new("x", 1.0));
        assert!(commit.parent.is_none());
        assert!(commit.task_id.is_none());
    }

    #[test]
    fn context_round_trips_through_commit() {
        let ctx = AgentContext::new("refactor", 0.5).with_reasoning("cleaner");
        let commit = AgentCommit::new(snapshot_hash(b"tree-d"), ctx.clone());
        assert_eq!(commit.context(), ctx);
    }

    #[test]
    fn differing_timestamps_produce_distinct_hashes() {
        // Two commits over the same tree at different times are different
        // objects — this is what lets a linear history exist.
        let (_dir, store) = store();
        let tree = snapshot_hash(b"tree-e");
        let t1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();
        let a = AgentCommit::new(tree, AgentContext::new("same", 1.0)).at(t1);
        let b = AgentCommit::new(tree, AgentContext::new("same", 1.0)).at(t2);
        assert_ne!(a.store(&store).unwrap(), b.store(&store).unwrap());
    }

    #[test]
    fn a_chain_of_commits_links_by_hash() {
        let (_dir, store) = store();
        let root = AgentCommit::new(snapshot_hash(b"t0"), AgentContext::new("root", 1.0));
        let root_hash = root.store(&store).unwrap();

        let child = AgentCommit::new(snapshot_hash(b"t1"), AgentContext::new("child", 0.8))
            .with_parent(root_hash);
        let child_hash = child.store(&store).unwrap();

        let loaded = AgentCommit::load(&store, &child_hash).unwrap();
        assert_eq!(loaded.parent, Some(root_hash));
    }
}
