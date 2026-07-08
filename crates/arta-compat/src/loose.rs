//! The git loose object store.
//!
//! Git keeps individual objects as zlib-compressed files under
//! `.git/objects/aa/bbbb…`, where `aa` is the first two hex characters of the
//! object's oid and `bbbb…` is the remaining thirty-eight. [`LooseObjectStore`]
//! reads and writes objects in exactly that layout, so the files it produces
//! are indistinguishable from git's own — the whole point of the compat layer.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::error::CompatError;
use crate::object::GitObject;
use crate::oid::GitOid;

/// A read/write store over git's loose object directory (`.git/objects`).
#[derive(Debug, Clone)]
pub struct LooseObjectStore {
    /// The `objects` directory (e.g. `<repo>/.git/objects`).
    objects_dir: PathBuf,
}

impl LooseObjectStore {
    /// Open a loose object store at `objects_dir` (typically
    /// `<repo>/.git/objects`), creating the directory if necessary.
    pub fn open(objects_dir: impl Into<PathBuf>) -> Result<Self, CompatError> {
        let objects_dir = objects_dir.into();
        fs::create_dir_all(&objects_dir).map_err(|e| CompatError::io(&objects_dir, e))?;
        Ok(LooseObjectStore { objects_dir })
    }

    /// The on-disk path a loose object with `oid` occupies.
    fn object_path(&self, oid: &GitOid) -> PathBuf {
        let hex = oid.to_hex();
        self.objects_dir.join(&hex[..2]).join(&hex[2..])
    }

    /// Whether a loose object with `oid` is present.
    pub fn contains(&self, oid: &GitOid) -> bool {
        self.object_path(oid).exists()
    }

    /// Write `object` into the store, returning its oid.
    ///
    /// The object is framed, zlib-compressed, and written under its
    /// content-addressed path. Because the path is derived from the oid, this
    /// is idempotent: an object already present is left untouched.
    pub fn write(&self, object: &GitObject) -> Result<GitOid, CompatError> {
        let framed = object.to_bytes();
        let oid = GitOid::of_framed(&framed);
        let path = self.object_path(&oid);
        if path.exists() {
            return Ok(oid);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| CompatError::io(parent, e))?;
        }

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&framed)
            .map_err(|e| CompatError::io(&path, e))?;
        let compressed = encoder.finish().map_err(|e| CompatError::io(&path, e))?;

        // Write to a temp file then rename, so a reader never sees a partially
        // written object under its final content-addressed name.
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, &compressed).map_err(|e| CompatError::io(&tmp, e))?;
        fs::rename(&tmp, &path).map_err(|e| CompatError::io(&path, e))?;
        Ok(oid)
    }

    /// Read and parse the loose object identified by `oid`.
    ///
    /// Returns [`CompatError::NotFound`] if no such loose object exists, and
    /// verifies that the decompressed bytes actually hash to `oid`.
    pub fn read(&self, oid: &GitOid) -> Result<GitObject, CompatError> {
        let path = self.object_path(oid);
        let compressed = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(CompatError::NotFound(oid.to_hex()))
            }
            Err(e) => return Err(CompatError::io(&path, e)),
        };

        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut framed = Vec::new();
        decoder
            .read_to_end(&mut framed)
            .map_err(|e| CompatError::io(&path, e))?;

        let actual = GitOid::of_framed(&framed);
        if actual != *oid {
            return Err(CompatError::Corrupt {
                expected: oid.to_hex(),
                actual: actual.to_hex(),
            });
        }
        GitObject::parse(&framed)
    }

    /// The `objects` directory this store operates on.
    pub fn objects_dir(&self) -> &Path {
        &self.objects_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{CommitObject, FileMode, Signature, TreeRecord};

    fn store() -> (tempfile::TempDir, LooseObjectStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = LooseObjectStore::open(dir.path().join("objects")).unwrap();
        (dir, store)
    }

    #[test]
    fn blob_write_then_read_round_trips() {
        let (_dir, store) = store();
        let obj = GitObject::Blob(b"loose content".to_vec());
        let oid = store.write(&obj).unwrap();
        assert_eq!(store.read(&oid).unwrap(), obj);
    }

    #[test]
    fn write_uses_gits_sharded_layout() {
        let (_dir, store) = store();
        let obj = GitObject::Blob(b"hello".to_vec());
        let oid = store.write(&obj).unwrap();
        let hex = oid.to_hex();
        let expected = store.objects_dir().join(&hex[..2]).join(&hex[2..]);
        assert!(expected.exists(), "object not at git's sharded path");
    }

    #[test]
    fn write_is_idempotent() {
        let (_dir, store) = store();
        let obj = GitObject::Blob(b"same".to_vec());
        let a = store.write(&obj).unwrap();
        let b = store.write(&obj).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn read_missing_is_not_found() {
        let (_dir, store) = store();
        let err = store.read(&GitObject::Blob(b"absent".to_vec()).oid()).unwrap_err();
        assert!(matches!(err, CompatError::NotFound(_)));
    }

    #[test]
    fn tree_and_commit_round_trip_through_disk() {
        let (_dir, store) = store();
        let blob_oid = store.write(&GitObject::Blob(b"data".to_vec())).unwrap();
        let tree = GitObject::Tree(vec![TreeRecord {
            mode: FileMode::Regular,
            name: b"file.txt".to_vec(),
            oid: blob_oid,
        }]);
        let tree_oid = store.write(&tree).unwrap();
        assert_eq!(store.read(&tree_oid).unwrap(), tree);

        let sig = Signature {
            name: "Tester".into(),
            email: "t@example.com".into(),
            timestamp: 1_600_000_000,
            timezone: "+0000".into(),
        };
        let commit = GitObject::Commit(Box::new(CommitObject {
            tree: tree_oid,
            parents: vec![],
            author: sig.clone(),
            committer: sig,
            message: "add file\n".into(),
        }));
        let commit_oid = store.write(&commit).unwrap();
        assert_eq!(store.read(&commit_oid).unwrap(), commit);
    }
}
