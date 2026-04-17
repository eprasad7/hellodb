//! Query engine — executes queries against StorageEngine with access control.
//!
//! The QueryEngine is the "inverse SQL warehouse": instead of a cloud cluster
//! processing your query, it runs entirely on YOUR device against YOUR data.

use hellodb_auth::AccessGate;
use hellodb_core::{Namespace, Record};
use hellodb_crypto::VerifyingKey;
use hellodb_storage::StorageEngine;

use crate::cursor::Cursor;
use crate::error::QueryError;
use crate::query::{Query, QueryResult};
use crate::sort::apply_sort;

/// The query engine. Wraps a StorageEngine and AccessGate to execute
/// access-controlled queries against hellodb records.
pub struct QueryEngine<'a> {
    storage: &'a dyn StorageEngine,
    access: &'a AccessGate,
}

impl<'a> QueryEngine<'a> {
    /// Create a new query engine.
    pub fn new(storage: &'a dyn StorageEngine, access: &'a AccessGate) -> Self {
        Self { storage, access }
    }

    /// Execute a query within a single namespace on a specific branch.
    ///
    /// Checks read access for the requester before querying.
    pub fn execute(
        &self,
        query: &Query,
        requester: &VerifyingKey,
        branch: &str,
        now_ms: u64,
    ) -> Result<QueryResult, QueryError> {
        // Determine namespace from query or branch
        let namespace_id = query
            .namespace
            .as_deref()
            .or_else(|| branch.split('/').next())
            .ok_or_else(|| {
                QueryError::NamespaceNotFound("no namespace in query or branch".into())
            })?;

        // Access check
        let namespace = self
            .storage
            .get_namespace(namespace_id)?
            .ok_or_else(|| QueryError::NamespaceNotFound(namespace_id.into()))?;

        let decision = self.access.check_read(requester, &namespace, now_ms);
        if !decision.is_allowed() {
            return Err(QueryError::AccessDenied(format!(
                "read access denied for namespace '{}'",
                namespace_id
            )));
        }

        // Fetch + filter + sort + paginate
        let candidates = self.fetch_candidates(query, branch)?;
        self.filter_sort_paginate(candidates, query)
    }

    /// Execute a query across multiple namespaces (agent cross-namespace query).
    ///
    /// Checks delegation-based access for each namespace. Results are merged
    /// across namespaces, sorted, and paginated.
    pub fn execute_cross_namespace(
        &self,
        query: &Query,
        agent: &VerifyingKey,
        namespaces: &[(&str, &str)], // (namespace_id, branch_id) pairs
        now_ms: u64,
    ) -> Result<QueryResult, QueryError> {
        // Collect Namespace objects for access check
        let mut ns_objects: Vec<Namespace> = Vec::new();
        for &(ns_id, _) in namespaces {
            let ns = self
                .storage
                .get_namespace(ns_id)?
                .ok_or_else(|| QueryError::NamespaceNotFound(ns_id.into()))?;
            ns_objects.push(ns);
        }

        // Check cross-namespace access
        let ns_refs: Vec<&Namespace> = ns_objects.iter().collect();
        let decision = self
            .access
            .check_cross_namespace_query(agent, &ns_refs, now_ms);
        if !decision.is_allowed() {
            return Err(QueryError::AccessDenied(
                "cross-namespace query denied".into(),
            ));
        }

        // Fetch candidates from all namespaces
        let mut all_candidates = Vec::new();
        for &(ns_id, branch_id) in namespaces {
            let ns_query = Query {
                namespace: Some(ns_id.to_string()),
                schema: query.schema.clone(),
                filter: query.filter.clone(),
                sort: Vec::new(), // sort after merge
                limit: usize::MAX,
                after: None,
                offset: 0,
            };
            let candidates = self.fetch_candidates(&ns_query, branch_id)?;
            all_candidates.extend(candidates);
        }

        // Filter + sort + paginate the merged set
        self.filter_sort_paginate(all_candidates, query)
    }

    /// Count matching records without fetching them all.
    pub fn count(
        &self,
        query: &Query,
        requester: &VerifyingKey,
        branch: &str,
        now_ms: u64,
    ) -> Result<u64, QueryError> {
        // Same access check as execute
        let namespace_id = query
            .namespace
            .as_deref()
            .or_else(|| branch.split('/').next())
            .ok_or_else(|| {
                QueryError::NamespaceNotFound("no namespace in query or branch".into())
            })?;

        let namespace = self
            .storage
            .get_namespace(namespace_id)?
            .ok_or_else(|| QueryError::NamespaceNotFound(namespace_id.into()))?;

        let decision = self.access.check_read(requester, &namespace, now_ms);
        if !decision.is_allowed() {
            return Err(QueryError::AccessDenied(format!(
                "read access denied for namespace '{}'",
                namespace_id
            )));
        }

        // If no filter, use the optimized count
        if query.filter.is_none() {
            if let Some(schema) = &query.schema {
                return Ok(self.storage.count_records_by_schema(schema, branch)?);
            }
        }

        // Otherwise, fetch and count matching
        let candidates = self.fetch_candidates(query, branch)?;
        let filtered: Vec<_> = candidates
            .into_iter()
            .filter(|r| query.filter.as_ref().is_none_or(|f| f.matches(r)))
            .collect();
        Ok(filtered.len() as u64)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Fetch candidate records from storage based on query scope.
    fn fetch_candidates(&self, query: &Query, branch: &str) -> Result<Vec<Record>, QueryError> {
        // Use the most specific index available
        let records = if let Some(schema) = &query.schema {
            self.storage
                .list_records_by_schema(schema, branch, usize::MAX, 0)?
        } else if let Some(namespace) = &query.namespace {
            self.storage
                .list_records_by_namespace(namespace, branch, usize::MAX, 0)?
        } else {
            // Fallback: infer namespace from branch ID (format: "namespace/branch_name")
            let namespace = branch.split('/').next().unwrap_or(branch);
            self.storage
                .list_records_by_namespace(namespace, branch, usize::MAX, 0)?
        };

        Ok(records)
    }

    /// Apply filter, sort, and pagination to a set of candidate records.
    fn filter_sort_paginate(
        &self,
        candidates: Vec<Record>,
        query: &Query,
    ) -> Result<QueryResult, QueryError> {
        // Step 1: Apply filter
        let mut filtered: Vec<Record> = if let Some(ref filter) = query.filter {
            candidates
                .into_iter()
                .filter(|r| filter.matches(r))
                .collect()
        } else {
            candidates
        };

        let total_count = filtered.len() as u64;

        // Step 2: Sort
        apply_sort(&mut filtered, &query.sort);

        // Step 3: Apply cursor (skip records until we pass the cursor position)
        if let Some(ref cursor) = query.after {
            let cursor_pos = filtered
                .iter()
                .position(|r| r.record_id == cursor.record_id);
            if let Some(pos) = cursor_pos {
                filtered = filtered.split_off(pos + 1);
            }
            // If cursor record not found, start from beginning
        }

        // Step 4: Apply offset
        if query.offset > 0 && query.offset < filtered.len() {
            filtered = filtered.split_off(query.offset);
        } else if query.offset >= filtered.len() {
            filtered.clear();
        }

        // Step 5: Apply limit
        let has_more = filtered.len() > query.limit;
        filtered.truncate(query.limit);

        // Step 6: Generate next cursor from last record
        let next_cursor = if has_more {
            filtered
                .last()
                .map(|r| Cursor::new(r.record_id.clone(), r.created_at_ms))
        } else {
            None
        };

        Ok(QueryResult {
            records: filtered,
            total_count,
            next_cursor,
            has_more,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::Filter;
    use crate::sort::SortField;
    use hellodb_auth::{
        AccessGate, ConsentAction, ConsentProof, DelegationCredential, DelegationScope,
    };
    use hellodb_core::{FieldType, Namespace, Record, Schema, SchemaField};
    use hellodb_crypto::KeyPair;
    use hellodb_storage::MemoryEngine;
    use serde_json::json;

    /// Set up a test environment with namespaces, schemas, and records.
    fn setup() -> (MemoryEngine, KeyPair, KeyPair, KeyPair) {
        let mut engine = MemoryEngine::new();
        let owner = KeyPair::generate();
        let app_b = KeyPair::generate();
        let agent = KeyPair::generate();

        // Create namespace + schema
        let ns = Namespace::new(
            "commerce".into(),
            "Commerce".into(),
            owner.verifying.clone(),
            false,
        );
        engine.create_namespace(ns).unwrap();

        let schema = Schema {
            id: "commerce.listing".into(),
            version: "1".into(),
            namespace: "commerce".into(),
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
            ],
            registered_at_ms: 1000,
        };
        engine.register_schema(schema).unwrap();

        // Write test records
        let listings = vec![
            json!({"title": "Ceramic Bowl", "price": 24.99, "currency": "USD"}),
            json!({"title": "Honey Jar", "price": 12.50, "currency": "USD"}),
            json!({"title": "Woven Basket", "price": 39.99, "currency": "EUR"}),
            json!({"title": "Glass Vase", "price": 55.00, "currency": "USD"}),
            json!({"title": "Candle Set", "price": 18.75, "currency": "EUR"}),
        ];

        for (i, data) in listings.into_iter().enumerate() {
            let rec = Record::new_with_timestamp(
                &owner.signing,
                "commerce.listing".into(),
                "commerce".into(),
                data,
                None,
                1000 + i as u64,
            )
            .unwrap();
            engine.put_record(rec, "commerce/main").unwrap();
        }

        (engine, owner, app_b, agent)
    }

    #[test]
    fn basic_query_no_filter() {
        let (engine, owner, _, _) = setup();
        let gate = AccessGate::new();
        let qe = QueryEngine::new(&engine, &gate);

        let result = qe
            .execute(
                &Query::new().namespace("commerce"),
                &owner.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();

        assert_eq!(result.records.len(), 5);
        assert_eq!(result.total_count, 5);
        assert!(!result.has_more);
    }

    #[test]
    fn filtered_query() {
        let (engine, owner, _, _) = setup();
        let gate = AccessGate::new();
        let qe = QueryEngine::new(&engine, &gate);

        let result = qe
            .execute(
                &Query::new()
                    .namespace("commerce")
                    .filter(Filter::Eq("currency".into(), json!("USD"))),
                &owner.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();

        assert_eq!(result.records.len(), 3);
        for rec in &result.records {
            assert_eq!(rec.data["currency"], "USD");
        }
    }

    #[test]
    fn sorted_query() {
        let (engine, owner, _, _) = setup();
        let gate = AccessGate::new();
        let qe = QueryEngine::new(&engine, &gate);

        let result = qe
            .execute(
                &Query::new()
                    .namespace("commerce")
                    .sort(SortField::asc("price")),
                &owner.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();

        let prices: Vec<f64> = result
            .records
            .iter()
            .map(|r| r.data["price"].as_f64().unwrap())
            .collect();
        assert_eq!(prices, vec![12.5, 18.75, 24.99, 39.99, 55.0]);
    }

    #[test]
    fn paginated_query() {
        let (engine, owner, _, _) = setup();
        let gate = AccessGate::new();
        let qe = QueryEngine::new(&engine, &gate);

        // Page 1
        let page1 = qe
            .execute(
                &Query::new()
                    .namespace("commerce")
                    .sort(SortField::asc("price"))
                    .limit(2),
                &owner.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();

        assert_eq!(page1.records.len(), 2);
        assert!(page1.has_more);
        assert!(page1.next_cursor.is_some());

        // Page 2
        let page2 = qe
            .execute(
                &Query::new()
                    .namespace("commerce")
                    .sort(SortField::asc("price"))
                    .limit(2)
                    .after(page1.next_cursor.unwrap()),
                &owner.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();

        assert_eq!(page2.records.len(), 2);
        // Should not overlap with page 1
        assert_ne!(page1.records[0].record_id, page2.records[0].record_id,);
    }

    #[test]
    fn access_denied_without_consent() {
        let (engine, _, app_b, _) = setup();
        let gate = AccessGate::new();
        let qe = QueryEngine::new(&engine, &gate);

        let result = qe.execute(
            &Query::new().namespace("commerce"),
            &app_b.verifying,
            "commerce/main",
            5000,
        );

        assert!(result.is_err());
        assert!(matches!(result, Err(QueryError::AccessDenied(_))));
    }

    #[test]
    fn access_with_consent() {
        let (engine, owner, app_b, _) = setup();
        let mut gate = AccessGate::new();

        let consent = ConsentProof::new_with_timestamp(
            &owner.signing,
            ConsentAction::CrossNamespaceRead,
            "Grant read".into(),
            app_b.verifying.to_base64(),
            Some("commerce".into()),
            1000,
            Some(99999),
        )
        .unwrap();
        gate.add_consent(consent).unwrap();

        let qe = QueryEngine::new(&engine, &gate);
        let result = qe
            .execute(
                &Query::new().namespace("commerce"),
                &app_b.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();

        assert_eq!(result.records.len(), 5);
    }

    #[test]
    fn cross_namespace_query_with_delegation() {
        let (mut engine, owner, _, agent) = setup();

        // Create second namespace
        let health_ns = Namespace::new(
            "health".into(),
            "Health".into(),
            owner.verifying.clone(),
            false,
        );
        engine.create_namespace(health_ns).unwrap();

        let hr = Record::new_with_timestamp(
            &owner.signing,
            "health.vitals".into(),
            "health".into(),
            json!({"bpm": 72, "device": "watch"}),
            None,
            2000,
        )
        .unwrap();
        engine.put_record(hr, "health/main").unwrap();

        // Delegation
        let deleg = DelegationCredential::new(
            &owner.signing,
            agent.verifying.clone(),
            vec![
                DelegationScope::CrossNamespaceQuery,
                DelegationScope::ReadNamespace,
            ],
            vec!["commerce".into(), "health".into()],
            1000,
            3_600_000,
            100,
        )
        .unwrap();

        let mut gate = AccessGate::new();
        gate.add_delegation(deleg).unwrap();

        let qe = QueryEngine::new(&engine, &gate);
        let result = qe
            .execute_cross_namespace(
                &Query::new(),
                &agent.verifying,
                &[("commerce", "commerce/main"), ("health", "health/main")],
                5000,
            )
            .unwrap();

        // 5 commerce + 1 health = 6
        assert_eq!(result.records.len(), 6);
    }

    #[test]
    fn cross_namespace_denied_without_delegation() {
        let (engine, _, _, agent) = setup();
        let gate = AccessGate::new();
        let qe = QueryEngine::new(&engine, &gate);

        let result = qe.execute_cross_namespace(
            &Query::new(),
            &agent.verifying,
            &[("commerce", "commerce/main")],
            5000,
        );

        assert!(result.is_err());
        assert!(matches!(result, Err(QueryError::AccessDenied(_))));
    }

    #[test]
    fn count_query() {
        let (engine, owner, _, _) = setup();
        let gate = AccessGate::new();
        let qe = QueryEngine::new(&engine, &gate);

        // Unfiltered count (uses optimized path)
        let count = qe
            .count(
                &Query::new()
                    .schema("commerce.listing")
                    .namespace("commerce"),
                &owner.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();
        assert_eq!(count, 5);

        // Filtered count
        let count = qe
            .count(
                &Query::new()
                    .namespace("commerce")
                    .filter(Filter::Gt("price".into(), json!(25.0))),
                &owner.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();
        assert_eq!(count, 2); // Woven Basket (39.99), Glass Vase (55.00)
    }

    #[test]
    fn query_by_schema() {
        let (engine, owner, _, _) = setup();
        let gate = AccessGate::new();
        let qe = QueryEngine::new(&engine, &gate);

        let result = qe
            .execute(
                &Query::new()
                    .schema("commerce.listing")
                    .namespace("commerce"),
                &owner.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();

        assert_eq!(result.records.len(), 5);
        for rec in &result.records {
            assert_eq!(rec.schema, "commerce.listing");
        }
    }

    #[test]
    fn empty_result() {
        let (engine, owner, _, _) = setup();
        let gate = AccessGate::new();
        let qe = QueryEngine::new(&engine, &gate);

        let result = qe
            .execute(
                &Query::new()
                    .namespace("commerce")
                    .filter(Filter::Eq("currency".into(), json!("JPY"))),
                &owner.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();

        assert_eq!(result.records.len(), 0);
        assert_eq!(result.total_count, 0);
        assert!(!result.has_more);
    }

    #[test]
    fn range_filter() {
        let (engine, owner, _, _) = setup();
        let gate = AccessGate::new();
        let qe = QueryEngine::new(&engine, &gate);

        let result = qe
            .execute(
                &Query::new().namespace("commerce").filter(Filter::And(vec![
                    Filter::Gte("price".into(), json!(15.0)),
                    Filter::Lte("price".into(), json!(40.0)),
                ])),
                &owner.verifying,
                "commerce/main",
                5000,
            )
            .unwrap();

        // 18.75, 24.99, 39.99
        assert_eq!(result.records.len(), 3);
    }
}
