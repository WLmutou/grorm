use gorust::runtime;
use grorm::DeriveModel;
use grorm::{
    ConnectionConfig, ConnectionPool, Error, QueryBuilder, SqliteDriverFactory, Transaction, Value,
};

#[derive(Debug, DeriveModel)]
#[table = "users"]
struct User {
    id: i64, // 自动主键
    #[index] // 单列索引
    name: String,
    #[unique] // 单列唯一约束
    email: String,
    #[unique_index = "idx_name_age"] // 联合唯一索引（同名的列组成一组）
    first_name: String,
    #[unique_index = "idx_name_age"]
    last_name: String,
    age: i32,
}

#[runtime]
fn main() -> std::result::Result<(), Error> {
    let config = ConnectionConfig::new("localhost", 0, "", "", "target/testdb");

    let pool = ConnectionPool::new(SqliteDriverFactory, config, 1);

    let mut conn = pool.get()?;

    {
        let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
        qb.create_table()?;
    }

    // seed data
    {
        let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
        qb.insert(&User {
            id: 0,
            name: "Alice".into(),
            email: "alice@x.com".into(),
            first_name: "Alice".into(),
            last_name: "Doe".into(),
            age: 30,
        })?;
        qb.insert(&User {
            id: 0,
            name: "Bob".into(),
            email: "bob@x.com".into(),
            first_name: "Bob".into(),
            last_name: "Doe".into(),
            age: 25,
        })?;
        qb.insert(&User {
            id: 0,
            name: "Charlie".into(),
            email: "charlie@x.com".into(),
            first_name: "Charlie".into(),
            last_name: "Doe".into(),
            age: 35,
        })?;
    }

    // where_in
    {
        let mut qb = QueryBuilder::<User>::new(conn.driver_mut());
        let users = qb
            .where_in("name", vec![Value::from("Alice"), Value::from("Bob")])
            .find()?;
        println!("where_in [Alice, Bob]: {:?}", users);
    }

    // transaction: update + insert atomically
    {
        let mut tx = Transaction::<User>::begin(conn.driver_mut())?;
        tx.where_eq("name", Value::from("Alice"))
            .update_one("age", Value::from(31))?;
        tx.insert(&User {
            id: 0,
            name: "Dave".into(),
            email: "dave@x.com".into(),
            first_name: "Dave".into(),
            last_name: "Doe".into(),
            age: 40,
        })?;
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
