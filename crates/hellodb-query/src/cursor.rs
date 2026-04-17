//! Opaque pagination cursors for query results.
//!
//! A cursor encodes a position in a result set as the last record's
//! `record_id` + `created_at_ms`. This enables stable, efficient
//! cursor-based pagination that doesn't skip or duplicate records
//! even as new records are inserted.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use crate::error::QueryError;

/// An opaque pagination cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// The record_id of the last record in the previous page.
    pub record_id: String,
    /// The created_at_ms of the last record (for time-ordered pagination).
    pub created_at_ms: u64,
}

impl Cursor {
    /// Create a cursor from a record's position.
    pub fn new(record_id: String, created_at_ms: u64) -> Self {
        Self {
            record_id,
            created_at_ms,
        }
    }

    /// Encode this cursor as an opaque base64 string for API consumers.
    pub fn encode(&self) -> String {
        let raw = format!("{}:{}", self.record_id, self.created_at_ms);
        URL_SAFE_NO_PAD.encode(raw.as_bytes())
    }

    /// Decode a cursor from an opaque base64 string.
    pub fn decode(encoded: &str) -> Result<Self, QueryError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|e| QueryError::InvalidCursor(format!("base64 decode: {}", e)))?;

        let raw = String::from_utf8(bytes)
            .map_err(|e| QueryError::InvalidCursor(format!("utf8 decode: {}", e)))?;

        let (record_id, ts_str) = raw
            .rsplit_once(':')
            .ok_or_else(|| QueryError::InvalidCursor("missing separator".into()))?;

        let created_at_ms: u64 = ts_str
            .parse()
            .map_err(|e| QueryError::InvalidCursor(format!("timestamp parse: {}", e)))?;

        Ok(Self {
            record_id: record_id.to_string(),
            created_at_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_roundtrip() {
        let cursor = Cursor::new("abc123def456".into(), 1700000000000);
        let encoded = cursor.encode();
        let decoded = Cursor::decode(&encoded).unwrap();
        assert_eq!(decoded.record_id, "abc123def456");
        assert_eq!(decoded.created_at_ms, 1700000000000);
    }

    #[test]
    fn cursor_with_colon_in_record_id() {
        // Record IDs are BLAKE3 hex hashes (no colons), but test robustness
        let cursor = Cursor::new("ns:rec:id".into(), 5000);
        let encoded = cursor.encode();
        let decoded = Cursor::decode(&encoded).unwrap();
        assert_eq!(decoded.record_id, "ns:rec:id");
        assert_eq!(decoded.created_at_ms, 5000);
    }

    #[test]
    fn invalid_cursor_fails() {
        assert!(Cursor::decode("not-valid-base64!!!").is_err());
        // Valid base64 but no separator
        let no_sep = URL_SAFE_NO_PAD.encode(b"noseparator");
        assert!(Cursor::decode(&no_sep).is_err());
    }

    #[test]
    fn cursor_is_opaque_string() {
        let cursor = Cursor::new("test".into(), 1000);
        let encoded = cursor.encode();
        // Should be a base64 string, not human-readable
        assert!(!encoded.contains("test"));
    }
}
