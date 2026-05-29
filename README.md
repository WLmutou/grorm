# grorm

**GRoutines + ORM** — A goroutine-native async ORM for Rust with multi-database support.

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Features

- **Multi-database**: PostgreSQL, MySQL, SQLite
- **Goroutine-native**: Built on [gorust](https://crates.io/crates/gorust), no tokio required
- **Chainable API**: `where_eq().limit().offset().order().find()`
- **Transactions**: `Transaction::begin()` with auto-rollback on drop
- **Auto table creation**: `create_table()` generates DDL from model definitions
- **Index & constraints**: `#[index]`, `#[unique]`, `#[unique_index = "name"]`
- **JOIN support**: `left_join()`, `inner_join()`, `right_join()`
- **IN queries**: `where_in("name", vec![...])`
- **Connection pooling**: gorust channel-based connection pool
- **Derive macros**: `#[derive(DeriveModel)]` auto-generates `Model` trait
- **SQL injection protection**: Built-in detection and prevention

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
grorm = "0.1.2"
gorust = "1.5"
```

### SQLite Example

```rust
use grorm::{ConnectionConfig, ConnectionPool, SqliteDriverFactory, QueryBuilder, Value, Error};
use grorm::DeriveModel;
use gorust::runtime;

#[derive(Debug, DeriveModel)]
#[table = "users"]
struct User {
    id: i64,
    #[index]
    name: String,
    #[unique]
    email: String,
    age: i32,
}

#[runtime]
fn main() -> Result<(), Error> {
    let config = ConnectionConfig::sqlite("test.db");
    let pool = ConnectionPool::new(SqliteDriverFactory, config, 4);
    let mut conn = pool.get()?;

    let mut qb = QueryBuilder::<User>::new(conn.driver_mut());

    qb.create_table()?;

    let user = User { id: 0, name: "Alice".into(), email: "alice@x.com".into(), age: 30 };
    qb.insert(&user)?;

    let users = qb.where_eq("name", Value::from("Alice")).find()?;
    println!("{:?}", users);

    Ok(())
}
```

### PostgreSQL Example

```rust
use grorm::{ConnectionConfig, ConnectionPool, PostgresDriverFactory, QueryBuilder, Value, Error};
use grorm::DeriveModel;
use gorust::runtime;

#[derive(Debug, DeriveModel)]
#[table = "users"]
struct User {
    id: i64,
    name: String,
    email: String,
    age: i32,
}

#[runtime]
fn main() -> Result<(), Error> {
    let config = ConnectionConfig::postgres("localhost", 5432, "mydb", "user", "pass");
    let pool = ConnectionPool::new(PostgresDriverFactory, config, 4);
    let mut conn = pool.get()?;

    let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
    qb.create_table()?;

    qb.insert(&User { id: 0, name: "Alice".into(), email: "alice@x.com".into(), age: 30 })?;

    let users = qb.find_all()?;
    println!("{:?}", users);

    Ok(())
}
```

### MySQL Example

```rust
use grorm::{ConnectionConfig, ConnectionPool, MysqlDriverFactory, QueryBuilder, Value, Error};
use grorm::DeriveModel;
use gorust::runtime;

#[derive(Debug, DeriveModel)]
#[table = "users"]
struct User {
    id: i64,
    name: String,
    email: String,
    age: i32,
}

#[runtime]
fn main() -> Result<(), Error> {
    let config = ConnectionConfig::mysql("localhost", 3306, "mydb", "user", "pass");
    let pool = ConnectionPool::new(MysqlDriverFactory, config, 4);
    let mut conn = pool.get()?;

    let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
    qb.create_table()?;

    qb.insert(&User { id: 0, name: "Alice".into(), email: "alice@x.com".into(), age: 30 })?;

    let users = qb.find_all()?;
    println!("{:?}", users);

    Ok(())
}
```

## Model Definition

Use `#[derive(DeriveModel)]` to define your model:

```rust
use grorm::DeriveModel;

#[derive(Debug, DeriveModel)]
#[table = "users"]                    // Override table name (default: snake_case + "s")
#[primary_key = "uuid"]               // Override primary key (default: "id")
struct User {
    id: i64,                          // Auto-increment primary key (when id = 0)
    #[index]                          // Create a regular index
    name: String,
    #[unique]                         // Create a unique constraint
    email: String,
    #[unique_index = "uq_name_age"]   // Composite unique index (same group name)
    first_name: String,
    #[unique_index = "uq_name_age"]
    last_name: String,
    age: i32,
}
```

### Available Attributes

| Attribute | Scope | Description |
|-----------|-------|-------------|
| `#[table = "name"]` | struct | Override table name |
| `#[primary_key = "col"]` | struct | Override primary key column |
| `#[index]` | field | Create a regular index |
| `#[unique]` | field | Create a unique constraint |
| `#[unique_index = "name"]` | field | Group into composite unique index |

## CRUD Operations

### Create Table

```rust
let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
qb.create_table()?;
```

### Insert

```rust
let user = User { id: 0, name: "Alice".into(), email: "alice@x.com".into(), age: 30 };
let id = qb.insert(&user)?;  // Returns Option<i64> (auto-generated id)
```

### Query

```rust
// Find all
let all = qb.find_all()?;

// Find by id
let user = qb.find_by_id(1)?;

// Find one with conditions
let user = qb.where_eq("age", Value::from(30)).find_one()?;

// Find with conditions
let users = qb.where_eq("age", Value::from(30)).find()?;

// Find with column name
let users = qb.find_where("name", Value::from("Alice"))?;

// Find with model (non-zero fields become conditions)
let filter = User { id: 0, name: "".into(), email: "".into(), age: 30 };
let users = qb.where_model(&filter).find()?;

// IN query
let users = qb.where_in("name", vec![Value::from("Alice"), Value::from("Bob")]).find()?;

// Pagination
let users = qb.order("age", true).limit(10).offset(0).find()?;

// Count
let total = qb.count()?;
```

### Update

```rust
// Update single column
let rows = qb.where_eq("name", Value::from("Alice"))
    .update_one("age", Value::from(31))?;

// Update from model (non-zero/non-empty fields)
let update = User { id: 0, name: "".into(), email: "".into(), age: 31 };
let rows = qb.where_eq("name", Value::from("Alice"))
    .update_model(&update)?;
```

### Delete

```rust
// Delete with conditions
let rows = qb.where_eq("name", Value::from("Alice")).delete()?;

// Delete all
let rows = qb.delete_all()?;
```

## Transactions

```rust
let mut tx = Transaction::<User>::begin(conn.driver_mut())?;

tx.insert(&User { id: 0, name: "Alice".into(), email: "alice@x.com".into(), age: 30 })?;
tx.where_eq("name", Value::from("Bob")).update_one("age", Value::from(26))?;

tx.commit()?;
// If tx goes out of scope without commit, it auto-rolls back
```

All `QueryBuilder` methods are available on `Transaction`:
- `insert`, `find`, `find_all`, `find_one`, `find_by_id`, `find_where`
- `where_eq`, `where_in`, `where_model`
- `update_one`, `update_model`
- `delete`, `delete_all`
- `count`, `limit`, `offset`, `order`

## JOIN Support

```rust
let mut qb = QueryBuilder::<User>::new(conn.driver_mut());

qb.left_join("orders", "users.id = orders.user_id")
  .inner_join("profiles", "users.id = profiles.user_id")
  .right_join("scores", "users.id = scores.user_id");

let results = qb.find()?;
```

## Connection Pool

```rust
// Create pool
let config = ConnectionConfig::sqlite("test.db");
let pool = ConnectionPool::new(SqliteDriverFactory, config, 4);

// Get connection (blocks if pool exhausted)
let mut conn = pool.get()?;

// Use driver directly
let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
```

## Error Handling

All public APIs return `Result<T, grorm::Error>`. The `Error` enum covers:

| Variant | Description |
|---------|-------------|
| `Connection` | Auth, network errors |
| `Query` | SQL syntax, constraint violations |
| `Execute` | Write operation errors |
| `Protocol` | Wire format, parsing errors |
| `Model` | Serialization/deserialization errors |
| `Pool` | Pool exhausted, closed |
| `Config` | Invalid DSN, configuration |
| `Io` | Wrapped I/O errors |
| `NotFound` | Entity not found |
| `Transaction` | Begin, commit, rollback errors |
| `SqlInjection` | SQL injection detection |

## Security

grorm 内置多层 SQL 注入防护：

### 1. 参数化查询
所有值通过 `?` 占位符传递，避免字符串拼接：

```rust
// 安全：使用参数化查询
let users = qb.where_eq("name", Value::from(user_input)).find()?;
```

### 2. 标识符验证
表名、列名只允许字母、数字、下划线：

```rust
use grorm::validate_identifier;

// 验证列名
validate_identifier("user_name")?;  // Ok
validate_identifier("user;drop")?;  // Err
```

### 3. 注入检测
自动检测以下模式：
- SQL 注释符 (`--`, `/*`, `#`)
- 多语句注入（分号后跟 SQL 关键字）
- 恒真条件 (`OR 1=1`, `AND 1=1` 等)
- 危险 SQL 关键字（在用户输入中）

```rust
use grorm::check_sql_injection;

// 检测注入
check_sql_injection("Alice'--")?;  // Err: SQL comment detected
check_sql_injection("'; DROP TABLE users")?;  // Err: dangerous pattern
check_sql_injection("' OR 1=1")?;  // Err: tautology condition
```

### 4. 自动防护
QueryBuilder 的以下方法自动应用防护：
- `where_in()`: 验证列名
- `order()`: 验证列名
- `left_join()`, `inner_join()`, `right_join()`: 验证表名和 ON 条件
- 所有 SQL 执行：检测注入模式

## Project Structure

```
grorm/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs              # Library root, re-exports
│   ├── error.rs            # Unified error types
│   ├── driver/             # Database driver abstraction
│   │   ├── mod.rs          # ConnectionConfig, DatabaseDriver trait
│   │   ├── postgres.rs     # PostgreSQL driver
│   │   ├── mysql.rs        # MySQL driver
│   │   └── sqlite.rs       # SQLite driver
│   ├── protocol/           # Database wire protocols
│   │   ├── mod.rs
│   │   ├── pg.rs           # PostgreSQL protocol
│   │   ├── myproto.rs      # MySQL protocol
│   │   └── sqlite_proto.rs # SQLite protocol (mock)
│   ├── query/              # Low-level SQL builders
│   │   ├── mod.rs
│   │   ├── select.rs
│   │   ├── insert.rs
│   │   ├── update.rs
│   │   └── delete.rs
│   ├── types/              # Type mapping (Rust ↔ SQL)
│   │   ├── mod.rs
│   │   ├── value.rs        # Value enum
│   │   ├── from_sql.rs     # FromSql trait
│   │   └── to_sql.rs       # ToSql trait
│   ├── orm/                # ORM core
│   │   ├── mod.rs
│   │   ├── model.rs        # Model trait, ColumnInfo
│   │   ├── query.rs        # QueryBuilder (chainable API)
│   │   └── transaction.rs  # Transaction support
│   └── pool/               # Connection pool (gorust channels)
│       └── mod.rs
├── grorm-macros/           # Procedural macros
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs          # #[derive(DeriveModel)]
└── examples/
    ├── sqlite_demo.rs
    ├── postgres_demo.rs
    └── mysql_demo.rs
```

## License

MIT