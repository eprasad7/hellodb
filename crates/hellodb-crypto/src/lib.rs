//! hellodb Cryptographic Primitives
//!
//! Provides Ed25519 signing, X25519 key exchange, ChaCha20-Poly1305 AEAD
//! encryption, BLAKE3 content hashing, and hierarchical key derivation
//! for the hellodb sovereign data layer.

pub mod identity;
pub mod encryption;
pub mod hash;
pub mod keychain;
pub mod error;

pub use identity::{SigningKey, VerifyingKey, Signature, KeyPair};
pub use encryption::{EncryptionKey, DecryptionKey, SharedSecret, SealedBox, seal, open};
pub use hash::{content_hash, content_hash_bytes};
pub use keychain::{MasterKey, NamespaceKey};
pub use error::CryptoError;
