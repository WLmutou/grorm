use grorm::{ConnectionConfig, ConnectionPool, PostgresDriverFactory, QueryBuilder, Transaction, Value};
use grorm_macros::Model;
use gorust::{go, runtime, channel};

#[derive(Debug, Model)]
#[table = "users"]
struct User {
    id: i64,
    name: String,
    email: String,
    age: i32,
}

#[runtime]
fn main() {
    let (tx, rx) = channel::new();

    go(move || {
        let config = ConnectionConfig::new("127.0.0.1", 5432, "odoo", "odoo", "testdb");

        let pool = ConnectionPool::new(PostgresDriverFactory, config, 5);
        println!("Pool created");
        let mut conn = pool.get().expect("Failed to get connection");
        println!("==== Connection created");

        conn.driver_mut().execute("CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(200) NOT NULL,
            age INTEGER DEFAULT 0
        )", &[]).expect("Failed to create table");

        // seed data
        {
            let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
            qb.insert(&User { id: 0, name: "Alice".into(), email: "alice@x.com".into(), age: 30 }).unwrap();
            qb.insert(&User { id: 0, name: "Bob".into(), email: "bob@x.com".into(), age: 25 }).unwrap();
            qb.insert(&User { id: 0, name: "Charlie".into(), email: "charlie@x.com".into(), age: 35 }).unwrap();
        }

        // where_in
        {
            let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
            let users = qb.where_in("name", vec![
                Value::from("Alice"),
                Value::from("Bob"),
            ]).find().expect("where_in find");
            println!("where_in [Alice, Bob]: {:?}", users);
        }

        // transaction: update + insert atomically
        {
            let mut tx = Transaction::<User>::begin(conn.driver_mut()).expect("begin tx");
            tx.where_eq("name", Value::from("Alice"))
                .update_one("age", Value::from(31))
                .expect("tx update_one");
            tx.insert(&User { id: 0, name: "Dave".into(), email: "dave@x.com".into(), age: 40 })
                .expect("tx insert");
            tx.commit().expect("commit");
            println!("Transaction committed");
        }

        {
            let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
            let all = qb.find_all().expect("find_all");
            println!("All users: {:?}", all);
        }

        // transaction: rollback on drop
        {
            let mut tx = Transaction::<User>::begin(conn.driver_mut()).expect("begin tx");
            tx.where_eq("name", Value::from("Bob"))
                .update_one("age", Value::from(99))
                .expect("tx update");
            println!("Transaction rolled back (drop)");
        }

        {
            let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
            let bob = qb.where_eq("name", Value::from("Bob")).find_one().expect("find_one");
            println!("Bob after rollback: {:?}", bob);
        }

        let _ = tx.send(());
    });

    rx.recv().expect("Failed to receive completion signal");
}