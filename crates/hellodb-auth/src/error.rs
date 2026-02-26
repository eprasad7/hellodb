use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("access denied: {0}")]
    AccessDenied(String),

    #[error("invalid consent proof: {0}")]
    InvalidConsent(String),

    #[error("consent expired")]
    ConsentExpired,

    #[error("invalid delegation: {0}")]
    InvalidDelegation(String),

    #[error("delegation expired")]
    DelegationExpired,

    #[error("scope denied: {0}")]
    ScopeDenied(String),

    #[error("crypto error: {0}")]
    Crypto(#[from] hellodb_crypto::CryptoError),

    #[error("core error: {0}")]
    Core(#[from] hellodb_core::CoreError),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
