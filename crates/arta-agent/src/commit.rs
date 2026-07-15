//! Intent-aware commits.
//!
//! An [`AgentCommit`] is arta's answer to a git commit: it points at a
//! content-addressed [`TreeSnapshot`](arta_core::TreeSnapshot) and its parent
//! commit, but it also carries the agent's [`AgentContext`] — intent,
//! reasoning, tool calls, and confidence — plus the task it belongs to and
//! when it was made. Like every other arta object it is stored as JSON and
//! addressed by its [`ContentHash`], so identical commits deduplicate and a
//! commit's hash is a stable function of its content.

use arta_core::{AgentContext, BlobStore, ContentHash};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AgentError;

/// A single intent-aware commit in an arta history.
///
/// The intent, reasoning, tool calls, and confidence live in the embedded
/// [`AgentContext`], which is flattened into the serialized form so a commit's
/// JSON reads as one flat object (matching how the compat layer embeds it in a
/// git commit body).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCommit {
    /// The root hash of the working-tree snapshot this commit records.
    pub snapshot: ContentHash,
    /// The parent commit, or `None` for the first commit in a history.
    pub parent: Option<ContentHash>,
    /// The task this commit was made under, if any.
    pub task_id: Option<Uuid>,
    /// When the commit was created.
    pub timestamp: DateTime<Utc>,
    /// The agent's intent, reasoning, tool calls, and confidence.
    #[serde(flatten)]
    pub context: AgentContext,
}

impl AgentCommit {
    /// Create a commit for `snapshot` carrying `context`, timestamped now.
    ///
    /// The commit has no parent and belongs to no task; attach those with
    /// [`AgentCommit::with_parent`] and [`AgentCommit::with_task`].
    pub fn new(snapshot: ContentHash, context: AgentContext) -> Self {
        AgentCommit {
            snapshot,
            parent: None,
            task_id: None,
            timestamp: Utc::now(),
            context,
        }
    }

    /// Set the parent commit, consuming and returning `self` for chaining.
    pub fn with_parent(mut self, parent: ContentHash) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Associate the commit with a task, consuming and returning `self`.
    pub fn with_task(mut self, task_id: Uuid) -> Self {
        self.task_id = Some(task_id);
        self
    }

    /// Override the timestamp, consuming and returning `self`.
    ///
    /// Useful for reproducing a commit exactly (its hash depends on the
    /// timestamp) and for deterministic tests.
    pub fn at(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Serialize to the canonical JSON byte form used for storage.
    pub fn to_json(&self) -> Result<Vec<u8>, AgentError> {
        Ok(serde_json::to_vec_pretty(self)?)
    }

    /// Deserialize from the JSON byte form produced by [`AgentCommit::to_json`].
    pub fn from_json(bytes: &[u8]) -> Result<Self, AgentError> {
        Ok(serde_json::from_slice(bytes)?)
    }

    /// The content address this commit will occupy once stored.
    ///
    /// Computed from the serialized bytes without touching the store, so it is
    /// safe to call before (or without) [`AgentCommit::store`].
    pub fn hash(&self) -> Result<ContentHash, AgentError> {
        Ok(ContentHash::of(&self.to_json()?))
    }

    /// Store this commit in `store`, returning its content hash.
    pub fn store(&self, store: &BlobStore) -> Result<ContentHash, AgentError> {
        Ok(store.put(&self.to_json()?)?)
    }

    /// Load a commit from `store` by its hash.
    pub fn load(store: &BlobStore, hash: &ContentHash) -> Result<Self, AgentError> {
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

    fn snapshot_hash(seed: &[u8]) -> ContentHash {
        ContentHash::of(seed)
    }

    #[test]
    fn store_and_load_round_trips() {
        let (_dir, store) = store();
        let commit = AgentCommit::new(
            snapshot_hash(b"tree"),
            AgentContext::new("add retry to token refresh", 0.9),
        );
        let hash = commit.store(&store).unwrap();
        assert_eq!(AgentCommit::load(&store, &hash).unwrap(), commit);
    }

    #[test]
    fn hash_matches_stored_address() {
        let (_dir, store) = store();
        let commit = AgentCommit::new(snapshot_hash(b"tree"), AgentContext::new("x", 0.5));
        // The precomputed hash equals the address the store assigns on write.
        assert_eq!(commit.hash().unwrap(), commit.store(&store).unwrap());
    }

    #[test]
    fn parent_and_task_are_recorded() {
        let parent = snapshot_hash(b"parent-commit");
        let task = Uuid::from_u128(0x1234);
        let commit = AgentCommit::new(snapshot_hash(b"tree"), AgentContext::new("y", 1.0))
            .with_parent(parent)
            .with_task(task);
        assert_eq!(commit.parent, Some(parent));
        assert_eq!(commit.task_id, Some(task));
    }

    #[test]
    fn json_is_flat() {
        // The embedded context's fields appear at the top level, not nested.
        let ts = DateTime::parse_from_rfc3339("2026-07-15T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let commit =
            AgentCommit::new(snapshot_hash(b"tree"), AgentContext::new("flat intent", 0.7)).at(ts);
        let value: serde_json::Value = serde_json::from_slice(&commit.to_json().unwrap()).unwrap();
        assert_eq!(value["intent"], "flat intent");
        assert_eq!(value["confidence"], 0.7);
        // `snapshot` sits at the top level (a ContentHash serializes as its
        // raw byte array, matching arta-core's tree serialization).
        assert!(value["snapshot"].is_array());
        // No nested "context" object.
        assert!(value.get("context").is_none());
    }

    #[test]
    fn timestamp_override_is_deterministic() {
        let ts = DateTime::parse_from_rfc3339("2026-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let a = AgentCommit::new(snapshot_hash(b"t"), AgentContext::new("i", 0.5)).at(ts);
        let b = AgentCommit::new(snapshot_hash(b"t"), AgentContext::new("i", 0.5)).at(ts);
        // Identical content — including the fixed timestamp — hashes identically.
        assert_eq!(a.hash().unwrap(), b.hash().unwrap());
    }
}
