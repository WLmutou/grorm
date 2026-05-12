use super::*;

pub struct MysqlDriver {
    connected: bool,
}

impl MysqlDriver {
    pub fn new() -> Self {
        MysqlDriver { connected: false }
    }
}

impl DatabaseDriver for MysqlDriver {
    fn db_type(&self) -> DatabaseType {
        DatabaseType::Mysql
    }

    fn connect(&mut self, _config: &ConnectionConfig) -> Result<(), Box<dyn Error>> {
        self.connected = true;
        Ok(())
    }

    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        self.connected = false;
        Ok(())
    }

    fn query(&mut self, _sql: &str, _params: &[Parameter]) -> Result<QueryResult, Box<dyn Error>> {
        Ok(QueryResult {
            rows: Vec::new(),
            affected_rows: 0,
            last_insert_id: None,
        })
    }

    fn execute(&mut self, _sql: &str, _params: &[Parameter]) -> Result<u64, Box<dyn Error>> {
        Ok(0)
    }

    fn prepare(&mut self, _name: &str, _sql: &str) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute_prepared(&mut self, _name: &str, _params: &[Parameter]) -> Result<QueryResult, Box<dyn Error>> {
        Ok(QueryResult {
            rows: Vec::new(),
            affected_rows: 0,
            last_insert_id: None,
        })
    }

    fn begin(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn commit(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn escape_identifier(&self, ident: &str) -> String {
        format!("`{}`", ident.replace('`', "``"))
    }

    fn last_insert_id(&mut self) -> Result<Option<i64>, Box<dyn Error>> {
        Ok(None)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn version(&mut self) -> Result<String, Box<dyn Error>> {
        Ok("MySQL 8.0".to_string())
    }

    fn limit_offset_clause(&self, limit: Option<usize>, offset: Option<usize>) -> String {
        match (limit, offset) {
            (Some(l), Some(o)) => format!("LIMIT {}, {}", o, l),
            (Some(l), None) => format!("LIMIT {}", l),
            (None, Some(o)) => format!("LIMIT {}, 18446744073709551615", o),
            (None, None) => String::new(),
        }
    }

    fn placeholder_style(&self) -> PlaceholderStyle {
        PlaceholderStyle::Positional
    }
}

pub struct MysqlDriverFactory;

impl DriverFactory for MysqlDriverFactory {
    fn create(&self) -> Box<dyn DatabaseDriver> {
        Box::new(MysqlDriver::new())
    }

    fn db_type(&self) -> DatabaseType {
        DatabaseType::Mysql
    }
}