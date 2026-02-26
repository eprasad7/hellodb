//! In-memory sync backend for testing.

use std::collections::BTreeMap;

use crate::backend::SyncBackend;
use crate::error::SyncError;

/// In-memory backend using a BTreeMap. Ideal for unit tests and
/// integration tests where we want deterministic ordering.
#[derive(Debug, Default)]
pub struct MemorySyncBackend {
    blobs: BTreeMap<String, Vec<u8>>,
}

impl MemorySyncBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many blobs are stored. Useful in assertions.
    pub fn blob_count(&self) -> usize {
        self.blobs.len()
    }
}

impl SyncBackend for MemorySyncBackend {
    fn put_blob(&mut self, key: &str, data: &[u8]) -> Result<(), SyncError> {
        self.blobs.insert(key.to_string(), data.to_vec());
        Ok(())
    }

    fn get_blob(&self, key: &str) -> Result<Option<Vec<u8>>, SyncError> {
        Ok(self.blobs.get(key).cloned())
    }

    fn list_blobs(&self, prefix: &str) -> Result<Vec<String>, SyncError> {
        // BTreeMap range scan — efficient prefix query
        let keys: Vec<String> = self
            .blobs
            .range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(k, _)| k.clone())
            .collect();
        Ok(keys)
    }

    fn delete_blob(&mut self, key: &str) -> Result<(), SyncError> {
        self.blobs.remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_roundtrip() {
        let mut backend = MemorySyncBackend::new();
        backend.put_blob("ns/data/1.bin", b"hello").unwrap();
        let data = backend.get_blob("ns/data/1.bin").unwrap().unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn get_missing_returns_none() {
        let backend = MemorySyncBackend::new();
        assert!(backend.get_blob("nonexistent").unwrap().is_none());
    }

    #[test]
    fn list_blobs_by_prefix() {
        let mut backend = MemorySyncBackend::new();
        backend.put_blob("commerce/deltas/a/1.delta", b"d1").unwrap();
        backend.put_blob("commerce/deltas/a/2.delta", b"d2").unwrap();
        backend.put_blob("commerce/deltas/b/1.delta", b"d3").unwrap();
        backend.put_blob("health/deltas/a/1.delta", b"d4").unwrap();

        let commerce = backend.list_blobs("commerce/deltas/").unwrap();
        assert_eq!(commerce.len(), 3);

        let device_a = backend.list_blobs("commerce/deltas/a/").unwrap();
        assert_eq!(device_a.len(), 2);
    }

    #[test]
    fn delete_blob() {
        let mut backend = MemorySyncBackend::new();
        backend.put_blob("key", b"value").unwrap();
        assert!(backend.get_blob("key").unwrap().is_some());
        backend.delete_blob("key").unwrap();
        assert!(backend.get_blob("key").unwrap().is_none());
    }

    #[test]
    fn delete_missing_is_noop() {
        let mut backend = MemorySyncBackend::new();
        // Should not error
        backend.delete_blob("nonexistent").unwrap();
    }

    #[test]
    fn overwrite_existing() {
        let mut backend = MemorySyncBackend::new();
        backend.put_blob("key", b"v1").unwrap();
        backend.put_blob("key", b"v2").unwrap();
        let data = backend.get_blob("key").unwrap().unwrap();
        assert_eq!(data, b"v2");
        assert_eq!(backend.blob_count(), 1);
    }
}
