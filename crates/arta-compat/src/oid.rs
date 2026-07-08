//! Git object identifiers.
//!
//! Git addresses every object by the SHA1 of its framed bytes (see
//! [`crate::object`]). arta's native addressing is BLAKE3
//! ([`arta_core::ContentHash`]); a [`GitOid`] is the SHA1 identity a git remote
//! expects, produced by this compat layer when arta objects are serialized into
//! the `.git` wire format.

use crate::error::CompatError;

/// Length in bytes of a SHA1 digest.
pub const OID_LEN: usize = 20;

/// A git object identifier: the SHA1 digest of an object's framed bytes.
///
/// `GitOid` is a fixed-size, copyable value. It renders as a 40-character
/// lowercase hex string via [`Display`](std::fmt::Display) and round-trips
/// through [`GitOid::to_hex`] / [`GitOid::from_hex`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GitOid([u8; OID_LEN]);

impl GitOid {
    /// Compute the object id of already-framed git bytes.
    ///
    /// The input must be the full `"<type> <len>\0<body>"` framing, not the raw
    /// body — see [`crate::object::GitObject::to_bytes`].
    pub fn of_framed(framed: &[u8]) -> Self {
        let digest = sha1_smol::Sha1::from(framed).digest();
        GitOid(digest.bytes())
    }

    /// Construct an id directly from its raw 20 bytes.
    pub fn from_bytes(bytes: [u8; OID_LEN]) -> Self {
        GitOid(bytes)
    }

    /// Borrow the raw digest bytes.
    pub fn as_bytes(&self) -> &[u8; OID_LEN] {
        &self.0
    }

    /// Render the id as a 40-character lowercase hex string.
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(OID_LEN * 2);
        for byte in &self.0 {
            s.push(nibble(byte >> 4));
            s.push(nibble(byte & 0x0f));
        }
        s
    }

    /// Parse an id from a 40-character hex string.
    ///
    /// Returns [`CompatError::InvalidOid`] if the input is not exactly 40 hex
    /// characters.
    pub fn from_hex(hex: &str) -> Result<Self, CompatError> {
        if hex.len() != OID_LEN * 2 {
            return Err(CompatError::InvalidOid(hex.to_string()));
        }
        let mut out = [0u8; OID_LEN];
        let bytes = hex.as_bytes();
        for (i, slot) in out.iter_mut().enumerate() {
            let hi = from_nibble(bytes[i * 2]).ok_or_else(|| CompatError::InvalidOid(hex.to_string()))?;
            let lo =
                from_nibble(bytes[i * 2 + 1]).ok_or_else(|| CompatError::InvalidOid(hex.to_string()))?;
            *slot = (hi << 4) | lo;
        }
        Ok(GitOid(out))
    }
}

/// Map a 4-bit value (0..=15) to its lowercase hex character.
fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}

/// Map a hex character to its 4-bit value, or `None` if not a hex digit.
fn from_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

impl std::fmt::Display for GitOid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl std::fmt::Debug for GitOid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Short prefix keeps debug output readable, mirroring git's habit.
        write!(f, "GitOid({}…)", &self.to_hex()[..8])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_blob_matches_git() {
        // `git hash-object -t blob /dev/null` — the canonical empty-blob oid.
        let framed = b"blob 0\0";
        let oid = GitOid::of_framed(framed);
        assert_eq!(oid.to_hex(), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    }

    #[test]
    fn hex_round_trips() {
        let oid = GitOid::of_framed(b"blob 5\0hello");
        let hex = oid.to_hex();
        assert_eq!(hex.len(), 40);
        assert_eq!(GitOid::from_hex(&hex).unwrap(), oid);
    }

    #[test]
    fn from_hex_rejects_bad_input() {
        assert!(GitOid::from_hex("nope").is_err());
        assert!(GitOid::from_hex(&"z".repeat(40)).is_err());
    }

    #[test]
    fn display_matches_hex() {
        let oid = GitOid::of_framed(b"blob 1\0x");
        assert_eq!(format!("{oid}"), oid.to_hex());
    }
}
