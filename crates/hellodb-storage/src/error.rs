use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("record not found: {0}")]
    RecordNotFound(String),

    #[error("namespace not found: {0}")]
    NamespaceNotFound(String),

    #[error("namespace already exists: {0}")]
    NamespaceExists(String),

    #[error("branch not found: {0}")]
    BranchNotFound(String),

    #[error("branch not active: {0}")]
    BranchNotActive(String),

    #[error("schema not found: {0}")]
    SchemaNotFound(String),

    #[error("merge conflict on branch {0}")]
    MergeConflict(String),

    #[error("WAL error: {0}")]
    Wal(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("core error: {0}")]
    Core(#[from] hellodb_core::CoreError),
}
