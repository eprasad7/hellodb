//! In-memory storage backend.
//!
//! Used for testing and ephemeral workspaces.
//! All data lost when the engine is dropped.

use std::collections::HashMap;

use hellodb_core::*;

use crate::engine::{RecordMetadata, StorageEngine, TailEntry};
use crate::error::StorageError;

/// A single entry in the monotonic write log, used to implement tail_records.
#[derive(Debug, Clone)]
struct LogEntry {
    seq: u64,
    branch: String,
    record_id: String,
}

pub struct MemoryEngine {
    namespaces: HashMap<NamespaceId, Namespace>,
    schemas: HashMap<String, Schema>,
    branches: HashMap<BranchId, Branch>,
    /// Records keyed by (branch_id, record_id).
    records: HashMap<(String, String), Record>,
    /// Tombstones: set of (branch_id, record_id) that have been deleted.
    tombstones: HashMap<String, Vec<String>>,
    /// Monotonic per-put_record log — seq grows with each call.
    /// Used by tail_records to let passive observers consume writes.
    write_log: Vec<LogEntry>,
    next_seq: u64,
    /// Per-record reinforcement metadata, keyed by record_id only
    /// (not branch) — reinforcement is content-level, not instance-level.
    record_metadata: HashMap<String, RecordMetadata>,
}

impl MemoryEngine {
    pub fn new() -> Self {
        Self {
            namespaces: HashMap::new(),
            schemas: HashMap::new(),
            branches: HashMap::new(),
            records: HashMap::new(),
            tombstones: HashMap::new(),
            write_log: Vec::new(),
            next_seq: 0,
            record_metadata: HashMap::new(),
        }
    }

    /// Walk up the branch ancestry to find a record.
    fn find_record_in_ancestry(&self, record_id: &str, branch_id: &str) -> Option<Record> {
        let mut current = Some(branch_id.to_string());

        while let Some(bid) = current {
            // Check if tombstoned on this branch
            if let Some(ts) = self.tombstones.get(&bid) {
                if ts.contains(&record_id.to_string()) {
                    return None; // Explicitly deleted on this branch
                }
            }

            // Check if record exists on this branch
            let key = (bid.clone(), record_id.to_string());
            if let Some(rec) = self.records.get(&key) {
                return Some(rec.clone());
            }

            // Walk to parent
            current = self.branches.get(&bid).and_then(|b| b.parent.clone());
        }

        None
    }

    /// Collect all records visible on a branch (including inherited from ancestors).
    fn collect_visible_records(
        &self,
        branch_id: &str,
        filter_schema: Option<&str>,
        filter_namespace: Option<&str>,
    ) -> Vec<Record> {
        let mut seen: HashMap<String, Record> = HashMap::new();
        let mut tombstoned: Vec<String> = Vec::new();
        let mut current = Some(branch_id.to_string());

        while let Some(bid) = current {
            // Collect tombstones from this branch
            if let Some(ts) = self.tombstones.get(&bid) {
                tombstoned.extend(ts.iter().cloned());
            }

            // Collect records from this branch (only if not already seen or tombstoned)
            for ((b, rid), rec) in &self.records {
                if b == &bid && !seen.contains_key(rid) && !tombstoned.contains(rid) {
                    let schema_match = filter_schema.is_none_or(|s| rec.schema == s);
                    let ns_match = filter_namespace.is_none_or(|n| rec.namespace == n);
                    if schema_match && ns_match {
                        seen.insert(rid.clone(), rec.clone());
                    }
                }
            }

            current = self.branches.get(&bid).and_then(|b| b.parent.clone());
        }

        let mut result: Vec<Record> = seen.into_values().collect();
        // newest first — sort by descending created_at_ms via Reverse key
        result.sort_by_key(|r| std::cmp::Reverse(r.created_at_ms));
        result
    }
}

impl Default for MemoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageEngine for MemoryEngine {
    fn create_namespace(&mut self, namespace: Namespace) -> Result<(), StorageError> {
        if self.namespaces.contains_key(&namespace.id) {
            return Err(StorageError::NamespaceExists(namespace.id));
        }
        let main_branch = Branch::main(namespace.id.clone());
        self.branches.insert(main_branch.id.clone(), main_branch);
        self.namespaces.insert(namespace.id.clone(), namespace);
        Ok(())
    }

    fn get_namespace(&self, id: &str) -> Result<Option<Namespace>, StorageError> {
        Ok(self.namespaces.get(id).cloned())
    }

    fn list_namespaces(&self) -> Result<Vec<Namespace>, StorageError> {
        Ok(self.namespaces.values().cloned().collect())
    }

    fn register_schema(&mut self, schema: Schema) -> Result<(), StorageError> {
        // Verify namespace exists
        if !self.namespaces.contains_key(&schema.namespace) {
            return Err(StorageError::NamespaceNotFound(schema.namespace.clone()));
        }
        // Register schema in namespace
        if let Some(ns) = self.namespaces.get_mut(&schema.namespace) {
            ns.register_schema(schema.id.clone());
        }
        self.schemas.insert(schema.id.clone(), schema);
        Ok(())
    }

    fn get_schema(&self, schema_id: &str) -> Result<Option<Schema>, StorageError> {
        Ok(self.schemas.get(schema_id).cloned())
    }

    fn list_schemas(&self, namespace: &str) -> Result<Vec<Schema>, StorageError> {
        Ok(self
            .schemas
            .values()
            .filter(|s| s.namespace == namespace)
            .cloned()
            .collect())
    }

    fn create_branch(&mut self, branch: Branch) -> Result<(), StorageError> {
        // Verify parent exists
        if let Some(ref parent_id) = branch.parent {
            if !self.branches.contains_key(parent_id) {
                return Err(StorageError::BranchNotFound(parent_id.clone()));
            }
        }
        self.branches.insert(branch.id.clone(), branch);
        Ok(())
    }

    fn get_branch(&self, branch_id: &str) -> Result<Option<Branch>, StorageError> {
        Ok(self.branches.get(branch_id).cloned())
    }

    fn list_branches(&self, namespace: &str) -> Result<Vec<Branch>, StorageError> {
        Ok(self
            .branches
            .values()
            .filter(|b| b.namespace == namespace)
            .cloned()
            .collect())
    }

    fn merge_branch(&mut self, branch_id: &str) -> Result<MergeResult, StorageError> {
        let branch = self
            .branches
            .get(branch_id)
            .ok_or_else(|| StorageError::BranchNotFound(branch_id.to_string()))?
            .clone();

        if branch.state != BranchState::Active {
            return Err(StorageError::BranchNotActive(branch_id.to_string()));
        }

        let parent_id = branch
            .parent
            .as_ref()
            .ok_or_else(|| StorageError::BranchNotFound("cannot merge main branch".into()))?
            .clone();

        let parent = self
            .branches
            .get(&parent_id)
            .ok_or_else(|| StorageError::BranchNotFound(parent_id.clone()))?
            .clone();

        let merge_result = branch
            .fast_forward_merge(&parent)
            .map_err(StorageError::Core)?;

        if !merge_result.conflicts.is_empty() {
            return Err(StorageError::MergeConflict(branch_id.to_string()));
        }

        // Apply: move records from source branch to parent
        for record_id in &merge_result.merged_records {
            let key = (branch_id.to_string(), record_id.clone());
            if let Some(rec) = self.records.remove(&key) {
                let parent_key = (parent_id.clone(), record_id.clone());
                self.records.insert(parent_key, rec);
                // Track on parent branch
                if let Some(parent_branch) = self.branches.get_mut(&parent_id) {
                    parent_branch.add_change(record_id.clone());
                }
            }
        }

        // Also move tombstones
        if let Some(ts) = self.tombstones.remove(branch_id) {
            let parent_ts = self.tombstones.entry(parent_id).or_default();
            parent_ts.extend(ts);
        }

        // Mark branch as merged
        if let Some(b) = self.branches.get_mut(branch_id) {
            b.mark_merged();
        }

        Ok(merge_result)
    }

    fn put_record(&mut self, record: Record, branch: &str) -> Result<(), StorageError> {
        // Verify branch exists and is active
        let b = self
            .branches
            .get(branch)
            .ok_or_else(|| StorageError::BranchNotFound(branch.to_string()))?;
        if b.state != BranchState::Active {
            return Err(StorageError::BranchNotActive(branch.to_string()));
        }

        let record_id = record.record_id.clone();
        let key = (branch.to_string(), record_id.clone());
        // Track on branch
        if let Some(b) = self.branches.get_mut(branch) {
            b.add_change(record_id.clone());
        }
        self.records.insert(key, record);

        // Record the write in the monotonic log so tail_records can surface it.
        self.next_seq += 1;
        self.write_log.push(LogEntry {
            seq: self.next_seq,
            branch: branch.to_string(),
            record_id,
        });
        Ok(())
    }

    fn get_record(&self, record_id: &str, branch: &str) -> Result<Option<Record>, StorageError> {
        Ok(self.find_record_in_ancestry(record_id, branch))
    }

    fn has_record(&self, record_id: &str, branch: &str) -> Result<bool, StorageError> {
        Ok(self.find_record_in_ancestry(record_id, branch).is_some())
    }

    fn list_records_by_schema(
        &self,
        schema: &str,
        branch: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Record>, StorageError> {
        let all = self.collect_visible_records(branch, Some(schema), None);
        Ok(all.into_iter().skip(offset).take(limit).collect())
    }

    fn list_records_by_namespace(
        &self,
        namespace: &str,
        branch: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Record>, StorageError> {
        let all = self.collect_visible_records(branch, None, Some(namespace));
        Ok(all.into_iter().skip(offset).take(limit).collect())
    }

    fn count_records_by_schema(&self, schema: &str, branch: &str) -> Result<u64, StorageError> {
        let all = self.collect_visible_records(branch, Some(schema), None);
        Ok(all.len() as u64)
    }

    fn delete_record(&mut self, record_id: &str, branch: &str) -> Result<(), StorageError> {
        // Verify branch exists and is active
        let b = self
            .branches
            .get(branch)
            .ok_or_else(|| StorageError::BranchNotFound(branch.to_string()))?;
        if b.state != BranchState::Active {
            return Err(StorageError::BranchNotActive(branch.to_string()));
        }

        // Remove from this branch's records if present
        self.records
            .remove(&(branch.to_string(), record_id.to_string()));

        // Add tombstone
        self.tombstones
            .entry(branch.to_string())
            .or_default()
            .push(record_id.to_string());

        // Track on branch
        if let Some(b) = self.branches.get_mut(branch) {
            b.add_deletion(record_id.to_string());
        }

        Ok(())
    }

    fn reinforce_record(
        &mut self,
        record_id: &str,
        delta: f32,
        now_ms: u64,
    ) -> Result<RecordMetadata, StorageError> {
        let entry = self
            .record_metadata
            .entry(record_id.to_string())
            .or_insert_with(|| RecordMetadata {
                record_id: record_id.to_string(),
                score: 0.0,
                reinforce_count: 0,
                last_reinforced_at_ms: now_ms,
                first_seen_ms: now_ms,
                archived_at_ms: None,
            });
        entry.score += delta;
        entry.reinforce_count += 1;
        entry.last_reinforced_at_ms = now_ms;
        Ok(entry.clone())
    }

    fn get_record_metadata(&self, record_id: &str) -> Result<Option<RecordMetadata>, StorageError> {
        Ok(self.record_metadata.get(record_id).cloned())
    }

    fn archive_record(&mut self, record_id: &str, now_ms: u64) -> Result<(), StorageError> {
        let entry = self
            .record_metadata
            .entry(record_id.to_string())
            .or_insert_with(|| RecordMetadata {
                record_id: record_id.to_string(),
                score: 0.0,
                reinforce_count: 0,
                last_reinforced_at_ms: 0,
                first_seen_ms: now_ms,
                archived_at_ms: None,
            });
        entry.archived_at_ms = Some(now_ms);
        Ok(())
    }

    fn tail_records(
        &self,
        namespace: &str,
        after_seq: u64,
        limit: usize,
        branch_filter: Option<&str>,
    ) -> Result<Vec<TailEntry>, StorageError> {
        let mut out = Vec::new();
        for entry in &self.write_log {
            if entry.seq <= after_seq {
                continue;
            }
            if let Some(b) = branch_filter {
                if entry.branch != b {
                    continue;
                }
            }
            // Resolve record by (branch, record_id) and filter by namespace.
            let key = (entry.branch.clone(), entry.record_id.clone());
            let Some(record) = self.records.get(&key) else {
                continue; // record no longer present (e.g. tombstoned)
            };
            if record.namespace != namespace {
                continue;
            }
            out.push(TailEntry {
                seq: entry.seq,
                branch: entry.branch.clone(),
                record: record.clone(),
            });
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }
}
