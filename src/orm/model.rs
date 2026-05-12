use crate::types::Value;

pub trait Model: Sized {
    fn table_name() -> &'static str;
    fn primary_key() -> &'static str;
    fn columns() -> &'static [&'static str];
    fn from_row(row: &[Value]) -> Result<Self, String>;
    fn to_values(&self) -> Vec<Value>;
}