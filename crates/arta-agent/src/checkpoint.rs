//! Named rollback points.
//!
//! A [`Checkpoint`] is a labelled marker an agent drops before doing something
//! risky, so it can return to a known-good state by name rather than by digging
//! a hash out of history. It captures the commit that was current (`commit_at`)
//! and that commit's snapshot, together with a human-readable `reason`.

use arta_core::{BlobStore, ContentHash};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Result;

/// A saved point an agent can roll back to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Stable identifier for this checkpoint.
    pub id: Uuid,
    /// The snapshot that was current when the checkpoint was taken.
    pub snapshot: ContentHash,
    /// Why the checkpoint was created (e.g. `"before_risky_refactor"`).
    pub reason: String,
    /// The commit that `HEAD` pointed at when the checkpoint was taken.
    pub commit_at: ContentHash,
}

impl Checkpoint {
    /// Create a checkpoint marking `commit_at` (whose tree is `snapshot`) with a
    /// human-readable `reason`. A fresh id is generated.
    pub fn new(reason: impl Into<String>, commit_at: ContentHash, snapshot: ContentHash) -> Self {
        Checkpoint {
            id: Uuid::new_v4(),
            snapshot,
            reason: reason.into(),
            commit_at,
        }
    }

    /// Serialize to the canonical JSON byte form used for storage.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self).map_err(arta_core::ArtaError::from)?)
    }

    /// Deserialize from the JSON byte form produced by [`Checkpoint::to_json`].
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes).map_err(arta_core::ArtaError::from)?)
    }

    /// Store this checkpoint in `store`, returning its content hash.
    pub fn store(&self, store: &BlobStore) -> Result<ContentHash> {
        Ok(store.put(&self.to_json()?)?)
    }

    /// Load a checkpoint from `store` by its hash.
    pub fn load(store: &BlobStore, hash: &ContentHash) -> Result<Self> {
        Checkpoint::from_json(&store.get(hash)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(dir.path().join("objects")).unwrap();
        let cp = Checkpoint::new(
            "before_risky_refactor",
            ContentHash::of(b"commit"),
            ContentHash::of(b"tree"),
        );
        let hash = cp.store(&store).unwrap();
        assert_eq!(Checkpoint::load(&store, &hash).unwrap(), cp);
    }

    #[test]
    fn distinct_checkpoints_get_distinct_ids() {
        let a = Checkpoint::new("r", ContentHash::of(b"c"), ContentHash::of(b"t"));
        let b = Checkpoint::new("r", ContentHash::of(b"c"), ContentHash::of(b"t"));
        assert_ne!(a.id, b.id);
    }
}
