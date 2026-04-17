//! Delegation credentials for hellodb.
//!
//! A user delegates limited authority to agents or apps.
//! Agents can then query across namespaces within their granted scopes.

use hellodb_crypto::{content_hash, Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::error::AuthError;
use hellodb_core::canonical::canonicalize_value;

/// What a delegate is allowed to do in hellodb.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationScope {
    /// Read records from a specific namespace.
    ReadNamespace,
    /// Write records to a specific namespace.
    WriteNamespace,
    /// Query across multiple namespaces (agent query).
    CrossNamespaceQuery,
    /// Create new namespaces.
    CreateNamespace,
    /// Manage branches within a namespace.
    ManageBranches,
    /// Full authority (all scopes).
    Full,
}

/// A delegation credential granting scoped authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationCredential {
    /// Unique delegation ID (content hash).
    pub delegation_id: String,
    /// User/device that created the delegation.
    pub delegator: VerifyingKey,
    /// Agent/app receiving the delegation.
    pub delegate: VerifyingKey,
    /// What the delegate is allowed to do.
    pub scopes: Vec<DelegationScope>,
    /// Namespaces this delegation applies to (empty = all).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub namespaces: Vec<String>,
    /// When this delegation was created (unix ms).
    pub created_at_ms: u64,
    /// When this delegation expires (unix ms).
    pub expires_at_ms: u64,
    /// Maximum number of queries the delegate can perform (0 = unlimited).
    pub max_queries: u64,
    /// Queries performed so far.
    pub queries_used: u64,
    /// Whether this delegation has been revoked.
    pub revoked: bool,
    /// Delegator's signature over the delegation fields.
    pub sig: Signature,
}

/// Intermediate for signing.
#[derive(Serialize)]
struct DelegationForSigning<'a> {
    delegator: &'a VerifyingKey,
    delegate: &'a VerifyingKey,
    scopes: &'a [DelegationScope],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    namespaces: &'a Vec<String>,
    created_at_ms: u64,
    expires_at_ms: u64,
    max_queries: u64,
}

impl DelegationCredential {
    /// Create and sign a new delegation.
    pub fn new(
        signing_key: &SigningKey,
        delegate: VerifyingKey,
        scopes: Vec<DelegationScope>,
        namespaces: Vec<String>,
        now_ms: u64,
        ttl_ms: u64,
        max_queries: u64,
    ) -> Result<Self, AuthError> {
        let delegator = signing_key.verifying_key();

        let signable = DelegationForSigning {
            delegator: &delegator,
            delegate: &delegate,
            scopes: &scopes,
            namespaces: &namespaces,
            created_at_ms: now_ms,
            expires_at_ms: now_ms + ttl_ms,
            max_queries,
        };

        let canonical = canonicalize_value(&signable).map_err(AuthError::Core)?;
        let delegation_id = content_hash(&canonical);
        let sig = signing_key.sign(&canonical);

        Ok(Self {
            delegation_id,
            delegator,
            delegate,
            scopes,
            namespaces,
            created_at_ms: now_ms,
            expires_at_ms: now_ms + ttl_ms,
            max_queries,
            queries_used: 0,
            revoked: false,
            sig,
        })
    }

    /// Verify the delegation's signature.
    pub fn verify_signature(&self) -> Result<(), AuthError> {
        let signable = DelegationForSigning {
            delegator: &self.delegator,
            delegate: &self.delegate,
            scopes: &self.scopes,
            namespaces: &self.namespaces,
            created_at_ms: self.created_at_ms,
            expires_at_ms: self.expires_at_ms,
            max_queries: self.max_queries,
        };

        let canonical = canonicalize_value(&signable).map_err(AuthError::Core)?;
        self.delegator
            .verify(&canonical, &self.sig)
            .map_err(AuthError::Crypto)
    }

    /// Check if this delegation is currently valid.
    pub fn is_valid(&self, now_ms: u64) -> bool {
        !self.revoked
            && now_ms <= self.expires_at_ms
            && (self.max_queries == 0 || self.queries_used < self.max_queries)
    }

    /// Check if a scope is permitted.
    pub fn has_scope(&self, scope: &DelegationScope) -> bool {
        self.scopes.contains(&DelegationScope::Full) || self.scopes.contains(scope)
    }

    /// Check if this delegation applies to a specific namespace.
    pub fn covers_namespace(&self, namespace: &str) -> bool {
        self.namespaces.is_empty() || self.namespaces.contains(&namespace.to_string())
    }

    /// Record a query (increment counter).
    pub fn record_query(&mut self) {
        self.queries_used += 1;
    }

    /// Revoke this delegation.
    pub fn revoke(&mut self) {
        self.revoked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hellodb_crypto::KeyPair;

    #[test]
    fn create_and_verify() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();

        let deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![
                DelegationScope::ReadNamespace,
                DelegationScope::CrossNamespaceQuery,
            ],
            vec!["ainp.commerce".into()],
            1000,
            3600_000, // 1 hour
            100,
        )
        .unwrap();

        assert!(deleg.verify_signature().is_ok());
        assert!(deleg.is_valid(2000));
        assert!(deleg.has_scope(&DelegationScope::ReadNamespace));
        assert!(deleg.has_scope(&DelegationScope::CrossNamespaceQuery));
        assert!(!deleg.has_scope(&DelegationScope::WriteNamespace));
    }

    #[test]
    fn full_scope_grants_everything() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();

        let deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::Full],
            vec![],
            1000,
            3600_000,
            0,
        )
        .unwrap();

        assert!(deleg.has_scope(&DelegationScope::ReadNamespace));
        assert!(deleg.has_scope(&DelegationScope::WriteNamespace));
        assert!(deleg.has_scope(&DelegationScope::CrossNamespaceQuery));
    }

    #[test]
    fn expired_delegation() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();

        let deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::ReadNamespace],
            vec![],
            1000,
            1000,
            0,
        )
        .unwrap();

        assert!(deleg.is_valid(1500));
        assert!(!deleg.is_valid(3000));
    }

    #[test]
    fn query_limit() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();

        let mut deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::CrossNamespaceQuery],
            vec![],
            1000,
            3600_000,
            2,
        )
        .unwrap();

        assert!(deleg.is_valid(2000));
        deleg.record_query();
        assert!(deleg.is_valid(2000));
        deleg.record_query();
        assert!(!deleg.is_valid(2000)); // exhausted
    }

    #[test]
    fn revocation() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();

        let mut deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::ReadNamespace],
            vec![],
            1000,
            3600_000,
            0,
        )
        .unwrap();

        assert!(deleg.is_valid(2000));
        deleg.revoke();
        assert!(!deleg.is_valid(2000));
    }

    #[test]
    fn tampered_delegation_fails() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();

        let mut deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::ReadNamespace],
            vec![],
            1000,
            3600_000,
            0,
        )
        .unwrap();

        deleg.scopes.push(DelegationScope::Full);
        assert!(deleg.verify_signature().is_err());
    }

    #[test]
    fn namespace_scoped_delegation() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();

        let deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::ReadNamespace],
            vec!["ainp.commerce".into(), "health.vitals".into()],
            1000,
            3600_000,
            0,
        )
        .unwrap();

        assert!(deleg.covers_namespace("ainp.commerce"));
        assert!(deleg.covers_namespace("health.vitals"));
        assert!(!deleg.covers_namespace("finance.transactions"));
    }

    #[test]
    fn empty_namespaces_covers_all() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();

        let deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::ReadNamespace],
            vec![], // empty = all namespaces
            1000,
            3600_000,
            0,
        )
        .unwrap();

        assert!(deleg.covers_namespace("anything"));
    }

    #[test]
    fn serde_roundtrip() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();

        let deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::CrossNamespaceQuery],
            vec!["ainp.commerce".into()],
            1000,
            3600_000,
            50,
        )
        .unwrap();

        let json = serde_json::to_string(&deleg).unwrap();
        let restored: DelegationCredential = serde_json::from_str(&json).unwrap();
        assert!(restored.verify_signature().is_ok());
        assert_eq!(restored.delegation_id, deleg.delegation_id);
    }
}
