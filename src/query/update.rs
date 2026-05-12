use crate::types::Value;

pub struct UpdateBuilder {
    table: String,
    sets: Vec<(String, Value)>,
    conditions: Vec<(String, String, Value)>,
}

impl UpdateBuilder {
    pub fn new(table: &str) -> Self {
        UpdateBuilder {
            table: table.to_string(),
            sets: Vec::new(),
            conditions: Vec::new(),
        }
    }

    pub fn set(mut self, column: &str, value: Value) -> Self {
        self.sets.push((column.to_string(), value));
        self
    }

    pub fn where_eq(mut self, column: &str, value: Value) -> Self {
        self.conditions.push((column.to_string(), "=".to_string(), value));
        self
    }

    pub fn where_ne(mut self, column: &str, value: Value) -> Self {
        self.conditions.push((column.to_string(), "!=".to_string(), value));
        self
    }

    pub fn build(&self) -> (String, Vec<Value>) {
        let mut sql = format!("UPDATE {}", self.table);
        let mut params = Vec::new();

        if !self.sets.is_empty() {
            let set_clauses: Vec<String> = self.sets.iter().map(|(col, val)| {
                params.push(val.clone());
                format!("{} = ?", col)
            }).collect();
            sql.push_str(&format!(" SET {}", set_clauses.join(", ")));
        }

        if !self.conditions.is_empty() {
            let clauses: Vec<String> = self.conditions.iter().map(|(col, op, val)| {
                params.push(val.clone());
                format!("{} {} ?", col, op)
            }).collect();
            sql.push_str(&format!(" WHERE {}", clauses.join(" AND ")));
        }

        (sql, params)
    }
}