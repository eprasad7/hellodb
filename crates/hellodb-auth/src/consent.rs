//! Consent proofs for hellodb.
//!
//! Cross-namespace reads and sensitive operations require explicit
//! consent from the user, proven by an Ed25519 signature.

use hellodb_crypto::{content_hash, Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::error::AuthError;
use hellodb_core::canonical::canonicalize_value;

/// Actions that require consent in hellodb.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentAction {
    /// Grant an app read access to a namespace it does not own.
    CrossNamespaceRead,
    /// Grant an agent query access across multiple namespaces.
    AgentQueryAccess,
    /// Export data from a namespace.
    ExportData,
    /// Share a namespace's encryption key with another device.
    ShareNamespaceKey,
    /// Delete all records in a namespace.
    PurgeNamespace,
    /// Grant write access to another app within a namespace.
    GrantWriteAccess,
}

/// A cryptographic proof of user consent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentProof {
    /// What action is being consented to.
    pub action: ConsentAction,
    /// Human-readable description shown to user before signing.
    pub description: String,
    /// The user/device giving consent.
    pub consenter: VerifyingKey,
    /// Target of consent (namespace ID, agent pubkey, app ID, etc.).
    pub target: String,
    /// Optional: specific namespace this consent applies to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Unix timestamp when consent was given.
    pub consented_at_ms: u64,
    /// Optional expiry. None = no expiry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<u64>,
    /// Signature over the canonical consent fields.
    pub sig: Signature,
}

/// Intermediate for signing.
#[derive(Serialize)]
struct ConsentForSigning<'a> {
    action: &'a ConsentAction,
    description: &'a str,
    consenter: &'a VerifyingKey,
    target: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace: &'a Option<String>,
    consented_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at_ms: &'a Option<u64>,
}

impl ConsentProof {
    /// Create and sign a consent proof.
    pub fn new(
        signing_key: &SigningKey,
        action: ConsentAction,
        description: String,
        target: String,
        namespace: Option<String>,
        expires_at_ms: Option<u64>,
    ) -> Result<Self, AuthError> {
        let consenter = signing_key.verifying_key();
        let consented_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let signable = ConsentForSigning {
            action: &action,
            description: &description,
            consenter: &consenter,
            target: &target,
            namespace: &namespace,
            consented_at_ms,
            expires_at_ms: &expires_at_ms,
        };

        let canonical = canonicalize_value(&signable).map_err(AuthError::Core)?;
        let sig = signing_key.sign(&canonical);

        Ok(Self {
            action,
            description,
            consenter,
            target,
            namespace,
            consented_at_ms,
            expires_at_ms,
            sig,
        })
    }

    /// Create with explicit timestamp (for testing).
    pub fn new_with_timestamp(
        signing_key: &SigningKey,
        action: ConsentAction,
        description: String,
        target: String,
        namespace: Option<String>,
        consented_at_ms: u64,
        expires_at_ms: Option<u64>,
    ) -> Result<Self, AuthError> {
        let consenter = signing_key.verifying_key();

        let signable = ConsentForSigning {
            action: &action,
            description: &description,
            consenter: &consenter,
            target: &target,
            namespace: &namespace,
            consented_at_ms,
            expires_at_ms: &expires_at_ms,
        };

        let canonical = canonicalize_value(&signable).map_err(AuthError::Core)?;
        let sig = signing_key.sign(&canonical);

        Ok(Self {
            action,
            description,
            consenter,
            target,
            namespace,
            consented_at_ms,
            expires_at_ms,
            sig,
        })
    }

    /// Verify the consent proof signature.
    pub fn verify(&self) -> Result<(), AuthError> {
        let signable = ConsentForSigning {
            action: &self.action,
            description: &self.description,
            consenter: &self.consenter,
            target: &self.target,
            namespace: &self.namespace,
            consented_at_ms: self.consented_at_ms,
            expires_at_ms: &self.expires_at_ms,
        };

        let canonical = canonicalize_value(&signable).map_err(AuthError::Core)?;
        self.consenter
            .verify(&canonical, &self.sig)
            .map_err(AuthError::Crypto)
    }

    /// Check if this consent is still valid (not expired).
    pub fn is_valid(&self, now_ms: u64) -> bool {
        match self.expires_at_ms {
            Some(exp) => now_ms <= exp,
            None => true, // No expiry
        }
    }

    /// Content hash of this consent proof.
    pub fn content_hash(&self) -> Result<String, AuthError> {
        let signable = ConsentForSigning {
            action: &self.action,
            description: &self.description,
            consenter: &self.consenter,
            target: &self.target,
            namespace: &self.namespace,
            consented_at_ms: self.consented_at_ms,
            expires_at_ms: &self.expires_at_ms,
        };
        let canonical = canonicalize_value(&signable).map_err(AuthError::Core)?;
        Ok(content_hash(&canonical))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hellodb_crypto::KeyPair;

    #[test]
    fn create_and_verify() {
        let kp = KeyPair::generate();
        let consent = ConsentProof::new(
            &kp.signing,
            ConsentAction::CrossNamespaceRead,
            "Grant App B read access to ainp.commerce".into(),
            "app-b-pubkey".into(),
            Some("ainp.commerce".into()),
            None,
        )
        .unwrap();

        assert!(consent.verify().is_ok());
    }

    #[test]
    fn wrong_key_fails() {
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();
        let mut consent = ConsentProof::new(
            &kp1.signing,
            ConsentAction::CrossNamespaceRead,
            "test".into(),
            "target".into(),
            None,
            None,
        )
        .unwrap();

        consent.consenter = kp2.verifying;
        assert!(consent.verify().is_err());
    }

    #[test]
    fn tampered_description_fails() {
        let kp = KeyPair::generate();
        let mut consent = ConsentProof::new(
            &kp.signing,
            ConsentAction::ExportData,
            "Export health data".into(),
            "export-target".into(),
            None,
            None,
        )
        .unwrap();

        consent.description = "Export ALL data".into();
        assert!(consent.verify().is_err());
    }

    #[test]
    fn expired_consent() {
        let kp = KeyPair::generate();
        let consent = ConsentProof::new_with_timestamp(
            &kp.signing,
            ConsentAction::CrossNamespaceRead,
            "test".into(),
            "target".into(),
            None,
            1000,
            Some(2000),
        )
        .unwrap();

        assert!(consent.is_valid(1500));
        assert!(!consent.is_valid(3000));
    }

    #[test]
    fn no_expiry_always_valid() {
        let kp = KeyPair::generate();
        let consent = ConsentProof::new(
            &kp.signing,
            ConsentAction::CrossNamespaceRead,
            "test".into(),
            "target".into(),
            None,
            None,
        )
        .unwrap();

        assert!(consent.is_valid(u64::MAX));
    }

    #[test]
    fn namespace_scoped_consent() {
        let kp = KeyPair::generate();
        let consent = ConsentProof::new(
            &kp.signing,
            ConsentAction::CrossNamespaceRead,
            "Read commerce data".into(),
            "agent-key".into(),
            Some("ainp.commerce".into()),
            None,
        )
        .unwrap();

        assert!(consent.verify().is_ok());
        assert_eq!(consent.namespace, Some("ainp.commerce".into()));
    }

    #[test]
    fn serde_roundtrip() {
        let kp = KeyPair::generate();
        let consent = ConsentProof::new(
            &kp.signing,
            ConsentAction::AgentQueryAccess,
            "Agent query all".into(),
            "agent-x".into(),
            None,
            Some(999999999999),
        )
        .unwrap();

        let json = serde_json::to_string(&consent).unwrap();
        let restored: ConsentProof = serde_json::from_str(&json).unwrap();
        assert!(restored.verify().is_ok());
    }
}
