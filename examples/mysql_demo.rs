use grorm::{ConnectionConfig, ConnectionPool, DeriveModel, Error, MysqlDriverFactory, QueryBuilder};

use gorust::runtime;

#[derive(Debug, DeriveModel)]
#[table = "users"]
struct User {
    id: i64,
    name: String,
    email: String,
    age: i32,
}

#[runtime]
fn main() -> std::result::Result<(), Error> {
    let config = ConnectionConfig::new("127.0.0.1", 3306, "root", "password", "testdb");

    let pool = ConnectionPool::new(MysqlDriverFactory, config, 5);

    let mut conn = pool.get()?;

    {
        let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
        qb.create_table()?;
    }

    let user = User {
        id: 0,
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
        age: 25,
    };

    let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
    let inserted_id = qb.insert(&user)?;
    println!("Inserted user with id: {:?}", inserted_id);

    let users = qb.find_all()?;
    for u in &users {
        println!("User: {:?}", u);
    }

    Ok(())
}