use crate::types::Value;

pub struct InsertBuilder {
    table: String,
    columns: Vec<String>,
    values: Vec<Vec<Value>>,
    returning: Vec<String>,
}

impl InsertBuilder {
    pub fn new(table: &str) -> Self {
        InsertBuilder {
            table: table.to_string(),
            columns: Vec::new(),
            values: Vec::new(),
            returning: Vec::new(),
        }
    }

    pub fn columns(mut self, cols: &[&str]) -> Self {
        self.columns = cols.iter().map(|c| c.to_string()).collect();
        self
    }

    pub fn values(mut self, vals: Vec<Value>) -> Self {
        self.values.push(vals);
        self
    }

    pub fn returning(mut self, cols: &[&str]) -> Self {
        self.returning = cols.iter().map(|c| c.to_string()).collect();
        self
    }

    pub fn build(&self) -> (String, Vec<Value>) {
        let mut sql = format!("INSERT INTO {}", self.table);
        let mut params = Vec::new();

        if !self.columns.is_empty() {
            sql.push_str(&format!(" ({})", self.columns.join(", ")));
        }

        if !self.values.is_empty() {
            let value_groups: Vec<String> = self.values.iter().map(|row| {
                let placeholders: Vec<String> = row.iter().map(|v| {
                    params.push(v.clone());
                    "?".to_string()
                }).collect();
                format!("({})", placeholders.join(", "))
            }).collect();
            sql.push_str(&format!(" VALUES {}", value_groups.join(", ")));
        }

        if !self.returning.is_empty() {
            sql.push_str(&format!(" RETURNING {}", self.returning.join(", ")));
        }

        (sql, params)
    }
}