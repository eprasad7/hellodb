//! Hierarchical key derivation for hellodb.
//!
//! A single MasterKey (backed by Secure Enclave on iOS, Android Keystore on
//! Android) derives per-namespace encryption keys using BLAKE3 key derivation.
//! This ensures each namespace's data is encrypted independently — compromise
//! of one namespace key does not expose others.

use zeroize::Zeroize;

use crate::encryption::{open_with_key, seal_with_key, SealedBox};
use crate::error::CryptoError;

/// Device master key. 32 bytes of high-entropy secret material.
/// On mobile, this should be backed by hardware keystore.
pub struct MasterKey([u8; 32]);

/// Per-namespace symmetric encryption key derived from the master key.
/// Used to encrypt/decrypt all records within a single namespace.
pub struct NamespaceKey {
    /// The derived 32-byte key material.
    key: [u8; 32],
    /// The namespace this key was derived for.
    namespace: String,
}

impl MasterKey {
    /// Generate a new master key from OS entropy.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
        Self(bytes)
    }

    /// Restore from raw 32 bytes (e.g., from Secure Enclave export).
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Export raw bytes. Handle with extreme care.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    /// Derive a NamespaceKey for the given namespace identifier.
    /// Uses BLAKE3 keyed derivation: `blake3::derive_key("hellodb-ns-v0:{namespace}", &self.0)`
    pub fn derive_namespace_key(&self, namespace: &str) -> NamespaceKey {
        let context = format!("hellodb-ns-v0:{}", namespace);
        let derived = blake3::derive_key(&context, &self.0);
        NamespaceKey {
            key: derived,
            namespace: namespace.to_string(),
        }
    }
}

impl NamespaceKey {
    /// The namespace this key belongs to.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Encrypt plaintext using this namespace key (ChaCha20-Poly1305).
    pub fn encrypt(&self, plaintext: &[u8]) -> SealedBox {
        seal_with_key(&self.key, plaintext)
    }

    /// Decrypt ciphertext using this namespace key.
    pub fn decrypt(&self, sealed: &SealedBox) -> Result<Vec<u8>, CryptoError> {
        open_with_key(&self.key, sealed)
    }

    /// Export the raw key material. For internal use only.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.key
    }
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for NamespaceKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_deterministic() {
        let mk = MasterKey::from_bytes([42u8; 32]);
        let nk1 = mk.derive_namespace_key("ainp.commerce");
        let nk2 = mk.derive_namespace_key("ainp.commerce");
        assert_eq!(nk1.to_bytes(), nk2.to_bytes());
    }

    #[test]
    fn different_namespaces_different_keys() {
        let mk = MasterKey::from_bytes([42u8; 32]);
        let nk1 = mk.derive_namespace_key("ainp.commerce");
        let nk2 = mk.derive_namespace_key("health.vitals");
        assert_ne!(nk1.to_bytes(), nk2.to_bytes());
    }

    #[test]
    fn different_master_keys_different_namespace_keys() {
        let mk1 = MasterKey::from_bytes([1u8; 32]);
        let mk2 = MasterKey::from_bytes([2u8; 32]);
        let nk1 = mk1.derive_namespace_key("test");
        let nk2 = mk2.derive_namespace_key("test");
        assert_ne!(nk1.to_bytes(), nk2.to_bytes());
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let mk = MasterKey::generate();
        let nk = mk.derive_namespace_key("ainp.commerce");
        let plaintext = b"listing record data";
        let sealed = nk.encrypt(plaintext);
        let opened = nk.decrypt(&sealed).unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn wrong_namespace_key_fails() {
        let mk = MasterKey::generate();
        let nk1 = mk.derive_namespace_key("ainp.commerce");
        let nk2 = mk.derive_namespace_key("health.vitals");
        let sealed = nk1.encrypt(b"secret commerce data");
        assert!(nk2.decrypt(&sealed).is_err());
    }

    #[test]
    fn master_key_roundtrip_bytes() {
        let mk = MasterKey::generate();
        let bytes = mk.to_bytes();
        let restored = MasterKey::from_bytes(bytes);
        // Same master key should derive same namespace key
        let nk1 = mk.derive_namespace_key("test");
        let nk2 = restored.derive_namespace_key("test");
        assert_eq!(nk1.to_bytes(), nk2.to_bytes());
    }

    #[test]
    fn namespace_key_reports_namespace() {
        let mk = MasterKey::generate();
        let nk = mk.derive_namespace_key("finance.transactions");
        assert_eq!(nk.namespace(), "finance.transactions");
    }
}
