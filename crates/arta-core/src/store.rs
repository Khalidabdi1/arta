//! Content-addressable blob storage.
//!
//! [`BlobStore`] persists raw byte content on disk, addressed by its
//! [`ContentHash`]. Objects are sharded into subdirectories by the first two
//! hex characters of their hash — the same layout git uses for loose objects —
//! to keep directory sizes manageable.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{ArtaError, Result};
use crate::hash::ContentHash;

/// An on-disk, content-addressable blob store.
///
/// Writes are content-addressed and idempotent: storing the same bytes twice
/// yields the same hash and performs no second write. This is the deduplication
/// primitive the rest of arta builds snapshots on top of.
#[derive(Debug, Clone)]
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    /// Open (creating if necessary) a blob store rooted at `root`.
    ///
    /// The directory is created if it does not already exist.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|e| ArtaError::io(&root, e))?;
        Ok(BlobStore { root })
    }

    /// The on-disk path an object with `hash` would occupy.
    fn object_path(&self, hash: &ContentHash) -> PathBuf {
        let hex = hash.to_hex();
        // Shard by the first two hex chars, e.g. `af/1349b9…`.
        self.root.join(&hex[..2]).join(&hex[2..])
    }

    /// Store `bytes`, returning their content hash.
    ///
    /// If an object with the same hash already exists it is left untouched, so
    /// this is safe to call repeatedly with identical content.
    pub fn put(&self, bytes: &[u8]) -> Result<ContentHash> {
        let hash = ContentHash::of(bytes);
        let path = self.object_path(&hash);
        if path.exists() {
            return Ok(hash);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| ArtaError::io(parent, e))?;
        }
        // Write to a temp file then rename, so a reader never observes a
        // partially written object under its final content-addressed name.
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, bytes).map_err(|e| ArtaError::io(&tmp, e))?;
        fs::rename(&tmp, &path).map_err(|e| ArtaError::io(&path, e))?;
        Ok(hash)
    }

    /// Read the bytes of the object identified by `hash`.
    ///
    /// Returns [`ArtaError::NotFound`] if no such object exists.
    pub fn get(&self, hash: &ContentHash) -> Result<Vec<u8>> {
        let path = self.object_path(hash);
        match fs::read(&path) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(ArtaError::NotFound(hash.to_hex()))
            }
            Err(e) => Err(ArtaError::io(&path, e)),
        }
    }

    /// Whether an object with `hash` is present in the store.
    pub fn contains(&self, hash: &ContentHash) -> bool {
        self.object_path(hash).exists()
    }

    /// The root directory of this store.
    pub fn root(&self) -> &Path {
        &self.root
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
    fn put_then_get_round_trips() {
        let (_dir, store) = store();
        let hash = store.put(b"hello arta").unwrap();
        assert_eq!(store.get(&hash).unwrap(), b"hello arta");
    }

    #[test]
    fn put_is_content_addressed_and_idempotent() {
        let (_dir, store) = store();
        let a = store.put(b"same").unwrap();
        let b = store.put(b"same").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, ContentHash::of(b"same"));
    }

    #[test]
    fn contains_reflects_presence() {
        let (_dir, store) = store();
        let hash = store.put(b"present").unwrap();
        assert!(store.contains(&hash));
        assert!(!store.contains(&ContentHash::of(b"absent")));
    }

    #[test]
    fn get_missing_is_not_found() {
        let (_dir, store) = store();
        let err = store.get(&ContentHash::of(b"nope")).unwrap_err();
        assert!(matches!(err, ArtaError::NotFound(_)));
    }
}
