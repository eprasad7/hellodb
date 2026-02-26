//! hellodb Phase 0 Integration Test
//!
//! End-to-end scenario demonstrating the full sovereignty pipeline:
//!
//! 1. Device key generation + namespace key derivation
//! 2. Namespace registration with schemas
//! 3. Record creation, content addressing, and signature verification
//! 4. Git-like branching and merging
//! 5. Access denial without consent (namespace isolation)
//! 6. Consent-based cross-namespace read
//! 7. Delegation-based agent cross-namespace query
//! 8. Revocation and expiry

use hellodb_auth::{
    AccessGate, ConsentAction, ConsentProof, DelegationCredential, DelegationScope,
};
use hellodb_core::{
    Branch, FieldType, Namespace, Record, Schema, SchemaField, SchemaRegistry,
};
use hellodb_crypto::{KeyPair, MasterKey};
use hellodb_storage::{MemoryEngine, SqliteEngine, StorageEngine};

use serde_json::json;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn listing_schema() -> Schema {
    Schema {
        id: "app_a.commerce.listing".into(),
        version: "1".into(),
        namespace: "app_a.commerce".into(),
        name: "Listing".into(),
        fields: vec![
            SchemaField {
                name: "title".into(),
                field_type: FieldType::String,
                required: true,
                description: None,
            },
            SchemaField {
                name: "price".into(),
                field_type: FieldType::Float,
                required: true,
                description: None,
            },
            SchemaField {
                name: "currency".into(),
                field_type: FieldType::String,
                required: true,
                description: None,
            },
            SchemaField {
                name: "description".into(),
                field_type: FieldType::Optional(Box::new(FieldType::String)),
                required: false,
                description: None,
            },
        ],
        registered_at_ms: 1000,
    }
}

fn health_schema() -> Schema {
    Schema {
        id: "health.vitals.heart_rate".into(),
        version: "1".into(),
        namespace: "health.vitals".into(),
        name: "Heart Rate".into(),
        fields: vec![
            SchemaField {
                name: "bpm".into(),
                field_type: FieldType::Integer,
                required: true,
                description: None,
            },
            SchemaField {
                name: "measured_at".into(),
                field_type: FieldType::Timestamp,
                required: true,
                description: None,
            },
            SchemaField {
                name: "device".into(),
                field_type: FieldType::String,
                required: false,
                description: None,
            },
        ],
        registered_at_ms: 1000,
    }
}

/// Run the full Phase 0 integration scenario against a given StorageEngine.
fn run_full_scenario(engine: &mut dyn StorageEngine) {
    // -----------------------------------------------------------------------
    // Step 1: Device identity + key derivation
    // -----------------------------------------------------------------------
    let device = KeyPair::generate();
    let master = MasterKey::generate();
    let commerce_ns_key = master.derive_namespace_key("app_a.commerce");
    let health_ns_key = master.derive_namespace_key("health.vitals");

    // Different namespaces derive different keys
    assert_ne!(
        commerce_ns_key.to_bytes(),
        health_ns_key.to_bytes(),
        "namespace keys must be distinct"
    );

    // Verify encryption works with the namespace key
    let secret = b"commerce data payload";
    let ct = commerce_ns_key.encrypt(secret);
    let pt = commerce_ns_key.decrypt(&ct).unwrap();
    assert_eq!(pt, secret);

    // Wrong namespace key cannot decrypt
    assert!(health_ns_key.decrypt(&ct).is_err());

    // -----------------------------------------------------------------------
    // Step 2: Namespace + schema registration
    // -----------------------------------------------------------------------
    let mut commerce_ns = Namespace::new(
        "app_a.commerce".into(),
        "Commerce App".into(),
        device.verifying.clone(),
        true,
    );
    commerce_ns.created_at_ms = 1000;

    let mut health_ns = Namespace::new(
        "health.vitals".into(),
        "Health Vitals".into(),
        device.verifying.clone(),
        true,
    );
    health_ns.created_at_ms = 1000;

    engine.create_namespace(commerce_ns.clone()).unwrap();
    engine.create_namespace(health_ns.clone()).unwrap();

    // Register schemas
    let listing_schema = listing_schema();
    let heart_schema = health_schema();

    let mut registry = SchemaRegistry::new();
    registry.register(listing_schema.clone()).unwrap();
    registry.register(heart_schema.clone()).unwrap();

    engine.register_schema(listing_schema).unwrap();
    engine.register_schema(heart_schema).unwrap();

    // Verify schema lookup
    assert!(engine.get_schema("app_a.commerce.listing").unwrap().is_some());
    assert!(engine.get_schema("health.vitals.heart_rate").unwrap().is_some());
    assert!(engine.get_schema("nonexistent").unwrap().is_none());

    // Main branches are auto-created by create_namespace -- just get IDs
    let commerce_main_id = format!("{}/main", "app_a.commerce");
    let health_main_id = format!("{}/main", "health.vitals");

    // -----------------------------------------------------------------------
    // Step 3: Record creation + content addressing
    // -----------------------------------------------------------------------
    let listing_data = json!({
        "title": "Handmade Ceramic Bowl",
        "price": 24.99,
        "currency": "USD",
        "description": "Beautiful hand-thrown ceramic bowl"
    });

    // Validate against schema first
    registry
        .validate_data("app_a.commerce.listing", &listing_data)
        .unwrap();

    let listing = Record::new_with_timestamp(
        &device.signing,
        "app_a.commerce.listing".into(),
        "app_a.commerce".into(),
        listing_data.clone(),
        None,
        1000,
    )
    .unwrap();

    // Content-addressed: record_id is deterministic from content
    assert!(!listing.record_id.is_empty());
    assert!(listing.verify().is_ok(), "record signature must verify");

    // Store it
    engine
        .put_record(listing.clone(), &commerce_main_id)
        .unwrap();

    // Retrieve it
    let fetched = engine
        .get_record(&listing.record_id, &commerce_main_id)
        .unwrap()
        .expect("record must exist");
    assert_eq!(fetched.record_id, listing.record_id);
    assert_eq!(fetched.data, listing_data);
    assert!(fetched.verify().is_ok());

    // Write a health record
    let hr_data = json!({
        "bpm": 72,
        "measured_at": 1000,
        "device": "AppleWatch"
    });
    registry
        .validate_data("health.vitals.heart_rate", &hr_data)
        .unwrap();

    let hr_record = Record::new_with_timestamp(
        &device.signing,
        "health.vitals.heart_rate".into(),
        "health.vitals".into(),
        hr_data,
        None,
        1100,
    )
    .unwrap();
    engine.put_record(hr_record, &health_main_id).unwrap();

    // -----------------------------------------------------------------------
    // Step 4: Branching and merging
    // -----------------------------------------------------------------------
    let draft = Branch::new(
        "app_a.commerce/draft-batch".into(),
        "app_a.commerce".into(),
        commerce_main_id.clone(),
        "Draft Batch Upload".into(),
    );
    engine.create_branch(draft.clone()).unwrap();

    // Write two records to draft branch
    let item2 = Record::new_with_timestamp(
        &device.signing,
        "app_a.commerce.listing".into(),
        "app_a.commerce".into(),
        json!({
            "title": "Organic Honey Jar",
            "price": 12.50,
            "currency": "USD"
        }),
        None,
        2000,
    )
    .unwrap();

    let item3 = Record::new_with_timestamp(
        &device.signing,
        "app_a.commerce.listing".into(),
        "app_a.commerce".into(),
        json!({
            "title": "Woven Basket Set",
            "price": 39.99,
            "currency": "USD"
        }),
        None,
        2001,
    )
    .unwrap();

    engine.put_record(item2, &draft.id).unwrap();
    engine.put_record(item3, &draft.id).unwrap();

    // Draft branch inherits parent records -- should see 3 total
    let draft_records = engine
        .list_records_by_namespace("app_a.commerce", &draft.id, 100, 0)
        .unwrap();
    assert_eq!(
        draft_records.len(),
        3,
        "draft branch should see parent records + its own"
    );

    // Main branch still only has 1
    let main_records = engine
        .list_records_by_namespace("app_a.commerce", &commerce_main_id, 100, 0)
        .unwrap();
    assert_eq!(main_records.len(), 1, "main should still have 1 record");

    // Merge draft -> main
    engine.merge_branch(&draft.id).unwrap();

    // Now main should have all 3
    let main_after_merge = engine
        .list_records_by_namespace("app_a.commerce", &commerce_main_id, 100, 0)
        .unwrap();
    assert_eq!(
        main_after_merge.len(),
        3,
        "main should have 3 records after merge"
    );

    // -----------------------------------------------------------------------
    // Step 5: Access denial without consent (namespace isolation)
    // -----------------------------------------------------------------------
    let app_b = KeyPair::generate(); // Different app, different keys
    let gate = AccessGate::new();

    let decision = gate.check_read(&app_b.verifying, &commerce_ns, 5000);
    assert!(
        !decision.is_allowed(),
        "App B must be denied read without consent"
    );

    let decision = gate.check_write(&app_b.verifying, &commerce_ns, 5000);
    assert!(
        !decision.is_allowed(),
        "App B must be denied write without consent"
    );

    // Owner always has access
    let decision = gate.check_read(&device.verifying, &commerce_ns, 5000);
    assert!(decision.is_allowed(), "Owner always has read access");

    let decision = gate.check_write(&device.verifying, &commerce_ns, 5000);
    assert!(decision.is_allowed(), "Owner always has write access");

    // -----------------------------------------------------------------------
    // Step 6: Consent-based cross-namespace read
    // -----------------------------------------------------------------------
    let consent = ConsentProof::new_with_timestamp(
        &device.signing,
        ConsentAction::CrossNamespaceRead,
        "Grant App B read access to app_a.commerce".into(),
        app_b.verifying.to_base64(),
        Some("app_a.commerce".into()),
        1000,
        Some(99999),
    )
    .unwrap();

    assert!(consent.verify().is_ok(), "consent signature must verify");

    let mut gate = AccessGate::new();
    gate.add_consent(consent).unwrap();

    // Now App B CAN read commerce namespace
    let decision = gate.check_read(&app_b.verifying, &commerce_ns, 5000);
    assert!(
        decision.is_allowed(),
        "App B should be allowed to read with consent"
    );

    // But still CANNOT write
    let decision = gate.check_write(&app_b.verifying, &commerce_ns, 5000);
    assert!(
        !decision.is_allowed(),
        "App B should NOT be able to write with CrossNamespaceRead consent"
    );

    // And CANNOT read health namespace (consent is scoped to commerce)
    let decision = gate.check_read(&app_b.verifying, &health_ns, 5000);
    assert!(
        !decision.is_allowed(),
        "App B should NOT be able to read health namespace"
    );

    // -----------------------------------------------------------------------
    // Step 7: Delegation-based agent cross-namespace query
    // -----------------------------------------------------------------------
    let agent = KeyPair::generate();

    let delegation = DelegationCredential::new(
        &device.signing,
        agent.verifying.clone(),
        vec![DelegationScope::CrossNamespaceQuery, DelegationScope::ReadNamespace],
        vec!["app_a.commerce".into(), "health.vitals".into()],
        1000,
        3600_000, // 1 hour
        100,      // max 100 queries
    )
    .unwrap();

    assert!(
        delegation.verify_signature().is_ok(),
        "delegation signature must verify"
    );
    assert!(delegation.is_valid(5000));
    assert!(delegation.has_scope(&DelegationScope::CrossNamespaceQuery));
    assert!(delegation.has_scope(&DelegationScope::ReadNamespace));
    assert!(!delegation.has_scope(&DelegationScope::WriteNamespace));

    let mut agent_gate = AccessGate::new();
    agent_gate.add_delegation(delegation).unwrap();

    // Agent can query across both namespaces
    let decision = agent_gate.check_cross_namespace_query(
        &agent.verifying,
        &[&commerce_ns, &health_ns],
        5000,
    );
    assert!(
        decision.is_allowed(),
        "Agent should be allowed cross-namespace query"
    );

    // Agent can read individual namespaces too
    assert!(
        agent_gate
            .check_read(&agent.verifying, &commerce_ns, 5000)
            .is_allowed(),
        "Agent should be able to read commerce"
    );
    assert!(
        agent_gate
            .check_read(&agent.verifying, &health_ns, 5000)
            .is_allowed(),
        "Agent should be able to read health"
    );

    // Agent CANNOT write (no WriteNamespace scope)
    assert!(
        !agent_gate
            .check_write(&agent.verifying, &commerce_ns, 5000)
            .is_allowed(),
        "Agent should NOT be able to write"
    );

    // Agent CANNOT query a namespace not in the delegation
    let finance_ns = Namespace::new(
        "finance.tx".into(),
        "Finance".into(),
        device.verifying.clone(),
        true,
    );
    let decision = agent_gate.check_cross_namespace_query(
        &agent.verifying,
        &[&commerce_ns, &finance_ns],
        5000,
    );
    assert!(
        !decision.is_allowed(),
        "Agent should be denied query including uncovered namespace"
    );

    // -----------------------------------------------------------------------
    // Step 8: Revocation and expiry
    // -----------------------------------------------------------------------

    // Expired consent is denied
    let expired_consent = ConsentProof::new_with_timestamp(
        &device.signing,
        ConsentAction::CrossNamespaceRead,
        "Short-lived consent".into(),
        app_b.verifying.to_base64(),
        None,
        1000,
        Some(2000), // expires at 2000
    )
    .unwrap();

    let mut expiry_gate = AccessGate::new();
    expiry_gate.add_consent(expired_consent).unwrap();

    assert!(
        expiry_gate
            .check_read(&app_b.verifying, &commerce_ns, 1500)
            .is_allowed(),
        "Consent should be valid before expiry"
    );
    assert!(
        !expiry_gate
            .check_read(&app_b.verifying, &commerce_ns, 3000)
            .is_allowed(),
        "Consent should be denied after expiry"
    );

    // Revoked delegation is denied
    let revocable_deleg = DelegationCredential::new(
        &device.signing,
        agent.verifying.clone(),
        vec![DelegationScope::ReadNamespace],
        vec![],
        1000,
        3600_000,
        0,
    )
    .unwrap();
    let deleg_id = revocable_deleg.delegation_id.clone();

    let mut revoke_gate = AccessGate::new();
    revoke_gate.add_delegation(revocable_deleg).unwrap();

    assert!(
        revoke_gate
            .check_read(&agent.verifying, &commerce_ns, 5000)
            .is_allowed(),
        "Delegation should work before revocation"
    );

    revoke_gate.revoke_delegation(&deleg_id);

    assert!(
        !revoke_gate
            .check_read(&agent.verifying, &commerce_ns, 5000)
            .is_allowed(),
        "Delegation should be denied after revocation"
    );

    // Cleanup expired
    expiry_gate.cleanup_expired(5000);

    // -----------------------------------------------------------------------
    // Step 9: Record versioning (previous_version chain)
    // -----------------------------------------------------------------------
    let updated_listing = Record::new_with_timestamp(
        &device.signing,
        "app_a.commerce.listing".into(),
        "app_a.commerce".into(),
        json!({
            "title": "Handmade Ceramic Bowl",
            "price": 29.99,
            "currency": "USD",
            "description": "Beautiful hand-thrown ceramic bowl — price updated"
        }),
        Some(listing.record_id.clone()),
        3000,
    )
    .unwrap();

    assert_ne!(
        updated_listing.record_id, listing.record_id,
        "updated record should have different content hash"
    );
    assert_eq!(
        updated_listing.previous_version,
        Some(listing.record_id.clone()),
        "should reference the previous version"
    );
    assert!(updated_listing.verify().is_ok());

    engine
        .put_record(updated_listing, &commerce_main_id)
        .unwrap();

    // -----------------------------------------------------------------------
    // Step 10: Counts and queries
    // -----------------------------------------------------------------------
    let listing_count = engine
        .count_records_by_schema("app_a.commerce.listing", &commerce_main_id)
        .unwrap();
    assert_eq!(
        listing_count, 4,
        "should have 4 listing records on main (3 from merge + 1 updated)"
    );

    let health_count = engine
        .count_records_by_schema("health.vitals.heart_rate", &health_main_id)
        .unwrap();
    assert_eq!(health_count, 1, "should have 1 health record");

    // List namespaces
    let all_ns = engine.list_namespaces().unwrap();
    assert_eq!(all_ns.len(), 2);

    // List schemas for a namespace
    let commerce_schemas = engine.list_schemas("app_a.commerce").unwrap();
    assert_eq!(commerce_schemas.len(), 1);
    assert_eq!(commerce_schemas[0].id, "app_a.commerce.listing");
}

// ---------------------------------------------------------------------------
// Tests: Run the full scenario against both engines
// ---------------------------------------------------------------------------

#[test]
fn integration_memory_engine() {
    let mut engine = MemoryEngine::new();
    run_full_scenario(&mut engine);
}

#[test]
fn integration_sqlite_engine() {
    let mut engine = SqliteEngine::open_in_memory().unwrap();
    run_full_scenario(&mut engine);
}

// ---------------------------------------------------------------------------
// Focused test: Write access delegation
// ---------------------------------------------------------------------------

#[test]
fn integration_write_delegation() {
    let user = KeyPair::generate();
    let app = KeyPair::generate();

    let ns = Namespace::new(
        "shared.notes".into(),
        "Shared Notes".into(),
        user.verifying.clone(),
        false,
    );

    // Grant write access via delegation
    let deleg = DelegationCredential::new(
        &user.signing,
        app.verifying.clone(),
        vec![DelegationScope::WriteNamespace, DelegationScope::ReadNamespace],
        vec!["shared.notes".into()],
        1000,
        3600_000,
        0,
    )
    .unwrap();

    let mut gate = AccessGate::new();
    gate.add_delegation(deleg).unwrap();

    assert!(gate.check_write(&app.verifying, &ns, 5000).is_allowed());
    assert!(gate.check_read(&app.verifying, &ns, 5000).is_allowed());
}

// ---------------------------------------------------------------------------
// Focused test: Write access via consent
// ---------------------------------------------------------------------------

#[test]
fn integration_write_consent() {
    let owner = KeyPair::generate();
    let writer = KeyPair::generate();

    let ns = Namespace::new(
        "collab.doc".into(),
        "Collab Doc".into(),
        owner.verifying.clone(),
        true,
    );

    let consent = ConsentProof::new_with_timestamp(
        &owner.signing,
        ConsentAction::GrantWriteAccess,
        "Grant writer write access to collab.doc".into(),
        writer.verifying.to_base64(),
        Some("collab.doc".into()),
        1000,
        Some(99999),
    )
    .unwrap();

    let mut gate = AccessGate::new();
    gate.add_consent(consent).unwrap();

    assert!(gate.check_write(&writer.verifying, &ns, 5000).is_allowed());
    // CrossNamespaceRead was not granted, so read should fail
    assert!(!gate.check_read(&writer.verifying, &ns, 5000).is_allowed());
}

// ---------------------------------------------------------------------------
// Focused test: Namespace key isolation
// ---------------------------------------------------------------------------

#[test]
fn integration_namespace_key_isolation() {
    let master = MasterKey::generate();

    let key_a = master.derive_namespace_key("app.notes");
    let key_b = master.derive_namespace_key("app.photos");

    let plaintext = b"sensitive notes data";
    let encrypted = key_a.encrypt(plaintext);

    // Correct key decrypts
    assert_eq!(key_a.decrypt(&encrypted).unwrap(), plaintext);

    // Wrong namespace key fails
    assert!(key_b.decrypt(&encrypted).is_err());

    // Different master key, same namespace name -> different key
    let master2 = MasterKey::generate();
    let key_a2 = master2.derive_namespace_key("app.notes");
    assert_ne!(key_a.to_bytes(), key_a2.to_bytes());
    assert!(key_a2.decrypt(&encrypted).is_err());
}

// ---------------------------------------------------------------------------
// Focused test: Delegation query limit enforcement
// ---------------------------------------------------------------------------

#[test]
fn integration_delegation_query_limit() {
    let user = KeyPair::generate();
    let _agent = KeyPair::generate();

    let mut deleg = DelegationCredential::new(
        &user.signing,
        _agent.verifying.clone(),
        vec![DelegationScope::ReadNamespace],
        vec![],
        1000,
        3600_000,
        3, // max 3 queries
    )
    .unwrap();

    assert!(deleg.is_valid(5000));
    deleg.record_query();
    assert!(deleg.is_valid(5000));
    deleg.record_query();
    assert!(deleg.is_valid(5000));
    deleg.record_query();
    assert!(
        !deleg.is_valid(5000),
        "delegation should be exhausted after 3 queries"
    );
}

// ---------------------------------------------------------------------------
// Focused test: Schema validation rejects bad data
// ---------------------------------------------------------------------------

#[test]
fn integration_schema_validation() {
    let mut registry = SchemaRegistry::new();
    registry.register(listing_schema()).unwrap();

    // Valid
    let valid = json!({
        "title": "Widget",
        "price": 9.99,
        "currency": "EUR"
    });
    assert!(registry.validate_data("app_a.commerce.listing", &valid).is_ok());

    // Missing required field
    let missing_price = json!({
        "title": "Widget",
        "currency": "EUR"
    });
    assert!(registry
        .validate_data("app_a.commerce.listing", &missing_price)
        .is_err());

    // Wrong type (price should be float, not string)
    let wrong_type = json!({
        "title": "Widget",
        "price": "not a number",
        "currency": "EUR"
    });
    assert!(registry
        .validate_data("app_a.commerce.listing", &wrong_type)
        .is_err());

    // Unknown schema
    assert!(registry.validate_data("nonexistent.schema", &valid).is_err());
}

// ---------------------------------------------------------------------------
// Focused test: Record tamper detection
// ---------------------------------------------------------------------------

#[test]
fn integration_record_tamper_detection() {
    let device = KeyPair::generate();

    let mut record = Record::new_with_timestamp(
        &device.signing,
        "test.schema".into(),
        "test.ns".into(),
        json!({"key": "value"}),
        None,
        1000,
    )
    .unwrap();

    // Unmodified record verifies
    assert!(record.verify().is_ok());

    // Tamper with data
    record.data = json!({"key": "tampered"});
    assert!(
        record.verify().is_err(),
        "tampered record must fail verification"
    );
}
