pub mod driver;
pub mod error;
pub mod pool;
pub mod protocol;
pub mod query;
pub mod types;
pub mod orm;

pub use driver::{ConnectionConfig, DatabaseDriver, DatabaseType, DriverFactory, DriverRegistry};
pub use driver::{PostgresDriver, PostgresDriverFactory};
pub use driver::{MysqlDriver, MysqlDriverFactory};
pub use driver::{SqliteDriver, SqliteDriverFactory};
pub use error::{Error, Result};
pub use pool::ConnectionPool;
pub use orm::{ColumnInfo, Model, QueryBuilder, Transaction};
pub use types::{FromSql, ToSql, Value};


// 明确区分 trait 和 derive macro
pub use grorm_macros::Model as DeriveModel;
pub use grorm_macros::Table as DeriveTable;
