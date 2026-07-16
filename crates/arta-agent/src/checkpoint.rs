//! Named rollback points.
//!
//! A [`Checkpoint`] is a labelled marker an agent can drop before a risky
//! operation ("before_risky_refactor") and later roll back to. It pins both the
//! commit that was current at the time and that commit's snapshot, so a
//! rollback can restore the working tree without re-deriving it.

use arta_core::ContentHash;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A saved rollback point in the agent history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Stable identifier for this checkpoint.
    pub id: Uuid,
    /// Human/agent-supplied reason the checkpoint was taken.
    pub reason: String,
    /// The commit that was current when the checkpoint was taken.
    pub commit_at: ContentHash,
    /// The snapshot of that commit, pinned for direct restore.
    pub snapshot: ContentHash,
    /// When the checkpoint was taken.
    pub timestamp: DateTime<Utc>,
}

impl Checkpoint {
    /// Create a checkpoint pinning `commit_at` and its `snapshot`.
    ///
    /// A fresh v4 [`Uuid`] is generated and the current time is stamped.
    pub fn new(reason: impl Into<String>, commit_at: ContentHash, snapshot: ContentHash) -> Self {
        Checkpoint {
            id: Uuid::new_v4(),
            reason: reason.into(),
            commit_at,
            snapshot,
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoints_get_distinct_ids() {
        let c1 = Checkpoint::new("a", ContentHash::of(b"1"), ContentHash::of(b"s"));
        let c2 = Checkpoint::new("a", ContentHash::of(b"1"), ContentHash::of(b"s"));
        assert_ne!(c1.id, c2.id);
    }

    #[test]
    fn checkpoint_pins_commit_and_snapshot() {
        let commit = ContentHash::of(b"commit");
        let snap = ContentHash::of(b"snap");
        let cp = Checkpoint::new("before refactor", commit, snap);
        assert_eq!(cp.commit_at, commit);
        assert_eq!(cp.snapshot, snap);
        assert_eq!(cp.reason, "before refactor");
    }
}
