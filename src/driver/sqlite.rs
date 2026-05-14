use super::*;
use crate::protocol::sqlite_proto::{SqliteConnection, SqliteResult};

pub struct SqliteDriver {
    conn: Option<SqliteConnection>,
    db_path: String,
    last_row_id: i64,
}

impl SqliteDriver {
    pub fn new() -> Self {
        SqliteDriver {
            conn: None,
            db_path: String::new(),
            last_row_id: 0,
        }
    }

    fn get_conn(&mut self) -> Result<&mut SqliteConnection, Error> {
        self.conn
            .as_mut()
            .ok_or_else(|| Error::Connection("not connected".into()))
    }

    fn extract_table_name(&self, sql: &str) -> String {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("INTO") {
            let rest = &sql[pos + 4..].trim();
            rest.split(|c: char| c.is_whitespace() || c == '(')
                .next()
                .unwrap_or("unknown")
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                .to_string()
        } else {
            String::new()
        }
    }

    fn substitute_params(sql: &str, params: &[Parameter]) -> String {
        let mut result = sql.to_string();
        for param in params {
            let replacement = param.as_sql_string(DatabaseType::Sqlite);
            if let Some(pos) = result.find('?') {
                result.replace_range(pos..pos + 1, &replacement);
            }
        }
        result
    }
}

impl DatabaseDriver for SqliteDriver {
    fn db_type(&self) -> DatabaseType {
        DatabaseType::Sqlite
    }

    fn connect(&mut self, config: &ConnectionConfig) -> Result<(), Error> {
        let path = if config.host == ":memory:" {
            ":memory:".to_string()
        } else {
            format!("{}.db", config.database)
        };
        self.db_path = path.clone();
        let conn = SqliteConnection::connect(&path)?;
        self.conn = Some(conn);
        Ok(())
    }

    fn close(&mut self) -> Result<(), Error> {
        self.conn = None;
        Ok(())
    }

    fn query(&mut self, sql: &str, params: &[Parameter]) -> Result<QueryResult, Error> {
        let final_sql = Self::substitute_params(sql, params);
        let sql_upper = sql.trim().to_uppercase();
        match self.get_conn()?.execute_query(&final_sql)? {
            SqliteResult::Rows(row_data, columns_info) => {
                let mut rows = Vec::new();
                for row_values in row_data {
                    let mut row = Row::with_capacity(columns_info.len());
                    for (idx, value) in row_values.iter().enumerate() {
                        let col_info = &columns_info[idx];
                        let data_type = DataType::Text;
                        let column = Column::new(&col_info.name, data_type);
                        let val = if value == "NULL" {
                            None
                        } else {
                            Some(value.clone())
                        };
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
            SqliteResult::Done(affected) => {
                if sql_upper.starts_with("INSERT") {
                    let table_name = self.extract_table_name(sql);
                    self.last_row_id = self.get_conn()?.count_rows(&table_name)? as i64;
                }
                Ok(QueryResult {
                    rows: Vec::new(),
                    affected_rows: affected,
                    last_insert_id: None,
                })
            }
        }
    }

    fn execute(&mut self, sql: &str, params: &[Parameter]) -> Result<u64, Error> {
        let result = self.query(sql, params)?;
        Ok(result.affected_rows)
    }

    fn prepare(&mut self, _name: &str, _sql: &str) -> Result<(), Error> {
        Ok(())
    }

    fn execute_prepared(
        &mut self,
        _name: &str,
        _params: &[Parameter],
    ) -> Result<QueryResult, Error> {
        Ok(QueryResult {
            rows: Vec::new(),
            affected_rows: 0,
            last_insert_id: None,
        })
    }

    fn begin(&mut self) -> Result<(), Error> {
        self.get_conn()?.execute_query("BEGIN")?;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), Error> {
        self.get_conn()?.execute_query("COMMIT")?;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), Error> {
        self.get_conn()?.execute_query("ROLLBACK")?;
        Ok(())
    }

    fn escape_identifier(&self, ident: &str) -> String {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn last_insert_id(&mut self) -> Result<Option<i64>, Error> {
        Ok(Some(self.last_row_id))
    }

    fn is_connected(&self) -> bool {
        self.conn.is_some()
    }

    fn version(&mut self) -> Result<String, Error> {
        let result = self.query("SELECT sqlite_version()", &[])?;
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
            (None, Some(o)) => format!("LIMIT -1 OFFSET {}", o),
            (None, None) => String::new(),
        }
    }

    fn placeholder_style(&self) -> PlaceholderStyle {
        PlaceholderStyle::PositionalSqlite
    }
}

pub struct SqliteDriverFactory;

impl DriverFactory for SqliteDriverFactory {
    fn create(&self) -> Box<dyn DatabaseDriver> {
        Box::new(SqliteDriver::new())
    }

    fn db_type(&self) -> DatabaseType {
        DatabaseType::Sqlite
    }
}
