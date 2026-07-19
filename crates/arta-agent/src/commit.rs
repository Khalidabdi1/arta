//! Intent-aware commits.
//!
//! An [`AgentCommit`] is arta's answer to a git commit. Instead of a free-form
//! message it carries a structured [`AgentContext`] — the agent's intent, the
//! tool calls it issued, its reasoning, and its confidence — alongside the
//! snapshot it records and a link to its parent. Commits are content-addressed
//! and stored in the same [`BlobStore`] as every other object, so identical
//! commits deduplicate and a commit's hash is a stable identifier for its
//! history.

use serde::{Deserialize, Serialize};

use arta_core::{AgentContext, BlobStore, ContentHash};

use crate::error::Result;

/// An intent-aware commit: a tree snapshot plus the context explaining *why*.
///
/// This is the agent-layer analogue of a git commit. The `snapshot` and
/// `parent` fields form the history DAG exactly as git's tree/parent do; the
/// [`AgentContext`] is the richer payload that standard git lacks. When the
/// compat layer emits this as a git commit, the context is serialized into the
/// commit body as JSON, invisible to git tooling but recoverable by arta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCommit {
    /// Content hash of the [`TreeSnapshot`](arta_core::TreeSnapshot) this
    /// commit records.
    pub snapshot: ContentHash,
    /// The parent commit, or `None` for the first commit on a history.
    pub parent: Option<ContentHash>,
    /// The task this commit belongs to, if it was made under an open task.
    pub task_id: Option<String>,
    /// The agent context: intent, tool calls, reasoning, and confidence.
    pub context: AgentContext,
    /// Creation time as whole seconds since the Unix epoch (UTC).
    ///
    /// The timestamp is supplied by the caller rather than read from the clock,
    /// so commit construction stays pure and deterministic in tests; the CLI is
    /// responsible for stamping wall-clock time.
    pub created_at: u64,
}

impl AgentCommit {
    /// Create a commit recording `snapshot` under `context`.
    ///
    /// `parent` links to the previous commit (`None` for a root commit) and
    /// `created_at` is whole seconds since the Unix epoch.
    pub fn new(
        snapshot: ContentHash,
        parent: Option<ContentHash>,
        context: AgentContext,
        created_at: u64,
    ) -> Self {
        AgentCommit {
            snapshot,
            parent,
            task_id: None,
            context,
            created_at,
        }
    }

    /// Attach a task id, consuming and returning `self` for chaining.
    pub fn with_task(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    /// The intent recorded on this commit.
    pub fn intent(&self) -> &str {
        &self.context.intent
    }

    /// The confidence recorded on this commit (`0.0`..=`1.0`).
    pub fn confidence(&self) -> f32 {
        self.context.confidence
    }

    /// Whether this commit is a root (has no parent).
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
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
    /// Storage is content-addressed and idempotent: committing identical
    /// content (same snapshot, parent, context, task, and timestamp) yields the
    /// same hash and performs no second write.
    pub fn store(&self, store: &BlobStore) -> Result<ContentHash> {
        Ok(store.put(&self.to_json()?)?)
    }

    /// Load a commit from `store` by its content hash.
    pub fn load(store: &BlobStore, hash: &ContentHash) -> Result<Self> {
        AgentCommit::from_json(&store.get(hash)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arta_core::ToolCall;

    fn store() -> (tempfile::TempDir, BlobStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(dir.path().join("objects")).unwrap();
        (dir, store)
    }

    fn sample_context() -> AgentContext {
        AgentContext::new("add retry to token refresh", 0.85)
            .with_reasoning("the refresh path had no backoff")
            .with_tool_call(ToolCall {
                name: "edit_file".into(),
                arguments: serde_json::json!({ "path": "auth.rs" }),
            })
    }

    #[test]
    fn new_defaults_to_no_task() {
        let snap = ContentHash::of(b"tree");
        let commit = AgentCommit::new(snap, None, sample_context(), 1_700_000_000);
        assert!(commit.task_id.is_none());
        assert!(commit.is_root());
        assert_eq!(commit.intent(), "add retry to token refresh");
        assert_eq!(commit.confidence(), 0.85);
    }

    #[test]
    fn with_task_attaches_id() {
        let commit = AgentCommit::new(ContentHash::of(b"tree"), None, sample_context(), 1)
            .with_task("refactor-auth");
        assert_eq!(commit.task_id.as_deref(), Some("refactor-auth"));
    }

    #[test]
    fn parent_marks_non_root() {
        let root = AgentCommit::new(ContentHash::of(b"t0"), None, sample_context(), 1);
        let child = AgentCommit::new(
            ContentHash::of(b"t1"),
            Some(ContentHash::of(b"t0")),
            sample_context(),
            2,
        );
        assert!(root.is_root());
        assert!(!child.is_root());
    }

    #[test]
    fn json_round_trips() {
        let commit = AgentCommit::new(
            ContentHash::of(b"tree"),
            Some(ContentHash::of(b"parent")),
            sample_context(),
            1_700_000_000,
        )
        .with_task("t-1");
        let bytes = commit.to_json().unwrap();
        assert_eq!(AgentCommit::from_json(&bytes).unwrap(), commit);
    }

    #[test]
    fn store_and_load_round_trips() {
        let (_dir, store) = store();
        let commit = AgentCommit::new(ContentHash::of(b"tree"), None, sample_context(), 42);
        let hash = commit.store(&store).unwrap();
        assert_eq!(AgentCommit::load(&store, &hash).unwrap(), commit);
    }

    #[test]
    fn store_is_content_addressed_and_idempotent() {
        let (_dir, store) = store();
        let commit = AgentCommit::new(ContentHash::of(b"tree"), None, sample_context(), 42);
        let a = commit.store(&store).unwrap();
        let b = commit.store(&store).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn distinct_commits_get_distinct_hashes() {
        let (_dir, store) = store();
        let base = AgentCommit::new(ContentHash::of(b"tree"), None, sample_context(), 42);
        // A different timestamp is a different commit.
        let later = AgentCommit::new(ContentHash::of(b"tree"), None, sample_context(), 43);
        assert_ne!(base.store(&store).unwrap(), later.store(&store).unwrap());
    }

    #[test]
    fn load_missing_is_error() {
        let (_dir, store) = store();
        assert!(AgentCommit::load(&store, &ContentHash::of(b"absent")).is_err());
    }
}
