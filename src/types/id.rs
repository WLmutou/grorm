use std::fmt;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use super::value::Value;
use super::{FromSql, ToSql};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Id(i64);

impl Id {
    pub fn new() -> Self {
        Id(0)
    }

    pub fn value(&self) -> i64 {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl ToSql for Id {
    fn to_sql(&self) -> Value {
        Value::I64(self.0)
    }
}

impl FromSql for Id {
    fn from_sql(value: &Value) -> Result<Self, String> {
        value
            .as_i64()
            .map(Id)
            .ok_or_else(|| format!("cannot convert {:?} to Id", value))
    }
}

impl From<i64> for Id {
    fn from(v: i64) -> Self {
        Id(v)
    }
}

impl From<Id> for i64 {
    fn from(id: Id) -> Self {
        id.0
    }
}
