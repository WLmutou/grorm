use grorm::{ConnectionConfig, SqliteDriverFactory, ConnectionPool, QueryBuilder, Transaction, Value, Error};
use grorm::Model;
use gorust::runtime;

#[derive(Debug, Model)]
#[table = "users"]
struct User {
    id: i64,
    name: String,
    email: String,
    age: i32,
}

#[runtime]
fn main() -> std::result::Result<(), Error> {
    let config = ConnectionConfig::new("localhost", 0, "", "", "target/testdb");

    let pool = ConnectionPool::new(SqliteDriverFactory, config, 1);

    let mut conn = pool.get()?;

    conn.driver_mut().execute("CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        email TEXT NOT NULL,
        age INTEGER DEFAULT 0
    )", &[])?;

    // seed data
    {
        let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
        qb.insert(&User { id: 0, name: "Alice".into(), email: "alice@x.com".into(), age: 30 })?;
        qb.insert(&User { id: 0, name: "Bob".into(), email: "bob@x.com".into(), age: 25 })?;
        qb.insert(&User { id: 0, name: "Charlie".into(), email: "charlie@x.com".into(), age: 35 })?;
    }

    // where_in
    {
        let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
        let users = qb.where_in("name", vec![
            Value::from("Alice"),
            Value::from("Bob"),
        ]).find()?;
        println!("where_in [Alice, Bob]: {:?}", users);
    }

    // transaction: update + insert atomically
    {
        let mut tx = Transaction::<User>::begin(conn.driver_mut())?;
        tx.where_eq("name", Value::from("Alice"))
            .update_one("age", Value::from(31))?;
        tx.insert(&User { id: 0, name: "Dave".into(), email: "dave@x.com".into(), age: 40 })?;
        tx.commit()?;
        println!("Transaction committed");
    }

    {
        let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
        let all = qb.find_all()?;
        println!("All users: {:?}", all);
    }

    // transaction: rollback on drop
    {
        let mut tx = Transaction::<User>::begin(conn.driver_mut())?;
        tx.where_eq("name", Value::from("Bob"))
            .update_one("age", Value::from(99))?;
        println!("Transaction rolled back (drop)");
    }

    {
        let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
        let bob = qb.where_eq("name", Value::from("Bob")).find_one()?;
        println!("Bob after rollback: {:?}", bob);
    }

    Ok(())
}