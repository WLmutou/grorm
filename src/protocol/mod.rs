pub mod pg;
pub mod myproto;
pub mod sqlite_proto;

pub use pg::{PgConnection, PgResult, PgColumnInfo};
pub use myproto::{MyConnection, MyResult, MyColumnInfo};
pub use sqlite_proto::{SqliteConnection, SqliteResult, SqliteColumnInfo};