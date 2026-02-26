//! Git-like branching for hellodb.
//!
//! `main` is committed reality. Branches are draft transactions,
//! agent workspaces, or sync conflict resolution spaces.
//! Branches are metadata-only: they track a pointer to the latest
//! record set, using copy-on-write semantics over immutable records.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::CoreError;
use crate::namespace::NamespaceId;
use crate::record::RecordId;

/// Branch identifier.
pub type BranchId = String;

/// The state of a branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchState {
    /// Active branch, can accept writes.
    Active,
    /// Branch has been merged to its parent.
    Merged,
    /// Branch was abandoned/deleted.
    Abandoned,
}

/// A branch in the hellodb record graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    /// Unique branch identifier.
    pub id: BranchId,
    /// The namespace this branch belongs to.
    pub namespace: NamespaceId,
    /// Parent branch ID (None for "main").
    pub parent: Option<BranchId>,
    /// Current state of the branch.
    pub state: BranchState,
    /// Unix timestamp when created.
    pub created_at_ms: u64,
    /// Human-readable label (e.g., "agent-draft-2024-01", "sync-conflict-device-b").
    pub label: String,
    /// Record IDs added or modified on this branch (not on parent).
    /// Maps record_id -> true for added, false for deleted (tombstone).
    #[serde(default)]
    pub changes: HashMap<RecordId, bool>,
}

/// Result of merging a branch into its parent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    /// Records successfully merged.
    pub merged_records: Vec<RecordId>,
    /// Records that had conflicts (same record_id modified on both branches).
    pub conflicts: Vec<MergeConflict>,
}

/// A merge conflict: same record modified on both source and target branches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflict {
    pub record_id: RecordId,
    /// Description of the conflict.
    pub description: String,
}

impl Branch {
    /// Create the "main" branch for a namespace.
    pub fn main(namespace: NamespaceId) -> Self {
        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            id: format!("{}/main", namespace),
            namespace,
            parent: None,
            state: BranchState::Active,
            created_at_ms,
            label: "main".into(),
            changes: HashMap::new(),
        }
    }

    /// Create a new branch off a parent.
    pub fn new(
        id: BranchId,
        namespace: NamespaceId,
        parent: BranchId,
        label: String,
    ) -> Self {
        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            id,
            namespace,
            parent: Some(parent),
            state: BranchState::Active,
            created_at_ms,
            label,
            changes: HashMap::new(),
        }
    }

    /// Record an addition or modification on this branch.
    pub fn add_change(&mut self, record_id: RecordId) {
        self.changes.insert(record_id, true);
    }

    /// Record a deletion (tombstone) on this branch.
    pub fn add_deletion(&mut self, record_id: RecordId) {
        self.changes.insert(record_id, false);
    }

    /// Check if this branch has any changes relative to its parent.
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Mark this branch as merged.
    pub fn mark_merged(&mut self) {
        self.state = BranchState::Merged;
    }

    /// Mark this branch as abandoned.
    pub fn mark_abandoned(&mut self) {
        self.state = BranchState::Abandoned;
    }

    /// Attempt a fast-forward merge. Returns records that would be merged
    /// and any conflicts (records changed on both branches).
    pub fn fast_forward_merge(
        &self,
        target: &Branch,
    ) -> Result<MergeResult, CoreError> {
        if self.state != BranchState::Active {
            return Err(CoreError::BranchNotActive(self.id.clone()));
        }

        let mut merged = Vec::new();
        let mut conflicts = Vec::new();

        for record_id in self.changes.keys() {
            if target.changes.contains_key(record_id) {
                conflicts.push(MergeConflict {
                    record_id: record_id.clone(),
                    description: format!(
                        "record {} modified on both {} and {}",
                        record_id, self.id, target.id
                    ),
                });
            } else {
                merged.push(record_id.clone());
            }
        }

        Ok(MergeResult {
            merged_records: merged,
            conflicts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_main_branch() {
        let main = Branch::main("ainp.commerce".into());
        assert_eq!(main.id, "ainp.commerce/main");
        assert_eq!(main.namespace, "ainp.commerce");
        assert!(main.parent.is_none());
        assert_eq!(main.state, BranchState::Active);
        assert_eq!(main.label, "main");
    }

    #[test]
    fn create_child_branch() {
        let main = Branch::main("test".into());
        let child = Branch::new(
            "test/draft-1".into(),
            "test".into(),
            main.id.clone(),
            "draft-1".into(),
        );
        assert_eq!(child.parent.as_ref().unwrap(), &main.id);
        assert_eq!(child.state, BranchState::Active);
    }

    #[test]
    fn add_changes() {
        let mut branch = Branch::main("test".into());
        assert!(!branch.has_changes());

        branch.add_change("rec-001".into());
        branch.add_change("rec-002".into());
        assert!(branch.has_changes());
        assert_eq!(branch.changes.len(), 2);
        assert_eq!(branch.changes.get("rec-001"), Some(&true));
    }

    #[test]
    fn add_deletion() {
        let mut branch = Branch::main("test".into());
        branch.add_deletion("rec-003".into());
        assert_eq!(branch.changes.get("rec-003"), Some(&false));
    }

    #[test]
    fn fast_forward_merge_no_conflicts() {
        let target = Branch::main("test".into());
        let mut source = Branch::new(
            "test/feature".into(),
            "test".into(),
            target.id.clone(),
            "feature".into(),
        );
        source.add_change("rec-001".into());
        source.add_change("rec-002".into());

        let result = source.fast_forward_merge(&target).unwrap();
        assert_eq!(result.merged_records.len(), 2);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn fast_forward_merge_with_conflicts() {
        let mut target = Branch::main("test".into());
        target.add_change("rec-001".into()); // changed on target too

        let mut source = Branch::new(
            "test/feature".into(),
            "test".into(),
            target.id.clone(),
            "feature".into(),
        );
        source.add_change("rec-001".into()); // conflict!
        source.add_change("rec-002".into()); // no conflict

        let result = source.fast_forward_merge(&target).unwrap();
        assert_eq!(result.merged_records.len(), 1);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].record_id, "rec-001");
    }

    #[test]
    fn mark_merged_state() {
        let mut branch = Branch::main("test".into());
        branch.mark_merged();
        assert_eq!(branch.state, BranchState::Merged);
    }

    #[test]
    fn mark_abandoned_state() {
        let mut branch = Branch::main("test".into());
        branch.mark_abandoned();
        assert_eq!(branch.state, BranchState::Abandoned);
    }

    #[test]
    fn inactive_branch_cannot_merge() {
        let target = Branch::main("test".into());
        let mut source = Branch::new(
            "test/feature".into(),
            "test".into(),
            target.id.clone(),
            "feature".into(),
        );
        source.mark_abandoned();
        assert!(source.fast_forward_merge(&target).is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let mut branch = Branch::main("test".into());
        branch.add_change("rec-001".into());
        branch.add_deletion("rec-002".into());

        let json = serde_json::to_string(&branch).unwrap();
        let restored: Branch = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, branch.id);
        assert_eq!(restored.changes.len(), 2);
    }
}
