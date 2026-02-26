//! SyncBackend trait — the "personal cloud" abstraction.
//!
//! Where Delta Lake ingests into Databricks' cloud, hellodb syncs to YOUR cloud.
//! This trait abstracts the blob storage target: could be a local directory,
//! S3 bucket, GCS bucket, or any key-value blob store the user controls.

use crate::error::SyncError;

/// Abstraction over personal cloud blob storage.
///
/// Keys are path-like strings (e.g. `"commerce/deltas/device-a/1234.delta"`).
/// Values are opaque byte blobs (encrypted delta bundles, manifests, etc.).
pub trait SyncBackend {
    /// Store a blob at the given key. Overwrites if exists.
    fn put_blob(&mut self, key: &str, data: &[u8]) -> Result<(), SyncError>;

    /// Retrieve a blob by key. Returns None if key doesn't exist.
    fn get_blob(&self, key: &str) -> Result<Option<Vec<u8>>, SyncError>;

    /// List all blob keys that start with the given prefix.
    fn list_blobs(&self, prefix: &str) -> Result<Vec<String>, SyncError>;

    /// Delete a blob by key. No-op if key doesn't exist.
    fn delete_blob(&mut self, key: &str) -> Result<(), SyncError>;
}
