use grorm::{ConnectionConfig, ConnectionPool, PostgresDriverFactory, QueryBuilder, Transaction, Value, Error};
use grorm::DeriveModel;
use gorust::{go, runtime, channel};

#[derive(Debug, DeriveModel)]
#[table = "users"]
struct User {
    id: i64,
    name: String,
    email: String,
    age: i32,
}

fn run() -> std::result::Result<(), Error> {
    let config = ConnectionConfig::new("127.0.0.1", 5432, "odoo", "odoo", "testdb");

    let pool = ConnectionPool::new(PostgresDriverFactory, config, 5);
    println!("Pool created");
    let mut conn = pool.get()?;
    println!("==== Connection created");

    {
        let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
        qb.create_table()?;
    }

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

#[runtime]
fn main() {
    let (tx, rx) = channel::new();

    go(move || {
        if let Err(e) = run() {
            eprintln!("Error: {}", e);
        }
        let _ = tx.send(());
    });

    rx.recv().unwrap();
}