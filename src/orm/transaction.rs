use crate::driver::DatabaseDriver;
use crate::orm::model::Model;
use crate::orm::query::QueryBuilder;
use crate::types::Value;
use crate::error::Error;

pub struct Transaction<'a, M: Model> {
    qb: QueryBuilder<'a, M>,
    active: bool,
}

impl<'a, M: Model> Transaction<'a, M> {
    pub fn begin(driver: &'a mut dyn DatabaseDriver) -> Result<Self, Error> {
        let mut qb = QueryBuilder::new(driver);
        qb.begin_tx()?;
        Ok(Transaction { qb, active: true })
    }

    pub fn commit(mut self) -> Result<(), Error> {
        if self.active {
            self.qb.commit_tx()?;
            self.active = false;
        }
        Ok(())
    }

    pub fn rollback(mut self) -> Result<(), Error> {
        if self.active {
            self.qb.rollback_tx()?;
            self.active = false;
        }
        Ok(())
    }

    pub fn where_eq(&mut self, column: &str, value: Value) -> &mut Self {
        self.qb.where_eq(column, value);
        self
    }

    pub fn where_model(&mut self, model: &M) -> &mut Self {
        self.qb.where_model(model);
        self
    }

    pub fn where_in(&mut self, column: &str, values: Vec<Value>) -> &mut Self {
        self.qb.where_in(column, values);
        self
    }

    pub fn limit(&mut self, n: usize) -> &mut Self {
        self.qb.limit(n);
        self
    }

    pub fn offset(&mut self, n: usize) -> &mut Self {
        self.qb.offset(n);
        self
    }

    pub fn order(&mut self, column: &str, asc: bool) -> &mut Self {
        self.qb.order(column, asc);
        self
    }

    pub fn find(&mut self) -> Result<Vec<M>, Error> {
        self.qb.find()
    }

    pub fn find_one(&mut self) -> Result<Option<M>, Error> {
        self.qb.find_one()
    }

    pub fn count(&mut self) -> Result<i64, Error> {
        self.qb.count()
    }

    pub fn find_all(&mut self) -> Result<Vec<M>, Error> {
        self.qb.find_all()
    }

    pub fn find_by_id(&mut self, id: i64) -> Result<Option<M>, Error> {
        self.qb.find_by_id(id)
    }

    pub fn find_where(&mut self, column: &str, value: Value) -> Result<Vec<M>, Error> {
        self.qb.find_where(column, value)
    }

    pub fn insert(&mut self, model: &M) -> Result<Option<i64>, Error> {
        self.qb.insert(model)
    }

    pub fn update_one(&mut self, column: &str, value: Value) -> Result<u64, Error> {
        self.qb.update_one(column, value)
    }

    pub fn update_model(&mut self, model: &M) -> Result<u64, Error> {
        self.qb.update_model(model)
    }

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