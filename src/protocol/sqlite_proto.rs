use std::fs;
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::PathBuf;

const SQLITE_HEADER: &[u8] = b"SQLite format 3\0";
const PAGE_SIZE: usize = 4096;

#[derive(Debug, Clone)]
pub struct SqliteColumnInfo {
    pub name: String,
    pub data_type: String,
}

pub enum SqliteResult {
    Rows(Vec<Vec<String>>, Vec<SqliteColumnInfo>),
    Done(u64),
}

struct WhereFilter {
    col_idx: usize,
    operator: String,
    value: String,
}

impl WhereFilter {
    fn matches(&self, _header: &[&str], values: &[&str]) -> bool {
        if self.col_idx >= values.len() {
            return false;
        }
        let actual = values[self.col_idx];
        match self.operator.as_str() {
            "=" => actual == self.value,
            "!=" | "<>" => actual != self.value,
            ">" => actual > self.value.as_str(),
            "<" => actual < self.value.as_str(),
            ">=" => actual >= self.value.as_str(),
            "<=" => actual <= self.value.as_str(),
            _ => false,
        }
    }
}

pub struct SqliteConnection {
    db_path: PathBuf,
    file: Option<std::fs::File>,
    page_size: usize,
    page_count: usize,
}

impl SqliteConnection {
    pub fn connect(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let db_path = PathBuf::from(path);
        let exists = db_path.exists();

        let mut conn = SqliteConnection {
            db_path: db_path.clone(),
            file: None,
            page_size: PAGE_SIZE,
            page_count: 0,
        };

        if exists {
            conn.open_existing()?;
        } else {
            conn.create_new()?;
        }

        Ok(conn)
    }

    fn open_existing(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.db_path)?;

        let mut header = [0u8; 16];
        file.read(&mut header)?;

        if &header != SQLITE_HEADER {
            return Err("Not a valid SQLite database file".into());
        }

        let mut page_size_buf = [0u8; 2];
        file.seek(SeekFrom::Start(16))?;
        file.read(&mut page_size_buf)?;
        self.page_size = u16::from_be_bytes(page_size_buf) as usize;
        if self.page_size == 0 {
            self.page_size = PAGE_SIZE;
        }

        let file_size = file.metadata()?.len() as usize;
        self.page_count = file_size / self.page_size;

        self.file = Some(file);
        Ok(())
    }

    fn create_new(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.db_path)?;

        let mut page = vec![0u8; self.page_size];
        page[..16].copy_from_slice(SQLITE_HEADER);
        page[16..18].copy_from_slice(&(self.page_size as u16).to_be_bytes());
        page[18] = 1;
        page[19] = 1;
        file.write_all(&page)?;
        file.flush()?;

        self.page_count = 1;
        self.file = Some(file);
        Ok(())
    }

    pub fn execute_query(&mut self, sql: &str) -> Result<SqliteResult, Box<dyn std::error::Error>> {
        let sql_upper = sql.trim().to_uppercase();

        if sql_upper.starts_with("CREATE TABLE") || sql_upper.starts_with("CREATE INDEX")
            || sql_upper.starts_with("CREATE VIEW") || sql_upper.starts_with("CREATE TRIGGER") {
            return self.execute_ddl(sql);
        }

        if sql_upper.starts_with("INSERT") || sql_upper.starts_with("UPDATE")
            || sql_upper.starts_with("DELETE") || sql_upper.starts_with("DROP")
            || sql_upper.starts_with("ALTER") {
            return self.execute_dml(sql);
        }

        if sql_upper.starts_with("SELECT") || sql_upper.starts_with("PRAGMA")
            || sql_upper.starts_with("EXPLAIN") {
            return self.execute_select(sql);
        }

        if sql_upper.starts_with("BEGIN") || sql_upper.starts_with("COMMIT")
            || sql_upper.starts_with("ROLLBACK") {
            return Ok(SqliteResult::Done(0));
        }

        Ok(SqliteResult::Done(0))
    }

    fn execute_ddl(&mut self, sql: &str) -> Result<SqliteResult, Box<dyn std::error::Error>> {
        let sql_upper = sql.trim().to_uppercase();
        if sql_upper.starts_with("CREATE TABLE") {
            let table_name = self.extract_create_table_name(sql)?;
            self.store_schema(&table_name, sql)?;
        }
        Ok(SqliteResult::Done(0))
    }

    fn execute_dml(&mut self, sql: &str) -> Result<SqliteResult, Box<dyn std::error::Error>> {
        let sql_upper = sql.trim().to_uppercase();
        if sql_upper.starts_with("INSERT") {
            let table_name = self.extract_table_name_from_insert(sql)?;
            let columns = self.extract_columns_from_insert(sql)?;
            let values = self.extract_values_from_insert(sql)?;
            self.store_row(&table_name, &columns, &values)?;
            Ok(SqliteResult::Done(1))
        } else if sql_upper.starts_with("DELETE") {
            let table_name = self.extract_table_name_from_delete(sql)?;
            self.delete_rows(&table_name, sql)?;
            Ok(SqliteResult::Done(0))
        } else if sql_upper.starts_with("UPDATE") {
            Ok(SqliteResult::Done(0))
        } else {
            Ok(SqliteResult::Done(0))
        }
    }

    fn execute_select(&mut self, sql: &str) -> Result<SqliteResult, Box<dyn std::error::Error>> {
        let sql_upper = sql.trim().to_uppercase();
        if sql_upper.starts_with("SELECT") {
            if !sql_upper.contains("FROM") {
                let col_name = sql[6..].trim()
                    .trim_end_matches(|c: char| c == ';')
                    .to_string();
                let col_info = SqliteColumnInfo {
                    name: col_name.clone(),
                    data_type: "TEXT".to_string(),
                };
                return Ok(SqliteResult::Rows(vec![vec!["0".to_string()]], vec![col_info]));
            }

            if sql_upper.contains("COUNT(*)") || sql_upper.contains("COUNT(") {
                let table_name = self.extract_table_name_from_select(sql)?;
                let count = self.count_rows(&table_name)?;
                let col_info = SqliteColumnInfo {
                    name: "COUNT(*)".to_string(),
                    data_type: "INTEGER".to_string(),
                };
                return Ok(SqliteResult::Rows(vec![vec![count.to_string()]], vec![col_info]));
            }

            let table_name = self.extract_table_name_from_select(sql)?;
            let columns = self.extract_select_columns(sql)?;
            let rows = self.read_rows(&table_name, &columns, sql)?;

            let col_infos: Vec<SqliteColumnInfo> = if columns.len() == 1 && columns[0] == "*" {
                self.read_header_columns(&table_name)?
            } else {
                columns.iter().map(|c| {
                    SqliteColumnInfo {
                        name: c.clone(),
                        data_type: "TEXT".to_string(),
                    }
                }).collect()
            };

            Ok(SqliteResult::Rows(rows, col_infos))
        } else {
            Ok(SqliteResult::Rows(Vec::new(), Vec::new()))
        }
    }

    fn read_header_columns(&self, table_name: &str) -> Result<Vec<SqliteColumnInfo>, Box<dyn std::error::Error>> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!("{}.{}.data",
            self.db_path.file_stem().unwrap_or_default().to_string_lossy(),
            table_name));

        if !data_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&data_path)?;
        if let Some(header_line) = content.lines().next() {
            Ok(header_line.split('|')
                .map(|c| SqliteColumnInfo {
                    name: c.trim().to_string(),
                    data_type: "TEXT".to_string(),
                })
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    fn store_schema(&mut self, table_name: &str, sql: &str) -> Result<(), Box<dyn std::error::Error>> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let schema_path = schema_dir.join(format!("{}.schema", 
            self.db_path.file_stem().unwrap_or_default().to_string_lossy()));
        
        let mut content = if schema_path.exists() {
            fs::read_to_string(&schema_path)?
        } else {
            String::new()
        };

        if !content.contains(&format!("[{}]", table_name)) {
            content.push_str(&format!("[{}]\n{}\n\n", table_name, sql));
            fs::write(&schema_path, content)?;
        }

        let data_path = schema_dir.join(format!("{}.{}.data",
            self.db_path.file_stem().unwrap_or_default().to_string_lossy(),
            table_name));
        
        if !data_path.exists() {
            fs::write(&data_path, "")?;
        }

        Ok(())
    }

    fn store_row(&mut self, table_name: &str, columns: &[String], values: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!("{}.{}.data",
            self.db_path.file_stem().unwrap_or_default().to_string_lossy(),
            table_name));

        let mut content = if data_path.exists() {
            fs::read_to_string(&data_path)?
        } else {
            String::new()
        };

        let header = columns.join("|");
        if !content.starts_with(&header) {
            content = format!("{}\n{}", header, content);
        }

        let row_count = if content.is_empty() { 0 } else { content.lines().count() };

        let mut final_values: Vec<String> = values.to_vec();
        for (i, col) in columns.iter().enumerate() {
            let col_lower = col.to_lowercase();
            if col_lower == "id" && i < final_values.len() && final_values[i] == "0" {
                final_values[i] = row_count.to_string();
            }
        }

        let row = final_values.join("|");
        content.push_str(&row);
        content.push('\n');

        fs::write(&data_path, content)?;
        Ok(())
    }

    fn read_rows(&mut self, table_name: &str, columns: &[String], sql: &str) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!("{}.{}.data",
            self.db_path.file_stem().unwrap_or_default().to_string_lossy(),
            table_name));

        if !data_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&data_path)?;
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return Ok(Vec::new());
        }

        let header: Vec<&str> = lines[0].split('|').collect();
        let col_indices: Vec<usize> = if columns.len() == 1 && columns[0] == "*" {
            (0..header.len()).collect()
        } else {
            columns.iter()
                .filter_map(|c| header.iter().position(|h| h.trim() == c.trim()))
                .collect()
        };

        let where_filter = self.parse_where_clause(sql, &header);

        let mut rows = Vec::new();
        for line in &lines[1..] {
            if line.is_empty() { continue; }
            let values: Vec<&str> = line.split('|').collect();

            if let Some(ref filter) = where_filter {
                if !filter.matches(&header, &values) {
                    continue;
                }
            }

            let mut row = Vec::new();
            for &idx in &col_indices {
                if idx < values.len() {
                    row.push(values[idx].to_string());
                } else {
                    row.push("NULL".to_string());
                }
            }
            if !row.is_empty() {
                rows.push(row);
            }
        }

        Ok(rows)
    }

    fn parse_where_clause(&self, sql: &str, header: &[&str]) -> Option<WhereFilter> {
        let sql_upper = sql.to_uppercase();
        if let Some(where_pos) = sql_upper.find("WHERE") {
            let rest = &sql[where_pos + 5..].trim();
            let rest_upper = &sql_upper[where_pos + 5..].trim();

            if let Some(and_pos) = rest_upper.find("AND") {
                let cond = &rest[..and_pos].trim();
                return self.parse_condition(cond, header);
            }

            if let Some(or_pos) = rest_upper.find("OR") {
                let cond = &rest[..or_pos].trim();
                return self.parse_condition(cond, header);
            }

            let cond = rest.trim_end_matches(|c: char| c == ';' || c == ' ');
            return self.parse_condition(cond, header);
        }
        None
    }

    fn parse_condition(&self, cond: &str, header: &[&str]) -> Option<WhereFilter> {
        let cond = cond.trim();
        let operators = ["=", "!=", "<>", ">=", "<=", ">", "<"];
        for op in &operators {
            if let Some(pos) = cond.find(op) {
                let col_name = cond[..pos].trim().trim_matches(|c: char| c == '"' || c == '`' || c == '\'');
                let value = cond[pos + op.len()..].trim().trim_matches(|c: char| c == '"' || c == '\'');
                if let Some(col_idx) = header.iter().position(|h| *h == col_name) {
                    return Some(WhereFilter {
                        col_idx,
                        operator: op.to_string(),
                        value: value.to_string(),
                    });
                }
            }
        }
        None
    }

    pub fn count_rows(&self, table_name: &str) -> Result<usize, Box<dyn std::error::Error>> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!("{}.{}.data",
            self.db_path.file_stem().unwrap_or_default().to_string_lossy(),
            table_name));

        if !data_path.exists() {
            return Ok(0);
        }

        let content = fs::read_to_string(&data_path)?;
        let count = content.lines().count();
        if count > 0 {
            Ok(count - 1)
        } else {
            Ok(0)
        }
    }

    fn delete_rows(&mut self, table_name: &str, _sql: &str) -> Result<(), Box<dyn std::error::Error>> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!("{}.{}.data",
            self.db_path.file_stem().unwrap_or_default().to_string_lossy(),
            table_name));

        if data_path.exists() {
            let content = fs::read_to_string(&data_path)?;
            let lines: Vec<&str> = content.lines().collect();
            if !lines.is_empty() {
                let header = lines[0];
                fs::write(&data_path, format!("{}\n", header))?;
            }
        }

        Ok(())
    }

    fn extract_create_table_name(&self, sql: &str) -> Result<String, Box<dyn std::error::Error>> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("CREATE TABLE") {
            let rest = &sql[pos + 13..].trim();
            if rest.starts_with("IF NOT EXISTS") {
                let rest = rest[14..].trim();
                let name = rest.split(|c: char| c.is_whitespace() || c == '(')
                    .next()
                    .unwrap_or("unknown")
                    .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                    .to_string();
                return Ok(name);
            }
            let name = rest.split(|c: char| c.is_whitespace() || c == '(')
                .next()
                .unwrap_or("unknown")
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                .to_string();
            return Ok(name);
        }
        Err("Cannot extract table name".into())
    }

    fn extract_table_name_from_insert(&self, sql: &str) -> Result<String, Box<dyn std::error::Error>> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("INTO") {
            let rest = &sql[pos + 4..].trim();
            let name = rest.split(|c: char| c.is_whitespace() || c == '(')
                .next()
                .unwrap_or("unknown")
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                .to_string();
            return Ok(name);
        }
        Err("Cannot extract table name".into())
    }

    fn extract_table_name_from_delete(&self, sql: &str) -> Result<String, Box<dyn std::error::Error>> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("FROM") {
            let rest = &sql[pos + 4..].trim();
            let name = rest.split(|c: char| c.is_whitespace() || c == ';')
                .next()
                .unwrap_or("unknown")
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                .to_string();
            return Ok(name);
        }
        Err("Cannot extract table name".into())
    }

    fn extract_table_name_from_select(&self, sql: &str) -> Result<String, Box<dyn std::error::Error>> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("FROM") {
            let rest = &sql[pos + 4..].trim();
            let name = rest.split(|c: char| c.is_whitespace() || c == ';' || c == ',')
                .next()
                .unwrap_or("unknown")
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                .to_string();
            return Ok(name);
        }
        Err("Cannot extract table name".into())
    }

    fn extract_columns_from_insert(&self, sql: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        if let Some(start) = sql.find('(') {
            if let Some(end) = sql[start..].find(')') {
                let cols_str = &sql[start + 1..start + end];
                let cols: Vec<String> = cols_str.split(',')
                    .map(|c| c.trim().trim_matches(|ch: char| ch == '"' || ch == '`' || ch == '\'').to_string())
                    .collect();
                return Ok(cols);
            }
        }
        Ok(Vec::new())
    }

    fn extract_values_from_insert(&self, sql: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("VALUES") {
            let rest = &sql[pos + 6..].trim();
            if let Some(start) = rest.find('(') {
                if let Some(end) = rest[start..].find(')') {
                    let vals_str = &rest[start + 1..start + end];
                    let vals: Vec<String> = vals_str.split(',')
                        .map(|v| v.trim().trim_matches('\'').to_string())
                        .collect();
                    return Ok(vals);
                }
            }
        }
        Ok(Vec::new())
    }

    fn extract_select_columns(&self, sql: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let sql_upper = sql.to_uppercase();
        if let Some(select_pos) = sql_upper.find("SELECT") {
            if let Some(from_pos) = sql_upper.find("FROM") {
                let cols_str = &sql[select_pos + 6..from_pos].trim();
                if *cols_str == "*" {
                    return Ok(vec!["*".to_string()]);
                }
                let cols: Vec<String> = cols_str.split(',')
                    .map(|c| {
                        let c = c.trim();
                        if let Some(as_pos) = c.to_uppercase().rfind(" AS ") {
                            c[as_pos + 4..].trim().to_string()
                        } else if let Some(space_pos) = c.rfind(' ') {
                            let after = c[space_pos + 1..].trim();
                            if after.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
                                after.to_string()
                            } else {
                                c.split('.').last().unwrap_or(c).to_string()
                            }
                        } else {
                            c.split('.').last().unwrap_or(c).to_string()
                        }
                    })
                    .collect();
                return Ok(cols);
            }
        }
        Ok(vec!["*".to_string()])
    }
}