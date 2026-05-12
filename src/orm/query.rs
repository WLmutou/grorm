use crate::driver::{DatabaseDriver, Parameter, Row};
use crate::orm::model::Model;
use crate::types::Value;
use std::error::Error;

pub struct QueryBuilder<'a, M: Model> {
    driver: &'a mut dyn DatabaseDriver,
    _marker: std::marker::PhantomData<M>,
    conditions: Vec<(String, Vec<Parameter>)>,
    limit_val: Option<usize>,
    offset_val: Option<usize>,
    order_by: Vec<(String, bool)>,
}

impl<'a, M: Model> QueryBuilder<'a, M> {
    pub fn new(driver: &'a mut dyn DatabaseDriver) -> Self {
        QueryBuilder {
            driver,
            _marker: std::marker::PhantomData,
            conditions: Vec::new(),
            limit_val: None,
            offset_val: None,
            order_by: Vec::new(),
        }
    }

    fn reset_chain(&mut self) {
        self.conditions.clear();
        self.limit_val = None;
        self.offset_val = None;
        self.order_by.clear();
    }

    pub fn where_eq(&mut self, column: &str, value: Value) -> &mut Self {
        self.conditions.push((format!("{} = ?", column), vec![value_to_param(&value)]));
        self
    }

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

    pub fn limit(&mut self, n: usize) -> &mut Self {
        self.limit_val = Some(n);
        self
    }

    pub fn offset(&mut self, n: usize) -> &mut Self {
        self.offset_val = Some(n);
        self
    }

    pub fn order(&mut self, column: &str, asc: bool) -> &mut Self {
        self.order_by.push((column.to_string(), asc));
        self
    }

    pub fn find(&mut self) -> Result<Vec<M>, Box<dyn Error>> {
        let (sql, params) = self.build_select_sql();
        let result = self.driver.query(&sql, &params)?;
        self.reset_chain();
        self.parse_rows(&result.rows)
    }

    pub fn find_one(&mut self) -> Result<Option<M>, Box<dyn Error>> {
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

    pub fn count(&mut self) -> Result<i64, Box<dyn Error>> {
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

    fn build_select_sql(&self) -> (String, Vec<Parameter>) {
        let mut sql = format!("SELECT * FROM {}", M::table_name());
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

    pub fn find_all(&mut self) -> Result<Vec<M>, Box<dyn Error>> {
        let sql = format!("SELECT * FROM {}", M::table_name());
        let result = self.driver.query(&sql, &[])?;
        self.parse_rows(&result.rows)
    }

    pub fn find_by_id(&mut self, id: i64) -> Result<Option<M>, Box<dyn Error>> {
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

    pub fn find_where(&mut self, column: &str, value: Value) -> Result<Vec<M>, Box<dyn Error>> {
        let sql = format!("SELECT * FROM {} WHERE {} = ?", M::table_name(), column);
        let params = vec![value_to_param(&value)];
        let result = self.driver.query(&sql, &params)?;
        self.parse_rows(&result.rows)
    }

    pub fn insert(&mut self, model: &M) -> Result<Option<i64>, Box<dyn Error>> {
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

    pub fn update_one(&mut self, column: &str, value: Value) -> Result<u64, Box<dyn Error>> {
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

    pub fn update_model(&mut self, model: &M) -> Result<u64, Box<dyn Error>> {
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

    pub fn delete(&mut self) -> Result<u64, Box<dyn Error>> {
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

    pub(crate) fn begin_tx(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver.begin()
    }

    pub(crate) fn commit_tx(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver.commit()
    }

    pub(crate) fn rollback_tx(&mut self) -> Result<(), Box<dyn Error>> {
        self.driver.rollback()
    }

    fn parse_rows(&self, rows: &[Row]) -> Result<Vec<M>, Box<dyn Error>> {
        let mut results = Vec::new();
        for row in rows {
            let values = self.row_to_values(row);
            let model = M::from_row(&values).map_err(|e| -> Box<dyn Error> { e.into() })?;
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