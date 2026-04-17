//! Typed filter predicates for querying hellodb records.
//!
//! Filters form a tree: leaf predicates match field values or record-level
//! properties, and combinators (And, Or, Not) compose them.

use hellodb_core::Record;
use hellodb_crypto::VerifyingKey;
use serde_json::Value;
use std::cmp::Ordering;

/// A typed predicate tree for filtering records.
#[derive(Debug, Clone)]
pub enum Filter {
    // --- Field predicates (operate on record.data fields) ---
    /// field == value
    Eq(String, Value),
    /// field != value
    Ne(String, Value),
    /// field > value
    Gt(String, Value),
    /// field < value
    Lt(String, Value),
    /// field >= value
    Gte(String, Value),
    /// field <= value
    Lte(String, Value),
    /// String field contains substring (case-sensitive).
    Contains(String, String),
    /// String field starts with prefix (case-sensitive).
    StartsWith(String, String),

    // --- Record-level predicates ---
    /// record.created_by == key
    CreatedBy(VerifyingKey),
    /// record.created_at_ms > timestamp
    CreatedAfter(u64),
    /// record.created_at_ms < timestamp
    CreatedBefore(u64),
    /// record.previous_version.is_some()
    HasPreviousVersion,

    // --- Combinators ---
    /// All sub-filters must match.
    And(Vec<Filter>),
    /// At least one sub-filter must match.
    Or(Vec<Filter>),
    /// Negate a filter.
    Not(Box<Filter>),
}

impl Filter {
    /// Evaluate this filter against a record. Returns true if the record matches.
    pub fn matches(&self, record: &Record) -> bool {
        match self {
            // Field predicates
            Filter::Eq(field, val) => get_field(&record.data, field).is_some_and(|v| v == val),
            Filter::Ne(field, val) => get_field(&record.data, field).is_none_or(|v| v != val),
            Filter::Gt(field, val) => get_field(&record.data, field)
                .is_some_and(|v| compare_values(v, val) == Some(Ordering::Greater)),
            Filter::Lt(field, val) => get_field(&record.data, field)
                .is_some_and(|v| compare_values(v, val) == Some(Ordering::Less)),
            Filter::Gte(field, val) => get_field(&record.data, field).is_some_and(|v| {
                matches!(
                    compare_values(v, val),
                    Some(Ordering::Greater | Ordering::Equal)
                )
            }),
            Filter::Lte(field, val) => get_field(&record.data, field).is_some_and(|v| {
                matches!(
                    compare_values(v, val),
                    Some(Ordering::Less | Ordering::Equal)
                )
            }),
            Filter::Contains(field, substr) => get_field(&record.data, field)
                .and_then(|v| v.as_str())
                .is_some_and(|s| s.contains(substr.as_str())),
            Filter::StartsWith(field, prefix) => get_field(&record.data, field)
                .and_then(|v| v.as_str())
                .is_some_and(|s| s.starts_with(prefix.as_str())),

            // Record-level predicates
            Filter::CreatedBy(key) => record.created_by == *key,
            Filter::CreatedAfter(ts) => record.created_at_ms > *ts,
            Filter::CreatedBefore(ts) => record.created_at_ms < *ts,
            Filter::HasPreviousVersion => record.previous_version.is_some(),

            // Combinators
            Filter::And(filters) => filters.iter().all(|f| f.matches(record)),
            Filter::Or(filters) => filters.iter().any(|f| f.matches(record)),
            Filter::Not(filter) => !filter.matches(record),
        }
    }
}

/// Extract a field value from a JSON object by name.
fn get_field<'a>(data: &'a Value, field: &str) -> Option<&'a Value> {
    data.as_object().and_then(|obj| obj.get(field))
}

/// Type-aware comparison of two JSON values.
/// Returns None if the types are incompatible.
pub fn compare_values(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        // Number comparison (promote to f64)
        (Value::Number(an), Value::Number(bn)) => {
            let af = an.as_f64()?;
            let bf = bn.as_f64()?;
            af.partial_cmp(&bf)
        }
        // String comparison (lexicographic)
        (Value::String(a_str), Value::String(b_str)) => Some(a_str.cmp(b_str)),
        // Bool comparison (false < true)
        (Value::Bool(ab), Value::Bool(bb)) => Some(ab.cmp(bb)),
        // Null == Null
        (Value::Null, Value::Null) => Some(Ordering::Equal),
        // Incompatible types
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hellodb_crypto::KeyPair;
    use serde_json::json;

    fn make_record(data: Value) -> Record {
        let kp = KeyPair::generate();
        Record::new_with_timestamp(
            &kp.signing,
            "test.schema".into(),
            "test.ns".into(),
            data,
            None,
            5000,
        )
        .unwrap()
    }

    fn make_record_with_key(data: Value, kp: &KeyPair, ts: u64) -> Record {
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
    fn eq_filter() {
        let rec = make_record(json!({"name": "Alice", "age": 30}));
        assert!(Filter::Eq("name".into(), json!("Alice")).matches(&rec));
        assert!(!Filter::Eq("name".into(), json!("Bob")).matches(&rec));
        assert!(Filter::Eq("age".into(), json!(30)).matches(&rec));
    }

    #[test]
    fn ne_filter() {
        let rec = make_record(json!({"name": "Alice"}));
        assert!(Filter::Ne("name".into(), json!("Bob")).matches(&rec));
        assert!(!Filter::Ne("name".into(), json!("Alice")).matches(&rec));
        // Missing field: Ne returns true (field != value because field is absent)
        assert!(Filter::Ne("missing".into(), json!("anything")).matches(&rec));
    }

    #[test]
    fn gt_lt_filter() {
        let rec = make_record(json!({"price": 25.0}));
        assert!(Filter::Gt("price".into(), json!(20.0)).matches(&rec));
        assert!(!Filter::Gt("price".into(), json!(30.0)).matches(&rec));
        assert!(Filter::Lt("price".into(), json!(30.0)).matches(&rec));
        assert!(!Filter::Lt("price".into(), json!(20.0)).matches(&rec));
    }

    #[test]
    fn gte_lte_filter() {
        let rec = make_record(json!({"score": 100}));
        assert!(Filter::Gte("score".into(), json!(100)).matches(&rec));
        assert!(Filter::Gte("score".into(), json!(99)).matches(&rec));
        assert!(!Filter::Gte("score".into(), json!(101)).matches(&rec));
        assert!(Filter::Lte("score".into(), json!(100)).matches(&rec));
        assert!(Filter::Lte("score".into(), json!(101)).matches(&rec));
        assert!(!Filter::Lte("score".into(), json!(99)).matches(&rec));
    }

    #[test]
    fn contains_starts_with() {
        let rec = make_record(json!({"title": "Handmade Ceramic Bowl"}));
        assert!(Filter::Contains("title".into(), "Ceramic".into()).matches(&rec));
        assert!(!Filter::Contains("title".into(), "Glass".into()).matches(&rec));
        assert!(Filter::StartsWith("title".into(), "Hand".into()).matches(&rec));
        assert!(!Filter::StartsWith("title".into(), "Cera".into()).matches(&rec));
    }

    #[test]
    fn created_by_filter() {
        let kp = KeyPair::generate();
        let other = KeyPair::generate();
        let rec = make_record_with_key(json!({"x": 1}), &kp, 5000);
        assert!(Filter::CreatedBy(kp.verifying.clone()).matches(&rec));
        assert!(!Filter::CreatedBy(other.verifying).matches(&rec));
    }

    #[test]
    fn created_time_filters() {
        let rec = make_record(json!({"x": 1}));
        // rec.created_at_ms = 5000
        assert!(Filter::CreatedAfter(4000).matches(&rec));
        assert!(!Filter::CreatedAfter(6000).matches(&rec));
        assert!(Filter::CreatedBefore(6000).matches(&rec));
        assert!(!Filter::CreatedBefore(4000).matches(&rec));
    }

    #[test]
    fn has_previous_version() {
        let kp = KeyPair::generate();
        let rec = Record::new_with_timestamp(
            &kp.signing,
            "test.schema".into(),
            "test.ns".into(),
            json!({"v": 2}),
            Some("prev-id".into()),
            5000,
        )
        .unwrap();
        assert!(Filter::HasPreviousVersion.matches(&rec));

        let rec_no_prev = make_record(json!({"v": 1}));
        assert!(!Filter::HasPreviousVersion.matches(&rec_no_prev));
    }

    #[test]
    fn and_combinator() {
        let rec = make_record(json!({"price": 25.0, "currency": "USD"}));
        let filter = Filter::And(vec![
            Filter::Gt("price".into(), json!(20.0)),
            Filter::Eq("currency".into(), json!("USD")),
        ]);
        assert!(filter.matches(&rec));

        let filter_fail = Filter::And(vec![
            Filter::Gt("price".into(), json!(20.0)),
            Filter::Eq("currency".into(), json!("EUR")),
        ]);
        assert!(!filter_fail.matches(&rec));
    }

    #[test]
    fn or_combinator() {
        let rec = make_record(json!({"status": "active"}));
        let filter = Filter::Or(vec![
            Filter::Eq("status".into(), json!("active")),
            Filter::Eq("status".into(), json!("pending")),
        ]);
        assert!(filter.matches(&rec));

        let filter_fail = Filter::Or(vec![
            Filter::Eq("status".into(), json!("closed")),
            Filter::Eq("status".into(), json!("pending")),
        ]);
        assert!(!filter_fail.matches(&rec));
    }

    #[test]
    fn not_combinator() {
        let rec = make_record(json!({"active": true}));
        assert!(Filter::Not(Box::new(Filter::Eq("active".into(), json!(false)))).matches(&rec));
        assert!(!Filter::Not(Box::new(Filter::Eq("active".into(), json!(true)))).matches(&rec));
    }

    #[test]
    fn nested_combinators() {
        let rec = make_record(json!({"price": 50.0, "currency": "USD", "in_stock": true}));
        // (price > 40 AND currency == "USD") OR in_stock == false
        let filter = Filter::Or(vec![
            Filter::And(vec![
                Filter::Gt("price".into(), json!(40.0)),
                Filter::Eq("currency".into(), json!("USD")),
            ]),
            Filter::Eq("in_stock".into(), json!(false)),
        ]);
        assert!(filter.matches(&rec));
    }

    #[test]
    fn missing_field_returns_false_for_comparisons() {
        let rec = make_record(json!({"name": "test"}));
        assert!(!Filter::Eq("missing".into(), json!("val")).matches(&rec));
        assert!(!Filter::Gt("missing".into(), json!(0)).matches(&rec));
        assert!(!Filter::Contains("missing".into(), "x".into()).matches(&rec));
    }

    #[test]
    fn compare_values_types() {
        assert_eq!(
            compare_values(&json!(10), &json!(5)),
            Some(Ordering::Greater)
        );
        assert_eq!(compare_values(&json!(5), &json!(10)), Some(Ordering::Less));
        assert_eq!(compare_values(&json!(5), &json!(5)), Some(Ordering::Equal));
        assert_eq!(
            compare_values(&json!("b"), &json!("a")),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare_values(&json!(true), &json!(false)),
            Some(Ordering::Greater)
        );
        // Incompatible types
        assert_eq!(compare_values(&json!("str"), &json!(5)), None);
    }
}
