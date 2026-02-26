//! Storage engine trait.
//!
//! All storage backends implement this trait. The trait defines
//! the complete interface for namespace, branch, schema, and record
//! operations that hellodb needs.

use hellodb_core::{
    Branch, MergeResult, Namespace,
    Record, Schema,
};

use crate::error::StorageError;

/// The core storage engine trait. Implemented by MemoryEngine and SqliteEngine.
/// Thread safety is provided at the application layer via `Mutex<dyn StorageEngine>`.
pub trait StorageEngine {
    // --- Namespace operations ---

    /// Create a new namespace. Automatically creates a "main" branch.
    fn create_namespace(&mut self, namespace: Namespace) -> Result<(), StorageError>;

    /// Get a namespace by ID.
    fn get_namespace(&self, id: &str) -> Result<Option<Namespace>, StorageError>;

    /// List all namespaces.
    fn list_namespaces(&self) -> Result<Vec<Namespace>, StorageError>;

    // --- Schema operations ---

    /// Register a schema in a namespace.
    fn register_schema(&mut self, schema: Schema) -> Result<(), StorageError>;

    /// Get a schema by ID.
    fn get_schema(&self, schema_id: &str) -> Result<Option<Schema>, StorageError>;

    /// List schemas in a namespace.
    fn list_schemas(&self, namespace: &str) -> Result<Vec<Schema>, StorageError>;

    // --- Branch operations ---

    /// Create a new branch off a parent branch.
    fn create_branch(&mut self, branch: Branch) -> Result<(), StorageError>;

    /// Get a branch by ID.
    fn get_branch(&self, branch_id: &str) -> Result<Option<Branch>, StorageError>;

    /// List branches in a namespace.
    fn list_branches(&self, namespace: &str) -> Result<Vec<Branch>, StorageError>;

    /// Merge a branch into its parent. Returns merge result with any conflicts.
    fn merge_branch(&mut self, branch_id: &str) -> Result<MergeResult, StorageError>;

    // --- Record operations ---

    /// Store a record on a specific branch. Deduplicates by record_id.
    fn put_record(&mut self, record: Record, branch: &str) -> Result<(), StorageError>;

    /// Get a record by ID. Searches the specified branch and its ancestors.
    fn get_record(&self, record_id: &str, branch: &str) -> Result<Option<Record>, StorageError>;

    /// Check if a record exists on a branch (or its ancestors).
    fn has_record(&self, record_id: &str, branch: &str) -> Result<bool, StorageError>;

    /// List records by schema on a branch.
    fn list_records_by_schema(
        &self,
        schema: &str,
        branch: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Record>, StorageError>;

    /// List records in a namespace on a branch.
    fn list_records_by_namespace(
        &self,
        namespace: &str,
        branch: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Record>, StorageError>;

    /// Count records by schema on a branch.
    fn count_records_by_schema(&self, schema: &str, branch: &str) -> Result<u64, StorageError>;

    /// Delete a record (tombstone) on a branch.
    fn delete_record(&mut self, record_id: &str, branch: &str) -> Result<(), StorageError>;
}
