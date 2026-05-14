use crate::types::Value;

pub struct SelectBuilder {
    table: String,
    columns: Vec<String>,
    conditions: Vec<(String, String, Value)>,
    order_by: Vec<(String, bool)>,
    limit_val: Option<usize>,
    offset_val: Option<usize>,
    joins: Vec<String>,
    group_by: Vec<String>,
}

impl SelectBuilder {
    pub fn new(table: &str) -> Self {
        SelectBuilder {
            table: table.to_string(),
            columns: vec!["*".to_string()],
            conditions: Vec::new(),
            order_by: Vec::new(),
            limit_val: None,
            offset_val: None,
            joins: Vec::new(),
            group_by: Vec::new(),
        }
    }

    pub fn columns(mut self, cols: &[&str]) -> Self {
        self.columns = cols.iter().map(|c| c.to_string()).collect();
        self
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

    pub fn where_like(mut self, column: &str, value: Value) -> Self {
        self.conditions
            .push((column.to_string(), "LIKE".to_string(), value));
        self
    }

    pub fn where_in(mut self, column: &str, values: Vec<Value>) -> Self {
        let in_vals: Vec<String> = values.iter().map(|v| v.to_string()).collect();
        let val_str = format!("({})", in_vals.join(", "));
        self.conditions
            .push((column.to_string(), "IN".to_string(), Value::String(val_str)));
        self
    }

    pub fn where_null(mut self, column: &str) -> Self {
        self.conditions.push((
            column.to_string(),
            "IS".to_string(),
            Value::String("NULL".to_string()),
        ));
        self
    }

    pub fn where_not_null(mut self, column: &str) -> Self {
        self.conditions.push((
            column.to_string(),
            "IS NOT".to_string(),
            Value::String("NULL".to_string()),
        ));
        self
    }

    pub fn order_by_asc(mut self, column: &str) -> Self {
        self.order_by.push((column.to_string(), true));
        self
    }

    pub fn order_by_desc(mut self, column: &str) -> Self {
        self.order_by.push((column.to_string(), false));
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit_val = Some(limit);
        self
    }

    pub fn offset(mut self, offset: usize) -> Self {
        self.offset_val = Some(offset);
        self
    }

    pub fn join(mut self, table: &str, on: &str) -> Self {
        self.joins.push(format!("JOIN {} ON {}", table, on));
        self
    }

    pub fn left_join(mut self, table: &str, on: &str) -> Self {
        self.joins.push(format!("LEFT JOIN {} ON {}", table, on));
        self
    }

    pub fn group_by(mut self, columns: &[&str]) -> Self {
        self.group_by = columns.iter().map(|c| c.to_string()).collect();
        self
    }

    pub fn build(&self) -> (String, Vec<Value>) {
        let mut sql = format!("SELECT {} FROM {}", self.columns.join(", "), self.table);
        let mut params = Vec::new();

        for join in &self.joins {
            sql.push_str(&format!(" {}", join));
        }

        if !self.conditions.is_empty() {
            let clauses: Vec<String> = self
                .conditions
                .iter()
                .map(|(col, op, val)| {
                    if *op == "IN" || *op == "IS" || *op == "IS NOT" {
                        format!("{} {} {}", col, op, val)
                    } else {
                        params.push(val.clone());
                        format!("{} {} ?", col, op)
                    }
                })
                .collect();
            sql.push_str(&format!(" WHERE {}", clauses.join(" AND ")));
        }

        if !self.group_by.is_empty() {
            sql.push_str(&format!(" GROUP BY {}", self.group_by.join(", ")));
        }

        if !self.order_by.is_empty() {
            let orders: Vec<String> = self
                .order_by
                .iter()
                .map(|(col, asc)| {
                    if *asc {
                        format!("{} ASC", col)
                    } else {
                        format!("{} DESC", col)
                    }
                })
                .collect();
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
}
