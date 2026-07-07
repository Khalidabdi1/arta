//! Directory tree snapshots.
//!
//! A [`TreeSnapshot`] captures the full state of a directory: every file's
//! content is stored as a blob, and the directory structure is recorded as a
//! recursively content-addressed tree. Because both files and trees are
//! addressed by hash, identical subtrees deduplicate automatically and two
//! snapshots of the same working tree produce the same root hash.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{ArtaError, Result};
use crate::hash::ContentHash;
use crate::store::BlobStore;

/// A single entry within a [`TreeSnapshot`]: a named file or subdirectory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeEntry {
    /// The file or directory name (not a full path).
    pub name: String,
    /// The kind of entry and the hash it points at.
    pub target: EntryTarget,
}

/// What a [`TreeEntry`] points to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryTarget {
    /// A file, addressed by the hash of its content blob.
    Blob(ContentHash),
    /// A subdirectory, addressed by the hash of its serialized tree.
    Tree(ContentHash),
}

/// A content-addressed snapshot of a directory's immediate contents.
///
/// Entries are kept sorted by name so that serialization — and therefore the
/// resulting hash — is deterministic regardless of filesystem iteration order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TreeSnapshot {
    /// The entries in this directory, sorted by name.
    pub entries: Vec<TreeEntry>,
}

impl TreeSnapshot {
    /// Recursively snapshot the directory at `path` into `store`.
    ///
    /// Every regular file is written as a blob and every subdirectory as a
    /// nested tree. Returns the snapshot along with the [`ContentHash`] of its
    /// serialized form (the tree's content address).
    pub fn snapshot_dir(store: &BlobStore, path: &Path) -> Result<(TreeSnapshot, ContentHash)> {
        let mut entries = Vec::new();

        let read_dir = fs::read_dir(path).map_err(|e| ArtaError::io(path, e))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| ArtaError::io(path, e))?;
            let file_type = entry.file_type().map_err(|e| ArtaError::io(entry.path(), e))?;
            let name = entry.file_name().to_string_lossy().into_owned();

            // The `.git`/`.arta` metadata directories are not part of the tree.
            if name == ".git" || name == ".arta" {
                continue;
            }

            let target = if file_type.is_dir() {
                let (_sub, hash) = TreeSnapshot::snapshot_dir(store, &entry.path())?;
                EntryTarget::Tree(hash)
            } else {
                let bytes = fs::read(entry.path()).map_err(|e| ArtaError::io(entry.path(), e))?;
                EntryTarget::Blob(store.put(&bytes)?)
            };
            entries.push(TreeEntry { name, target });
        }

        // Sort for a deterministic, order-independent hash.
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let tree = TreeSnapshot { entries };
        let hash = tree.store(store)?;
        Ok((tree, hash))
    }

    /// Serialize this tree and write it into `store`, returning its hash.
    pub fn store(&self, store: &BlobStore) -> Result<ContentHash> {
        let bytes = serde_json::to_vec(self)?;
        store.put(&bytes)
    }

    /// Load and deserialize a tree from `store` by its hash.
    pub fn load(store: &BlobStore, hash: &ContentHash) -> Result<TreeSnapshot> {
        let bytes = store.get(hash)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, BlobStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(dir.path().join("objects")).unwrap();
        (dir, store)
    }

    #[test]
    fn snapshot_captures_files_and_subdirs() {
        let (dir, store) = setup();
        let work = dir.path().join("work");
        fs::create_dir_all(work.join("sub")).unwrap();
        fs::write(work.join("a.txt"), b"alpha").unwrap();
        fs::write(work.join("sub").join("b.txt"), b"beta").unwrap();

        let (tree, hash) = TreeSnapshot::snapshot_dir(&store, &work).unwrap();
        assert_eq!(tree.entries.len(), 2);
        // Reloading by hash reproduces the same tree.
        assert_eq!(TreeSnapshot::load(&store, &hash).unwrap(), tree);
    }

    #[test]
    fn identical_trees_hash_identically() {
        let (dir, store) = setup();
        for name in ["one", "two"] {
            let work = dir.path().join(name);
            fs::create_dir_all(&work).unwrap();
            fs::write(work.join("f.txt"), b"same content").unwrap();
        }
        let (_t1, h1) = TreeSnapshot::snapshot_dir(&store, &dir.path().join("one")).unwrap();
        let (_t2, h2) = TreeSnapshot::snapshot_dir(&store, &dir.path().join("two")).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn changing_a_file_changes_the_root_hash() {
        let (dir, store) = setup();
        let work = dir.path().join("work");
        fs::create_dir_all(&work).unwrap();
        fs::write(work.join("f.txt"), b"before").unwrap();
        let (_t, before) = TreeSnapshot::snapshot_dir(&store, &work).unwrap();

        fs::write(work.join("f.txt"), b"after").unwrap();
        let (_t, after) = TreeSnapshot::snapshot_dir(&store, &work).unwrap();
        assert_ne!(before, after);
    }

    #[test]
    fn git_directory_is_ignored() {
        let (dir, store) = setup();
        let work = dir.path().join("work");
        fs::create_dir_all(work.join(".git")).unwrap();
        fs::write(work.join(".git").join("HEAD"), b"ref: refs/heads/main").unwrap();
        fs::write(work.join("real.txt"), b"tracked").unwrap();

        let (tree, _hash) = TreeSnapshot::snapshot_dir(&store, &work).unwrap();
        assert_eq!(tree.entries.len(), 1);
        assert_eq!(tree.entries[0].name, "real.txt");
    }
}
