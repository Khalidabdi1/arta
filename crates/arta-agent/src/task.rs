//! The task graph.
//!
//! Agents do not work linearly: they open a goal, spawn sub-goals under it,
//! and complete or abandon them. A [`TaskNode`] is one node in that graph. Each
//! node records the commits made under it and points at its parent task, so the
//! full set of nodes forms a DAG of intent that a linear commit history is just
//! one projection of.

use arta_core::ContentHash;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The lifecycle state of a [`TaskNode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Created but not yet started.
    Pending,
    /// Currently being worked on; commits are attributed here.
    Active,
    /// Finished successfully and merged back.
    Complete,
    /// Given up on; its commits are kept for history but not merged.
    Abandoned,
}

/// A single node in the task graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskNode {
    /// A stable unique identifier for this task.
    pub id: Uuid,
    /// A human- and agent-readable description of the goal.
    pub description: String,
    /// The task this one was opened under, if any.
    pub parent_task: Option<Uuid>,
    /// The commits made while this task was active, in order.
    pub commits: Vec<ContentHash>,
    /// The current lifecycle state.
    pub status: TaskStatus,
}

impl TaskNode {
    /// Open a new, [`Active`](TaskStatus::Active) task with the given
    /// description and optional parent. A fresh v4 id is generated.
    pub fn open(description: impl Into<String>, parent_task: Option<Uuid>) -> Self {
        TaskNode {
            id: Uuid::new_v4(),
            description: description.into(),
            parent_task,
            commits: Vec::new(),
            status: TaskStatus::Active,
        }
    }

    /// Record that a commit was made under this task.
    pub fn record_commit(&mut self, commit: ContentHash) {
        self.commits.push(commit);
    }

    /// Whether this task is currently active.
    pub fn is_active(&self) -> bool {
        self.status == TaskStatus::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_starts_active_with_a_unique_id() {
        let a = TaskNode::open("refactor auth", None);
        let b = TaskNode::open("refactor auth", None);
        assert!(a.is_active());
        assert_ne!(a.id, b.id, "each task gets its own id");
        assert!(a.commits.is_empty());
    }

    #[test]
    fn record_commit_appends_in_order() {
        let mut task = TaskNode::open("t", None);
        let c1 = ContentHash::of(b"c1");
        let c2 = ContentHash::of(b"c2");
        task.record_commit(c1);
        task.record_commit(c2);
        assert_eq!(task.commits, vec![c1, c2]);
    }

    #[test]
    fn sub_tasks_point_at_their_parent() {
        let parent = TaskNode::open("parent", None);
        let child = TaskNode::open("child", Some(parent.id));
        assert_eq!(child.parent_task, Some(parent.id));
    }
}
