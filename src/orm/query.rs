use crate::driver::{DatabaseDriver, DatabaseType, Parameter, Row};
use crate::orm::model::Model;
use crate::types::Value;
use crate::error::Error;

/// SQL JOIN type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    /// `LEFT JOIN` — returns all rows from the left table
    Left,
    /// `INNER JOIN` — returns only matching rows from both tables
    Inner,
    /// `RIGHT JOIN` — returns all rows from the right table
    Right,
}

impl JoinType {
    pub fn as_sql(&self) -> &'static str {
        match self {
            JoinType::Left => "LEFT JOIN",
            JoinType::Inner => "INNER JOIN",
            JoinType::Right => "RIGHT JOIN",
        }
    }
}

/// A JOIN clause with type, target table, and ON condition.
#[derive(Debug, Clone)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub table: String,
    pub on_clause: String,
}

/// Chainable query builder for a model `M`.
///
/// Provides a fluent API for building and executing SQL queries.
/// All chain methods return `&mut Self` for method chaining.
///
/// # Example
///
/// ```rust,ignore
/// use grorm::{QueryBuilder, Value};
///
/// let mut qb = QueryBuilder::<User>::new(driver);
///
/// // Chainable query
/// let users = qb
///     .where_eq("age", Value::from(30))
///     .order("name", true)
///     .limit(10)
///     .offset(0)
///     .find()?;
///
/// // Chainable update
/// let rows = qb
///     .where_eq("id", Value::from(1))
///     .update_one("age", Value::from(31))?;
///
/// // Chainable delete
/// let rows = qb
///     .where_eq("id", Value::from(99))
///     .delete()?;
/// ```
pub struct QueryBuilder<'a, M: Model> {
    driver: &'a mut dyn DatabaseDriver,
    _marker: std::marker::PhantomData<M>,
    conditions: Vec<(String, Vec<Parameter>)>,
    limit_val: Option<usize>,
    offset_val: Option<usize>,
    order_by: Vec<(String, bool)>,
    joins: Vec<JoinClause>,
}

impl<'a, M: Model> QueryBuilder<'a, M> {
    /// Creates a new `QueryBuilder` with the given database driver.
    pub fn new(driver: &'a mut dyn DatabaseDriver) -> Self {
        QueryBuilder {
            driver,
            _marker: std::marker::PhantomData,
            conditions: Vec::new(),
            limit_val: None,
            offset_val: None,
            order_by: Vec::new(),
            joins: Vec::new(),
        }
    }

    fn reset_chain(&mut self) {
        self.conditions.clear();
        self.limit_val = None;
        self.offset_val = None;
        self.order_by.clear();
        self.joins.clear();
    }

    /// Returns the table name for the model.
    pub fn table_name(&self) -> &str {
        M::table_name()
    }

    /// Adds a `WHERE column = value` condition.
    ///
    /// ```rust,ignore
    /// qb.where_eq("name", Value::from("Alice")).find()?;
    /// ```
    pub fn where_eq(&mut self, column: &str, value: Value) -> &mut Self {
        self.conditions.push((format!("{} = ?", column), vec![value_to_param(&value)]));
        self
    }

    /// Adds WHERE conditions from non-zero/non-empty fields of a model.
    ///
    /// ```rust,ignore
    /// let filter = User { id: 1, name: "".into(), email: "".into(), age: 0 };
    /// qb.where_model(&filter).find()?;
    /// // SELECT * FROM users WHERE id = 1
    /// ```
    pub fn where_model(&mut self, model: &M) -> &mut Self {
        let values = model.to_values();
        let columns = M::columns();
        for (i, col) in columns.iter().enumerate() {
            if !is_value_default(&values[i]) {
                self.conditions.push((format!("{} = ?", col), vec![value_to_param(&values[i])]));
            }
        }
        self
    }

    /// Adds a `WHERE column IN (v1, v2, ...)` condition.
    ///
    /// ```rust,ignore
    /// qb.where_in("name", vec![Value::from("Alice"), Value::from("Bob")]).find()?;
    /// ```
    pub fn where_in(&mut self, column: &str, values: Vec<Value>) -> &mut Self {
        if values.is_empty() {
            self.conditions.push(("1 = 0".to_string(), vec![]));
            return self;
        }
        let placeholders: Vec<&str> = vec!["?"; values.len()];
        let clause = format!("{} IN ({})", column, placeholders.join(", "));
        let params: Vec<Parameter> = values.iter().map(|v| value_to_param(v)).collect();
        self.conditions.push((clause, params));
        self
    }

    /// Sets the LIMIT clause.
    pub fn limit(&mut self, n: usize) -> &mut Self {
        self.limit_val = Some(n);
        self
    }

    /// Sets the OFFSET clause.
    pub fn offset(&mut self, n: usize) -> &mut Self {
        self.offset_val = Some(n);
        self
    }

    /// Adds an ORDER BY clause. `asc = true` for ascending, `false` for descending.
    pub fn order(&mut self, column: &str, asc: bool) -> &mut Self {
        self.order_by.push((column.to_string(), asc));
        self
    }

    /// Adds a LEFT JOIN clause.
    ///
    /// ```rust,ignore
    /// qb.left_join("orders", "users.id = orders.user_id").find()?;
    /// ```
    pub fn left_join(&mut self, table: &str, on_clause: &str) -> &mut Self {
        self.joins.push(JoinClause {
            join_type: JoinType::Left,
            table: table.to_string(),
            on_clause: on_clause.to_string(),
        });
        self
    }

    /// Adds an INNER JOIN clause.
    pub fn inner_join(&mut self, table: &str, on_clause: &str) -> &mut Self {
        self.joins.push(JoinClause {
            join_type: JoinType::Inner,
            table: table.to_string(),
            on_clause: on_clause.to_string(),
        });
        self
    }

    /// Adds a RIGHT JOIN clause.
    pub fn right_join(&mut self, table: &str, on_clause: &str) -> &mut Self {
        self.joins.push(JoinClause {
            join_type: JoinType::Right,
            table: table.to_string(),
            on_clause: on_clause.to_string(),
        });
        self
    }

    /// Executes the query and returns all matching rows.
    /// Resets chain conditions after execution.
    pub fn find(&mut self) -> Result<Vec<M>, Error> {
        let (sql, params) = self.build_select_sql();
        let result = self.driver.query(&sql, &params)?;
        self.reset_chain();
        self.parse_rows(&result.rows)
    }

    /// Executes the query and returns the first matching row, if any.
    /// Resets chain conditions after execution.
    pub fn find_one(&mut self) -> Result<Option<M>, Error> {
        self.limit_val = Some(1);
        let (sql, params) = self.build_select_sql();
        let result = self.driver.query(&sql, &params)?;
        self.reset_chain();
        if result.rows.is_empty() {
            Ok(None)
        } else {
            let vals = self.row_to_values(&result.rows[0]);
            M::from_row(&vals).map(Some).map_err(|e| e.into())
        }
    }

    /// Returns the count of rows matching the current conditions.
    /// Resets chain conditions after execution.
    pub fn count(&mut self) -> Result<i64, Error> {
        let mut sql = format!("SELECT COUNT(*) FROM {}", M::table_name());
        let params = self.flatten_conditions(&mut sql);
        self.reset_chain();
        let result = self.driver.query(&sql, &params)?;
        if let Some(row) = result.rows.first() {
            if let Some(val) = row.get(0) {
                return Ok(val.parse().unwrap_or(0));
            }
        }
        Ok(0)
    }

    pub fn build_select_sql(&self) -> (String, Vec<Parameter>) {
        let mut sql = format!("SELECT * FROM {}", M::table_name());

        for join in &self.joins {
            sql.push_str(&format!(
                " {} {} ON {}",
                join.join_type.as_sql(),
                join.table,
                join.on_clause
            ));
        }

        let params = self.flatten_conditions(&mut sql);

        if !self.order_by.is_empty() {
            let orders: Vec<String> = self.order_by.iter().map(|(col, asc)| {
                if *asc { format!("{} ASC", col) } else { format!("{} DESC", col) }
            }).collect();
            sql.push_str(&format!(" ORDER BY {}", orders.join(", ")));
        }

        if let Some(limit) = self.limit_val {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = self.offset_val {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        (sql, params)
    }

    fn flatten_conditions(&self, sql: &mut String) -> Vec<Parameter> {
        if self.conditions.is_empty() {
            return Vec::new();
        }
        let clauses: Vec<String> = self.conditions.iter().map(|(c, _)| c.clone()).collect();
        let mut params = Vec::new();
        for (_, p) in &self.conditions {
            params.extend(p.clone());
        }
        sql.push_str(&format!(" WHERE {}", clauses.join(" AND ")));
        params
    }

    /// Returns all rows from the table without any conditions.
    /// Resets chain conditions after execution.
    pub fn find_all(&mut self) -> Result<Vec<M>, Error> {
        let (sql, params) = self.build_select_sql();
        let result = self.driver.query(&sql, &params)?;
        self.reset_chain();
        self.parse_rows(&result.rows)
    }

    /// Creates the table for this model if it does not exist.
    ///
    /// Automatically generates DDL from the model's schema, including:
    /// - Column types mapped to the target database
    /// - Primary key constraints (including composite)
    /// - Regular indexes (`#[index]`)
    /// - Unique constraints (`#[unique]`, `#[unique_index = "name"]`)
    pub fn create_table(&mut self) -> Result<(), Error> {
        let schema = M::table_schema();
        let db_type = self.driver.db_type();
        let table = M::table_name();

        let mut col_defs: Vec<String> = Vec::new();
        let mut pk_cols: Vec<&str> = Vec::new();

        for col in schema {
            let sql_type = rust_to_sql_type(col.rust_type, db_type, col.is_auto_increment);
            let mut def = format!("{} {}", col.name, sql_type);

            if col.is_primary_key {
                pk_cols.push(col.name);
            }

            if !col.is_primary_key {
                def.push_str(" NOT NULL");
            }

            col_defs.push(def);
        }

        if !pk_cols.is_empty() {
            let pk_names: Vec<String> = pk_cols.iter().map(|c| c.to_string()).collect();
            col_defs.push(format!("PRIMARY KEY ({})", pk_names.join(", ")));
        }

        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (\n    {}\n)",
            table,
            col_defs.join(",\n    ")
        );

        self.driver.execute(&sql, &[])?;

        for col in schema {
            if col.is_index {
                let idx_name = format!("idx_{}_{}", table, col.name);
                let idx_sql = format!(
                    "CREATE INDEX IF NOT EXISTS {} ON {} ({})",
                    idx_name, table, col.name
                );
                self.driver.execute(&idx_sql, &[])?;
            }
        }

        let mut unique_groups: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
        for col in schema {
            if let Some(idx_name) = col.unique_index_name {
                unique_groups.entry(idx_name).or_default().push(col.name);
            }
        }

        for (idx_name, cols) in &unique_groups {
            let uniq_name = if idx_name.is_empty() {
                format!("uq_{}_{}", table, cols.join("_"))
            } else {
                idx_name.to_string()
            };
            let uniq_sql = format!(
                "CREATE UNIQUE INDEX IF NOT EXISTS {} ON {} ({})",
                uniq_name, table, cols.join(", ")
            );
            self.driver.execute(&uniq_sql, &[])?;
        }

        Ok(())
    }

    /// Finds a single row by its primary key value.
    pub fn find_by_id(&mut self, id: i64) -> Result<Option<M>, Error> {
        let sql = format!("SELECT * FROM {} WHERE {} = ?", M::table_name(), M::primary_key());
        let params = vec![Parameter::Int(id)];
        let result = self.driver.query(&sql, &params)?;
        if result.rows.is_empty() {
            Ok(None)
        } else {
            let values = self.row_to_values(&result.rows[0]);
            M::from_row(&values).map(Some).map_err(|e| e.into())
        }
    }

    /// Finds rows where a column equals a value (simple convenience method).
    pub fn find_where(&mut self, column: &str, value: Value) -> Result<Vec<M>, Error> {
        let sql = format!("SELECT * FROM {} WHERE {} = ?", M::table_name(), column);
        let params = vec![value_to_param(&value)];
        let result = self.driver.query(&sql, &params)?;
        self.parse_rows(&result.rows)
    }

    /// Inserts a model into the table.
    ///
    /// Returns the last insert ID if the primary key is auto-increment.
    pub fn insert(&mut self, model: &M) -> Result<Option<i64>, Error> {
        let values = model.to_values();
        let columns = M::columns();
        let pk = M::primary_key();

        let pk_idx = columns.iter().position(|&c| c == pk);
        let is_auto_increment = pk_idx.map(|i| {
            matches!(&values[i], Value::I64(0) | Value::I32(0) | Value::Null)
        }).unwrap_or(false);

        let (cols, vals): (Vec<&str>, Vec<&Value>) = if is_auto_increment {
            columns.iter().enumerate()
                .filter(|(i, _)| Some(*i) != pk_idx)
                .map(|(_, &c)| c)
                .zip(values.iter().enumerate()
                    .filter(|(i, _)| Some(*i) != pk_idx)
                    .map(|(_, v)| v))
                .unzip()
        } else {
            (columns.to_vec(), values.iter().collect())
        };

        let placeholders: Vec<&str> = vec!["?"; cols.len()];
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            M::table_name(),
            cols.join(", "),
            placeholders.join(", ")
        );
        let params: Vec<Parameter> = vals.iter().map(|v| value_to_param(v)).collect();
        self.driver.execute(&sql, &params)?;
        self.driver.last_insert_id()
    }

    /// Updates a single column on rows matching the current WHERE conditions.
    ///
    /// Returns the number of rows affected.
    /// Requires at least one WHERE condition for safety.
    pub fn update_one(&mut self, column: &str, value: Value) -> Result<u64, Error> {
        let (where_clause, mut where_params) = self.build_where_clause();
        if where_clause.is_empty() {
            return Err("update requires at least one WHERE condition".into());
        }

        let sql = format!(
            "UPDATE {} SET {} = ?{}",
            M::table_name(),
            column,
            where_clause,
        );
        let mut params = vec![value_to_param(&value)];
        params.append(&mut where_params);

        self.reset_chain();
        self.driver.execute(&sql, &params)
    }

    /// Updates multiple columns from a model's non-zero/non-empty fields.
    ///
    /// Returns the number of rows affected.
    /// Requires at least one WHERE condition for safety.
    pub fn update_model(&mut self, model: &M) -> Result<u64, Error> {
        let values = model.to_values();
        let columns = M::columns();

        let set_clauses: Vec<String> = columns.iter().enumerate()
            .filter(|(i, _)| !is_value_default(&values[*i]))
            .map(|(_, col)| format!("{} = ?", col))
            .collect();

        if set_clauses.is_empty() {
            return Err("no non-zero fields to update".into());
        }

        let (where_clause, mut where_params) = self.build_where_clause();
        if where_clause.is_empty() {
            return Err("update requires at least one WHERE condition".into());
        }

        let sql = format!(
            "UPDATE {} SET {}{}",
            M::table_name(),
            set_clauses.join(", "),
            where_clause,
        );

        let mut set_params: Vec<Parameter> = columns.iter().enumerate()
            .filter(|(i, _)| !is_value_default(&values[*i]))
            .map(|(i, _)| value_to_param(&values[i]))
            .collect();
        set_params.append(&mut where_params);

        self.reset_chain();
        self.driver.execute(&sql, &set_params)
    }

    /// Deletes rows matching the current WHERE conditions.
    ///
    /// Returns the number of rows affected.
    /// Requires at least one WHERE condition for safety.
    pub fn delete(&mut self) -> Result<u64, Error> {
        let (where_clause, params) = self.build_where_clause();
        if where_clause.is_empty() {
            return Err("delete requires at least one WHERE condition".into());
        }

        let sql = format!("DELETE FROM {}{}", M::table_name(), where_clause);
        self.reset_chain();
        self.driver.execute(&sql, &params)
    }

    fn build_where_clause(&self) -> (String, Vec<Parameter>) {
        if self.conditions.is_empty() {
            return (String::new(), Vec::new());
        }
        let clauses: Vec<String> = self.conditions.iter().map(|(c, _)| c.clone()).collect();
        let mut params = Vec::new();
        for (_, p) in &self.conditions {
            params.extend(p.clone());
        }
        (format!(" WHERE {}", clauses.join(" AND ")), params)
    }

    pub(crate) fn begin_tx(&mut self) -> Result<(), Error> {
        self.driver.begin()
    }

    pub(crate) fn commit_tx(&mut self) -> Result<(), Error> {
        self.driver.commit()
    }

    pub(crate) fn rollback_tx(&mut self) -> Result<(), Error> {
        self.driver.rollback()
    }

    fn parse_rows(&self, rows: &[Row]) -> Result<Vec<M>, Error> {
        let mut results = Vec::new();
        for row in rows {
            let values = self.row_to_values(row);
            let model = M::from_row(&values).map_err(|e| -> Error { e.into() })?;
            results.push(model);
        }
        Ok(results)
    }

    fn row_to_values(&self, row: &Row) -> Vec<Value> {
        let mut values = Vec::new();
        for i in 0..row.column_count() {
            if let Some(val) = row.get(i) {
                if val == "NULL" {
                    values.push(Value::Null);
                } else if let Ok(i) = val.parse::<i64>() {
                    values.push(Value::I64(i));
                } else if let Ok(f) = val.parse::<f64>() {
                    values.push(Value::F64(f));
                } else if val == "true" {
                    values.push(Value::Bool(true));
                } else if val == "false" {
                    values.push(Value::Bool(false));
                } else {
                    values.push(Value::String(val.to_string()));
                }
            } else {
                values.push(Value::Null);
            }
        }
        values
    }
}

fn value_to_param(value: &Value) -> Parameter {
    match value {
        Value::Null => Parameter::Null,
        Value::Bool(v) => Parameter::Bool(*v),
        Value::I8(v) => Parameter::Int(*v as i64),
        Value::I16(v) => Parameter::Int(*v as i64),
        Value::I32(v) => Parameter::Int(*v as i64),
        Value::I64(v) => Parameter::Int(*v),
        Value::U8(v) => Parameter::Int(*v as i64),
        Value::U16(v) => Parameter::Int(*v as i64),
        Value::U32(v) => Parameter::Int(*v as i64),
        Value::U64(v) => Parameter::Int(*v as i64),
        Value::F32(v) => Parameter::Float(*v as f64),
        Value::F64(v) => Parameter::Float(*v),
        Value::String(v) => Parameter::String(v.clone()),
        Value::Bytes(v) => Parameter::Bytes(v.clone()),
        Value::Array(_) => Parameter::String(value.to_string()),
        Value::Map(_) => Parameter::String(value.to_string()),
    }
}

fn is_value_default(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(v) => !*v,
        Value::I8(v) => *v == 0,
        Value::I16(v) => *v == 0,
        Value::I32(v) => *v == 0,
        Value::I64(v) => *v == 0,
        Value::U8(v) => *v == 0,
        Value::U16(v) => *v == 0,
        Value::U32(v) => *v == 0,
        Value::U64(v) => *v == 0,
        Value::F32(v) => *v == 0.0,
        Value::F64(v) => *v == 0.0,
        Value::String(v) => v.is_empty(),
        Value::Bytes(v) => v.is_empty(),
        Value::Array(v) => v.is_empty(),
        Value::Map(v) => v.is_empty(),
    }
}




// fn rust_to_sql_type(rust_type: &str, db_type: DatabaseType, auto_increment: bool) -> &'static str {
//     match rust_type {
//         "bool" => "BOOLEAN",
//         "i8" | "i16" => "SMALLINT",
//         "i32" => {
//             if auto_increment {
//                 match db_type {
//                     DatabaseType::Postgresql => "SERIAL",
//                     _ => "INTEGER",
//                 }
//             } else {
//                 "INTEGER"
//             }
//         }
//         "i64" | "isize" => {
//             if auto_increment {
//                 match db_type {
//                     DatabaseType::Postgresql => "BIGSERIAL",
//                     _ => "BIGINT",
//                 }
//             } else {
//                 "BIGINT"
//             }
//         }
//         "u8" | "u16" => "SMALLINT",
//         "u32" => "INTEGER",
//         "u64" | "usize" => "BIGINT",
//         "f32" => "REAL",
//         "f64" => match db_type {
//             DatabaseType::Postgresql => "DOUBLE PRECISION",
//             _ => "DOUBLE",
//         },
//         "String" | "str" => match db_type {
//             DatabaseType::Postgresql => "VARCHAR(255)",
//             DatabaseType::Mysql => "VARCHAR(255)",
//             DatabaseType::Sqlite => "TEXT",
//         },
//         _ => "TEXT",
//     }
// }


fn rust_to_sql_type(rust_type: &str, db_type: DatabaseType, auto_increment: bool) -> &'static str {
    let normalized = rust_type
        .trim()
        .split('<')
        .next()
        .unwrap_or(rust_type)
        .to_lowercase();
    
    match normalized.as_str() {
        // ========== 布尔类型 ==========
        "bool" => "BOOLEAN",
        
        // ========== 整数类型 ==========
        "i8" => match db_type {
            DatabaseType::Postgresql => "SMALLINT",
            DatabaseType::Mysql => "TINYINT",
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "i16" => match db_type {
            DatabaseType::Postgresql => "SMALLINT",
            DatabaseType::Mysql => "SMALLINT",
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "i32" => match db_type {
            DatabaseType::Postgresql =>  {
                if auto_increment {
                     "SERIAL"
                } else {
                    "INTEGER"
                }
            },
            DatabaseType::Mysql => {
                if auto_increment {
                    "BIGINT"
                } else {
                    "INT"
                }
            },
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "i64" => match db_type {
            DatabaseType::Postgresql => {
                if auto_increment {
                    "BIGSERIAL"
                } else {
                    "BIGINT"
                }
            },
            DatabaseType::Mysql => "BIGINT",
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "id" | "grorm::id" => match db_type {
            DatabaseType::Postgresql => {
                if auto_increment {
                    "BIGSERIAL"
                } else {
                    "BIGINT"
                }
            },
            DatabaseType::Mysql => {
                if auto_increment {
                    "BIGINT AUTO_INCREMENT"
                } else {
                    "BIGINT"
                }
            },
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "i128" => match db_type {
            DatabaseType::Postgresql => "NUMERIC(39,0)",
            DatabaseType::Mysql => "DECIMAL(39,0)",
            DatabaseType::Sqlite => "TEXT", // SQLite 不支持大整数
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "isize" => match db_type {
            DatabaseType::Postgresql => "BIGINT",
            DatabaseType::Mysql => "BIGINT",
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // 无符号整数
        "u8" => match db_type {
            DatabaseType::Postgresql => "SMALLINT",
            DatabaseType::Mysql => "TINYINT UNSIGNED",
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "u16" => match db_type {
            DatabaseType::Postgresql => "INTEGER",
            DatabaseType::Mysql => "SMALLINT UNSIGNED",
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "u32" => match db_type {
            DatabaseType::Postgresql => "BIGINT",
            DatabaseType::Mysql => "INT UNSIGNED",
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "u64" => match db_type {
            DatabaseType::Postgresql => "NUMERIC(20,0)",
            DatabaseType::Mysql => "BIGINT UNSIGNED",
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "u128" => match db_type {
            DatabaseType::Postgresql => "NUMERIC(39,0)",
            DatabaseType::Mysql => "DECIMAL(39,0) UNSIGNED",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "usize" => match db_type {
            DatabaseType::Postgresql => "BIGINT",
            DatabaseType::Mysql => "BIGINT UNSIGNED",
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // ========== 浮点类型 ==========
        "f32" => match db_type {
            DatabaseType::Postgresql => "REAL",
            DatabaseType::Mysql => "FLOAT",
            DatabaseType::Sqlite => "REAL",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "f64" => match db_type {
            DatabaseType::Postgresql => "DOUBLE PRECISION",
            DatabaseType::Mysql => "DOUBLE",
            DatabaseType::Sqlite => "REAL",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // ========== 字符串类型 ==========
        "string" => match db_type {
            DatabaseType::Postgresql => "VARCHAR(255)",
            DatabaseType::Mysql => "VARCHAR(255)",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "char" => match db_type {
            DatabaseType::Postgresql => "CHAR(1)",
            DatabaseType::Mysql => "CHAR(1)",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // ========== 文本类型 ==========
        "text" => match db_type {
            DatabaseType::Postgresql => "TEXT",
            DatabaseType::Mysql => "TEXT",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "str" => match db_type {
            DatabaseType::Postgresql => "TEXT",
            DatabaseType::Mysql => "TEXT",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // ========== JSON 类型 ==========
        "json" | "jsonb" | "serde_json::value" | "value" | "serde_json::map" | "serde_json::number" => match db_type {
            DatabaseType::Postgresql => "JSONB",
            DatabaseType::Mysql => "JSON",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // ========== 日期时间类型 ==========
        // NaiveDate (只有日期)
        "chrono::naivedate" | "naivedate" | "date" => match db_type {
            DatabaseType::Postgresql => "DATE",
            DatabaseType::Mysql => "DATE",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // NaiveTime (只有时间)
        "chrono::naivetime" | "naivetime" | "time" => match db_type {
            DatabaseType::Postgresql => "TIME",
            DatabaseType::Mysql => "TIME",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // NaiveDateTime (无时区)
        "chrono::naivedatetime" | "naivedatetime" => match db_type {
            DatabaseType::Postgresql => "TIMESTAMP",
            DatabaseType::Mysql => "DATETIME",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // DateTime (有时区)
        "chrono::datetime" | "datetime" => match db_type {
            DatabaseType::Postgresql => "TIMESTAMPTZ",
            DatabaseType::Mysql => "TIMESTAMP",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "chrono::datetime<chrono::fixedoffset>" => match db_type {
            DatabaseType::Postgresql => "TIMESTAMPTZ",
            DatabaseType::Mysql => "TIMESTAMP",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "chrono::datetime<chrono::utc>" => match db_type {
            DatabaseType::Postgresql => "TIMESTAMPTZ",
            DatabaseType::Mysql => "TIMESTAMP",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "chrono::datetime<chrono::local>" => match db_type {
            DatabaseType::Postgresql => "TIMESTAMPTZ",
            DatabaseType::Mysql => "TIMESTAMP",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // Duration
        "chrono::duration" | "duration" => match db_type {
            DatabaseType::Postgresql => "INTERVAL",
            DatabaseType::Mysql => "BIGINT", // MySQL 中没有原生 interval，用秒数
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // ========== UUID ==========
        "uuid" | "uuid::uuid" => match db_type {
            DatabaseType::Postgresql => "UUID",
            DatabaseType::Mysql => "CHAR(36)",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // ========== 二进制数据 ==========
        "vec<u8>" | "bytes" | "bytearray" | "&[u8]" => match db_type {
            DatabaseType::Postgresql => "BYTEA",
            DatabaseType::Mysql => "BLOB",
            DatabaseType::Sqlite => "BLOB",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "byte" => match db_type {
            DatabaseType::Postgresql => "SMALLINT",
            DatabaseType::Mysql => "TINYINT",
            DatabaseType::Sqlite => "INTEGER",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // ========== 数组类型 ==========
        vec if vec.starts_with("vec<") || vec.starts_with("array<") => {
            // let inner_type = &rust_type[rust_type.find('<').unwrap() + 1..rust_type.rfind('>').unwrap()];
            match db_type {
                DatabaseType::Postgresql => "text[]",
                DatabaseType::Mysql => "JSON", // MySQL 存储数组用 JSON
                DatabaseType::Sqlite => "TEXT",
                DatabaseType::None => panic!("Unsupported database type"),
            }
        }
        
        // ========== 集合类型 ==========
        "hashmap" | "hashmap<" => match db_type {
            DatabaseType::Postgresql => "JSONB",
            DatabaseType::Mysql => "JSON",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "hashset" | "hashset<" => match db_type {
            DatabaseType::Postgresql => "JSONB",
            DatabaseType::Mysql => "JSON",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "btreemap" | "btreemap<" => match db_type {
            DatabaseType::Postgresql => "JSONB",
            DatabaseType::Mysql => "JSON",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "btreeset" | "btreeset<" => match db_type {
            DatabaseType::Postgresql => "JSONB",
            DatabaseType::Mysql => "JSON",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // ========== Option 类型 ==========
        "option" | "option<" => {
            let inner_type = &rust_type[rust_type.find('<').unwrap() + 1..rust_type.rfind('>').unwrap()];
            rust_to_sql_type(inner_type, db_type, auto_increment)
        }
        
        // ========== 网络类型 ==========
        "std::net::ipaddr" | "ipaddr" | "std::net::ipv4addr" | "ipv4addr" => match db_type {
            DatabaseType::Postgresql => "INET",
            DatabaseType::Mysql => "VARCHAR(45)",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "std::net::ipv6addr" | "ipv6addr" => match db_type {
            DatabaseType::Postgresql => "INET",
            DatabaseType::Mysql => "VARCHAR(45)",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "std::net::socketaddr" | "socketaddr" => match db_type {
            DatabaseType::Postgresql => "INET",
            DatabaseType::Mysql => "VARCHAR(45)",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // ========== 其他常见类型 ==========
        "decimal" | "rust_decimal::decimal" => match db_type {
            DatabaseType::Postgresql => "DECIMAL(10,2)",
            DatabaseType::Mysql => "DECIMAL(10,2)",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        "bigdecimal" | "bigdecimal::bigdecimal" => match db_type {
            DatabaseType::Postgresql => "DECIMAL(20,6)",
            DatabaseType::Mysql => "DECIMAL(20,6)",
            DatabaseType::Sqlite => "TEXT",
            DatabaseType::None => panic!("Unsupported database type"),
        },
        
        // 枚举类型
        _e if !normalized.contains("::") && 
             normalized.chars().next().unwrap_or('a').is_uppercase() &&
             !matches!(normalized.as_str(), "string" | "text" | "json" | "uuid") =>
        {
            match db_type {
                DatabaseType::Postgresql => "VARCHAR(50)",
                DatabaseType::Mysql => "VARCHAR(50)",
                DatabaseType::Sqlite => "TEXT",
                DatabaseType::None => panic!("Unsupported database type"),
            }
        }
        
        // ========== 默认 ==========
        _ => "TEXT",
    }
}