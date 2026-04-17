//! X25519 key exchange and ChaCha20-Poly1305 AEAD encryption.
//!
//! Used for end-to-end encrypted data exchange between devices and
//! for namespace-level encryption of records at rest and in transit.

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::error::CryptoError;

/// X25519 private key for Diffie-Hellman exchange.
pub struct DecryptionKey(StaticSecret);

/// X25519 public key, shared with peers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionKey(PublicKey);

/// Shared secret derived from X25519 exchange.
pub struct SharedSecret([u8; 32]);

/// Encrypted payload with nonce prepended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedBox {
    /// 12-byte nonce (prepended to ciphertext in wire format).
    pub nonce: [u8; 12],
    /// Ciphertext with 16-byte Poly1305 auth tag appended.
    pub ciphertext: Vec<u8>,
}

impl DecryptionKey {
    /// Generate from OS entropy.
    pub fn generate() -> Self {
        let rng = rand::rngs::OsRng;
        Self(StaticSecret::random_from_rng(rng))
    }

    /// Restore from raw 32-byte scalar.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(StaticSecret::from(bytes))
    }

    /// Export raw scalar. Handle with care.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Derive the corresponding public encryption key.
    pub fn encryption_key(&self) -> EncryptionKey {
        EncryptionKey(PublicKey::from(&self.0))
    }

    /// Perform X25519 key exchange with a peer's public key.
    pub fn exchange(&self, peer: &EncryptionKey) -> SharedSecret {
        let raw = self.0.diffie_hellman(&peer.0);
        // HKDF-like derivation via BLAKE3 keyed hash
        let derived = blake3::derive_key("hellodb-e2e-v0", raw.as_bytes());
        SharedSecret(derived)
    }
}

impl EncryptionKey {
    /// From raw 32-byte Montgomery point.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(PublicKey::from(bytes))
    }

    /// Export raw 32-byte Montgomery point.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Base64url-encoded public key (no padding).
    pub fn to_base64(&self) -> String {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.to_bytes())
    }

    /// Decode from base64url (no padding).
    pub fn from_base64(s: &str) -> Result<Self, CryptoError> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s)?;
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength {
                expected: 32,
                got: bytes.len(),
            });
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(arr))
    }
}

impl Serialize for EncryptionKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_base64())
    }
}

impl<'de> Deserialize<'de> for EncryptionKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_base64(&s).map_err(serde::de::Error::custom)
    }
}

/// Encrypt plaintext with a shared secret (ChaCha20-Poly1305).
pub fn seal(shared: &SharedSecret, plaintext: &[u8]) -> SealedBox {
    let cipher =
        ChaCha20Poly1305::new_from_slice(&shared.0).expect("shared secret is always 32 bytes");

    let mut nonce_bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .expect("encryption should not fail with valid key/nonce");

    SealedBox {
        nonce: nonce_bytes,
        ciphertext,
    }
}

/// Decrypt ciphertext with a shared secret.
pub fn open(shared: &SharedSecret, sealed: &SealedBox) -> Result<Vec<u8>, CryptoError> {
    let cipher =
        ChaCha20Poly1305::new_from_slice(&shared.0).expect("shared secret is always 32 bytes");

    let nonce = Nonce::from_slice(&sealed.nonce);

    cipher
        .decrypt(nonce, sealed.ciphertext.as_ref())
        .map_err(|_| CryptoError::DecryptionFailed)
}

/// Encrypt plaintext directly with a 32-byte symmetric key.
/// Used by NamespaceKey for at-rest encryption.
pub fn seal_with_key(key: &[u8; 32], plaintext: &[u8]) -> SealedBox {
    let shared = SharedSecret(*key);
    seal(&shared, plaintext)
}

/// Decrypt ciphertext directly with a 32-byte symmetric key.
pub fn open_with_key(key: &[u8; 32], sealed: &SealedBox) -> Result<Vec<u8>, CryptoError> {
    let shared = SharedSecret(*key);
    open(&shared, sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_exchange_symmetric() {
        let alice = DecryptionKey::generate();
        let bob = DecryptionKey::generate();

        let shared_ab = alice.exchange(&bob.encryption_key());
        let shared_ba = bob.exchange(&alice.encryption_key());

        assert_eq!(shared_ab.0, shared_ba.0);
    }

    #[test]
    fn seal_and_open() {
        let alice = DecryptionKey::generate();
        let bob = DecryptionKey::generate();
        let shared = alice.exchange(&bob.encryption_key());

        let plaintext = b"secret hellodb record";
        let sealed = seal(&shared, plaintext);
        let opened = open(&shared, &sealed).unwrap();

        assert_eq!(opened, plaintext);
    }

    #[test]
    fn wrong_key_fails_open() {
        let alice = DecryptionKey::generate();
        let bob = DecryptionKey::generate();
        let eve = DecryptionKey::generate();

        let shared_ab = alice.exchange(&bob.encryption_key());
        let shared_ae = alice.exchange(&eve.encryption_key());

        let sealed = seal(&shared_ab, b"secret");
        assert!(open(&shared_ae, &sealed).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let alice = DecryptionKey::generate();
        let bob = DecryptionKey::generate();
        let shared = alice.exchange(&bob.encryption_key());

        let mut sealed = seal(&shared, b"secret");
        if let Some(byte) = sealed.ciphertext.first_mut() {
            *byte ^= 0xFF;
        }
        assert!(open(&shared, &sealed).is_err());
    }

    #[test]
    fn encryption_key_base64_roundtrip() {
        let dk = DecryptionKey::generate();
        let ek = dk.encryption_key();
        let b64 = ek.to_base64();
        let restored = EncryptionKey::from_base64(&b64).unwrap();
        assert_eq!(ek, restored);
    }

    #[test]
    fn seal_and_open_with_key() {
        let key = [42u8; 32];
        let plaintext = b"namespace encrypted data";
        let sealed = seal_with_key(&key, plaintext);
        let opened = open_with_key(&key, &sealed).unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn wrong_symmetric_key_fails() {
        let key1 = [42u8; 32];
        let key2 = [99u8; 32];
        let sealed = seal_with_key(&key1, b"secret");
        assert!(open_with_key(&key2, &sealed).is_err());
    }
}
