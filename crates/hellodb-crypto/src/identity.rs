//! Ed25519 identity key management.
//!
//! Each device/user has a signing key pair used for record authentication
//! and identity. Keys are derived from secure random bytes and should
//! be stored in hardware-backed keystores on mobile devices.

use ed25519_dalek::{
    Signature as DalekSignature, Signer, SigningKey as DalekSigningKey, Verifier,
    VerifyingKey as DalekVerifyingKey,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use crate::error::CryptoError;

/// Ed25519 signing key (private). Must be stored securely.
#[derive(Clone)]
pub struct SigningKey(DalekSigningKey);

/// Ed25519 verifying key (public). Safe to share.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VerifyingKey(DalekVerifyingKey);

/// Ed25519 signature over a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(DalekSignature);

/// A complete key pair for a hellodb identity.
pub struct KeyPair {
    pub signing: SigningKey,
    pub verifying: VerifyingKey,
}

impl SigningKey {
    /// Generate a new random signing key from OS entropy.
    pub fn generate() -> Self {
        Self(DalekSigningKey::generate(&mut OsRng))
    }

    /// Restore a signing key from raw 32-byte seed.
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(DalekSigningKey::from_bytes(bytes))
    }

    /// Export the raw 32-byte seed. Handle with care.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Get the corresponding verifying (public) key.
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey(self.0.verifying_key())
    }

    /// Sign a message, returning the signature.
    pub fn sign(&self, message: &[u8]) -> Signature {
        Signature(self.0.sign(message))
    }
}

impl VerifyingKey {
    /// Restore from raw 32-byte compressed Edwards point.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        DalekVerifyingKey::from_bytes(bytes)
            .map(Self)
            .map_err(|_| CryptoError::InvalidSignature)
    }

    /// Export as raw 32-byte compressed Edwards point.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Base64url-encoded public key (no padding), used as identity.
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
        Self::from_bytes(&arr)
    }

    /// Verify a signature over a message.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), CryptoError> {
        self.0
            .verify(message, &signature.0)
            .map_err(|_| CryptoError::InvalidSignature)
    }

    /// Short fingerprint (first 8 bytes of BLAKE3 hash, hex-encoded).
    pub fn fingerprint(&self) -> String {
        let hash = blake3::hash(&self.to_bytes());
        hex::encode(&hash.as_bytes()[..8])
    }
}

impl Signature {
    /// Raw 64-byte signature.
    pub fn to_bytes(&self) -> [u8; 64] {
        self.0.to_bytes()
    }

    /// Restore from raw 64-byte signature.
    pub fn from_bytes(bytes: &[u8; 64]) -> Self {
        Self(DalekSignature::from_bytes(bytes))
    }

    /// Base64url-encoded signature (no padding).
    pub fn to_base64(&self) -> String {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.to_bytes())
    }

    /// Decode from base64url (no padding).
    pub fn from_base64(s: &str) -> Result<Self, CryptoError> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s)?;
        if bytes.len() != 64 {
            return Err(CryptoError::InvalidKeyLength {
                expected: 64,
                got: bytes.len(),
            });
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(&arr))
    }
}

impl KeyPair {
    /// Generate a new identity key pair.
    pub fn generate() -> Self {
        let signing = SigningKey::generate();
        let verifying = signing.verifying_key();
        Self { signing, verifying }
    }
}

impl Serialize for VerifyingKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_base64())
    }
}

impl<'de> Deserialize<'de> for VerifyingKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_base64(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Signature {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_base64())
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_base64(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_verify() {
        let kp = KeyPair::generate();
        let msg = b"hello hellodb";
        let sig = kp.signing.sign(msg);
        assert!(kp.verifying.verify(msg, &sig).is_ok());
    }

    #[test]
    fn wrong_message_fails() {
        let kp = KeyPair::generate();
        let sig = kp.signing.sign(b"correct");
        assert!(kp.verifying.verify(b"wrong", &sig).is_err());
    }

    #[test]
    fn wrong_key_fails() {
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();
        let sig = kp1.signing.sign(b"msg");
        assert!(kp2.verifying.verify(b"msg", &sig).is_err());
    }

    #[test]
    fn roundtrip_bytes() {
        let kp = KeyPair::generate();
        let bytes = kp.signing.to_bytes();
        let restored = SigningKey::from_bytes(&bytes);
        assert_eq!(restored.verifying_key().to_bytes(), kp.verifying.to_bytes());
    }

    #[test]
    fn roundtrip_base64() {
        let kp = KeyPair::generate();
        let b64 = kp.verifying.to_base64();
        let restored = VerifyingKey::from_base64(&b64).unwrap();
        assert_eq!(restored, kp.verifying);
    }

    #[test]
    fn signature_base64_roundtrip() {
        let kp = KeyPair::generate();
        let sig = kp.signing.sign(b"test");
        let b64 = sig.to_base64();
        let restored = Signature::from_base64(&b64).unwrap();
        assert_eq!(sig, restored);
    }

    #[test]
    fn serde_roundtrip() {
        let kp = KeyPair::generate();
        let json = serde_json::to_string(&kp.verifying).unwrap();
        let restored: VerifyingKey = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, kp.verifying);
    }

    #[test]
    fn fingerprint_deterministic() {
        let kp = KeyPair::generate();
        assert_eq!(kp.verifying.fingerprint(), kp.verifying.fingerprint());
        assert_eq!(kp.verifying.fingerprint().len(), 16); // 8 bytes hex
    }
}
