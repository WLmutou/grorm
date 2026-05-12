use super::value::Value;

pub trait ToSql {
    fn to_sql(&self) -> Value;
}

impl ToSql for bool {
    fn to_sql(&self) -> Value { Value::Bool(*self) }
}

impl ToSql for i8 {
    fn to_sql(&self) -> Value { Value::I8(*self) }
}

impl ToSql for i16 {
    fn to_sql(&self) -> Value { Value::I16(*self) }
}

impl ToSql for i32 {
    fn to_sql(&self) -> Value { Value::I32(*self) }
}

impl ToSql for i64 {
    fn to_sql(&self) -> Value { Value::I64(*self) }
}

impl ToSql for f32 {
    fn to_sql(&self) -> Value { Value::F32(*self) }
}

impl ToSql for f64 {
    fn to_sql(&self) -> Value { Value::F64(*self) }
}

impl ToSql for String {
    fn to_sql(&self) -> Value { Value::String(self.clone()) }
}

impl ToSql for &str {
    fn to_sql(&self) -> Value { Value::String(self.to_string()) }
}

impl ToSql for Vec<u8> {
    fn to_sql(&self) -> Value { Value::Bytes(self.clone()) }
}

impl<T: ToSql> ToSql for Option<T> {
    fn to_sql(&self) -> Value {
        match self {
            Some(v) => v.to_sql(),
            None => Value::Null,
        }
    }
}