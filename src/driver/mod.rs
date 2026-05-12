pub mod postgres;
pub mod mysql;
pub mod sqlite;

pub use postgres::{PostgresDriver, PostgresDriverFactory};
pub use mysql::{MysqlDriver, MysqlDriverFactory};
pub use sqlite::{SqliteDriver, SqliteDriverFactory};

use crate::error::Error;

/// 数据库连接配置
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub db_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub ssl_mode: SslMode,
    pub max_connections: usize,
}

impl ConnectionConfig {
    pub fn new(host: &str, port: u16, username: &str, password: &str, database: &str) -> Self {
        ConnectionConfig {
            db_type: String::new(),
            host: host.to_string(),
            port,
            username: username.to_string(),
            password: password.to_string(),
            database: database.to_string(),
            ssl_mode: SslMode::Disable,
            max_connections: 10,
        }
    }

    pub fn sqlite(db_path: &str) -> Self {
        ConnectionConfig {
            db_type: "sqlite".to_string(),
            host: db_path.to_string(),
            port: 0,
            username: String::new(),
            password: String::new(),
            database: db_path.to_string(),
            ssl_mode: SslMode::Disable,
            max_connections: 10,
        }
    }

    pub fn postgres(host: &str, port: u16, database: &str, username: &str, password: &str) -> Self {
        ConnectionConfig {
            db_type: "postgres".to_string(),
            host: host.to_string(),
            port,
            username: username.to_string(),
            password: password.to_string(),
            database: database.to_string(),
            ssl_mode: SslMode::Disable,
            max_connections: 10,
        }
    }

    pub fn mysql(host: &str, port: u16, database: &str, username: &str, password: &str) -> Self {
        ConnectionConfig {
            db_type: "mysql".to_string(),
            host: host.to_string(),
            port,
            username: username.to_string(),
            password: password.to_string(),
            database: database.to_string(),
            ssl_mode: SslMode::Disable,
            max_connections: 10,
        }
    }
    
    pub fn with_ssl(mut self, mode: SslMode) -> Self {
        self.ssl_mode = mode;
        self
    }
    
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SslMode {
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

/// 数据库类型标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseType {
    Postgresql,
    Mysql,
    Sqlite,
}

/// 查询结果
pub struct QueryResult {
    pub rows: Vec<Row>,
    pub affected_rows: u64,
    pub last_insert_id: Option<i64>,
}

/// 单行数据
pub struct Row {
    columns: Vec<Column>,
    values: Vec<Option<String>>,
}

impl Row {
    pub fn new() -> Self {
        Row {
            columns: Vec::new(),
            values: Vec::new(),
        }
    }
    
    pub fn with_capacity(capacity: usize) -> Self {
        Row {
            columns: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
        }
    }
    
    pub fn push(&mut self, column: Column, value: Option<String>) {
        self.columns.push(column);
        self.values.push(value);
    }
    
    pub fn get(&self, idx: usize) -> Option<&str> {
        self.values.get(idx).and_then(|v| v.as_deref())
    }
    
    pub fn get_by_name(&self, name: &str) -> Option<&str> {
        self.columns
            .iter()
            .position(|c| c.name == name)
            .and_then(|idx| self.get(idx))
    }
    
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
    
    pub fn iter(&self) -> RowIter<'_> {
        RowIter {
            row: self,
            idx: 0,
        }
    }
}

pub struct RowIter<'a> {
    row: &'a Row,
    idx: usize,
}

impl<'a> Iterator for RowIter<'a> {
    type Item = (&'a Column, Option<&'a str>);
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.row.columns.len() {
            None
        } else {
            let col = &self.row.columns[self.idx];
            let val = self.row.values[self.idx].as_deref();
            self.idx += 1;
            Some((col, val))
        }
    }
}

/// 列信息
#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

impl Column {
    pub fn new(name: &str, data_type: DataType) -> Self {
        Column {
            name: name.to_string(),
            data_type,
            nullable: true,
        }
    }
    
    pub fn not_null(mut self) -> Self {
        self.nullable = false;
        self
    }
}

/// 数据类型（数据库无关的抽象）
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Boolean,
    Int2,
    Int4,
    Int8,
    Float4,
    Float8,
    Text,
    Varchar(usize),
    Char(usize),
    Date,
    Time,
    Timestamp,
    Json,
    Jsonb,
    Uuid,
    Bytea,
    Array(Box<DataType>),
    Custom(String),
}

/// 参数化查询的参数
#[derive(Debug, Clone)]
pub enum Parameter {
    Null,
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Bytes(Vec<u8>),
}

impl Parameter {
    pub fn as_sql_string(&self, _db_type: DatabaseType) -> String {
        match self {
            Parameter::Null => "NULL".to_string(),
            Parameter::Int(v) => v.to_string(),
            Parameter::Float(v) => v.to_string(),
            Parameter::Bool(v) => v.to_string(),
            Parameter::String(v) => {
                let escaped = v.replace('\'', "''");
                format!("'{}'", escaped)
            }
            Parameter::Bytes(v) => {
                format!("'\\x{}'", hex::encode(v))
            }
        }
    }
}

// 方便的类型转换
impl From<i32> for Parameter {
    fn from(v: i32) -> Self {
        Parameter::Int(v as i64)
    }
}

impl From<i64> for Parameter {
    fn from(v: i64) -> Self {
        Parameter::Int(v)
    }
}

impl From<&str> for Parameter {
    fn from(v: &str) -> Self {
        Parameter::String(v.to_string())
    }
}

impl From<String> for Parameter {
    fn from(v: String) -> Self {
        Parameter::String(v)
    }
}

impl From<bool> for Parameter {
    fn from(v: bool) -> Self {
        Parameter::Bool(v)
    }
}

impl From<f64> for Parameter {
    fn from(v: f64) -> Self {
        Parameter::Float(v)
    }
}

/// 数据库驱动核心 trait
pub trait DatabaseDriver: Send + Sync {
    /// 驱动的数据库类型
    fn db_type(&self) -> DatabaseType;
    
    /// 建立连接
    fn connect(&mut self, config: &ConnectionConfig) -> Result<(), Error>;
    
    /// 关闭连接
    fn close(&mut self) -> Result<(), Error>;
    
    /// 执行查询（返回结果集）
    fn query(&mut self, sql: &str, params: &[Parameter]) -> Result<QueryResult, Error>;
    
    /// 执行命令（不返回结果集）
    fn execute(&mut self, sql: &str, params: &[Parameter]) -> Result<u64, Error>;
    
    /// 准备语句
    fn prepare(&mut self, name: &str, sql: &str) -> Result<(), Error>;
    
    /// 执行已准备的语句
    fn execute_prepared(&mut self, name: &str, params: &[Parameter]) -> Result<QueryResult, Error>;
    
    /// 开始事务
    fn begin(&mut self) -> Result<(), Error>;
    
    /// 提交事务
    fn commit(&mut self) -> Result<(), Error>;
    
    /// 回滚事务
    fn rollback(&mut self) -> Result<(), Error>;
    
    /// 转义标识符（表名、列名）
    fn escape_identifier(&self, ident: &str) -> String;
    
    /// 获取最后插入的 ID
    fn last_insert_id(&mut self) -> Result<Option<i64>, Error>;
    
    /// 连接是否有效
    fn is_connected(&self) -> bool;
    
    /// 获取当前连接的版本信息
    fn version(&mut self) -> Result<String, Error>;
    
    /// 分页查询的 LIMIT/OFFSET 语法
    fn limit_offset_clause(&self, limit: Option<usize>, offset: Option<usize>) -> String;
    
    /// 占位符风格（$1, ?, :name, 等）
    fn placeholder_style(&self) -> PlaceholderStyle;
}

/// 占位符风格
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaceholderStyle {
    /// PostgreSQL: $1, $2, $3
    DollarNumbered,
    /// MySQL: ?, ?, ?
    Positional,
    /// SQLite: ?, ?, ?
    PositionalSqlite,
    /// 命名占位符: :name, :name2
    Named,
}

/// 驱动工厂 trait
pub trait DriverFactory: Send + Sync {
    fn create(&self) -> Box<dyn DatabaseDriver>;
    fn db_type(&self) -> DatabaseType;
}

// 注册驱动的方式
pub struct DriverRegistry {
    drivers: std::collections::HashMap<DatabaseType, Box<dyn DriverFactory>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        DriverRegistry {
            drivers: std::collections::HashMap::new(),
        }
    }
    
    pub fn register<F>(&mut self, factory: F)
    where
        F: DriverFactory + 'static,
    {
        let db_type = factory.db_type();
        self.drivers.insert(db_type, Box::new(factory));
    }
    
    pub fn get(&self, db_type: DatabaseType) -> Option<&dyn DriverFactory> {
        self.drivers.get(&db_type).map(|f| f.as_ref())
    }
    
    pub fn create_driver(&self, db_type: DatabaseType) -> Option<Box<dyn DatabaseDriver>> {
        self.get(db_type).map(|f| f.create())
    }
}