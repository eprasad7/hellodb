//! Schema definition and registry.
//!
//! Apps register schemas with hellodb before writing records.
//! Schemas define the expected shape of record data within a namespace.
//! Schema identifiers use dot-notation: "app.domain.type"
//! (e.g., "ainp.commerce.listing", "health.vitals.heartrate").

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::CoreError;

/// Primitive field types supported by hellodb schemas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// UTF-8 string.
    String,
    /// Signed 64-bit integer.
    Integer,
    /// 64-bit floating point.
    Float,
    /// Boolean.
    Boolean,
    /// Unix timestamp in milliseconds, stored as u64.
    Timestamp,
    /// Content-addressed blob reference (BLAKE3 hash).
    Blob,
    /// Arbitrary nested JSON.
    Json,
    /// Ordered array of a single element type.
    Array(Box<FieldType>),
    /// Optional wrapper around another type.
    Optional(Box<FieldType>),
}

/// A single field in a schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    /// Field name.
    pub name: String,
    /// Field type.
    pub field_type: FieldType,
    /// Whether this field is required.
    pub required: bool,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A schema definition for a record type within a namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    /// Fully qualified schema ID (e.g., "ainp.commerce.listing").
    pub id: String,
    /// Schema version (semantic versioning).
    pub version: String,
    /// The namespace this schema belongs to.
    pub namespace: String,
    /// Human-readable name.
    pub name: String,
    /// The fields defined by this schema.
    pub fields: Vec<SchemaField>,
    /// Unix timestamp when registered.
    pub registered_at_ms: u64,
}

/// In-memory schema registry. Maps schema IDs to their definitions.
pub struct SchemaRegistry {
    schemas: HashMap<String, Schema>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
        }
    }

    /// Register a schema. Returns error if schema ID already exists
    /// with a different version.
    pub fn register(&mut self, schema: Schema) -> Result<(), CoreError> {
        if let Some(existing) = self.schemas.get(&schema.id) {
            if existing.version != schema.version {
                return Err(CoreError::DuplicateSchema(format!(
                    "{} already registered with version {}, cannot register version {}",
                    schema.id, existing.version, schema.version
                )));
            }
            // Same version = idempotent, no-op
            return Ok(());
        }
        self.schemas.insert(schema.id.clone(), schema);
        Ok(())
    }

    /// Get a schema by its fully qualified ID.
    pub fn get(&self, schema_id: &str) -> Option<&Schema> {
        self.schemas.get(schema_id)
    }

    /// List all schemas in a namespace.
    pub fn list_by_namespace(&self, namespace: &str) -> Vec<&Schema> {
        self.schemas
            .values()
            .filter(|s| s.namespace == namespace)
            .collect()
    }

    /// Validate that a JSON value conforms to a schema.
    pub fn validate_data(
        &self,
        schema_id: &str,
        data: &serde_json::Value,
    ) -> Result<(), CoreError> {
        let schema = self
            .schemas
            .get(schema_id)
            .ok_or_else(|| CoreError::SchemaNotFound(schema_id.to_string()))?;

        let obj = data
            .as_object()
            .ok_or_else(|| CoreError::SchemaValidation("data must be a JSON object".into()))?;

        for field in &schema.fields {
            if field.required && !obj.contains_key(&field.name) {
                return Err(CoreError::SchemaValidation(format!(
                    "missing required field: {}",
                    field.name
                )));
            }

            if let Some(value) = obj.get(&field.name) {
                validate_field_type(&field.name, value, &field.field_type)?;
            }
        }

        Ok(())
    }

    /// Check if a schema ID is registered.
    pub fn has_schema(&self, schema_id: &str) -> bool {
        self.schemas.contains_key(schema_id)
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate that a JSON value matches the expected field type.
fn validate_field_type(
    field_name: &str,
    value: &serde_json::Value,
    expected: &FieldType,
) -> Result<(), CoreError> {
    match expected {
        FieldType::String => {
            if !value.is_string() {
                return Err(CoreError::SchemaValidation(format!(
                    "field '{}' expected string, got {}",
                    field_name,
                    value_type_name(value)
                )));
            }
        }
        FieldType::Integer | FieldType::Timestamp => {
            if !value.is_i64() && !value.is_u64() {
                return Err(CoreError::SchemaValidation(format!(
                    "field '{}' expected integer, got {}",
                    field_name,
                    value_type_name(value)
                )));
            }
        }
        FieldType::Float => {
            if !value.is_number() {
                return Err(CoreError::SchemaValidation(format!(
                    "field '{}' expected number, got {}",
                    field_name,
                    value_type_name(value)
                )));
            }
        }
        FieldType::Boolean => {
            if !value.is_boolean() {
                return Err(CoreError::SchemaValidation(format!(
                    "field '{}' expected boolean, got {}",
                    field_name,
                    value_type_name(value)
                )));
            }
        }
        FieldType::Blob => {
            // Blob references are stored as hex strings
            if !value.is_string() {
                return Err(CoreError::SchemaValidation(format!(
                    "field '{}' expected blob (string), got {}",
                    field_name,
                    value_type_name(value)
                )));
            }
        }
        FieldType::Json => {
            // Any JSON is valid
        }
        FieldType::Array(inner) => {
            if let Some(arr) = value.as_array() {
                for (i, item) in arr.iter().enumerate() {
                    validate_field_type(&format!("{}[{}]", field_name, i), item, inner)?;
                }
            } else {
                return Err(CoreError::SchemaValidation(format!(
                    "field '{}' expected array, got {}",
                    field_name,
                    value_type_name(value)
                )));
            }
        }
        FieldType::Optional(inner) => {
            if !value.is_null() {
                validate_field_type(field_name, value, inner)?;
            }
        }
    }
    Ok(())
}

fn value_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_listing_schema() -> Schema {
        Schema {
            id: "ainp.commerce.listing".into(),
            version: "1.0.0".into(),
            namespace: "ainp.commerce".into(),
            name: "Listing".into(),
            fields: vec![
                SchemaField {
                    name: "title".into(),
                    field_type: FieldType::String,
                    required: true,
                    description: Some("Listing title".into()),
                },
                SchemaField {
                    name: "price_micro_usdc".into(),
                    field_type: FieldType::Integer,
                    required: true,
                    description: None,
                },
                SchemaField {
                    name: "remote".into(),
                    field_type: FieldType::Boolean,
                    required: false,
                    description: None,
                },
                SchemaField {
                    name: "tags".into(),
                    field_type: FieldType::Array(Box::new(FieldType::String)),
                    required: false,
                    description: None,
                },
            ],
            registered_at_ms: 1000,
        }
    }

    #[test]
    fn register_and_get() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_listing_schema()).unwrap();
        let s = reg.get("ainp.commerce.listing").unwrap();
        assert_eq!(s.name, "Listing");
        assert_eq!(s.fields.len(), 4);
    }

    #[test]
    fn idempotent_registration() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_listing_schema()).unwrap();
        // Same schema, same version -> ok (idempotent)
        reg.register(sample_listing_schema()).unwrap();
    }

    #[test]
    fn duplicate_different_version_fails() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_listing_schema()).unwrap();
        let mut schema2 = sample_listing_schema();
        schema2.version = "2.0.0".into();
        assert!(reg.register(schema2).is_err());
    }

    #[test]
    fn validate_conforming_data() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_listing_schema()).unwrap();
        let data = json!({"title": "Rust Dev", "price_micro_usdc": 25000000});
        assert!(reg.validate_data("ainp.commerce.listing", &data).is_ok());
    }

    #[test]
    fn validate_with_optional_fields() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_listing_schema()).unwrap();
        let data = json!({
            "title": "Rust Dev",
            "price_micro_usdc": 25000000,
            "remote": true,
            "tags": ["rust", "backend"]
        });
        assert!(reg.validate_data("ainp.commerce.listing", &data).is_ok());
    }

    #[test]
    fn validate_missing_required_field() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_listing_schema()).unwrap();
        let data = json!({"title": "Rust Dev"}); // missing price_micro_usdc
        assert!(reg.validate_data("ainp.commerce.listing", &data).is_err());
    }

    #[test]
    fn validate_wrong_type() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_listing_schema()).unwrap();
        let data = json!({"title": 123, "price_micro_usdc": 25000000}); // title is number, not string
        assert!(reg.validate_data("ainp.commerce.listing", &data).is_err());
    }

    #[test]
    fn validate_wrong_array_element_type() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_listing_schema()).unwrap();
        let data = json!({
            "title": "Rust Dev",
            "price_micro_usdc": 25000000,
            "tags": [1, 2, 3]  // should be strings
        });
        assert!(reg.validate_data("ainp.commerce.listing", &data).is_err());
    }

    #[test]
    fn validate_schema_not_found() {
        let reg = SchemaRegistry::new();
        let data = json!({"title": "test"});
        assert!(reg.validate_data("nonexistent.schema", &data).is_err());
    }

    #[test]
    fn list_by_namespace() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_listing_schema()).unwrap();
        reg.register(Schema {
            id: "ainp.commerce.order".into(),
            version: "1.0.0".into(),
            namespace: "ainp.commerce".into(),
            name: "Order".into(),
            fields: vec![],
            registered_at_ms: 2000,
        })
        .unwrap();
        reg.register(Schema {
            id: "health.vitals.heartrate".into(),
            version: "1.0.0".into(),
            namespace: "health.vitals".into(),
            name: "Heart Rate".into(),
            fields: vec![],
            registered_at_ms: 3000,
        })
        .unwrap();

        let commerce = reg.list_by_namespace("ainp.commerce");
        assert_eq!(commerce.len(), 2);

        let health = reg.list_by_namespace("health.vitals");
        assert_eq!(health.len(), 1);
    }

    #[test]
    fn has_schema() {
        let mut reg = SchemaRegistry::new();
        assert!(!reg.has_schema("test.schema"));
        reg.register(sample_listing_schema()).unwrap();
        assert!(reg.has_schema("ainp.commerce.listing"));
    }

    #[test]
    fn serde_field_type_roundtrip() {
        let ft = FieldType::Array(Box::new(FieldType::Optional(Box::new(FieldType::String))));
        let json = serde_json::to_string(&ft).unwrap();
        let restored: FieldType = serde_json::from_str(&json).unwrap();
        assert_eq!(ft, restored);
    }
}
