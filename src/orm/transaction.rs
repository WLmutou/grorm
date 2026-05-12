use crate::driver::DatabaseDriver;
use crate::orm::model::Model;
use crate::orm::query::QueryBuilder;
use crate::types::Value;
use crate::error::Error;

/// A database transaction with chainable API.
///
/// Automatically rolls back on drop if not explicitly committed.
/// Delegates all query methods to the inner [`QueryBuilder`].
///
/// # Example
///
/// ```rust
/// use grorm::{Transaction, Value, SqliteDriverFactory, ConnectionConfig, ConnectionPool, Error};
/// use grorm::DeriveModel;
///
/// #[derive(Debug, DeriveModel)]
/// #[table = "users"]
/// struct User {
///     id: i64,
///     name: String,
///     email: String,
///     age: i32,
/// }
///fn main() -> Result<(), Error> {
///   let config = ConnectionConfig::new("localhost", 0, "", "", "target/testdb");
///   let pool = ConnectionPool::new(SqliteDriverFactory, config, 1);
///   let mut conn = pool.get()?;
///   let mut tx = Transaction::<User>::begin(conn.driver_mut())?;
///
///   let user = User { id: 0, name: "Alice".into(), email: "alice@x.com".into(), age: 30 };
///   tx.insert(&user)?;
///
///   tx.where_eq("name", Value::from("Bob"))
///     .update_one("age", Value::from(26))?;
///
///   tx.commit()?;
///   // If tx goes out of scope without commit, it auto-rolls back
///   Ok(())
/// }
/// ```
pub struct Transaction<'a, M: Model> {
    qb: QueryBuilder<'a, M>,
    active: bool,
}

impl<'a, M: Model> Transaction<'a, M> {
    /// Begins a new transaction.
    pub fn begin(driver: &'a mut dyn DatabaseDriver) -> Result<Self, Error> {
        let mut qb = QueryBuilder::new(driver);
        qb.begin_tx()?;
        Ok(Transaction { qb, active: true })
    }

    /// Commits the transaction. After this, the transaction is inactive.
    pub fn commit(mut self) -> Result<(), Error> {
        if self.active {
            self.qb.commit_tx()?;
            self.active = false;
        }
        Ok(())
    }

    /// Explicitly rolls back the transaction.
    pub fn rollback(mut self) -> Result<(), Error> {
        if self.active {
            self.qb.rollback_tx()?;
            self.active = false;
        }
        Ok(())
    }

    /// Adds a `WHERE column = value` condition.
    pub fn where_eq(&mut self, column: &str, value: Value) -> &mut Self {
        self.qb.where_eq(column, value);
        self
    }

    /// Adds WHERE conditions from non-zero/non-empty fields of a model.
    pub fn where_model(&mut self, model: &M) -> &mut Self {
        self.qb.where_model(model);
        self
    }

    /// Adds a `WHERE column IN (...)` condition.
    pub fn where_in(&mut self, column: &str, values: Vec<Value>) -> &mut Self {
        self.qb.where_in(column, values);
        self
    }

    /// Sets the LIMIT clause.
    pub fn limit(&mut self, n: usize) -> &mut Self {
        self.qb.limit(n);
        self
    }

    /// Sets the OFFSET clause.
    pub fn offset(&mut self, n: usize) -> &mut Self {
        self.qb.offset(n);
        self
    }

    /// Adds an ORDER BY clause.
    pub fn order(&mut self, column: &str, asc: bool) -> &mut Self {
        self.qb.order(column, asc);
        self
    }

    /// Executes the query and returns all matching rows.
    pub fn find(&mut self) -> Result<Vec<M>, Error> {
        self.qb.find()
    }

    /// Executes the query and returns the first matching row.
    pub fn find_one(&mut self) -> Result<Option<M>, Error> {
        self.qb.find_one()
    }

    /// Returns the count of matching rows.
    pub fn count(&mut self) -> Result<i64, Error> {
        self.qb.count()
    }

    /// Returns all rows from the table.
    pub fn find_all(&mut self) -> Result<Vec<M>, Error> {
        self.qb.find_all()
    }

    /// Finds a row by primary key.
    pub fn find_by_id(&mut self, id: i64) -> Result<Option<M>, Error> {
        self.qb.find_by_id(id)
    }

    /// Finds rows where a column equals a value.
    pub fn find_where(&mut self, column: &str, value: Value) -> Result<Vec<M>, Error> {
        self.qb.find_where(column, value)
    }

    /// Inserts a model into the table.
    pub fn insert(&mut self, model: &M) -> Result<Option<i64>, Error> {
        self.qb.insert(model)
    }

    /// Updates a single column on matching rows.
    pub fn update_one(&mut self, column: &str, value: Value) -> Result<u64, Error> {
        self.qb.update_one(column, value)
    }

    /// Updates multiple columns from a model's non-zero fields.
    pub fn update_model(&mut self, model: &M) -> Result<u64, Error> {
        self.qb.update_model(model)
    }

    /// Deletes matching rows.
    pub fn delete(&mut self) -> Result<u64, Error> {
        self.qb.delete()
    }
}

impl<'a, M: Model> Drop for Transaction<'a, M> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.qb.rollback_tx();
        }
    }
}