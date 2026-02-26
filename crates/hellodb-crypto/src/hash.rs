//! Content-addressable hashing using BLAKE3.
//!
//! Used for record IDs, deduplication, and integrity verification.

/// Compute the BLAKE3 hash of content, returning a hex string.
/// Used as the canonical content-address / record_id.
pub fn content_hash(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// Compute raw BLAKE3 hash bytes.
pub fn content_hash_bytes(data: &[u8]) -> [u8; 32] {
    *blake3::hash(data).as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let a = content_hash(b"hello");
        let b = content_hash(b"hello");
        assert_eq!(a, b);
    }

    #[test]
    fn different_input_different_hash() {
        let a = content_hash(b"hello");
        let b = content_hash(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn hash_length() {
        let h = content_hash(b"test");
        assert_eq!(h.len(), 64); // 32 bytes = 64 hex chars
    }
}
