//! Delta bundles — the unit of sync.
//!
//! A DeltaBundle captures all record changes since the last sync.
//! It gets encrypted with the NamespaceKey and uploaded to the backend
//! as a SealedDelta — the "inverse Delta Lake" unit of ingestion.

use hellodb_core::Record;
use hellodb_crypto::{NamespaceKey, SealedBox};
use serde::{Deserialize, Serialize};

use crate::error::SyncError;

/// A batch of record changes to sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaBundle {
    /// Device that produced this delta.
    pub device_id: String,
    /// Namespace this delta belongs to.
    pub namespace: String,
    /// Branch this delta applies to.
    pub branch: String,
    /// Records with created_at_ms > from_cursor were included.
    pub from_cursor: u64,
    /// Records with created_at_ms <= to_cursor were included.
    pub to_cursor: u64,
    /// Changed/new records in this delta.
    pub records: Vec<Record>,
    /// Record IDs that were deleted (tombstones).
    pub tombstones: Vec<String>,
    /// When this delta was created.
    pub created_at_ms: u64,
}

/// Unencrypted metadata about a sealed delta. Stored alongside the
/// encrypted payload so we can filter/list deltas without decrypting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaMetadata {
    pub device_id: String,
    pub namespace: String,
    pub branch: String,
    pub from_cursor: u64,
    pub to_cursor: u64,
    pub record_count: usize,
    pub tombstone_count: usize,
    pub created_at_ms: u64,
}

/// An encrypted delta: metadata (cleartext) + encrypted payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedDelta {
    /// Cleartext metadata for listing/filtering without decryption.
    pub metadata: DeltaMetadata,
    /// The encrypted DeltaBundle (ChaCha20-Poly1305 via NamespaceKey).
    pub sealed: SealedBox,
}

impl DeltaBundle {
    /// Extract metadata from this bundle.
    pub fn metadata(&self) -> DeltaMetadata {
        DeltaMetadata {
            device_id: self.device_id.clone(),
            namespace: self.namespace.clone(),
            branch: self.branch.clone(),
            from_cursor: self.from_cursor,
            to_cursor: self.to_cursor,
            record_count: self.records.len(),
            tombstone_count: self.tombstones.len(),
            created_at_ms: self.created_at_ms,
        }
    }
}

/// Encrypt a DeltaBundle with the namespace key.
pub fn seal_delta(bundle: &DeltaBundle, ns_key: &NamespaceKey) -> Result<SealedDelta, SyncError> {
    let metadata = bundle.metadata();
    let plaintext = serde_json::to_vec(bundle)?;
    let sealed = ns_key.encrypt(&plaintext);
    Ok(SealedDelta { metadata, sealed })
}

/// Decrypt a SealedDelta back into a DeltaBundle.
pub fn open_delta(sealed: &SealedDelta, ns_key: &NamespaceKey) -> Result<DeltaBundle, SyncError> {
    let plaintext = ns_key
        .decrypt(&sealed.sealed)
        .map_err(|e| SyncError::Decryption(e.to_string()))?;
    let bundle: DeltaBundle = serde_json::from_slice(&plaintext)?;
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hellodb_crypto::{KeyPair, MasterKey};
    use serde_json::json;

    fn make_test_bundle() -> (DeltaBundle, NamespaceKey) {
        let kp = KeyPair::generate();
        let mk = MasterKey::generate();
        let ns_key = mk.derive_namespace_key("commerce");

        let rec = Record::new_with_timestamp(
            &kp.signing,
            "commerce.listing".into(),
            "commerce".into(),
            json!({"title": "Bowl", "price": 24.99}),
            None,
            5000,
        )
        .unwrap();

        let bundle = DeltaBundle {
            device_id: "device-a".into(),
            namespace: "commerce".into(),
            branch: "commerce/main".into(),
            from_cursor: 0,
            to_cursor: 5000,
            records: vec![rec],
            tombstones: vec!["deleted-id-1".into()],
            created_at_ms: 6000,
        };

        (bundle, ns_key)
    }

    #[test]
    fn seal_open_roundtrip() {
        let (bundle, ns_key) = make_test_bundle();
        let sealed = seal_delta(&bundle, &ns_key).unwrap();
        let opened = open_delta(&sealed, &ns_key).unwrap();
        assert_eq!(opened.device_id, "device-a");
        assert_eq!(opened.records.len(), 1);
        assert_eq!(opened.tombstones, vec!["deleted-id-1"]);
        assert_eq!(opened.from_cursor, 0);
        assert_eq!(opened.to_cursor, 5000);
    }

    #[test]
    fn wrong_key_fails() {
        let (bundle, ns_key) = make_test_bundle();
        let sealed = seal_delta(&bundle, &ns_key).unwrap();

        let mk2 = MasterKey::generate();
        let wrong_key = mk2.derive_namespace_key("commerce");
        assert!(open_delta(&sealed, &wrong_key).is_err());
    }

    #[test]
    fn metadata_is_accurate() {
        let (bundle, _) = make_test_bundle();
        let meta = bundle.metadata();
        assert_eq!(meta.device_id, "device-a");
        assert_eq!(meta.namespace, "commerce");
        assert_eq!(meta.record_count, 1);
        assert_eq!(meta.tombstone_count, 1);
        assert_eq!(meta.from_cursor, 0);
        assert_eq!(meta.to_cursor, 5000);
    }

    #[test]
    fn sealed_metadata_cleartext() {
        let (bundle, ns_key) = make_test_bundle();
        let sealed = seal_delta(&bundle, &ns_key).unwrap();
        // Metadata is readable without decryption
        assert_eq!(sealed.metadata.device_id, "device-a");
        assert_eq!(sealed.metadata.record_count, 1);
    }

    #[test]
    fn empty_delta_roundtrip() {
        let mk = MasterKey::generate();
        let ns_key = mk.derive_namespace_key("empty");
        let bundle = DeltaBundle {
            device_id: "d1".into(),
            namespace: "empty".into(),
            branch: "empty/main".into(),
            from_cursor: 100,
            to_cursor: 100,
            records: vec![],
            tombstones: vec![],
            created_at_ms: 200,
        };

        let sealed = seal_delta(&bundle, &ns_key).unwrap();
        let opened = open_delta(&sealed, &ns_key).unwrap();
        assert!(opened.records.is_empty());
        assert!(opened.tombstones.is_empty());
    }
}
