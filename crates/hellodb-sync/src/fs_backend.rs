//! Filesystem sync backend — sync to a local directory.
//!
//! Useful for desktop apps, testing against real files, and as a reference
//! implementation for blob storage semantics.

use std::fs;
use std::path::{Path, PathBuf};

use crate::backend::SyncBackend;
use crate::error::SyncError;

/// Filesystem-backed sync storage. Each blob key maps to a file path
/// under the root directory.
pub struct FileSystemSyncBackend {
    root: PathBuf,
}

impl FileSystemSyncBackend {
    /// Create a new backend rooted at the given directory.
    /// Creates the directory if it doesn't exist.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, SyncError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Resolve a blob key to an absolute file path.
    fn blob_path(&self, key: &str) -> PathBuf {
        self.root.join(key)
    }
}

impl SyncBackend for FileSystemSyncBackend {
    fn put_blob(&mut self, key: &str, data: &[u8]) -> Result<(), SyncError> {
        let path = self.blob_path(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, data)?;
        Ok(())
    }

    fn get_blob(&self, key: &str) -> Result<Option<Vec<u8>>, SyncError> {
        let path = self.blob_path(key);
        if path.exists() {
            Ok(Some(fs::read(&path)?))
        } else {
            Ok(None)
        }
    }

    fn list_blobs(&self, prefix: &str) -> Result<Vec<String>, SyncError> {
        let search_dir = self.root.join(prefix);
        // Walk up to find the deepest existing directory
        let walk_dir = if search_dir.is_dir() {
            search_dir.clone()
        } else {
            search_dir
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| self.root.clone())
        };

        if !walk_dir.exists() {
            return Ok(Vec::new());
        }

        let mut keys = Vec::new();
        self.walk_dir(&walk_dir, prefix, &mut keys)?;
        keys.sort();
        Ok(keys)
    }

    fn delete_blob(&mut self, key: &str) -> Result<(), SyncError> {
        let path = self.blob_path(key);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}

impl FileSystemSyncBackend {
    /// Recursively walk a directory collecting blob keys that match the prefix.
    fn walk_dir(
        &self,
        dir: &Path,
        prefix: &str,
        keys: &mut Vec<String>,
    ) -> Result<(), SyncError> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.walk_dir(&path, prefix, keys)?;
            } else if path.is_file() {
                // Convert path back to a key relative to root
                if let Ok(rel) = path.strip_prefix(&self.root) {
                    let key = rel.to_string_lossy().to_string();
                    if key.starts_with(prefix) {
                        keys.push(key);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn put_get_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mut backend = FileSystemSyncBackend::new(tmp.path()).unwrap();
        backend.put_blob("data/test.bin", b"hello fs").unwrap();
        let data = backend.get_blob("data/test.bin").unwrap().unwrap();
        assert_eq!(data, b"hello fs");
    }

    #[test]
    fn get_missing_returns_none() {
        let tmp = TempDir::new().unwrap();
        let backend = FileSystemSyncBackend::new(tmp.path()).unwrap();
        assert!(backend.get_blob("nonexistent").unwrap().is_none());
    }

    #[test]
    fn list_blobs_by_prefix() {
        let tmp = TempDir::new().unwrap();
        let mut backend = FileSystemSyncBackend::new(tmp.path()).unwrap();
        backend.put_blob("ns/deltas/a/1.delta", b"d1").unwrap();
        backend.put_blob("ns/deltas/a/2.delta", b"d2").unwrap();
        backend.put_blob("ns/deltas/b/1.delta", b"d3").unwrap();
        backend.put_blob("other/deltas/1.delta", b"d4").unwrap();

        let ns_deltas = backend.list_blobs("ns/deltas/").unwrap();
        assert_eq!(ns_deltas.len(), 3);

        let device_a = backend.list_blobs("ns/deltas/a/").unwrap();
        assert_eq!(device_a.len(), 2);
    }

    #[test]
    fn delete_blob() {
        let tmp = TempDir::new().unwrap();
        let mut backend = FileSystemSyncBackend::new(tmp.path()).unwrap();
        backend.put_blob("key.bin", b"value").unwrap();
        assert!(backend.get_blob("key.bin").unwrap().is_some());
        backend.delete_blob("key.bin").unwrap();
        assert!(backend.get_blob("key.bin").unwrap().is_none());
    }

    #[test]
    fn delete_missing_is_noop() {
        let tmp = TempDir::new().unwrap();
        let mut backend = FileSystemSyncBackend::new(tmp.path()).unwrap();
        backend.delete_blob("nonexistent").unwrap();
    }

    #[test]
    fn overwrite_existing() {
        let tmp = TempDir::new().unwrap();
        let mut backend = FileSystemSyncBackend::new(tmp.path()).unwrap();
        backend.put_blob("key.bin", b"v1").unwrap();
        backend.put_blob("key.bin", b"v2").unwrap();
        let data = backend.get_blob("key.bin").unwrap().unwrap();
        assert_eq!(data, b"v2");
    }
}
