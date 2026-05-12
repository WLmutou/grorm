use grorm::{ConnectionConfig, SqliteDriverFactory, ConnectionPool, QueryBuilder, Value};
use grorm_macros::Model;
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
fn main() {
    let config = ConnectionConfig::new("localhost", 0, "", "", "target/testdb");

    let pool = ConnectionPool::new(SqliteDriverFactory, config, 1);

    let mut conn = pool.get().expect("Failed to get connection");

    conn.driver_mut().execute("CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        email TEXT NOT NULL,
        age INTEGER DEFAULT 0
    )", &[]).expect("Failed to create table");

    let user = User {
        id: 0,
        name: "Charlie".to_string(),
        email: "charlie@example.com".to_string(),
        age: 28,
    };

    let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
    let inserted_id = qb.insert(&user).expect("Failed to insert");
    println!("Inserted user with id: {:?}", inserted_id);

    let users = qb.find_all().expect("Failed to query");
    for u in &users {
        println!("User: {:?}", u);
    }

    let f_user = qb.find_where(&user.name, Value::from("Charlie")).expect("Failed to find");
    println!("Found user: {:?}", f_user);   

    if let Some(found) = qb.find_by_id(1).expect("Failed to find by id") {
        println!("Found by id: {:?}", found);
    }

    let count = qb.count().expect("Failed to count");
    println!("Total users: {}", count);

    qb.delete_by_id(1).expect("Failed to delete");
    println!("Deleted user with id 1");

    let count_after = qb.count().expect("Failed to count");
    println!("Total users after delete: {}", count_after);
}