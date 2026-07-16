//! The task graph.
//!
//! Agents do not work linearly: they spawn sub-tasks, complete them, and merge
//! results. A [`TaskNode`] models one goal in that graph. Nodes reference their
//! parent task (forming a DAG rooted at top-level goals) and accumulate the
//! commits made while the task was active.

use arta_core::ContentHash;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The lifecycle state of a [`TaskNode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Created but not yet started.
    Pending,
    /// Currently being worked on.
    Active,
    /// Finished successfully.
    Complete,
    /// Given up on without completing.
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
    /// The commits made while this task was active, oldest first.
    #[serde(default)]
    pub commits: Vec<ContentHash>,
    /// Current lifecycle state.
    pub status: TaskStatus,
}

impl TaskNode {
    /// Open a new task with the given description and optional parent.
    ///
    /// The task starts [`TaskStatus::Active`] — opening a task means beginning
    /// to work on it.
    pub fn open(description: impl Into<String>, parent_task: Option<Uuid>) -> Self {
        TaskNode {
            id: Uuid::new_v4(),
            description: description.into(),
            parent_task,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_task_is_active_and_empty() {
        let t = TaskNode::open("refactor auth", None);
        assert_eq!(t.status, TaskStatus::Active);
        assert!(t.commits.is_empty());
        assert!(t.is_open());
    }

    #[test]
    fn sub_task_links_to_parent() {
        let parent = TaskNode::open("build feature", None);
        let child = TaskNode::open("write tests", Some(parent.id));
        assert_eq!(child.parent_task, Some(parent.id));
    }

    #[test]
    fn lifecycle_transitions() {
        let mut t = TaskNode::open("x", None);
        t.record_commit(ContentHash::of(b"c1"));
        t.complete();
        assert_eq!(t.status, TaskStatus::Complete);
        assert!(!t.is_open());
        assert_eq!(t.commits.len(), 1);
    }
}
