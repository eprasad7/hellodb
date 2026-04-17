//! Storage engine trait.
//!
//! All storage backends implement this trait. The trait defines
//! the complete interface for namespace, branch, schema, and record
//! operations that hellodb needs.

use hellodb_core::{Branch, MergeResult, Namespace, Record, Schema};

use crate::error::StorageError;

/// Mutable per-record metadata used by decay/reinforcement pipelines.
///
/// Records themselves are immutable and content-addressed — you can't mutate
/// them without breaking their hash. Reinforcement signals (confidence score,
/// last-seen timestamp, archival state) live here instead. Keyed by
/// `record_id` only, not (record_id, branch): reinforcement is a statement
/// about the *content*, which is identical across any branch that holds it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordMetadata {
    pub record_id: String,
    /// Accumulated reinforcement signal; higher = more trusted/more relevant.
    pub score: f32,
    /// How many times this record has been reinforced.
    pub reinforce_count: u64,
    /// Last time reinforce was called (ms since epoch).
    pub last_reinforced_at_ms: u64,
    /// When this metadata row was first created (ms since epoch).
    pub first_seen_ms: u64,
    /// If Some, this record is archived — typically hidden from deep recall
    /// but not tombstoned. Use for "aged-out" facts that the digest no longer
    /// considers active but shouldn't forget.
    pub archived_at_ms: Option<u64>,
}

/// Exponential decay over raw reinforcement score.
///
/// Returns `score * 2^(-dt / half_life)`. Pure computation — does not touch
/// storage. Callers compute on demand at recall time so the DB never needs
/// a periodic decay job.
pub fn decayed_score(meta: &RecordMetadata, now_ms: u64, half_life_ms: u64) -> f32 {
    if half_life_ms == 0 || meta.last_reinforced_at_ms >= now_ms {
        return meta.score;
    }
    let dt = (now_ms - meta.last_reinforced_at_ms) as f32;
    let half_life = half_life_ms as f32;
    meta.score * (-std::f32::consts::LN_2 * dt / half_life).exp()
}

/// A record paired with its monotonic insertion cursor, used for tailing.
///
/// `seq` is a monotonically increasing value scoped to this storage engine:
/// later inserts have strictly higher seq values than earlier ones. Subscribers
/// pass the highest `seq` they've observed back as `after_seq` to resume.
///
/// Semantics:
/// - Direct writes (put_record) produce a new seq.
/// - INSERT OR REPLACE (writing a record that already exists on the same
///   branch) produces a new seq — subscribers observe the re-write.
/// - Merges (moving records between branches) do NOT produce a new seq.
///   Subscribers of the destination branch won't see merged records — they
///   already saw them via the source branch.
/// - Tombstones are excluded from tail output.
#[derive(Debug, Clone)]
pub struct TailEntry {
    pub seq: u64,
    pub branch: String,
    pub record: Record,
}

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

    /// Reinforce a record: bump its score by `delta`, increment its count,
    /// and stamp `last_reinforced_at_ms = now_ms`. Idempotent in the sense
    /// that repeated calls compose (they don't collapse to a single call).
    ///
    /// The record does NOT need to exist in storage for metadata to be written
    /// — that allows the digest pipeline to accumulate signal for content
    /// hashes it has seen elsewhere. Creates the metadata row on first call.
    fn reinforce_record(
        &mut self,
        record_id: &str,
        delta: f32,
        now_ms: u64,
    ) -> Result<RecordMetadata, StorageError>;

    /// Fetch a record's current metadata, if any reinforce has ever been
    /// recorded for it. Returns None if the record has never been reinforced.
    fn get_record_metadata(&self, record_id: &str) -> Result<Option<RecordMetadata>, StorageError>;

    /// Archive a record (soft, reversible). Distinct from `delete_record`,
    /// which tombstones on a branch. Archival hides from recall but preserves
    /// the record and its metadata for audit / rehydration.
    fn archive_record(&mut self, record_id: &str, now_ms: u64) -> Result<(), StorageError>;

    /// Tail records in a namespace after a given monotonic cursor.
    ///
    /// Returns up to `limit` entries with `seq > after_seq`, ordered by
    /// ascending seq. Use `after_seq = 0` to start from the beginning.
    /// Tombstones are excluded. If `branch_filter` is Some, only entries on
    /// that exact branch are returned; if None, all branches in the namespace.
    ///
    /// This is the primitive that enables passive, out-of-hot-path observers
    /// (e.g. a memory digest agent) to consume a namespace's write log without
    /// coupling to the primary writer.
    fn tail_records(
        &self,
        namespace: &str,
        after_seq: u64,
        limit: usize,
        branch_filter: Option<&str>,
    ) -> Result<Vec<TailEntry>, StorageError>;
}
