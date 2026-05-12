use crate::types::Value;

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: &'static str,
    pub rust_type: &'static str,
    pub is_primary_key: bool,
    pub is_auto_increment: bool,
    pub is_index: bool,
    pub is_unique: bool,
    pub unique_index_name: Option<&'static str>,
}

pub trait Model: Sized {
    fn table_name() -> &'static str;
    fn primary_key() -> &'static str;
    fn columns() -> &'static [&'static str];
    fn table_schema() -> &'static [ColumnInfo];
    fn from_row(row: &[Value]) -> Result<Self, String>;
    fn to_values(&self) -> Vec<Value>;
}