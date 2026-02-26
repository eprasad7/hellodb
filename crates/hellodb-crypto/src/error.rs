use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid signature")]
    InvalidSignature,

    #[error("invalid key length: expected {expected}, got {got}")]
    InvalidKeyLength { expected: usize, got: usize },

    #[error("decryption failed: ciphertext is corrupted or key is wrong")]
    DecryptionFailed,

    #[error("key derivation failed: {0}")]
    KeyDerivationFailed(String),

    #[error("invalid base64 encoding: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("serialization error: {0}")]
    Serialization(String),
}
