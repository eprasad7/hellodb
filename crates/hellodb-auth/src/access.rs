//! Namespace access control gate.
//!
//! Evaluates whether a given identity (app or agent) is allowed
//! to perform a specific operation on a namespace.

use hellodb_core::Namespace;
use hellodb_crypto::VerifyingKey;

use crate::consent::{ConsentAction, ConsentProof};
use crate::delegation::{DelegationCredential, DelegationScope};
use crate::error::AuthError;

/// An access decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessDecision {
    /// Access granted.
    Allowed,
    /// Access denied with reason.
    Denied(String),
}

impl AccessDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, AccessDecision::Allowed)
    }
}

/// The access control gate. Evaluates authorization for namespace operations.
pub struct AccessGate {
    /// Active consent proofs.
    consents: Vec<ConsentProof>,
    /// Active delegation credentials.
    delegations: Vec<DelegationCredential>,
}

impl AccessGate {
    pub fn new() -> Self {
        Self {
            consents: Vec::new(),
            delegations: Vec::new(),
        }
    }

    /// Register a consent proof. Verifies signature before accepting.
    pub fn add_consent(&mut self, consent: ConsentProof) -> Result<(), AuthError> {
        consent.verify()?;
        self.consents.push(consent);
        Ok(())
    }

    /// Register a delegation credential. Verifies signature before accepting.
    pub fn add_delegation(&mut self, delegation: DelegationCredential) -> Result<(), AuthError> {
        delegation.verify_signature()?;
        self.delegations.push(delegation);
        Ok(())
    }

    /// Evaluate whether a requester can read from a namespace.
    ///
    /// Rules:
    /// 1. Namespace owner always has read access.
    /// 2. Non-owner needs a valid ConsentProof (CrossNamespaceRead) or
    ///    DelegationCredential (ReadNamespace scope) targeting this namespace.
    pub fn check_read(
        &self,
        requester: &VerifyingKey,
        namespace: &Namespace,
        now_ms: u64,
    ) -> AccessDecision {
        // Rule 1: Owner always has access
        if namespace.is_owner(requester) {
            return AccessDecision::Allowed;
        }

        // Rule 2a: Check consent proofs
        let requester_b64 = requester.to_base64();
        for consent in &self.consents {
            if consent.action == ConsentAction::CrossNamespaceRead
                && consent.target == requester_b64
                && consent.is_valid(now_ms)
            {
                // Check namespace scope
                match &consent.namespace {
                    Some(ns) if ns == &namespace.id => return AccessDecision::Allowed,
                    None => return AccessDecision::Allowed, // No namespace scope = all
                    _ => {}                                 // Wrong namespace, continue
                }
            }
        }

        // Rule 2b: Check delegations
        for deleg in &self.delegations {
            if deleg.delegate == *requester
                && deleg.is_valid(now_ms)
                && deleg.has_scope(&DelegationScope::ReadNamespace)
                && deleg.covers_namespace(&namespace.id)
            {
                return AccessDecision::Allowed;
            }
        }

        AccessDecision::Denied(format!(
            "no valid consent or delegation for reading namespace '{}'",
            namespace.id
        ))
    }

    /// Evaluate whether a requester can write to a namespace.
    ///
    /// Rules:
    /// 1. Namespace owner always has write access.
    /// 2. Non-owner needs a ConsentProof (GrantWriteAccess) or
    ///    DelegationCredential (WriteNamespace scope).
    pub fn check_write(
        &self,
        requester: &VerifyingKey,
        namespace: &Namespace,
        now_ms: u64,
    ) -> AccessDecision {
        // Rule 1: Owner always has access
        if namespace.is_owner(requester) {
            return AccessDecision::Allowed;
        }

        // Rule 2a: Check consent proofs
        let requester_b64 = requester.to_base64();
        for consent in &self.consents {
            if consent.action == ConsentAction::GrantWriteAccess
                && consent.target == requester_b64
                && consent.is_valid(now_ms)
            {
                match &consent.namespace {
                    Some(ns) if ns == &namespace.id => return AccessDecision::Allowed,
                    None => return AccessDecision::Allowed,
                    _ => {}
                }
            }
        }

        // Rule 2b: Check delegations
        for deleg in &self.delegations {
            if deleg.delegate == *requester
                && deleg.is_valid(now_ms)
                && deleg.has_scope(&DelegationScope::WriteNamespace)
                && deleg.covers_namespace(&namespace.id)
            {
                return AccessDecision::Allowed;
            }
        }

        AccessDecision::Denied(format!(
            "no valid consent or delegation for writing to namespace '{}'",
            namespace.id
        ))
    }

    /// Evaluate whether an agent can query across namespaces.
    ///
    /// Requires DelegationCredential with CrossNamespaceQuery scope
    /// covering all requested namespaces.
    pub fn check_cross_namespace_query(
        &self,
        agent: &VerifyingKey,
        namespaces: &[&Namespace],
        now_ms: u64,
    ) -> AccessDecision {
        for deleg in &self.delegations {
            if deleg.delegate == *agent
                && deleg.is_valid(now_ms)
                && deleg.has_scope(&DelegationScope::CrossNamespaceQuery)
            {
                // Check all requested namespaces are covered
                let all_covered = namespaces.iter().all(|ns| deleg.covers_namespace(&ns.id));
                if all_covered {
                    return AccessDecision::Allowed;
                }
            }
        }

        AccessDecision::Denied("no valid delegation for cross-namespace query".into())
    }

    /// Revoke a delegation by ID.
    pub fn revoke_delegation(&mut self, delegation_id: &str) {
        for deleg in &mut self.delegations {
            if deleg.delegation_id == delegation_id {
                deleg.revoke();
            }
        }
    }

    /// Remove expired consents and delegations.
    pub fn cleanup_expired(&mut self, now_ms: u64) {
        self.consents.retain(|c| c.is_valid(now_ms));
        self.delegations.retain(|d| d.is_valid(now_ms));
    }
}

impl Default for AccessGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::ConsentAction;
    use crate::delegation::DelegationScope;
    use hellodb_crypto::KeyPair;

    fn make_namespace(owner: &KeyPair, id: &str) -> Namespace {
        Namespace {
            id: id.into(),
            name: format!("NS {}", id),
            owner: owner.verifying.clone(),
            description: None,
            created_at_ms: 1000,
            encrypted: true,
            schemas: Vec::new(),
        }
    }

    #[test]
    fn owner_always_has_access() {
        let owner = KeyPair::generate();
        let ns = make_namespace(&owner, "test.ns");
        let gate = AccessGate::new();

        assert!(gate.check_read(&owner.verifying, &ns, 5000).is_allowed());
        assert!(gate.check_write(&owner.verifying, &ns, 5000).is_allowed());
    }

    #[test]
    fn non_owner_denied_without_consent() {
        let owner = KeyPair::generate();
        let other = KeyPair::generate();
        let ns = make_namespace(&owner, "test.ns");
        let gate = AccessGate::new();

        assert!(!gate.check_read(&other.verifying, &ns, 5000).is_allowed());
        assert!(!gate.check_write(&other.verifying, &ns, 5000).is_allowed());
    }

    #[test]
    fn cross_namespace_read_with_consent() {
        let owner = KeyPair::generate();
        let reader = KeyPair::generate();
        let ns = make_namespace(&owner, "ainp.commerce");

        let consent = ConsentProof::new_with_timestamp(
            &owner.signing,
            ConsentAction::CrossNamespaceRead,
            "Grant reader access to ainp.commerce".into(),
            reader.verifying.to_base64(),
            Some("ainp.commerce".into()),
            1000,
            Some(99999),
        )
        .unwrap();

        let mut gate = AccessGate::new();
        gate.add_consent(consent).unwrap();

        assert!(gate.check_read(&reader.verifying, &ns, 5000).is_allowed());
        // Still can't write
        assert!(!gate.check_write(&reader.verifying, &ns, 5000).is_allowed());
    }

    #[test]
    fn expired_consent_denied() {
        let owner = KeyPair::generate();
        let reader = KeyPair::generate();
        let ns = make_namespace(&owner, "test.ns");

        let consent = ConsentProof::new_with_timestamp(
            &owner.signing,
            ConsentAction::CrossNamespaceRead,
            "test".into(),
            reader.verifying.to_base64(),
            None,
            1000,
            Some(2000), // expires at 2000
        )
        .unwrap();

        let mut gate = AccessGate::new();
        gate.add_consent(consent).unwrap();

        assert!(gate.check_read(&reader.verifying, &ns, 1500).is_allowed());
        assert!(!gate.check_read(&reader.verifying, &ns, 3000).is_allowed());
    }

    #[test]
    fn agent_cross_namespace_query_with_delegation() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();
        let ns1 = make_namespace(&user, "ainp.commerce");
        let ns2 = make_namespace(&user, "health.vitals");

        let deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::CrossNamespaceQuery],
            vec!["ainp.commerce".into(), "health.vitals".into()],
            1000,
            3_600_000,
            100,
        )
        .unwrap();

        let mut gate = AccessGate::new();
        gate.add_delegation(deleg).unwrap();

        let decision = gate.check_cross_namespace_query(&agent.verifying, &[&ns1, &ns2], 5000);
        assert!(decision.is_allowed());
    }

    #[test]
    fn delegation_wrong_namespace_denied() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();
        let ns_finance = make_namespace(&user, "finance.tx");

        let deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::CrossNamespaceQuery],
            vec!["ainp.commerce".into()], // only ainp.commerce
            1000,
            3_600_000,
            100,
        )
        .unwrap();

        let mut gate = AccessGate::new();
        gate.add_delegation(deleg).unwrap();

        // Can't query finance
        let decision = gate.check_cross_namespace_query(&agent.verifying, &[&ns_finance], 5000);
        assert!(!decision.is_allowed());
    }

    #[test]
    fn revoked_delegation_denied() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();
        let ns = make_namespace(&user, "test.ns");

        let deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::ReadNamespace],
            vec![],
            1000,
            3_600_000,
            0,
        )
        .unwrap();
        let deleg_id = deleg.delegation_id.clone();

        let mut gate = AccessGate::new();
        gate.add_delegation(deleg).unwrap();

        assert!(gate.check_read(&agent.verifying, &ns, 5000).is_allowed());

        gate.revoke_delegation(&deleg_id);

        assert!(!gate.check_read(&agent.verifying, &ns, 5000).is_allowed());
    }

    #[test]
    fn write_access_with_delegation() {
        let user = KeyPair::generate();
        let app = KeyPair::generate();
        let ns = make_namespace(&user, "test.ns");

        let deleg = DelegationCredential::new(
            &user.signing,
            app.verifying.clone(),
            vec![DelegationScope::WriteNamespace],
            vec!["test.ns".into()],
            1000,
            3_600_000,
            0,
        )
        .unwrap();

        let mut gate = AccessGate::new();
        gate.add_delegation(deleg).unwrap();

        assert!(gate.check_write(&app.verifying, &ns, 5000).is_allowed());
    }

    #[test]
    fn cleanup_expired() {
        let user = KeyPair::generate();
        let agent = KeyPair::generate();

        let deleg = DelegationCredential::new(
            &user.signing,
            agent.verifying.clone(),
            vec![DelegationScope::ReadNamespace],
            vec![],
            1000,
            1000, // expires at 2000
            0,
        )
        .unwrap();

        let mut gate = AccessGate::new();
        gate.add_delegation(deleg).unwrap();

        assert_eq!(gate.delegations.len(), 1);
        gate.cleanup_expired(3000);
        assert_eq!(gate.delegations.len(), 0);
    }
}
