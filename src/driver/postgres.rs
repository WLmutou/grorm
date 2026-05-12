// src/driver/postgres.rs
use super::*;
use crate::protocol::pg::{PgConnection, PgResult};
use std::net::ToSocketAddrs;

pub struct PostgresDriver {
    conn: Option<PgConnection>,
    config: Option<ConnectionConfig>,
    prepared_statements: std::collections::HashMap<String, String>,
}

impl PostgresDriver {
    pub fn new() -> Self {
        PostgresDriver {
            conn: None,
            config: None,
            prepared_statements: std::collections::HashMap::new(),
        }
    }
    
    fn get_conn(&mut self) -> &mut PgConnection {
        self.conn.as_mut().expect("Not connected")
    }
    
    #[allow(dead_code)]
    fn build_connection_string(&self, config: &ConnectionConfig) -> String {
        format!(
            "host={} port={} user={} password={} dbname={}",
            config.host, config.port, config.username, config.password, config.database
        )
    }
}

impl DatabaseDriver for PostgresDriver {
    fn db_type(&self) -> DatabaseType {
        DatabaseType::Postgresql
    }
    
    fn connect(&mut self, config: &ConnectionConfig) -> Result<(), Box<dyn Error>> {
        let addr_str = format!("{}:{}", config.host, config.port);
        let addr = addr_str.to_socket_addrs()?.next().ok_or("Failed to resolve address")?;
        
        let conn = PgConnection::connect(addr, &config.username, &config.database)?;
        self.conn = Some(conn);
        self.config = Some(config.clone());
        Ok(())
    }
    
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        self.conn = None;
        Ok(())
    }
    
    fn query(&mut self, sql: &str, params: &[Parameter]) -> Result<QueryResult, Box<dyn Error>> {
        // 将参数转换为字符串值数组
        let param_strs: Vec<String> = params.iter()
            .map(|p| p.as_sql_string(self.db_type()))
            .collect();
        
        // 简单替换占位符（生产环境需要更好的 SQL 构建）
        let mut final_sql = sql.to_string();
        for (i, val) in param_strs.iter().enumerate() {
            let placeholder = format!("${}", i + 1);
            final_sql = final_sql.replace(&placeholder, val);
        }
        
        match self.get_conn().execute_query(&final_sql)? {
            PgResult::Rows(row_data, columns_info) => {
                let mut query_result = QueryResult {
                    rows: Vec::new(),
                    affected_rows: 0,
                    last_insert_id: None,
                };
                
                for row_values in row_data {
                    let mut row = Row::with_capacity(columns_info.len());
                    for (idx, value) in row_values.iter().enumerate() {
                        let col_info = &columns_info[idx];
                        let data_type = Self::pg_type_to_data_type(col_info.data_type);
                        let column = Column::new(&col_info.name, data_type);
                        let val = if value == "NULL" { None } else { Some(value.clone()) };
                        row.push(column, val);
                    }
                    query_result.rows.push(row);
                }
                
                Ok(query_result)
            }
            PgResult::CommandComplete(cmd) => {
                let affected = parse_pg_affected_rows(&cmd);
                Ok(QueryResult {
                    rows: Vec::new(),
                    affected_rows: affected,
                    last_insert_id: None,
                })
            }
            PgResult::Empty => {
                Ok(QueryResult {
                    rows: Vec::new(),
                    affected_rows: 0,
                    last_insert_id: None,
                })
            }
        }
    }
    
    fn execute(&mut self, sql: &str, params: &[Parameter]) -> Result<u64, Box<dyn Error>> {
        let result = self.query(sql, params)?;
        Ok(result.affected_rows)
    }
    
    fn prepare(&mut self, name: &str, sql: &str) -> Result<(), Box<dyn Error>> {
        self.get_conn().prepare(name, sql)?;
        self.prepared_statements.insert(name.to_string(), sql.to_string());
        Ok(())
    }
    
    fn execute_prepared(&mut self, name: &str, params: &[Parameter]) -> Result<QueryResult, Box<dyn Error>> {
        let param_strs: Vec<String> = params.iter()
            .map(|p| p.as_sql_string(self.db_type()))
            .collect();
        
        let param_refs: Vec<&str> = param_strs.iter().map(|s| s.as_str()).collect();
        
        match self.get_conn().execute_prepared(name, &param_refs)? {
            PgResult::Rows(row_data, columns_info) => {
                let mut rows = Vec::new();
                for row_values in row_data {
                    let mut row = Row::with_capacity(columns_info.len());
                    for (idx, value) in row_values.iter().enumerate() {
                        let col_info = &columns_info[idx];
                        let data_type = Self::pg_type_to_data_type(col_info.data_type);
                        let column = Column::new(&col_info.name, data_type);
                        let val = if value == "NULL" { None } else { Some(value.clone()) };
                        row.push(column, val);
                    }
                    rows.push(row);
                }
                Ok(QueryResult {
                    rows,
                    affected_rows: 0,
                    last_insert_id: None,
                })
            }
            _ => Ok(QueryResult {
                rows: Vec::new(),
                affected_rows: 0,
                last_insert_id: None,
            }),
        }
    }
    
    fn begin(&mut self) -> Result<(), Box<dyn Error>> {
        self.get_conn().execute_query("BEGIN")?;
        Ok(())
    }
    
    fn commit(&mut self) -> Result<(), Box<dyn Error>> {
        self.get_conn().execute_query("COMMIT")?;
        Ok(())
    }
    
    fn rollback(&mut self) -> Result<(), Box<dyn Error>> {
        self.get_conn().execute_query("ROLLBACK")?;
        Ok(())
    }
    
    fn escape_identifier(&self, ident: &str) -> String {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }
    
    fn last_insert_id(&mut self) -> Result<Option<i64>, Box<dyn Error>> {
        // PostgreSQL uses RETURNING clause, so last_insert_id might not be directly available
        // This is a fallback using currval
        let result = self.query("SELECT LASTVAL()", &[])?;
        if let Some(row) = result.rows.first() {
            if let Some(val) = row.get(0) {
                return Ok(val.parse().ok());
            }
        }
        Ok(None)
    }
    
    fn is_connected(&self) -> bool {
        self.conn.is_some()
    }
    
    fn version(&mut self) -> Result<String, Box<dyn Error>> {
        let result = self.query("SELECT version()", &[])?;
        if let Some(row) = result.rows.first() {
            if let Some(version) = row.get(0) {
                return Ok(version.to_string());
            }
        }
        Ok("Unknown".to_string())
    }
    
    fn limit_offset_clause(&self, limit: Option<usize>, offset: Option<usize>) -> String {
        match (limit, offset) {
            (Some(l), Some(o)) => format!("LIMIT {} OFFSET {}", l, o),
            (Some(l), None) => format!("LIMIT {}", l),
            (None, Some(o)) => format!("OFFSET {}", o),
            (None, None) => String::new(),
        }
    }
    
    fn placeholder_style(&self) -> PlaceholderStyle {
        PlaceholderStyle::DollarNumbered
    }
}

impl PostgresDriver {
    fn pg_type_to_data_type(oid: i32) -> DataType {
        // PostgreSQL 常用类型 OID
        match oid {
            16 => DataType::Boolean,
            21 => DataType::Int2,
            23 => DataType::Int4,
            20 => DataType::Int8,
            700 => DataType::Float4,
            701 => DataType::Float8,
            25 | 1043 => DataType::Text,
            1042 => DataType::Char(0),
            1082 => DataType::Date,
            1083 => DataType::Time,
            1114 => DataType::Timestamp,
            114 => DataType::Json,
            3802 => DataType::Jsonb,
            2950 => DataType::Uuid,
            17 => DataType::Bytea,
            _ => DataType::Custom(format!("pg_oid_{}", oid)),
        }
    }
}

// 辅助函数
fn parse_pg_affected_rows(cmd: &str) -> u64 {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() >= 2 {
        if let Ok(count) = parts[1].parse() {
            return count;
        }
    }
    if let Ok(count) = parts.last().unwrap_or(&"0").parse() {
        count
    } else {
        0
    }
}

// 驱动工厂
pub struct PostgresDriverFactory;

impl DriverFactory for PostgresDriverFactory {
    fn create(&self) -> Box<dyn DatabaseDriver> {
        Box::new(PostgresDriver::new())
    }
    
    fn db_type(&self) -> DatabaseType {
        DatabaseType::Postgresql
    }
}