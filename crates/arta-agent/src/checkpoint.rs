//! Named rollback points.
//!
//! A [`Checkpoint`] captures a commit the agent may want to return to later:
//! its snapshot, the commit it was taken at, and a human-readable reason
//! ("before_risky_refactor"). Unlike a rollback query, a checkpoint is an
//! explicit, named marker saved ahead of time.

use arta_core::ContentHash;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A saved point the repository can be rolled back to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// A stable unique identifier for this checkpoint.
    pub id: Uuid,
    /// The tree snapshot captured at the checkpoint.
    pub snapshot: ContentHash,
    /// Why the checkpoint was taken.
    pub reason: String,
    /// The commit the repository was at when the checkpoint was saved.
    pub commit_at: ContentHash,
}

impl Checkpoint {
    /// Create a checkpoint at `commit_at` capturing `snapshot`, with a fresh
    /// v4 id.
    pub fn new(
        snapshot: ContentHash,
        commit_at: ContentHash,
        reason: impl Into<String>,
    ) -> Self {
        Checkpoint {
            id: Uuid::new_v4(),
            snapshot,
            reason: reason.into(),
            commit_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoints_carry_reason_and_target() {
        let snap = ContentHash::of(b"snap");
        let at = ContentHash::of(b"commit");
        let cp = Checkpoint::new(snap, at, "before_risky_refactor");
        assert_eq!(cp.reason, "before_risky_refactor");
        assert_eq!(cp.snapshot, snap);
        assert_eq!(cp.commit_at, at);
    }

    #[test]
    fn each_checkpoint_gets_a_unique_id() {
        let s = ContentHash::of(b"s");
        let a = Checkpoint::new(s, s, "r");
        let b = Checkpoint::new(s, s, "r");
        assert_ne!(a.id, b.id);
    }
}
