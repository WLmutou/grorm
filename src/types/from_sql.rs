use super::value::Value;

pub trait FromSql: Sized {
    fn from_sql(value: &Value) -> Result<Self, String>;
}

impl FromSql for bool {
    fn from_sql(value: &Value) -> Result<Self, String> {
        value.as_bool().ok_or_else(|| format!("cannot convert {:?} to bool", value))
    }
}

impl FromSql for i8 {
    fn from_sql(value: &Value) -> Result<Self, String> {
        value.as_i64().map(|v| v as i8).ok_or_else(|| format!("cannot convert {:?} to i8", value))
    }
}

impl FromSql for i16 {
    fn from_sql(value: &Value) -> Result<Self, String> {
        value.as_i64().map(|v| v as i16).ok_or_else(|| format!("cannot convert {:?} to i16", value))
    }
}

impl FromSql for i32 {
    fn from_sql(value: &Value) -> Result<Self, String> {
        value.as_i64().map(|v| v as i32).ok_or_else(|| format!("cannot convert {:?} to i32", value))
    }
}

impl FromSql for i64 {
    fn from_sql(value: &Value) -> Result<Self, String> {
        value.as_i64().ok_or_else(|| format!("cannot convert {:?} to i64", value))
    }
}

impl FromSql for f32 {
    fn from_sql(value: &Value) -> Result<Self, String> {
        value.as_f64().map(|v| v as f32).ok_or_else(|| format!("cannot convert {:?} to f32", value))
    }
}

impl FromSql for f64 {
    fn from_sql(value: &Value) -> Result<Self, String> {
        value.as_f64().ok_or_else(|| format!("cannot convert {:?} to f64", value))
    }
}

impl FromSql for String {
    fn from_sql(value: &Value) -> Result<Self, String> {
        value.as_string().ok_or_else(|| format!("cannot convert {:?} to String", value))
    }
}

impl FromSql for Vec<u8> {
    fn from_sql(value: &Value) -> Result<Self, String> {
        value.as_bytes().map(|v| v.to_vec()).ok_or_else(|| format!("cannot convert {:?} to Vec<u8>", value))
    }
}

impl<T: FromSql> FromSql for Option<T> {
    fn from_sql(value: &Value) -> Result<Self, String> {
        if value.is_null() {
            Ok(None)
        } else {
            T::from_sql(value).map(Some)
        }
    }
}