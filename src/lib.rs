//! # grorm — A goroutine-native async ORM for Rust
//!
//! `grorm` (GRoutines + ORM) is a database ORM built on top of [gorust](https://crates.io/crates/gorust),
//! providing a goroutine-native async experience without tokio. It supports PostgreSQL, MySQL, and SQLite.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use grorm::{ConnectionConfig, ConnectionPool, SqliteDriverFactory, QueryBuilder, Value};
//! use grorm::DeriveModel;
//! use gorust::runtime;
//!
//! #[derive(Debug, DeriveModel)]
//! #[table = "users"]
//! struct User {
//!     id: i64,
//!     #[index]
//!     name: String,
//!     #[unique]
//!     email: String,
//!     age: i32,
//! }
//!
//! #[runtime]
//! fn main() -> Result<(), grorm::Error> {
//!     let config = ConnectionConfig::sqlite("test.db");
//!     let pool = ConnectionPool::new(SqliteDriverFactory, config, 4);
//!     let mut conn = pool.get()?;
//!
//!     let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
//!
//!     // Create table with indexes
//!     qb.create_table()?;
//!
//!     // Insert
//!     let user = User { id: 0, name: "Alice".into(), email: "alice@x.com".into(), age: 30 };
//!     qb.insert(&user)?;
//!
//!     // Chainable query
//!     let users = qb.where_eq("name", Value::from("Alice")).find()?;
//!     println!("{:?}", users);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! - **Multi-database**: PostgreSQL, MySQL, SQLite
//! - **Chainable API**: `where_eq().limit().offset().order().find()`
//! - **Transactions**: `Transaction::begin()` with auto-rollback on drop
//! - **Auto table creation**: `create_table()` generates DDL from model
//! - **Index & unique constraints**: `#[index]`, `#[unique]`, `#[unique_index = "name"]`
//! - **JOIN support**: `left_join()`, `inner_join()`, `right_join()`
//! - **IN queries**: `where_in("name", vec![...])`
//! - **Connection pooling**: gorust channel-based pool
//! - **Derive macros**: `#[derive(DeriveModel)]` auto-generates Model trait

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
pub use orm::{ColumnInfo, JoinClause, JoinType, Model, QueryBuilder, Transaction};
pub use types::{FromSql, Id, ToSql, Value};


// 明确区分 trait 和 derive macro
pub use grorm_macros::Model as DeriveModel;
pub use grorm_macros::Table as DeriveTable;
