//! SQLite/SQLCipher storage backend.
//!
//! Encrypted at-rest storage using the same rusqlite + bundled-sqlcipher
//! pattern as AINP's ainp-store crate.

use rusqlite::{params, Connection};
use serde_json;
use std::collections::HashMap;

use hellodb_core::*;

use crate::engine::StorageEngine;
use crate::error::StorageError;

pub struct SqliteEngine {
    conn: Connection,
}

impl SqliteEngine {
    /// Open (or create) an encrypted database.
    pub fn open(path: &str, encryption_key: &str) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "key", encryption_key)?;
        let engine = Self { conn };
        engine.initialize_schema()?;
        Ok(engine)
    }

    /// Open in-memory (for testing without file I/O).
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let engine = Self { conn };
        engine.initialize_schema()?;
        Ok(engine)
    }

    fn initialize_schema(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS namespaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                owner_pubkey_b64 TEXT NOT NULL,
                description TEXT,
                encrypted INTEGER NOT NULL DEFAULT 1,
                created_at_ms INTEGER NOT NULL,
                schemas_json TEXT NOT NULL DEFAULT '[]'
            );

            CREATE TABLE IF NOT EXISTS schemas (
                id TEXT PRIMARY KEY,
                version TEXT NOT NULL,
                namespace TEXT NOT NULL,
                name TEXT NOT NULL,
                fields_json TEXT NOT NULL,
                registered_at_ms INTEGER NOT NULL,
                FOREIGN KEY (namespace) REFERENCES namespaces(id)
            );

            CREATE TABLE IF NOT EXISTS branches (
                id TEXT PRIMARY KEY,
                namespace TEXT NOT NULL,
                parent TEXT,
                state TEXT NOT NULL DEFAULT 'active',
                created_at_ms INTEGER NOT NULL,
                label TEXT NOT NULL,
                changes_json TEXT NOT NULL DEFAULT '{}',
                FOREIGN KEY (namespace) REFERENCES namespaces(id)
            );

            CREATE TABLE IF NOT EXISTS records (
                record_id TEXT NOT NULL,
                branch TEXT NOT NULL,
                schema_id TEXT NOT NULL,
                namespace TEXT NOT NULL,
                created_by_b64 TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                data_json TEXT NOT NULL,
                previous_version TEXT,
                signature_b64 TEXT NOT NULL,
                is_tombstone INTEGER NOT NULL DEFAULT 0,
                stored_at_ms INTEGER NOT NULL,
                PRIMARY KEY (record_id, branch)
            );

            CREATE INDEX IF NOT EXISTS idx_records_schema ON records(schema_id, branch);
            CREATE INDEX IF NOT EXISTS idx_records_namespace ON records(namespace, branch);
            CREATE INDEX IF NOT EXISTS idx_records_created ON records(created_at_ms);

            CREATE TABLE IF NOT EXISTS record_metadata (
                record_id TEXT PRIMARY KEY,
                score REAL NOT NULL DEFAULT 0.0,
                reinforce_count INTEGER NOT NULL DEFAULT 0,
                last_reinforced_at_ms INTEGER NOT NULL DEFAULT 0,
                first_seen_ms INTEGER NOT NULL,
                archived_at_ms INTEGER
            );
            ",
        )?;
        Ok(())
    }

    /// Helper: serialize a Record to JSON for insertion.
    fn record_to_row(record: &Record, branch: &str) -> Result<RecordRow, StorageError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Ok(RecordRow {
            record_id: record.record_id.clone(),
            branch: branch.to_string(),
            schema_id: record.schema.clone(),
            namespace: record.namespace.clone(),
            created_by_b64: serde_json::to_string(&record.created_by)?,
            created_at_ms: record.created_at_ms,
            data_json: serde_json::to_string(&record.data)?,
            previous_version: record.previous_version.clone(),
            signature_b64: serde_json::to_string(&record.sig)?,
            is_tombstone: false,
            stored_at_ms: now_ms,
        })
    }

    /// Helper: reconstruct a Record from a database row.
    fn row_to_record(row: &rusqlite::Row) -> Result<Record, rusqlite::Error> {
        let created_by_b64: String = row.get(4)?;
        let data_json: String = row.get(6)?;
        let signature_b64: String = row.get(8)?;

        let created_by: hellodb_crypto::VerifyingKey = serde_json::from_str(&created_by_b64)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
        let data: serde_json::Value = serde_json::from_str(&data_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let sig: hellodb_crypto::Signature = serde_json::from_str(&signature_b64).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
        })?;

        Ok(Record {
            record_id: row.get(0)?,
            schema: row.get(2)?,
            namespace: row.get(3)?,
            created_by,
            created_at_ms: row.get(5)?,
            data,
            previous_version: row.get(7)?,
            sig,
        })
    }

    /// Walk branch ancestry to find a record.
    fn find_in_ancestry(
        &self,
        record_id: &str,
        branch_id: &str,
    ) -> Result<Option<Record>, StorageError> {
        let mut current = Some(branch_id.to_string());

        while let Some(bid) = current {
            // Check if record exists on this branch
            let mut stmt = self.conn.prepare(
                "SELECT record_id, branch, schema_id, namespace, created_by_b64,
                        created_at_ms, data_json, previous_version, signature_b64, is_tombstone
                 FROM records WHERE record_id = ?1 AND branch = ?2",
            )?;
            let mut rows = stmt.query_map(params![record_id, bid], |row| {
                let is_tombstone: bool = row.get(9)?;
                if is_tombstone {
                    Ok(None)
                } else {
                    Self::row_to_record(row).map(Some)
                }
            })?;

            if let Some(Ok(maybe_rec)) = rows.next() {
                return Ok(maybe_rec);
            }

            // Walk to parent
            current = self
                .conn
                .query_row(
                    "SELECT parent FROM branches WHERE id = ?1",
                    params![bid],
                    |row| row.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten();
        }

        Ok(None)
    }

    /// Collect all records visible on a branch through its ancestry.
    fn collect_visible(
        &self,
        branch_id: &str,
        schema_filter: Option<&str>,
        ns_filter: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Record>, StorageError> {
        let mut seen: HashMap<String, Record> = HashMap::new();
        let mut tombstoned: Vec<String> = Vec::new();
        let mut current = Some(branch_id.to_string());

        while let Some(bid) = current {
            let mut stmt = self.conn.prepare(
                "SELECT record_id, branch, schema_id, namespace, created_by_b64,
                        created_at_ms, data_json, previous_version, signature_b64, is_tombstone
                 FROM records WHERE branch = ?1 ORDER BY created_at_ms DESC",
            )?;
            let rows = stmt.query_map(params![bid], |row| {
                let record_id: String = row.get(0)?;
                let is_tombstone: bool = row.get(9)?;
                if is_tombstone {
                    Ok((record_id, None))
                } else {
                    Self::row_to_record(row).map(|r| (record_id, Some(r)))
                }
            })?;

            for row_result in rows {
                let (rid, maybe_rec) = row_result?;
                if seen.contains_key(&rid) || tombstoned.contains(&rid) {
                    continue;
                }
                if let Some(rec) = maybe_rec {
                    let schema_match = schema_filter.is_none_or(|s| rec.schema == s);
                    let ns_match = ns_filter.is_none_or(|n| rec.namespace == n);
                    if schema_match && ns_match {
                        seen.insert(rid, rec);
                    }
                } else {
                    tombstoned.push(rid);
                }
            }

            current = self
                .conn
                .query_row(
                    "SELECT parent FROM branches WHERE id = ?1",
                    params![bid],
                    |row| row.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten();
        }

        let mut result: Vec<Record> = seen.into_values().collect();
        // newest first — sort by descending created_at_ms via Reverse key
        result.sort_by_key(|r| std::cmp::Reverse(r.created_at_ms));
        Ok(result.into_iter().skip(offset).take(limit).collect())
    }
}

#[allow(dead_code)]
struct RecordRow {
    record_id: String,
    branch: String,
    schema_id: String,
    namespace: String,
    created_by_b64: String,
    created_at_ms: u64,
    data_json: String,
    previous_version: Option<String>,
    signature_b64: String,
    is_tombstone: bool,
    stored_at_ms: u64,
}

impl StorageEngine for SqliteEngine {
    fn create_namespace(&mut self, namespace: Namespace) -> Result<(), StorageError> {
        // Check if exists
        let exists: bool = self.conn.query_row(
            "SELECT COUNT(*) > 0 FROM namespaces WHERE id = ?1",
            params![namespace.id],
            |row| row.get(0),
        )?;
        if exists {
            return Err(StorageError::NamespaceExists(namespace.id));
        }

        let owner_b64 = serde_json::to_string(&namespace.owner)?;
        let schemas_json = serde_json::to_string(&namespace.schemas)?;

        self.conn.execute(
            "INSERT INTO namespaces (id, name, owner_pubkey_b64, description, encrypted, created_at_ms, schemas_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                namespace.id,
                namespace.name,
                owner_b64,
                namespace.description,
                namespace.encrypted,
                namespace.created_at_ms,
                schemas_json,
            ],
        )?;

        // Create main branch
        let main_branch = Branch::main(namespace.id.clone());
        let changes_json = serde_json::to_string(&main_branch.changes)?;
        let state_str = serde_json::to_string(&main_branch.state)?;

        self.conn.execute(
            "INSERT INTO branches (id, namespace, parent, state, created_at_ms, label, changes_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                main_branch.id,
                main_branch.namespace,
                main_branch.parent,
                state_str.trim_matches('"'),
                main_branch.created_at_ms,
                main_branch.label,
                changes_json,
            ],
        )?;

        Ok(())
    }

    fn get_namespace(&self, id: &str) -> Result<Option<Namespace>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, owner_pubkey_b64, description, encrypted, created_at_ms, schemas_json
             FROM namespaces WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            let owner_b64: String = row.get(2)?;
            let schemas_json: String = row.get(6)?;

            let owner: hellodb_crypto::VerifyingKey =
                serde_json::from_str(&owner_b64).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            let schemas: Vec<String> = serde_json::from_str(&schemas_json).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok(Namespace {
                id: row.get(0)?,
                name: row.get(1)?,
                owner,
                description: row.get(3)?,
                encrypted: row.get(4)?,
                created_at_ms: row.get(5)?,
                schemas,
            })
        })?;

        match rows.next() {
            Some(Ok(ns)) => Ok(Some(ns)),
            Some(Err(e)) => Err(StorageError::Database(e)),
            None => Ok(None),
        }
    }

    fn list_namespaces(&self) -> Result<Vec<Namespace>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, owner_pubkey_b64, description, encrypted, created_at_ms, schemas_json
             FROM namespaces ORDER BY created_at_ms",
        )?;
        let rows = stmt.query_map([], |row| {
            let owner_b64: String = row.get(2)?;
            let schemas_json: String = row.get(6)?;

            let owner: hellodb_crypto::VerifyingKey =
                serde_json::from_str(&owner_b64).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            let schemas: Vec<String> = serde_json::from_str(&schemas_json).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok(Namespace {
                id: row.get(0)?,
                name: row.get(1)?,
                owner,
                description: row.get(3)?,
                encrypted: row.get(4)?,
                created_at_ms: row.get(5)?,
                schemas,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    fn register_schema(&mut self, schema: Schema) -> Result<(), StorageError> {
        let fields_json = serde_json::to_string(&schema.fields)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO schemas (id, version, namespace, name, fields_json, registered_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                schema.id,
                schema.version,
                schema.namespace,
                schema.name,
                fields_json,
                schema.registered_at_ms,
            ],
        )?;

        // Update namespace schemas list
        let ns = self.get_namespace(&schema.namespace)?;
        if let Some(mut ns) = ns {
            ns.register_schema(schema.id);
            let schemas_json = serde_json::to_string(&ns.schemas)?;
            self.conn.execute(
                "UPDATE namespaces SET schemas_json = ?1 WHERE id = ?2",
                params![schemas_json, ns.id],
            )?;
        }

        Ok(())
    }

    fn get_schema(&self, schema_id: &str) -> Result<Option<Schema>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, version, namespace, name, fields_json, registered_at_ms
             FROM schemas WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![schema_id], |row| {
            let fields_json: String = row.get(4)?;
            let fields: Vec<SchemaField> = serde_json::from_str(&fields_json).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok(Schema {
                id: row.get(0)?,
                version: row.get(1)?,
                namespace: row.get(2)?,
                name: row.get(3)?,
                fields,
                registered_at_ms: row.get(5)?,
            })
        })?;

        match rows.next() {
            Some(Ok(s)) => Ok(Some(s)),
            Some(Err(e)) => Err(StorageError::Database(e)),
            None => Ok(None),
        }
    }

    fn list_schemas(&self, namespace: &str) -> Result<Vec<Schema>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, version, namespace, name, fields_json, registered_at_ms
             FROM schemas WHERE namespace = ?1",
        )?;
        let rows = stmt.query_map(params![namespace], |row| {
            let fields_json: String = row.get(4)?;
            let fields: Vec<SchemaField> = serde_json::from_str(&fields_json).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

            Ok(Schema {
                id: row.get(0)?,
                version: row.get(1)?,
                namespace: row.get(2)?,
                name: row.get(3)?,
                fields,
                registered_at_ms: row.get(5)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    fn create_branch(&mut self, branch: Branch) -> Result<(), StorageError> {
        let changes_json = serde_json::to_string(&branch.changes)?;
        let state_str = serde_json::to_string(&branch.state)?;
        self.conn.execute(
            "INSERT INTO branches (id, namespace, parent, state, created_at_ms, label, changes_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                branch.id,
                branch.namespace,
                branch.parent,
                state_str.trim_matches('"'),
                branch.created_at_ms,
                branch.label,
                changes_json,
            ],
        )?;
        Ok(())
    }

    fn get_branch(&self, branch_id: &str) -> Result<Option<Branch>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, namespace, parent, state, created_at_ms, label, changes_json
             FROM branches WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![branch_id], |row| {
            let state_str: String = row.get(3)?;
            let changes_json: String = row.get(6)?;

            let state: BranchState =
                serde_json::from_str(&format!("\"{}\"", state_str)).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            let changes: HashMap<RecordId, bool> =
                serde_json::from_str(&changes_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

            Ok(Branch {
                id: row.get(0)?,
                namespace: row.get(1)?,
                parent: row.get(2)?,
                state,
                created_at_ms: row.get(4)?,
                label: row.get(5)?,
                changes,
            })
        })?;

        match rows.next() {
            Some(Ok(b)) => Ok(Some(b)),
            Some(Err(e)) => Err(StorageError::Database(e)),
            None => Ok(None),
        }
    }

    fn list_branches(&self, namespace: &str) -> Result<Vec<Branch>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, namespace, parent, state, created_at_ms, label, changes_json
             FROM branches WHERE namespace = ?1",
        )?;
        let rows = stmt.query_map(params![namespace], |row| {
            let state_str: String = row.get(3)?;
            let changes_json: String = row.get(6)?;

            let state: BranchState =
                serde_json::from_str(&format!("\"{}\"", state_str)).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
            let changes: HashMap<RecordId, bool> =
                serde_json::from_str(&changes_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

            Ok(Branch {
                id: row.get(0)?,
                namespace: row.get(1)?,
                parent: row.get(2)?,
                state,
                created_at_ms: row.get(4)?,
                label: row.get(5)?,
                changes,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::Database)
    }

    fn merge_branch(&mut self, branch_id: &str) -> Result<MergeResult, StorageError> {
        let branch = self
            .get_branch(branch_id)?
            .ok_or_else(|| StorageError::BranchNotFound(branch_id.to_string()))?;

        if branch.state != BranchState::Active {
            return Err(StorageError::BranchNotActive(branch_id.to_string()));
        }

        let parent_id = branch
            .parent
            .as_ref()
            .ok_or_else(|| StorageError::BranchNotFound("cannot merge main branch".into()))?;

        let parent = self
            .get_branch(parent_id)?
            .ok_or_else(|| StorageError::BranchNotFound(parent_id.clone()))?;

        let merge_result = branch
            .fast_forward_merge(&parent)
            .map_err(StorageError::Core)?;

        if !merge_result.conflicts.is_empty() {
            return Err(StorageError::MergeConflict(branch_id.to_string()));
        }

        // Move records from branch to parent
        for record_id in &merge_result.merged_records {
            self.conn.execute(
                "UPDATE records SET branch = ?1 WHERE record_id = ?2 AND branch = ?3",
                params![parent_id, record_id, branch_id],
            )?;
        }

        // Mark branch as merged
        self.conn.execute(
            "UPDATE branches SET state = 'merged' WHERE id = ?1",
            params![branch_id],
        )?;

        // Update parent's changes_json
        let mut updated_parent = parent;
        for record_id in &merge_result.merged_records {
            updated_parent.add_change(record_id.clone());
        }
        let changes_json = serde_json::to_string(&updated_parent.changes)?;
        self.conn.execute(
            "UPDATE branches SET changes_json = ?1 WHERE id = ?2",
            params![changes_json, parent_id],
        )?;

        Ok(merge_result)
    }

    fn put_record(&mut self, record: Record, branch: &str) -> Result<(), StorageError> {
        let row = Self::record_to_row(&record, branch)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO records
             (record_id, branch, schema_id, namespace, created_by_b64, created_at_ms,
              data_json, previous_version, signature_b64, is_tombstone, stored_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                row.record_id,
                row.branch,
                row.schema_id,
                row.namespace,
                row.created_by_b64,
                row.created_at_ms,
                row.data_json,
                row.previous_version,
                row.signature_b64,
                row.is_tombstone,
                row.stored_at_ms,
            ],
        )?;

        // Update branch changes
        let branch_obj = self.get_branch(branch)?;
        if let Some(mut b) = branch_obj {
            b.add_change(record.record_id);
            let changes_json = serde_json::to_string(&b.changes)?;
            self.conn.execute(
                "UPDATE branches SET changes_json = ?1 WHERE id = ?2",
                params![changes_json, branch],
            )?;
        }

        Ok(())
    }

    fn get_record(&self, record_id: &str, branch: &str) -> Result<Option<Record>, StorageError> {
        self.find_in_ancestry(record_id, branch)
    }

    fn has_record(&self, record_id: &str, branch: &str) -> Result<bool, StorageError> {
        Ok(self.find_in_ancestry(record_id, branch)?.is_some())
    }

    fn list_records_by_schema(
        &self,
        schema: &str,
        branch: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Record>, StorageError> {
        self.collect_visible(branch, Some(schema), None, limit, offset)
    }

    fn list_records_by_namespace(
        &self,
        namespace: &str,
        branch: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Record>, StorageError> {
        self.collect_visible(branch, None, Some(namespace), limit, offset)
    }

    fn count_records_by_schema(&self, schema: &str, branch: &str) -> Result<u64, StorageError> {
        let records = self.collect_visible(branch, Some(schema), None, usize::MAX, 0)?;
        Ok(records.len() as u64)
    }

    fn delete_record(&mut self, record_id: &str, branch: &str) -> Result<(), StorageError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Insert a tombstone record
        self.conn.execute(
            "INSERT OR REPLACE INTO records
             (record_id, branch, schema_id, namespace, created_by_b64, created_at_ms,
              data_json, previous_version, signature_b64, is_tombstone, stored_at_ms)
             VALUES (?1, ?2, '', '', '', 0, '{}', NULL, '', 1, ?3)",
            params![record_id, branch, now_ms],
        )?;

        // Update branch changes
        let branch_obj = self.get_branch(branch)?;
        if let Some(mut b) = branch_obj {
            b.add_deletion(record_id.to_string());
            let changes_json = serde_json::to_string(&b.changes)?;
            self.conn.execute(
                "UPDATE branches SET changes_json = ?1 WHERE id = ?2",
                params![changes_json, branch],
            )?;
        }

        Ok(())
    }

    fn reinforce_record(
        &mut self,
        record_id: &str,
        delta: f32,
        now_ms: u64,
    ) -> Result<crate::engine::RecordMetadata, StorageError> {
        // UPSERT: create on first call, compose on subsequent calls.
        self.conn.execute(
            "INSERT INTO record_metadata (record_id, score, reinforce_count, last_reinforced_at_ms, first_seen_ms)
             VALUES (?1, ?2, 1, ?3, ?3)
             ON CONFLICT(record_id) DO UPDATE SET
                score = record_metadata.score + excluded.score,
                reinforce_count = record_metadata.reinforce_count + 1,
                last_reinforced_at_ms = excluded.last_reinforced_at_ms",
            params![record_id, delta as f64, now_ms as i64],
        )?;

        // Read back the current state.
        self.get_record_metadata(record_id)?.ok_or_else(|| {
            StorageError::Internal("reinforce: metadata missing after upsert".into())
        })
    }

    fn get_record_metadata(
        &self,
        record_id: &str,
    ) -> Result<Option<crate::engine::RecordMetadata>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT record_id, score, reinforce_count, last_reinforced_at_ms, first_seen_ms, archived_at_ms
             FROM record_metadata WHERE record_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![record_id], |row| {
            let score: f64 = row.get(1)?;
            let reinforce_count: i64 = row.get(2)?;
            let last: i64 = row.get(3)?;
            let first: i64 = row.get(4)?;
            let archived: Option<i64> = row.get(5)?;
            Ok(crate::engine::RecordMetadata {
                record_id: row.get(0)?,
                score: score as f32,
                reinforce_count: reinforce_count as u64,
                last_reinforced_at_ms: last as u64,
                first_seen_ms: first as u64,
                archived_at_ms: archived.map(|v| v as u64),
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    fn archive_record(&mut self, record_id: &str, now_ms: u64) -> Result<(), StorageError> {
        // Ensure a metadata row exists (with zero score if never reinforced),
        // then mark it archived. This lets callers archive records they've
        // never touched with reinforce, which is useful for the digest pipeline.
        self.conn.execute(
            "INSERT INTO record_metadata (record_id, first_seen_ms, archived_at_ms)
             VALUES (?1, ?2, ?2)
             ON CONFLICT(record_id) DO UPDATE SET archived_at_ms = excluded.archived_at_ms",
            params![record_id, now_ms as i64],
        )?;
        Ok(())
    }

    fn tail_records(
        &self,
        namespace: &str,
        after_seq: u64,
        limit: usize,
        branch_filter: Option<&str>,
    ) -> Result<Vec<crate::engine::TailEntry>, StorageError> {
        // rowid is SQLite's built-in monotonic cursor. For a table without an
        // INTEGER PRIMARY KEY column, it is automatically assigned as max(rowid)+1
        // on each INSERT (including INSERT OR REPLACE). It is not reused.
        //
        // We exclude tombstones from the tail because the digest/consolidate
        // pipeline is interested in new information, not deletion events.
        let base_sql = "SELECT rowid, record_id, branch, schema_id, namespace, created_by_b64,
                    created_at_ms, data_json, previous_version, signature_b64
             FROM records
             WHERE namespace = ?1 AND rowid > ?2 AND is_tombstone = 0";

        let (sql, entries) = match branch_filter {
            Some(branch) => {
                let sql = format!("{base_sql} AND branch = ?3 ORDER BY rowid ASC LIMIT ?4");
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt
                    .query_map(
                        params![namespace, after_seq as i64, branch, limit as i64],
                        Self::row_to_tail_entry,
                    )?
                    .collect::<Result<Vec<_>, _>>()?;
                (sql, rows)
            }
            None => {
                let sql = format!("{base_sql} ORDER BY rowid ASC LIMIT ?3");
                let mut stmt = self.conn.prepare(&sql)?;
                let rows = stmt
                    .query_map(
                        params![namespace, after_seq as i64, limit as i64],
                        Self::row_to_tail_entry,
                    )?
                    .collect::<Result<Vec<_>, _>>()?;
                (sql, rows)
            }
        };
        let _ = sql; // silence unused warning from the shared binding
        Ok(entries)
    }
}

impl SqliteEngine {
    /// Convert a SELECT rowid,... row into a TailEntry.
    fn row_to_tail_entry(row: &rusqlite::Row) -> rusqlite::Result<crate::engine::TailEntry> {
        let seq: i64 = row.get(0)?;
        let record_id: String = row.get(1)?;
        let branch: String = row.get(2)?;
        let schema: String = row.get(3)?;
        let namespace: String = row.get(4)?;
        let created_by_b64: String = row.get(5)?;
        let created_at_ms: u64 = row.get(6)?;
        let data_json: String = row.get(7)?;
        let previous_version: Option<String> = row.get(8)?;
        let signature_b64: String = row.get(9)?;

        let created_by: hellodb_crypto::VerifyingKey = serde_json::from_str(&created_by_b64)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
        let data: serde_json::Value = serde_json::from_str(&data_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let sig: hellodb_crypto::Signature = serde_json::from_str(&signature_b64).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(e))
        })?;

        Ok(crate::engine::TailEntry {
            seq: seq as u64,
            branch,
            record: Record {
                record_id,
                schema,
                namespace,
                created_by,
                created_at_ms,
                data,
                previous_version,
                sig,
            },
        })
    }
}
