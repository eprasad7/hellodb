//! Query builder and result types.

use hellodb_core::Record;

use crate::cursor::Cursor;
use crate::filter::Filter;
use crate::sort::SortField;

/// A query against hellodb records.
///
/// Use the builder pattern to construct:
/// ```ignore
/// Query::new()
///     .schema("app.commerce.listing")
///     .filter(Filter::Gt("price".into(), json!(20.0)))
///     .sort(SortField::desc("price"))
///     .limit(50)
/// ```
#[derive(Debug, Clone)]
pub struct Query {
    /// Only match records with this schema (None = all schemas).
    pub schema: Option<String>,
    /// Only match records in this namespace (None = all in scope).
    pub namespace: Option<String>,
    /// Filter predicate tree (None = no filtering).
    pub filter: Option<Filter>,
    /// Sort fields (applied in order). Empty = default order.
    pub sort: Vec<SortField>,
    /// Maximum records to return per page.
    pub limit: usize,
    /// Cursor-based pagination: resume after this cursor.
    pub after: Option<Cursor>,
    /// Offset-based pagination fallback (0 = disabled).
    pub offset: usize,
}

impl Query {
    /// Create a new query with defaults (limit=100, no filters).
    pub fn new() -> Self {
        Self {
            schema: None,
            namespace: None,
            filter: None,
            sort: Vec::new(),
            limit: 100,
            after: None,
            offset: 0,
        }
    }

    /// Filter by schema ID.
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Filter by namespace.
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Set the filter predicate.
    pub fn filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Add a sort field.
    pub fn sort(mut self, field: SortField) -> Self {
        self.sort.push(field);
        self
    }

    /// Set page size limit.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set cursor for pagination.
    pub fn after(mut self, cursor: Cursor) -> Self {
        self.after = Some(cursor);
        self
    }

    /// Set offset for offset-based pagination.
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of executing a query.
#[derive(Debug)]
pub struct QueryResult {
    /// Matching records for this page.
    pub records: Vec<Record>,
    /// Total count of matching records (across all pages).
    pub total_count: u64,
    /// Cursor to fetch the next page (None if no more results).
    pub next_cursor: Option<Cursor>,
    /// Whether there are more results beyond this page.
    pub has_more: bool,
}
