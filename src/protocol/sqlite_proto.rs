use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
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

enum WhereFilter {
    Simple {
        col_idx: usize,
        operator: String,
        value: String,
    },
    In {
        col_idx: usize,
        values: Vec<String>,
    },
}

impl WhereFilter {
    fn matches(&self, _header: &[&str], values: &[&str]) -> bool {
        match self {
            WhereFilter::Simple {
                col_idx,
                operator,
                value,
            } => {
                if *col_idx >= values.len() {
                    return false;
                }
                let actual = values[*col_idx];
                match operator.as_str() {
                    "=" => actual == value,
                    "!=" | "<>" => actual != value,
                    ">" => actual > value.as_str(),
                    "<" => actual < value.as_str(),
                    ">=" => actual >= value.as_str(),
                    "<=" => actual <= value.as_str(),
                    _ => false,
                }
            }
            WhereFilter::In {
                col_idx,
                values: in_vals,
            } => {
                if *col_idx >= values.len() {
                    return false;
                }
                in_vals.iter().any(|v| v == values[*col_idx])
            }
        }
    }
}

pub struct SqliteConnection {
    db_path: PathBuf,
    file: Option<std::fs::File>,
    page_size: usize,
    page_count: usize,
    tx_backups: std::collections::HashMap<String, String>,
}

impl SqliteConnection {
    pub fn connect(path: &str) -> Result<Self, crate::error::Error> {
        let db_path = PathBuf::from(path);
        let exists = db_path.exists();

        let mut conn = SqliteConnection {
            db_path: db_path.clone(),
            file: None,
            page_size: PAGE_SIZE,
            page_count: 0,
            tx_backups: std::collections::HashMap::new(),
        };

        if exists {
            conn.open_existing()?;
        } else {
            conn.create_new()?;
        }

        Ok(conn)
    }

    fn open_existing(&mut self) -> Result<(), crate::error::Error> {
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

    fn create_new(&mut self) -> Result<(), crate::error::Error> {
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

    pub fn execute_query(&mut self, sql: &str) -> Result<SqliteResult, crate::error::Error> {
        let sql_upper = sql.trim().to_uppercase();

        if sql_upper.starts_with("CREATE TABLE")
            || sql_upper.starts_with("CREATE INDEX")
            || sql_upper.starts_with("CREATE VIEW")
            || sql_upper.starts_with("CREATE TRIGGER")
        {
            return self.execute_ddl(sql);
        }

        if sql_upper.starts_with("INSERT")
            || sql_upper.starts_with("UPDATE")
            || sql_upper.starts_with("DELETE")
            || sql_upper.starts_with("DROP")
            || sql_upper.starts_with("ALTER")
        {
            return self.execute_dml(sql);
        }

        if sql_upper.starts_with("SELECT")
            || sql_upper.starts_with("PRAGMA")
            || sql_upper.starts_with("EXPLAIN")
        {
            return self.execute_select(sql);
        }

        if sql_upper.starts_with("BEGIN") {
            self.begin_transaction()?;
            return Ok(SqliteResult::Done(0));
        }
        if sql_upper.starts_with("COMMIT") {
            self.commit_transaction()?;
            return Ok(SqliteResult::Done(0));
        }
        if sql_upper.starts_with("ROLLBACK") {
            self.rollback_transaction()?;
            return Ok(SqliteResult::Done(0));
        }

        Ok(SqliteResult::Done(0))
    }

    fn execute_ddl(&mut self, sql: &str) -> Result<SqliteResult, crate::error::Error> {
        let sql_upper = sql.trim().to_uppercase();
        if sql_upper.starts_with("CREATE TABLE") {
            let table_name = self.extract_create_table_name(sql)?;
            self.store_schema(&table_name, sql)?;
        }
        Ok(SqliteResult::Done(0))
    }

    fn execute_dml(&mut self, sql: &str) -> Result<SqliteResult, crate::error::Error> {
        let sql_upper = sql.trim().to_uppercase();
        if sql_upper.starts_with("INSERT") {
            let table_name = self.extract_table_name_from_insert(sql)?;
            let columns = self.extract_columns_from_insert(sql)?;
            let values = self.extract_values_from_insert(sql)?;
            self.store_row(&table_name, &columns, &values)?;
            Ok(SqliteResult::Done(1))
        } else if sql_upper.starts_with("DELETE") {
            let table_name = self.extract_table_name_from_delete(sql)?;
            let count = self.delete_rows(&table_name, sql)?;
            Ok(SqliteResult::Done(count as u64))
        } else if sql_upper.starts_with("UPDATE") {
            let table_name = self.extract_table_name_from_update(sql)?;
            let count = self.update_rows(&table_name, sql)?;
            Ok(SqliteResult::Done(count as u64))
        } else {
            Ok(SqliteResult::Done(0))
        }
    }

    fn execute_select(&mut self, sql: &str) -> Result<SqliteResult, crate::error::Error> {
        let sql_upper = sql.trim().to_uppercase();
        if sql_upper.starts_with("SELECT") {
            if !sql_upper.contains("FROM") {
                let col_name = sql[6..]
                    .trim()
                    .trim_end_matches(|c: char| c == ';')
                    .to_string();
                let col_info = SqliteColumnInfo {
                    name: col_name.clone(),
                    data_type: "TEXT".to_string(),
                };
                return Ok(SqliteResult::Rows(
                    vec![vec!["0".to_string()]],
                    vec![col_info],
                ));
            }

            if sql_upper.contains("COUNT(*)") || sql_upper.contains("COUNT(") {
                let table_name = self.extract_table_name_from_select(sql)?;
                let count = self.count_rows(&table_name)?;
                let col_info = SqliteColumnInfo {
                    name: "COUNT(*)".to_string(),
                    data_type: "INTEGER".to_string(),
                };
                return Ok(SqliteResult::Rows(
                    vec![vec![count.to_string()]],
                    vec![col_info],
                ));
            }

            let table_name = self.extract_table_name_from_select(sql)?;
            let columns = self.extract_select_columns(sql)?;
            let rows = self.read_rows(&table_name, &columns, sql)?;

            let col_infos: Vec<SqliteColumnInfo> = if columns.len() == 1 && columns[0] == "*" {
                self.read_header_columns(&table_name)?
            } else {
                columns
                    .iter()
                    .map(|c| SqliteColumnInfo {
                        name: c.clone(),
                        data_type: "TEXT".to_string(),
                    })
                    .collect()
            };

            Ok(SqliteResult::Rows(rows, col_infos))
        } else {
            Ok(SqliteResult::Rows(Vec::new(), Vec::new()))
        }
    }

    fn begin_transaction(&mut self) -> Result<(), crate::error::Error> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let prefix = self
            .db_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();

        if let Ok(entries) = fs::read_dir(schema_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if fname.starts_with(prefix.as_ref()) && fname.ends_with(".data") {
                        let path = entry.path();
                        if let Ok(content) = fs::read_to_string(&path) {
                            self.tx_backups.insert(fname, content);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn commit_transaction(&mut self) -> Result<(), crate::error::Error> {
        self.tx_backups.clear();
        Ok(())
    }

    fn rollback_transaction(&mut self) -> Result<(), crate::error::Error> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        for (fname, content) in &self.tx_backups {
            let path = schema_dir.join(fname);
            fs::write(&path, content)?;
        }
        self.tx_backups.clear();
        Ok(())
    }

    fn read_header_columns(
        &self,
        table_name: &str,
    ) -> Result<Vec<SqliteColumnInfo>, crate::error::Error> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!(
            "{}.{}.data",
            self.db_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy(),
            table_name
        ));

        if !data_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&data_path)?;
        if let Some(header_line) = content.lines().next() {
            Ok(header_line
                .split('\x1f')
                .map(|c| SqliteColumnInfo {
                    name: c.trim().to_string(),
                    data_type: "TEXT".to_string(),
                })
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    fn store_schema(&mut self, table_name: &str, sql: &str) -> Result<(), crate::error::Error> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let schema_path = schema_dir.join(format!(
            "{}.schema",
            self.db_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        ));

        let mut content = if schema_path.exists() {
            fs::read_to_string(&schema_path)?
        } else {
            String::new()
        };

        if !content.contains(&format!("[{}]", table_name)) {
            content.push_str(&format!("[{}]\n{}\n\n", table_name, sql));
            fs::write(&schema_path, content)?;
        }

        let data_path = schema_dir.join(format!(
            "{}.{}.data",
            self.db_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy(),
            table_name
        ));

        if !data_path.exists() {
            fs::write(&data_path, "")?;
        }

        Ok(())
    }

    fn store_row(
        &mut self,
        table_name: &str,
        columns: &[String],
        values: &[String],
    ) -> Result<(), crate::error::Error> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!(
            "{}.{}.data",
            self.db_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy(),
            table_name
        ));

        let mut content = if data_path.exists() {
            fs::read_to_string(&data_path)?
        } else {
            String::new()
        };

        let has_id = columns.iter().any(|c| c.to_lowercase() == "id");
        let (final_columns, mut final_values): (Vec<String>, Vec<String>) = if has_id {
            (columns.to_vec(), values.to_vec())
        } else {
            let mut cols = vec!["id".to_string()];
            cols.extend(columns.iter().cloned());
            let row_count = if content.is_empty() {
                0
            } else {
                content.lines().count()
            };
            let mut vals = vec![row_count.to_string()];
            vals.extend(values.iter().cloned());
            (cols, vals)
        };

        let header = final_columns.join("\x1f");
        if !content.starts_with(&header) {
            content = format!("{}\n", header);
        }

        for (i, col) in final_columns.iter().enumerate() {
            let col_lower = col.to_lowercase();
            if col_lower == "id" && i < final_values.len() && final_values[i] == "0" {
                let row_count = content.lines().count();
                final_values[i] = row_count.to_string();
            }
        }

        let row = final_values.join("\x1f");
        content.push_str(&row);
        content.push('\n');

        fs::write(&data_path, content)?;
        Ok(())
    }

    fn read_rows(
        &mut self,
        table_name: &str,
        columns: &[String],
        sql: &str,
    ) -> Result<Vec<Vec<String>>, crate::error::Error> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!(
            "{}.{}.data",
            self.db_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy(),
            table_name
        ));

        if !data_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&data_path)?;
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return Ok(Vec::new());
        }

        let header: Vec<&str> = lines[0].split('\x1f').collect();
        let col_indices: Vec<usize> = if columns.len() == 1 && columns[0] == "*" {
            (0..header.len()).collect()
        } else {
            columns
                .iter()
                .filter_map(|c| header.iter().position(|h| h.trim() == c.trim()))
                .collect()
        };

        let where_filters = self.parse_where_clause(sql, &header);

        let mut rows = Vec::new();
        for line in &lines[1..] {
            if line.is_empty() {
                continue;
            }
            let values: Vec<&str> = line.split('\x1f').collect();

            if !where_filters.is_empty() {
                let all_match = where_filters.iter().all(|f| f.matches(&header, &values));
                if !all_match {
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

    fn parse_where_clause(&self, sql: &str, header: &[&str]) -> Vec<WhereFilter> {
        let sql_upper = sql.to_uppercase();
        let mut filters = Vec::new();

        if let Some(where_pos) = sql_upper.find("WHERE") {
            let rest = &sql[where_pos + 5..].trim();
            let rest_upper = &sql_upper[where_pos + 5..].trim();

            let cond_end = rest_upper
                .find("ORDER")
                .or_else(|| rest_upper.find("LIMIT"))
                .unwrap_or(rest.len());
            let where_part = &rest[..cond_end].trim();

            let parts: Vec<&str> = where_part.split("AND").collect();

            for part in parts.iter() {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                if let Some(filter) = self.parse_condition(part, header) {
                    filters.push(filter);
                }
            }
        }

        filters
    }

    fn parse_condition(&self, cond: &str, header: &[&str]) -> Option<WhereFilter> {
        let cond = cond.trim();
        let cond_upper = cond.to_uppercase();

        if let Some(in_pos) = cond_upper.find("IN") {
            let col_name = cond[..in_pos]
                .trim()
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'');
            let after_in = &cond[in_pos + 2..].trim();
            let paren_start = after_in.find('(')?;
            let paren_end = after_in.rfind(')')?;
            let values_str = &after_in[paren_start + 1..paren_end];
            let values: Vec<String> = values_str
                .split(',')
                .map(|v| {
                    v.trim()
                        .trim_matches(|c: char| c == '\'' || c == '"')
                        .to_string()
                })
                .collect();
            if let Some(col_idx) = header.iter().position(|h| *h == col_name) {
                return Some(WhereFilter::In { col_idx, values });
            }
        }

        let operators = ["!=", "<>", ">=", "<=", "=", ">", "<"];
        for op in &operators {
            if let Some(pos) = cond.find(op) {
                let col_name = cond[..pos]
                    .trim()
                    .trim_matches(|c: char| c == '"' || c == '`' || c == '\'');
                let value = cond[pos + op.len()..]
                    .trim()
                    .trim_matches(|c: char| c == '"' || c == '\'');
                if let Some(col_idx) = header.iter().position(|h| *h == col_name) {
                    return Some(WhereFilter::Simple {
                        col_idx,
                        operator: op.to_string(),
                        value: value.to_string(),
                    });
                }
            }
        }
        None
    }

    pub fn count_rows(&self, table_name: &str) -> Result<usize, crate::error::Error> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!(
            "{}.{}.data",
            self.db_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy(),
            table_name
        ));

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

    fn delete_rows(&mut self, table_name: &str, sql: &str) -> Result<usize, crate::error::Error> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!(
            "{}.{}.data",
            self.db_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy(),
            table_name
        ));

        if !data_path.exists() {
            return Ok(0);
        }

        let content = fs::read_to_string(&data_path)?;
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return Ok(0);
        }

        let header: Vec<&str> = lines[0].split('\x1f').collect();
        let where_filters = self.parse_where_clause(sql, &header);

        let mut deleted = 0;
        let mut new_content = format!("{}\n", lines[0]);

        for line in &lines[1..] {
            if line.is_empty() {
                continue;
            }
            let values: Vec<&str> = line.split('\x1f').collect();

            let should_delete = if where_filters.is_empty() {
                true
            } else {
                where_filters.iter().all(|f| f.matches(&header, &values))
            };

            if should_delete {
                deleted += 1;
            } else {
                new_content.push_str(line);
                new_content.push('\n');
            }
        }

        fs::write(&data_path, new_content)?;
        Ok(deleted)
    }

    fn extract_create_table_name(&self, sql: &str) -> Result<String, crate::error::Error> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("CREATE TABLE") {
            let rest = &sql[pos + 13..].trim();
            if rest.starts_with("IF NOT EXISTS") {
                let rest = rest[14..].trim();
                let name = rest
                    .split(|c: char| c.is_whitespace() || c == '(')
                    .next()
                    .unwrap_or("unknown")
                    .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                    .to_string();
                return Ok(name);
            }
            let name = rest
                .split(|c: char| c.is_whitespace() || c == '(')
                .next()
                .unwrap_or("unknown")
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                .to_string();
            return Ok(name);
        }
        Err("Cannot extract table name".into())
    }

    fn extract_table_name_from_insert(&self, sql: &str) -> Result<String, crate::error::Error> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("INTO") {
            let rest = &sql[pos + 4..].trim();
            let name = rest
                .split(|c: char| c.is_whitespace() || c == '(')
                .next()
                .unwrap_or("unknown")
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                .to_string();
            return Ok(name);
        }
        Err("Cannot extract table name".into())
    }

    fn extract_table_name_from_delete(&self, sql: &str) -> Result<String, crate::error::Error> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("FROM") {
            let rest = &sql[pos + 4..].trim();
            let name = rest
                .split(|c: char| c.is_whitespace() || c == ';')
                .next()
                .unwrap_or("unknown")
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                .to_string();
            return Ok(name);
        }
        Err("Cannot extract table name".into())
    }

    fn extract_table_name_from_update(&self, sql: &str) -> Result<String, crate::error::Error> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("UPDATE") {
            let rest = &sql[pos + 6..].trim();
            let name = rest
                .split(|c: char| c.is_whitespace())
                .next()
                .unwrap_or("unknown")
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                .to_string();
            return Ok(name);
        }
        Err("Cannot extract table name".into())
    }

    fn update_rows(&mut self, table_name: &str, sql: &str) -> Result<usize, crate::error::Error> {
        let schema_dir = self.db_path.parent().unwrap_or(std::path::Path::new("."));
        let data_path = schema_dir.join(format!(
            "{}.{}.data",
            self.db_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy(),
            table_name
        ));

        if !data_path.exists() {
            return Ok(0);
        }

        let content = fs::read_to_string(&data_path)?;
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return Ok(0);
        }

        let header: Vec<&str> = lines[0].split('\x1f').collect();
        let where_filters = self.parse_where_clause(sql, &header);
        let set_pairs = self.parse_set_clause(sql, &header);

        let mut updated = 0;
        let mut new_content = format!("{}\n", lines[0]);

        for line in &lines[1..] {
            if line.is_empty() {
                continue;
            }
            let values: Vec<&str> = line.split('\x1f').collect();

            let should_update = if where_filters.is_empty() {
                true
            } else {
                where_filters.iter().all(|f| f.matches(&header, &values))
            };

            if should_update {
                let mut new_values: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                for (col_idx, new_val) in &set_pairs {
                    if *col_idx < new_values.len() {
                        new_values[*col_idx] = new_val.clone();
                    }
                }
                new_content.push_str(&new_values.join("\x1f"));
                new_content.push('\n');
                updated += 1;
            } else {
                new_content.push_str(line);
                new_content.push('\n');
            }
        }

        fs::write(&data_path, new_content)?;
        Ok(updated)
    }

    fn parse_set_clause(&self, sql: &str, header: &[&str]) -> Vec<(usize, String)> {
        let sql_upper = sql.to_uppercase();
        let mut pairs = Vec::new();

        if let Some(set_pos) = sql_upper.find("SET") {
            let after_set = &sql[set_pos + 3..].trim();
            let set_end = after_set
                .find("WHERE")
                .or_else(|| after_set.find("ORDER"))
                .or_else(|| after_set.find("LIMIT"))
                .unwrap_or(after_set.len());
            let set_part = &after_set[..set_end].trim();

            for part in set_part.split(',') {
                let part = part.trim();
                if let Some(eq_pos) = part.find('=') {
                    let col_name = part[..eq_pos]
                        .trim()
                        .trim_matches(|c: char| c == '"' || c == '`' || c == '\'');
                    let value = part[eq_pos + 1..]
                        .trim()
                        .trim_matches(|c: char| c == '"' || c == '\'');
                    if let Some(col_idx) = header.iter().position(|h| *h == col_name) {
                        pairs.push((col_idx, value.to_string()));
                    }
                }
            }
        }

        pairs
    }

    fn extract_table_name_from_select(&self, sql: &str) -> Result<String, crate::error::Error> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("FROM") {
            let rest = &sql[pos + 4..].trim();
            let name = rest
                .split(|c: char| c.is_whitespace() || c == ';' || c == ',')
                .next()
                .unwrap_or("unknown")
                .trim_matches(|c: char| c == '"' || c == '`' || c == '\'')
                .to_string();
            return Ok(name);
        }
        Err("Cannot extract table name".into())
    }

    fn extract_columns_from_insert(&self, sql: &str) -> Result<Vec<String>, crate::error::Error> {
        if let Some(start) = sql.find('(') {
            if let Some(end) = sql[start..].find(')') {
                let cols_str = &sql[start + 1..start + end];
                let cols: Vec<String> = cols_str
                    .split(',')
                    .map(|c| {
                        c.trim()
                            .trim_matches(|ch: char| ch == '"' || ch == '`' || ch == '\'')
                            .to_string()
                    })
                    .collect();
                return Ok(cols);
            }
        }
        Ok(Vec::new())
    }

    fn extract_values_from_insert(&self, sql: &str) -> Result<Vec<String>, crate::error::Error> {
        let sql_upper = sql.to_uppercase();
        if let Some(pos) = sql_upper.find("VALUES") {
            let rest = &sql[pos + 6..].trim();
            if let Some(start) = rest.find('(') {
                if let Some(end) = rest[start..].find(')') {
                    let vals_str = &rest[start + 1..start + end];
                    let vals = Self::split_sql_values(vals_str);
                    return Ok(vals);
                }
            }
        }
        Ok(Vec::new())
    }

    fn split_sql_values(input: &str) -> Vec<String> {
        let mut values = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut chars = input.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\'' {
                if in_quotes {
                    if chars.peek() == Some(&'\'') {
                        current.push(c);
                        current.push(chars.next().unwrap());
                    } else {
                        in_quotes = false;
                        current.push(c);
                    }
                } else {
                    in_quotes = true;
                    current.push(c);
                }
            } else if c == ',' && !in_quotes {
                values.push(current.trim().trim_matches('\'').to_string());
                current = String::new();
            } else {
                current.push(c);
            }
        }

        if !current.is_empty() {
            values.push(current.trim().trim_matches('\'').to_string());
        }

        values
    }

    fn extract_select_columns(&self, sql: &str) -> Result<Vec<String>, crate::error::Error> {
        let sql_upper = sql.to_uppercase();
        if let Some(select_pos) = sql_upper.find("SELECT") {
            if let Some(from_pos) = sql_upper.find("FROM") {
                let cols_str = &sql[select_pos + 6..from_pos].trim();
                if *cols_str == "*" {
                    return Ok(vec!["*".to_string()]);
                }
                let cols: Vec<String> = cols_str
                    .split(',')
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
