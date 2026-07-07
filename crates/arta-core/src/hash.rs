//! Content addressing for arta objects.
//!
//! Every object in arta is addressed by the BLAKE3 hash of its bytes. This
//! replaces git's SHA1; the compat layer is responsible for translating
//! between [`ContentHash`] and git's SHA1 when talking to remotes.

use serde::{Deserialize, Serialize};

use crate::error::ArtaError;

/// Length in bytes of a BLAKE3 digest.
pub const HASH_LEN: usize = 32;

/// A content-addressable identifier: the BLAKE3 hash of an object's bytes.
///
/// `ContentHash` is a fixed-size, copyable value. It renders as lowercase hex
/// via [`Display`](std::fmt::Display) and round-trips through
/// [`ContentHash::to_hex`] / [`ContentHash::from_hex`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ContentHash([u8; HASH_LEN]);

impl ContentHash {
    /// Hash a byte slice, producing its content address.
    pub fn of(bytes: &[u8]) -> Self {
        ContentHash(*blake3::hash(bytes).as_bytes())
    }

    /// Construct a hash directly from its raw 32 bytes.
    pub fn from_bytes(bytes: [u8; HASH_LEN]) -> Self {
        ContentHash(bytes)
    }

    /// Borrow the raw digest bytes.
    pub fn as_bytes(&self) -> &[u8; HASH_LEN] {
        &self.0
    }

    /// Render the hash as a 64-character lowercase hex string.
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(HASH_LEN * 2);
        for byte in &self.0 {
            // Each byte becomes exactly two lowercase hex digits.
            s.push(nibble(byte >> 4));
            s.push(nibble(byte & 0x0f));
        }
        s
    }

    /// Parse a hash from a 64-character hex string.
    ///
    /// Returns [`ArtaError::InvalidHash`] if the input is not exactly 64 hex
    /// characters.
    pub fn from_hex(hex: &str) -> Result<Self, ArtaError> {
        if hex.len() != HASH_LEN * 2 {
            return Err(ArtaError::InvalidHash(hex.to_string()));
        }
        let mut out = [0u8; HASH_LEN];
        let bytes = hex.as_bytes();
        for (i, slot) in out.iter_mut().enumerate() {
            let hi = from_nibble(bytes[i * 2]).ok_or_else(|| ArtaError::InvalidHash(hex.to_string()))?;
            let lo =
                from_nibble(bytes[i * 2 + 1]).ok_or_else(|| ArtaError::InvalidHash(hex.to_string()))?;
            *slot = (hi << 4) | lo;
        }
        Ok(ContentHash(out))
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

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl std::fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Short prefix keeps debug output readable, mirroring git's habit.
        write!(f, "ContentHash({}…)", &self.to_hex()[..12])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashing_is_deterministic() {
        assert_eq!(ContentHash::of(b"hello"), ContentHash::of(b"hello"));
        assert_ne!(ContentHash::of(b"hello"), ContentHash::of(b"world"));
    }

    #[test]
    fn hex_round_trips() {
        let h = ContentHash::of(b"arta");
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64);
        assert_eq!(ContentHash::from_hex(&hex).unwrap(), h);
    }

    #[test]
    fn display_matches_hex() {
        let h = ContentHash::of(b"snapshot");
        assert_eq!(format!("{h}"), h.to_hex());
    }

    #[test]
    fn from_hex_rejects_bad_input() {
        assert!(ContentHash::from_hex("not-hex").is_err());
        assert!(ContentHash::from_hex("zz").is_err());
        // 64 chars but not all hex.
        let almost = "z".repeat(64);
        assert!(ContentHash::from_hex(&almost).is_err());
    }

    #[test]
    fn known_vector_is_stable() {
        // Guards against an accidental change of hash algorithm or encoding.
        let h = ContentHash::of(b"");
        assert_eq!(
            h.to_hex(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }
}
