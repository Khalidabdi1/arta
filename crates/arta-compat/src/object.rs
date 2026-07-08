//! The git object model and its wire format.
//!
//! Git stores four object types; this module implements the three arta needs to
//! round-trip a history — [`GitObject::Blob`], [`GitObject::Tree`], and
//! [`GitObject::Commit`]. Each is serialized into git's canonical framing,
//!
//! ```text
//! <type> <length>\0<body>
//! ```
//!
//! and identified by the SHA1 of that framing (a [`GitOid`]). Producing
//! byte-identical framing is what lets a git remote accept objects arta writes:
//! the oids match exactly.

use crate::error::CompatError;
use crate::oid::{GitOid, OID_LEN};

/// The kind of a git object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitObjectKind {
    /// Raw file content.
    Blob,
    /// A directory listing.
    Tree,
    /// A commit.
    Commit,
}

impl GitObjectKind {
    /// The keyword used in the object header (`"blob"`, `"tree"`, `"commit"`).
    pub fn keyword(self) -> &'static str {
        match self {
            GitObjectKind::Blob => "blob",
            GitObjectKind::Tree => "tree",
            GitObjectKind::Commit => "commit",
        }
    }

    /// Parse an object kind from its header keyword.
    fn from_keyword(word: &[u8]) -> Result<Self, CompatError> {
        match word {
            b"blob" => Ok(GitObjectKind::Blob),
            b"tree" => Ok(GitObjectKind::Tree),
            b"commit" => Ok(GitObjectKind::Commit),
            other => Err(CompatError::UnsupportedObject(
                String::from_utf8_lossy(other).into_owned(),
            )),
        }
    }
}

/// The file mode of a [`TreeRecord`], mirroring git's fixed set of modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    /// A subdirectory (`40000`).
    Tree,
    /// A regular, non-executable file (`100644`).
    Regular,
    /// A regular, executable file (`100755`).
    Executable,
    /// A symbolic link (`120000`).
    Symlink,
    /// A gitlink — an embedded submodule commit (`160000`).
    Gitlink,
}

impl FileMode {
    /// The ASCII octal representation git writes into a tree body.
    pub fn as_octal(self) -> &'static str {
        match self {
            FileMode::Tree => "40000",
            FileMode::Regular => "100644",
            FileMode::Executable => "100755",
            FileMode::Symlink => "120000",
            FileMode::Gitlink => "160000",
        }
    }

    /// Parse a mode from its ASCII octal form.
    fn from_octal(bytes: &[u8]) -> Result<Self, CompatError> {
        match bytes {
            b"40000" => Ok(FileMode::Tree),
            b"100644" => Ok(FileMode::Regular),
            b"100755" => Ok(FileMode::Executable),
            b"120000" => Ok(FileMode::Symlink),
            b"160000" => Ok(FileMode::Gitlink),
            other => Err(CompatError::Malformed(format!(
                "unknown tree entry mode {:?}",
                String::from_utf8_lossy(other)
            ))),
        }
    }

    /// Whether this mode denotes a directory (used by git's tree sort order).
    fn is_dir(self) -> bool {
        matches!(self, FileMode::Tree)
    }
}

/// A single entry in a git tree: a name, a mode, and the oid it points at.
///
/// Names are stored as raw bytes because git paths are not required to be
/// valid UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRecord {
    /// The entry's mode.
    pub mode: FileMode,
    /// The entry's name (a single path component, not a full path).
    pub name: Vec<u8>,
    /// The oid of the blob or tree this entry points at.
    pub oid: GitOid,
}

impl TreeRecord {
    /// The entry's name rendered lossily as a string, for display.
    pub fn name_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.name)
    }
}

/// A person and time stamp as recorded in a commit's `author`/`committer`
/// lines: `Name <email> <unix-timestamp> <tz-offset>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// The display name.
    pub name: String,
    /// The email address (written between angle brackets).
    pub email: String,
    /// Seconds since the Unix epoch.
    pub timestamp: i64,
    /// The timezone offset as written, e.g. `"+0000"` or `"-0500"`.
    pub timezone: String,
}

impl Signature {
    /// Render the signature in git's `Name <email> ts tz` form.
    fn to_line(&self) -> String {
        format!(
            "{} <{}> {} {}",
            self.name, self.email, self.timestamp, self.timezone
        )
    }

    /// Parse a signature from the value portion of an `author`/`committer` line.
    fn parse(value: &str) -> Result<Self, CompatError> {
        let open = value
            .rfind('<')
            .ok_or_else(|| CompatError::Malformed(format!("signature missing '<': {value:?}")))?;
        let close = value[open..]
            .find('>')
            .map(|i| open + i)
            .ok_or_else(|| CompatError::Malformed(format!("signature missing '>': {value:?}")))?;

        let name = value[..open].trim_end().to_string();
        let email = value[open + 1..close].to_string();
        // After "> " come the timestamp and timezone, space-separated.
        let rest = value[close + 1..].trim();
        let mut parts = rest.split_whitespace();
        let timestamp = parts
            .next()
            .ok_or_else(|| CompatError::Malformed(format!("signature missing timestamp: {value:?}")))?
            .parse::<i64>()
            .map_err(|_| CompatError::Malformed(format!("bad timestamp in signature: {value:?}")))?;
        let timezone = parts.next().unwrap_or("+0000").to_string();
        Ok(Signature {
            name,
            email,
            timestamp,
            timezone,
        })
    }
}

/// A parsed git commit object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitObject {
    /// The oid of the tree this commit snapshots.
    pub tree: GitOid,
    /// Parent commit oids (empty for a root commit, more than one for a merge).
    pub parents: Vec<GitOid>,
    /// Who wrote the change.
    pub author: Signature,
    /// Who committed it.
    pub committer: Signature,
    /// The commit message, verbatim (including any trailing newline).
    pub message: String,
}

/// A git object: blob, tree, or commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitObject {
    /// Raw file content.
    Blob(Vec<u8>),
    /// A directory listing.
    Tree(Vec<TreeRecord>),
    /// A commit. Boxed because a [`CommitObject`] is markedly larger than the
    /// other variants.
    Commit(Box<CommitObject>),
}

impl GitObject {
    /// The kind of this object.
    pub fn kind(&self) -> GitObjectKind {
        match self {
            GitObject::Blob(_) => GitObjectKind::Blob,
            GitObject::Tree(_) => GitObjectKind::Tree,
            GitObject::Commit(_) => GitObjectKind::Commit,
        }
    }

    /// Serialize just the object body (without the `<type> <len>\0` header).
    pub fn body(&self) -> Vec<u8> {
        match self {
            GitObject::Blob(bytes) => bytes.clone(),
            GitObject::Tree(entries) => encode_tree(entries),
            GitObject::Commit(commit) => encode_commit(commit),
        }
    }

    /// Serialize this object into git's full framed form:
    /// `"<type> <len>\0<body>"`. This is the byte string that is hashed to
    /// produce the object's [`GitOid`] and, once zlib-compressed, stored as a
    /// loose object.
    pub fn to_bytes(&self) -> Vec<u8> {
        let body = self.body();
        let header = format!("{} {}\0", self.kind().keyword(), body.len());
        let mut out = Vec::with_capacity(header.len() + body.len());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// The object id: the SHA1 of this object's framed bytes.
    pub fn oid(&self) -> GitOid {
        GitOid::of_framed(&self.to_bytes())
    }

    /// Parse an object from its full framed bytes (`"<type> <len>\0<body>"`),
    /// the form produced by [`GitObject::to_bytes`] and stored on disk after
    /// zlib decompression.
    pub fn parse(framed: &[u8]) -> Result<Self, CompatError> {
        let nul = framed
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| CompatError::Malformed("object header missing NUL".into()))?;
        let header = &framed[..nul];
        let body = &framed[nul + 1..];

        let space = header
            .iter()
            .position(|&b| b == b' ')
            .ok_or_else(|| CompatError::Malformed("object header missing space".into()))?;
        let kind = GitObjectKind::from_keyword(&header[..space])?;
        let declared_len: usize = std::str::from_utf8(&header[space + 1..])
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| CompatError::Malformed("object header has bad length".into()))?;
        if declared_len != body.len() {
            return Err(CompatError::Malformed(format!(
                "object length mismatch: header says {declared_len}, body is {}",
                body.len()
            )));
        }

        Self::parse_body(kind, body)
    }

    /// Parse an object body given its already-known kind.
    pub fn parse_body(kind: GitObjectKind, body: &[u8]) -> Result<Self, CompatError> {
        match kind {
            GitObjectKind::Blob => Ok(GitObject::Blob(body.to_vec())),
            GitObjectKind::Tree => Ok(GitObject::Tree(decode_tree(body)?)),
            GitObjectKind::Commit => Ok(GitObject::Commit(Box::new(decode_commit(body)?))),
        }
    }
}

/// Encode tree entries into a git tree body, sorted in git's canonical order.
fn encode_tree(entries: &[TreeRecord]) -> Vec<u8> {
    let mut sorted: Vec<&TreeRecord> = entries.iter().collect();
    sorted.sort_by(|a, b| tree_entry_cmp(a, b));

    let mut out = Vec::new();
    for entry in sorted {
        out.extend_from_slice(entry.mode.as_octal().as_bytes());
        out.push(b' ');
        out.extend_from_slice(&entry.name);
        out.push(0);
        out.extend_from_slice(entry.oid.as_bytes());
    }
    out
}

/// Decode a git tree body into its entries.
fn decode_tree(body: &[u8]) -> Result<Vec<TreeRecord>, CompatError> {
    let mut entries = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let space = body[i..]
            .iter()
            .position(|&b| b == b' ')
            .map(|p| i + p)
            .ok_or_else(|| CompatError::Malformed("tree entry missing mode separator".into()))?;
        let mode = FileMode::from_octal(&body[i..space])?;

        let nul = body[space + 1..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| space + 1 + p)
            .ok_or_else(|| CompatError::Malformed("tree entry missing name terminator".into()))?;
        let name = body[space + 1..nul].to_vec();

        let oid_start = nul + 1;
        let oid_end = oid_start + OID_LEN;
        if oid_end > body.len() {
            return Err(CompatError::Malformed("tree entry truncated oid".into()));
        }
        let mut raw = [0u8; OID_LEN];
        raw.copy_from_slice(&body[oid_start..oid_end]);

        entries.push(TreeRecord {
            mode,
            name,
            oid: GitOid::from_bytes(raw),
        });
        i = oid_end;
    }
    Ok(entries)
}

/// Compare two tree entries using git's ordering, in which a directory name
/// sorts as though it had a trailing `/`. Reproducing this exactly is required
/// for tree oids to match git's.
fn tree_entry_cmp(a: &TreeRecord, b: &TreeRecord) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let common = a.name.len().min(b.name.len());
    match a.name[..common].cmp(&b.name[..common]) {
        Ordering::Equal => {}
        other => return other,
    }
    let ca = boundary_byte(&a.name, common, a.mode);
    let cb = boundary_byte(&b.name, common, b.mode);
    ca.cmp(&cb)
}

/// The byte git uses at the comparison boundary: the next name byte if present,
/// otherwise `/` for a directory and `0` for anything else.
fn boundary_byte(name: &[u8], at: usize, mode: FileMode) -> u8 {
    if at < name.len() {
        name[at]
    } else if mode.is_dir() {
        b'/'
    } else {
        0
    }
}

/// Encode a commit into its git object body.
fn encode_commit(commit: &CommitObject) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(&format!("tree {}\n", commit.tree));
    for parent in &commit.parents {
        s.push_str(&format!("parent {parent}\n"));
    }
    s.push_str(&format!("author {}\n", commit.author.to_line()));
    s.push_str(&format!("committer {}\n", commit.committer.to_line()));
    s.push('\n');
    s.push_str(&commit.message);
    s.into_bytes()
}

/// Decode a commit object body.
fn decode_commit(body: &[u8]) -> Result<CommitObject, CompatError> {
    let text = std::str::from_utf8(body)
        .map_err(|_| CompatError::Malformed("commit body is not valid UTF-8".into()))?;

    // Headers are separated from the message by the first blank line.
    let (header_block, message) = match text.find("\n\n") {
        Some(i) => (&text[..i], text[i + 2..].to_string()),
        None => (text, String::new()),
    };

    let mut tree = None;
    let mut parents = Vec::new();
    let mut author = None;
    let mut committer = None;

    for line in header_block.lines() {
        if let Some(rest) = line.strip_prefix("tree ") {
            tree = Some(GitOid::from_hex(rest)?);
        } else if let Some(rest) = line.strip_prefix("parent ") {
            parents.push(GitOid::from_hex(rest)?);
        } else if let Some(rest) = line.strip_prefix("author ") {
            author = Some(Signature::parse(rest)?);
        } else if let Some(rest) = line.strip_prefix("committer ") {
            committer = Some(Signature::parse(rest)?);
        }
        // Unknown headers (e.g. `encoding`, `gpgsig`) are ignored for now.
    }

    Ok(CommitObject {
        tree: tree.ok_or_else(|| CompatError::Malformed("commit missing tree header".into()))?,
        parents,
        author: author.ok_or_else(|| CompatError::Malformed("commit missing author".into()))?,
        committer: committer
            .ok_or_else(|| CompatError::Malformed("commit missing committer".into()))?,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_oid_matches_git() {
        // `printf 'hello' | git hash-object --stdin`
        let obj = GitObject::Blob(b"hello".to_vec());
        assert_eq!(obj.oid().to_hex(), "b6fc4c620b67d95f953a5c1c1230aaab5db5a1b0");
    }

    #[test]
    fn blob_round_trips_through_framing() {
        let obj = GitObject::Blob(b"content bytes".to_vec());
        let framed = obj.to_bytes();
        assert_eq!(GitObject::parse(&framed).unwrap(), obj);
    }

    #[test]
    fn tree_oid_matches_git() {
        // A tree with a single `hello` blob named `greeting.txt`, mode 100644.
        // Verified against `git mktree`.
        let blob = GitObject::Blob(b"hello".to_vec()).oid();
        let tree = GitObject::Tree(vec![TreeRecord {
            mode: FileMode::Regular,
            name: b"greeting.txt".to_vec(),
            oid: blob,
        }]);
        assert_eq!(tree.oid().to_hex(), "81eb4db555d88438a38ad694ec3e445734119867");
    }

    #[test]
    fn empty_tree_oid_matches_git() {
        // git's well-known empty-tree oid.
        assert_eq!(
            GitObject::Tree(vec![]).oid().to_hex(),
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
        );
    }

    #[test]
    fn tree_entries_are_sorted_canonically() {
        let oid = GitObject::Blob(b"x".to_vec()).oid();
        // Deliberately out of order, including a directory whose sort position
        // depends on the trailing-slash rule.
        let unsorted = vec![
            TreeRecord { mode: FileMode::Regular, name: b"foo.txt".to_vec(), oid },
            TreeRecord { mode: FileMode::Tree, name: b"foo".to_vec(), oid },
            TreeRecord { mode: FileMode::Regular, name: b"bar".to_vec(), oid },
        ];
        let tree = GitObject::Tree(unsorted);
        // Re-parsing the encoded body yields git's canonical order.
        let reparsed = GitObject::parse(&tree.to_bytes()).unwrap();
        let names: Vec<Vec<u8>> = match reparsed {
            GitObject::Tree(e) => e.into_iter().map(|r| r.name).collect(),
            _ => unreachable!(),
        };
        // "bar" < "foo" (file) < "foo/" (dir): the file `foo.txt` compares its
        // 4th byte '.' (0x2e) against the directory's boundary '/' (0x2f).
        assert_eq!(names, vec![b"bar".to_vec(), b"foo.txt".to_vec(), b"foo".to_vec()]);
    }

    #[test]
    fn commit_round_trips() {
        let tree = GitObject::Tree(vec![]).oid();
        let sig = Signature {
            name: "Ada Lovelace".into(),
            email: "ada@example.com".into(),
            timestamp: 1_700_000_000,
            timezone: "+0000".into(),
        };
        let commit = GitObject::Commit(Box::new(CommitObject {
            tree,
            parents: vec![],
            author: sig.clone(),
            committer: sig,
            message: "initial commit\n".into(),
        }));
        let framed = commit.to_bytes();
        assert_eq!(GitObject::parse(&framed).unwrap(), commit);
    }

    #[test]
    fn commit_with_parents_round_trips() {
        let tree = GitObject::Tree(vec![]).oid();
        let parent = GitObject::Blob(b"p".to_vec()).oid();
        let sig = Signature {
            name: "A".into(),
            email: "a@b.c".into(),
            timestamp: 42,
            timezone: "-0500".into(),
        };
        let commit = GitObject::Commit(Box::new(CommitObject {
            tree,
            parents: vec![parent, GitObject::Blob(b"q".to_vec()).oid()],
            author: sig.clone(),
            committer: sig,
            message: "merge\n\nbody text\n".into(),
        }));
        assert_eq!(GitObject::parse(&commit.to_bytes()).unwrap(), commit);
    }

    #[test]
    fn signature_parses_name_with_spaces() {
        let sig = Signature::parse("Grace M. Hopper <grace@navy.mil> 100 +0000").unwrap();
        assert_eq!(sig.name, "Grace M. Hopper");
        assert_eq!(sig.email, "grace@navy.mil");
        assert_eq!(sig.timestamp, 100);
        assert_eq!(sig.timezone, "+0000");
    }

    #[test]
    fn parse_rejects_length_mismatch() {
        // Header claims 99 bytes but body is 5.
        let err = GitObject::parse(b"blob 99\0hello").unwrap_err();
        assert!(matches!(err, CompatError::Malformed(_)));
    }
}
