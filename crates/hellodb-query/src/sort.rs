//! Multi-field sorting for query results.

use hellodb_core::Record;
use serde_json::Value;
use std::cmp::Ordering;

use crate::filter::compare_values;

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Asc,
    Desc,
}

/// A field to sort by, with direction.
#[derive(Debug, Clone)]
pub struct SortField {
    /// Field name. Special values:
    /// - `"created_at_ms"` → record.created_at_ms
    /// - Anything else → record.data[field]
    pub field: String,
    pub order: SortOrder,
}

impl SortField {
    pub fn asc(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            order: SortOrder::Asc,
        }
    }

    pub fn desc(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            order: SortOrder::Desc,
        }
    }
}

/// Sort records by a list of sort fields (multi-field stable sort).
pub fn apply_sort(records: &mut [Record], sort_fields: &[SortField]) {
    if sort_fields.is_empty() {
        return;
    }
    records.sort_by(|a, b| compare_records(a, b, sort_fields));
}

/// Compare two records by a list of sort fields. Falls through to the next
/// field if the current one is equal.
fn compare_records(a: &Record, b: &Record, sort_fields: &[SortField]) -> Ordering {
    for sf in sort_fields {
        let ordering = compare_by_field(a, b, sf);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

/// Compare two records by a single sort field.
fn compare_by_field(a: &Record, b: &Record, sf: &SortField) -> Ordering {
    let raw = if sf.field == "created_at_ms" {
        a.created_at_ms.cmp(&b.created_at_ms)
    } else {
        let av = a.data.as_object().and_then(|o| o.get(&sf.field));
        let bv = b.data.as_object().and_then(|o| o.get(&sf.field));
        match (av, bv) {
            (Some(a_val), Some(b_val)) => {
                compare_values(a_val, b_val).unwrap_or(Ordering::Equal)
            }
            (Some(_), None) => Ordering::Less,    // present before absent
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    };

    match sf.order {
        SortOrder::Asc => raw,
        SortOrder::Desc => raw.reverse(),
    }
}

/// Extract a sort key value from a record for cursor encoding.
pub fn extract_sort_value(record: &Record, field: &str) -> Value {
    if field == "created_at_ms" {
        Value::Number(record.created_at_ms.into())
    } else {
        record
            .data
            .as_object()
            .and_then(|o| o.get(field))
            .cloned()
            .unwrap_or(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hellodb_crypto::KeyPair;
    use serde_json::json;

    fn make_record(data: Value, ts: u64) -> Record {
        let kp = KeyPair::generate();
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
    fn sort_by_created_at_asc() {
        let mut records = vec![
            make_record(json!({}), 3000),
            make_record(json!({}), 1000),
            make_record(json!({}), 2000),
        ];
        apply_sort(&mut records, &[SortField::asc("created_at_ms")]);
        assert_eq!(records[0].created_at_ms, 1000);
        assert_eq!(records[1].created_at_ms, 2000);
        assert_eq!(records[2].created_at_ms, 3000);
    }

    #[test]
    fn sort_by_field_desc() {
        let mut records = vec![
            make_record(json!({"price": 10.0}), 1000),
            make_record(json!({"price": 30.0}), 1000),
            make_record(json!({"price": 20.0}), 1000),
        ];
        apply_sort(&mut records, &[SortField::desc("price")]);
        assert_eq!(records[0].data["price"], 30.0);
        assert_eq!(records[1].data["price"], 20.0);
        assert_eq!(records[2].data["price"], 10.0);
    }

    #[test]
    fn multi_field_sort() {
        let mut records = vec![
            make_record(json!({"category": "B", "price": 20.0}), 1000),
            make_record(json!({"category": "A", "price": 30.0}), 1000),
            make_record(json!({"category": "A", "price": 10.0}), 1000),
        ];
        apply_sort(
            &mut records,
            &[SortField::asc("category"), SortField::asc("price")],
        );
        assert_eq!(records[0].data["category"], "A");
        assert_eq!(records[0].data["price"], 10.0);
        assert_eq!(records[1].data["category"], "A");
        assert_eq!(records[1].data["price"], 30.0);
        assert_eq!(records[2].data["category"], "B");
    }

    #[test]
    fn missing_field_sorts_last() {
        let mut records = vec![
            make_record(json!({}), 1000),
            make_record(json!({"score": 50}), 1000),
            make_record(json!({"score": 10}), 1000),
        ];
        apply_sort(&mut records, &[SortField::asc("score")]);
        assert_eq!(records[0].data["score"], 10);
        assert_eq!(records[1].data["score"], 50);
        assert!(records[2].data.get("score").is_none());
    }

    #[test]
    fn empty_sort_preserves_order() {
        let r1 = make_record(json!({"id": 1}), 1000);
        let r2 = make_record(json!({"id": 2}), 2000);
        let id1 = r1.record_id.clone();
        let id2 = r2.record_id.clone();
        let mut records = vec![r1, r2];
        apply_sort(&mut records, &[]);
        assert_eq!(records[0].record_id, id1);
        assert_eq!(records[1].record_id, id2);
    }
}
