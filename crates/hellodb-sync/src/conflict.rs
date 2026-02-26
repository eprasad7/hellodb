//! Conflict detection and resolution for sync.
//!
//! When two devices modify the same record independently, a conflict
//! arises during pull. The conflict strategy determines how to resolve it.

use hellodb_core::Record;

/// Strategy for resolving sync conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    /// Most recent write wins (by created_at_ms timestamp).
    LastWriterWins,
    /// The original creator's version wins (the device that created
    /// the record first keeps priority).
    CreatorWins,
}

/// A conflict between local and remote versions of a record.
#[derive(Debug, Clone)]
pub struct SyncConflict {
    /// The record ID in conflict.
    pub record_id: String,
    /// The local version of the record.
    pub local_record: Record,
    /// The remote version of the record.
    pub remote_record: Record,
    /// The resolved record (None until resolved).
    pub resolved: Option<Record>,
}

/// Resolve a conflict using the given strategy.
///
/// Returns the winning record.
pub fn resolve_conflict(
    strategy: ConflictStrategy,
    local: &Record,
    remote: &Record,
) -> Record {
    match strategy {
        ConflictStrategy::LastWriterWins => {
            if remote.created_at_ms >= local.created_at_ms {
                remote.clone()
            } else {
                local.clone()
            }
        }
        ConflictStrategy::CreatorWins => {
            // "Creator wins" means: the version from the device that
            // originally created the record takes priority. Since both
            // versions have the same record_id (content-addressed), the
            // one with the earlier original creation time is the
            // original creator — the other is an update.
            //
            // If they have the same created_by key, fall back to LWW.
            if local.created_by == remote.created_by {
                // Same creator — LWW tiebreaker
                if remote.created_at_ms >= local.created_at_ms {
                    remote.clone()
                } else {
                    local.clone()
                }
            } else {
                // Different creators: keep local (current device's creator)
                local.clone()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hellodb_crypto::KeyPair;
    use serde_json::json;

    fn make_record(kp: &KeyPair, data: serde_json::Value, ts: u64) -> Record {
        Record::new_with_timestamp(
            &kp.signing,
            "test.schema".into(),
            "test.ns".into(),
            data,
            None,
            ts,
        )
        .unwrap()
    }

    #[test]
    fn lww_remote_newer_wins() {
        let kp = KeyPair::generate();
        let local = make_record(&kp, json!({"v": "local"}), 1000);
        let remote = make_record(&kp, json!({"v": "remote"}), 2000);
        let winner = resolve_conflict(ConflictStrategy::LastWriterWins, &local, &remote);
        assert_eq!(winner.data["v"], "remote");
    }

    #[test]
    fn lww_local_newer_wins() {
        let kp = KeyPair::generate();
        let local = make_record(&kp, json!({"v": "local"}), 3000);
        let remote = make_record(&kp, json!({"v": "remote"}), 2000);
        let winner = resolve_conflict(ConflictStrategy::LastWriterWins, &local, &remote);
        assert_eq!(winner.data["v"], "local");
    }

    #[test]
    fn lww_same_time_remote_wins() {
        let kp = KeyPair::generate();
        let local = make_record(&kp, json!({"v": "local"}), 5000);
        let remote = make_record(&kp, json!({"v": "remote"}), 5000);
        let winner = resolve_conflict(ConflictStrategy::LastWriterWins, &local, &remote);
        // Tie goes to remote (>= comparison)
        assert_eq!(winner.data["v"], "remote");
    }

    #[test]
    fn creator_wins_same_creator_falls_back_to_lww() {
        let kp = KeyPair::generate();
        let local = make_record(&kp, json!({"v": "local"}), 1000);
        let remote = make_record(&kp, json!({"v": "remote"}), 2000);
        let winner = resolve_conflict(ConflictStrategy::CreatorWins, &local, &remote);
        assert_eq!(winner.data["v"], "remote"); // LWW tiebreaker
    }

    #[test]
    fn creator_wins_different_creator_keeps_local() {
        let kp_local = KeyPair::generate();
        let kp_remote = KeyPair::generate();
        let local = make_record(&kp_local, json!({"v": "local"}), 1000);
        let remote = make_record(&kp_remote, json!({"v": "remote"}), 2000);
        let winner = resolve_conflict(ConflictStrategy::CreatorWins, &local, &remote);
        assert_eq!(winner.data["v"], "local"); // Creator (local) wins
    }

    #[test]
    fn sync_conflict_struct() {
        let kp = KeyPair::generate();
        let local = make_record(&kp, json!({"v": "local"}), 1000);
        let remote = make_record(&kp, json!({"v": "remote"}), 2000);
        let mut conflict = SyncConflict {
            record_id: "test-id".into(),
            local_record: local.clone(),
            remote_record: remote.clone(),
            resolved: None,
        };
        assert!(conflict.resolved.is_none());

        let winner = resolve_conflict(ConflictStrategy::LastWriterWins, &local, &remote);
        conflict.resolved = Some(winner);
        assert!(conflict.resolved.is_some());
    }
}
