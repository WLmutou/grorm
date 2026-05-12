use crate::types::Value;

/// Metadata about a single column in a model's table schema.
///
/// Generated automatically by `#[derive(DeriveModel)]` and used by
/// [`QueryBuilder::create_table`] to generate DDL statements.
///
/// # Example
///
/// ```rust
/// use grorm::{DeriveModel, ColumnInfo, Model};
///
/// #[derive(Debug, DeriveModel)]
/// #[table = "users"]
/// struct User {
///     id: i64,
///     #[index]
///     name: String,
///     #[unique]
///     email: String,
///     age: i32,
/// }
///
/// let schema = User::table_schema();
/// assert_eq!(schema.len(), 4);
/// assert!(schema[0].is_primary_key);
/// assert!(schema[1].is_index);
/// assert!(schema[2].is_unique);
/// ```
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    /// Column name in the database
    pub name: &'static str,
    /// Rust type name (e.g. `"i64"`, `"String"`)
    pub rust_type: &'static str,
    /// Whether this column is part of the primary key
    pub is_primary_key: bool,
    /// Whether this column is auto-increment (integer primary key)
    pub is_auto_increment: bool,
    /// Whether a regular index should be created on this column (`#[index]`)
    pub is_index: bool,
    /// Whether a unique constraint should be created on this column (`#[unique]`)
    pub is_unique: bool,
    /// Name of the composite unique index group (`#[unique_index = "name"]`).
    /// Columns with the same name form a composite unique index.
    pub unique_index_name: Option<&'static str>,
}

/// Trait implemented by `#[derive(DeriveModel)]` for ORM model types.
///
/// Provides table metadata, column information, and row serialization/deserialization.
///
/// # Derivable
///
/// This trait is typically derived using `#[derive(DeriveModel)]`:
///
/// ```rust
/// use grorm::DeriveModel;
///
/// #[derive(Debug, DeriveModel)]
/// #[table = "users"]
/// struct User {
///     id: i64,
///     name: String,
///     email: String,
///     age: i32,
/// }
/// ```
///
/// # Attributes
///
/// | Attribute | Scope | Description |
/// |-----------|-------|-------------|
/// | `#[table = "name"]` | struct | Override table name (default: lowercase struct name + "s") |
/// | `#[primary_key = "col"]` | struct | Override primary key column (default: `id`) |
/// | `#[index]` | field | Create a regular index on this column |
/// | `#[unique]` | field | Create a unique constraint on this column |
/// | `#[unique_index = "name"]` | field | Group columns into a composite unique index |
pub trait Model: Sized {
    /// Returns the database table name
    fn table_name() -> &'static str;
    /// Returns the primary key column name
    fn primary_key() -> &'static str;
    /// Returns all column names in order
    fn columns() -> &'static [&'static str];
    /// Returns full column metadata for DDL generation
    fn table_schema() -> &'static [ColumnInfo];
    /// Deserialize a database row into this model
    fn from_row(row: &[Value]) -> Result<Self, String>;
    /// Serialize this model into database values
    fn to_values(&self) -> Vec<Value>;
}