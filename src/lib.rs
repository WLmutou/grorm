pub mod driver;
pub mod pool;
pub mod protocol;
pub mod query;
pub mod types;
pub mod orm;

pub use driver::{ConnectionConfig, DatabaseDriver, DatabaseType, DriverFactory, DriverRegistry};
pub use driver::{PostgresDriver, PostgresDriverFactory};
pub use driver::{MysqlDriver, MysqlDriverFactory};
pub use driver::{SqliteDriver, SqliteDriverFactory};
pub use pool::ConnectionPool;
pub use orm::{Model, QueryBuilder, Transaction};
pub use types::{FromSql, ToSql, Value};

#[macro_export]
macro_rules! find_where {
    ($qb:expr, $obj:ident.$field:ident, $value:expr) => {
        $qb.find_where(stringify!($field), $value)
    };
}