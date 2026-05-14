use crate::types::Value;

pub struct DeleteBuilder {
    table: String,
    conditions: Vec<(String, String, Value)>,
}

impl DeleteBuilder {
    pub fn new(table: &str) -> Self {
        DeleteBuilder {
            table: table.to_string(),
            conditions: Vec::new(),
        }
    }

    pub fn where_eq(mut self, column: &str, value: Value) -> Self {
        self.conditions
            .push((column.to_string(), "=".to_string(), value));
        self
    }

    pub fn where_ne(mut self, column: &str, value: Value) -> Self {
        self.conditions
            .push((column.to_string(), "!=".to_string(), value));
        self
    }

    pub fn where_gt(mut self, column: &str, value: Value) -> Self {
        self.conditions
            .push((column.to_string(), ">".to_string(), value));
        self
    }

    pub fn where_lt(mut self, column: &str, value: Value) -> Self {
        self.conditions
            .push((column.to_string(), "<".to_string(), value));
        self
    }

    pub fn build(&self) -> (String, Vec<Value>) {
        let mut sql = format!("DELETE FROM {}", self.table);
        let mut params = Vec::new();

        if !self.conditions.is_empty() {
            let clauses: Vec<String> = self
                .conditions
                .iter()
                .map(|(col, op, val)| {
                    params.push(val.clone());
                    format!("{} {} ?", col, op)
                })
                .collect();
            sql.push_str(&format!(" WHERE {}", clauses.join(" AND ")));
        }

        (sql, params)
    }
}
