//! Namespace model.
//!
//! Namespaces provide data isolation between apps. Each app writes
//! to its own namespace. Cross-namespace reads require consent proofs.
//! Namespace IDs use reverse-domain notation (e.g., "ainp.commerce",
//! "health.vitals", "finance.transactions").

use serde::{Deserialize, Serialize};

use hellodb_crypto::VerifyingKey;

/// Namespace identifier (reverse-domain string).
pub type NamespaceId = String;

/// A namespace registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    /// Unique namespace identifier.
    pub id: NamespaceId,
    /// Human-readable name.
    pub name: String,
    /// The app/agent that owns this namespace.
    pub owner: VerifyingKey,
    /// Description of what data this namespace holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Unix timestamp when created.
    pub created_at_ms: u64,
    /// Whether this namespace is encrypted at rest.
    pub encrypted: bool,
    /// Schema IDs registered in this namespace.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schemas: Vec<String>,
}

impl Namespace {
    /// Create a new namespace.
    pub fn new(
        id: String,
        name: String,
        owner: VerifyingKey,
        encrypted: bool,
    ) -> Self {
        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            id,
            name,
            owner,
            description: None,
            created_at_ms,
            encrypted,
            schemas: Vec::new(),
        }
    }

    /// Check if a verifying key is the owner of this namespace.
    pub fn is_owner(&self, key: &VerifyingKey) -> bool {
        self.owner == *key
    }

    /// Add a schema ID to this namespace's registered schemas.
    pub fn register_schema(&mut self, schema_id: String) {
        if !self.schemas.contains(&schema_id) {
            self.schemas.push(schema_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hellodb_crypto::KeyPair;

    #[test]
    fn create_namespace() {
        let kp = KeyPair::generate();
        let ns = Namespace::new(
            "ainp.commerce".into(),
            "AINP Commerce".into(),
            kp.verifying.clone(),
            true,
        );
        assert_eq!(ns.id, "ainp.commerce");
        assert_eq!(ns.name, "AINP Commerce");
        assert!(ns.encrypted);
        assert!(ns.schemas.is_empty());
    }

    #[test]
    fn ownership_check() {
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();
        let ns = Namespace::new(
            "test".into(),
            "Test".into(),
            kp1.verifying.clone(),
            false,
        );
        assert!(ns.is_owner(&kp1.verifying));
        assert!(!ns.is_owner(&kp2.verifying));
    }

    #[test]
    fn register_schema() {
        let kp = KeyPair::generate();
        let mut ns = Namespace::new(
            "ainp.commerce".into(),
            "Commerce".into(),
            kp.verifying.clone(),
            true,
        );
        ns.register_schema("ainp.commerce.listing".into());
        ns.register_schema("ainp.commerce.order".into());
        ns.register_schema("ainp.commerce.listing".into()); // duplicate, should be ignored
        assert_eq!(ns.schemas.len(), 2);
    }

    #[test]
    fn serde_roundtrip() {
        let kp = KeyPair::generate();
        let ns = Namespace::new(
            "health.vitals".into(),
            "Health Vitals".into(),
            kp.verifying.clone(),
            true,
        );
        let json = serde_json::to_string(&ns).unwrap();
        let restored: Namespace = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "health.vitals");
        assert_eq!(restored.owner, kp.verifying);
    }
}
