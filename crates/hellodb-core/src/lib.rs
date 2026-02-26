//! hellodb Core
//!
//! Defines the record model, schema registry, namespace isolation,
//! branch metadata, and canonicalization rules for the hellodb
//! sovereign data layer.

pub mod canonical;
pub mod record;
pub mod schema;
pub mod namespace;
pub mod branch;
pub mod error;

pub use canonical::{canonicalize, canonicalize_value};
pub use record::{Record, RecordId};
pub use schema::{Schema, SchemaField, FieldType, SchemaRegistry};
pub use namespace::{Namespace, NamespaceId};
pub use branch::{Branch, BranchId, BranchState, MergeResult, MergeConflict};
pub use error::CoreError;
