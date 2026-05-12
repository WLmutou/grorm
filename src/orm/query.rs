use crate::driver::{DatabaseDriver, Parameter, Row};
use crate::orm::model::Model;
use crate::types::Value;
use std::error::Error;

pub struct QueryBuilder<'a, M: Model> {
    driver: &'a mut dyn DatabaseDriver,
    _marker: std::marker::PhantomData<M>,
}

impl<'a, M: Model> QueryBuilder<'a, M> {
    pub fn new(driver: &'a mut dyn DatabaseDriver) -> Self {
        QueryBuilder {
            driver,
            _marker: std::marker::PhantomData,
        }
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

    pub fn update(&mut self, model: &M) -> Result<u64, Box<dyn Error>> {
        let values = model.to_values();
        let columns = M::columns();
        let pk = M::primary_key();

        let pk_idx = columns.iter().position(|&c| c == pk)
            .ok_or("Primary key not found in columns")?;
        let pk_value = &values[pk_idx];

        let set_clauses: Vec<String> = columns.iter().enumerate()
            .filter(|(i, _)| *i != pk_idx)
            .map(|(_, col)| format!("{} = ?", col))
            .collect();

        let sql = format!(
            "UPDATE {} SET {} WHERE {} = ?",
            M::table_name(),
            set_clauses.join(", "),
            pk
        );

        let mut params: Vec<Parameter> = values.iter().enumerate()
            .filter(|(i, _)| *i != pk_idx)
            .map(|(_, v)| value_to_param(v))
            .collect();
        params.push(value_to_param(pk_value));

        self.driver.execute(&sql, &params)
    }

    pub fn delete(&mut self, model: &M) -> Result<u64, Box<dyn Error>> {
        let values = model.to_values();
        let columns = M::columns();
        let pk = M::primary_key();

        let pk_idx = columns.iter().position(|&c| c == pk)
            .ok_or("Primary key not found in columns")?;
        let pk_value = &values[pk_idx];

        let sql = format!("DELETE FROM {} WHERE {} = ?", M::table_name(), pk);
        let params = vec![value_to_param(pk_value)];
        self.driver.execute(&sql, &params)
    }

    pub fn delete_by_id(&mut self, id: i64) -> Result<u64, Box<dyn Error>> {
        let sql = format!("DELETE FROM {} WHERE {} = ?", M::table_name(), M::primary_key());
        let params = vec![Parameter::Int(id)];
        self.driver.execute(&sql, &params)
    }

    pub fn count(&mut self) -> Result<i64, Box<dyn Error>> {
        let sql = format!("SELECT COUNT(*) FROM {}", M::table_name());
        let result = self.driver.query(&sql, &[])?;
        if let Some(row) = result.rows.first() {
            if let Some(val) = row.get(0) {
                return Ok(val.parse().unwrap_or(0));
            }
        }
        Ok(0)
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