use grorm::{ConnectionConfig, MysqlDriverFactory, ConnectionPool, QueryBuilder};
use grorm_macros::Model;

#[derive(Debug, Model)]
#[table = "users"]
struct User {
    id: i64,
    name: String,
    email: String,
    age: i32,
}

fn main() {
    let config = ConnectionConfig::new("127.0.0.1", 3306, "root", "password", "testdb");

    let pool = ConnectionPool::new(MysqlDriverFactory, config, 5);

    let mut conn = pool.get().expect("Failed to get connection");

    conn.driver_mut().execute("CREATE TABLE IF NOT EXISTS users (
        id INT AUTO_INCREMENT PRIMARY KEY,
        name VARCHAR(100) NOT NULL,
        email VARCHAR(200) NOT NULL,
        age INT DEFAULT 0
    )", &[]).expect("Failed to create table");

    let user = User {
        id: 0,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
        age: 25,
    };

    let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
    let inserted_id = qb.insert(&user).expect("Failed to insert");
    println!("Inserted user with id: {:?}", inserted_id);

    let users = qb.find_all().expect("Failed to query");
    for u in &users {
        println!("User: {:?}", u);
    }
}