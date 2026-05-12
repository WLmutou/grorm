use grorm::{ConnectionConfig, ConnectionPool, PostgresDriverFactory, QueryBuilder, Value};
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

        let mut user = User {
            id: 0,
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            age: 30,
        };

        let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
        let inserted_id = qb.insert(&user).expect("Failed to insert");
        println!("Inserted user with id: {:?}", inserted_id);

        if let Some(id) = inserted_id {
            user.id = id;
        }

        let users = qb.find_all().expect("Failed to query");
        for u in &users {
            println!("User: {:?}", u);
        }

        user.age = 31;
        qb.update(&user).expect("Failed to update");

        let found = qb.find_where("name", Value::from("Alice")).expect("Failed to find");
        println!("Found user: {:?}", found);

        let users = qb.find_all().expect("Failed to query");
        for u in &users {
            println!("User: {:?}", u);
        }

        if let Some(found) = qb.find_by_id(1).expect("Failed to find") {
            println!("Found user: {:?}", found);
        }

        let count = qb.count().expect("Failed to count");
        println!("Total users: {}", count);

        let _ = tx.send(());
    });

    rx.recv().expect("Failed to receive completion signal");
}