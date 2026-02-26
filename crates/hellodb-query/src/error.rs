use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("access denied: {0}")]
    AccessDenied(String),

    #[error("invalid filter: {0}")]
    InvalidFilter(String),

    #[error("invalid cursor: {0}")]
    InvalidCursor(String),

    #[error("namespace not found: {0}")]
    NamespaceNotFound(String),

    #[error("storage error: {0}")]
    Storage(#[from] hellodb_storage::StorageError),

    #[error("auth error: {0}")]
    Auth(#[from] hellodb_auth::AuthError),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
