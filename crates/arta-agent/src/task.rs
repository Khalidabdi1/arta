//! The task graph.
//!
//! Agents do not work linearly: they open a goal, spawn sub-goals, complete
//! them, and merge the results. A [`TaskNode`] is one goal in that graph. Each
//! node links to its parent task (forming a DAG of intent) and accumulates the
//! commits made while working on it. Linear history is a projection of this
//! graph; the graph itself is the honest record of how the work happened.

use arta_core::{BlobStore, ContentHash};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Result;

/// The lifecycle state of a [`TaskNode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Created but not yet started.
    Pending,
    /// Currently being worked on.
    Active,
    /// Finished successfully.
    Complete,
    /// Given up on; its commits are kept but the goal was dropped.
    Abandoned,
}

/// A single goal in the task graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskNode {
    /// Stable identifier for this task.
    pub id: Uuid,
    /// A natural-language description of the goal.
    pub description: String,
    /// The parent task this one was spawned under, if any.
    pub parent_task: Option<Uuid>,
    /// The commits made while working on this task, in the order recorded.
    pub commits: Vec<ContentHash>,
    /// The task's lifecycle state.
    pub status: TaskStatus,
}

impl TaskNode {
    /// Open a new task with `description` under an optional `parent`.
    ///
    /// The task starts in [`TaskStatus::Active`] — opening a task is the act of
    /// starting to work on it.
    pub fn open(description: impl Into<String>, parent: Option<Uuid>) -> Self {
        TaskNode {
            id: Uuid::new_v4(),
            description: description.into(),
            parent_task: parent,
            commits: Vec::new(),
            status: TaskStatus::Active,
        }
    }

    /// Record a commit made under this task.
    pub fn record_commit(&mut self, commit: ContentHash) {
        self.commits.push(commit);
    }

    /// Mark the task complete.
    pub fn complete(&mut self) {
        self.status = TaskStatus::Complete;
    }

    /// Mark the task abandoned.
    pub fn abandon(&mut self) {
        self.status = TaskStatus::Abandoned;
    }

    /// Whether the task is still open (pending or active).
    pub fn is_open(&self) -> bool {
        matches!(self.status, TaskStatus::Pending | TaskStatus::Active)
    }

    /// Serialize to the canonical JSON byte form used for storage.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self).map_err(arta_core::ArtaError::from)?)
    }

    /// Deserialize from the JSON byte form produced by [`TaskNode::to_json`].
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes).map_err(arta_core::ArtaError::from)?)
    }

    /// Store this task in `store`, returning its content hash.
    ///
    /// Note: because a task mutates as commits are recorded, its content hash
    /// changes over its lifetime; the [`AgentRepo`](crate::AgentRepo) tracks the
    /// live task by its stable [`id`](TaskNode::id), not by hash.
    pub fn store(&self, store: &BlobStore) -> Result<ContentHash> {
        Ok(store.put(&self.to_json()?)?)
    }

    /// Load a task from `store` by its hash.
    pub fn load(store: &BlobStore, hash: &ContentHash) -> Result<Self> {
        TaskNode::from_json(&store.get(hash)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_task_is_active_and_empty() {
        let t = TaskNode::open("refactor auth module", None);
        assert_eq!(t.status, TaskStatus::Active);
        assert!(t.is_open());
        assert!(t.commits.is_empty());
        assert!(t.parent_task.is_none());
    }

    #[test]
    fn commits_accumulate_and_completion_closes() {
        let mut t = TaskNode::open("x", None);
        t.record_commit(ContentHash::of(b"c1"));
        t.record_commit(ContentHash::of(b"c2"));
        assert_eq!(t.commits.len(), 2);
        t.complete();
        assert_eq!(t.status, TaskStatus::Complete);
        assert!(!t.is_open());
    }

    #[test]
    fn parent_links_form_a_hierarchy() {
        let parent = TaskNode::open("parent", None);
        let child = TaskNode::open("child", Some(parent.id));
        assert_eq!(child.parent_task, Some(parent.id));
    }

    #[test]
    fn round_trips_through_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(dir.path().join("objects")).unwrap();
        let mut t = TaskNode::open("persist me", None);
        t.record_commit(ContentHash::of(b"c"));
        let hash = t.store(&store).unwrap();
        assert_eq!(TaskNode::load(&store, &hash).unwrap(), t);
    }
}
