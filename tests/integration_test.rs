use gorust::runtime;
use grorm::{
    ConnectionConfig, ConnectionPool, DeriveModel, Error, Model, QueryBuilder, SqliteDriverFactory,
    Transaction, Value,
};
use std::sync::atomic::{AtomicU64, Ordering};

static DB_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_db_path() -> String {
    let id = DB_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("target/test_integration_{}_{}.db", std::process::id(), id)
}

#[derive(Debug, PartialEq, DeriveModel)]
#[table = "test_users"]
struct TestUser {
    id: i64,
    #[index]
    name: String,
    #[unique]
    email: String,
    age: i32,
}

fn setup() -> Result<(ConnectionPool, String), Error> {
    let db_path = unique_db_path();
    let config = ConnectionConfig::sqlite(&db_path);
    let pool = ConnectionPool::new(SqliteDriverFactory, config, 2);
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());
    qb.create_table()?;
    Ok((pool, db_path))
}

fn cleanup(db_path: &str) {
    let _ = std::fs::remove_file(db_path);
}



fn case_create_table() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    let schema = TestUser::table_schema();
    assert_eq!(schema.len(), 4);
    assert_eq!(schema[0].name, "id");
    assert!(schema[0].is_primary_key);
    assert!(schema[0].is_auto_increment);
    assert_eq!(schema[1].name, "name");
    assert!(schema[1].is_index);
    assert_eq!(schema[2].name, "email");
    assert!(schema[2].is_unique);

    let count = qb.count()?;
    assert_eq!(count, 0);

    cleanup(&db_path);
    Ok(())
}



fn case_insert_and_find_by_id() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    let user = TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 };
    let id = qb.insert(&user)?;
    assert!(id.is_some());
    assert_eq!(id.unwrap(), 1);

    let found = qb.find_by_id(1)?;
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.name, "Alice");
    assert_eq!(found.email, "alice@test.com");
    assert_eq!(found.age, 30);

    let not_found = qb.find_by_id(999)?;
    assert!(not_found.is_none());

    cleanup(&db_path);
    Ok(())
}



fn case_find_all() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;

    let all = qb.find_all()?;
    assert_eq!(all.len(), 2);

    cleanup(&db_path);
    Ok(())
}



fn case_where_eq_and_find() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;

    let users = qb.where_eq("age", Value::from(30)).find()?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    cleanup(&db_path);
    Ok(())
}



fn case_where_in() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;
    qb.insert(&TestUser { id: 0, name: "Charlie".into(), email: "charlie@test.com".into(), age: 35 })?;

    let users = qb.where_in("name", vec![Value::from("Alice"), Value::from("Bob")]).find()?;
    assert_eq!(users.len(), 2);

    cleanup(&db_path);
    Ok(())
}



fn case_where_model() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;

    let filter = TestUser { id: 1, name: "".into(), email: "".into(), age: 0 };
    let users = qb.where_model(&filter).find()?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    cleanup(&db_path);
    Ok(())
}



fn case_limit_offset_order() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    for i in 0..10 {
        qb.insert(&TestUser { id: 0, name: format!("User{}", i), email: format!("user{}@test.com", i), age: 20 + i })?;
    }

    let all = qb.find_all()?;
    assert_eq!(all.len(), 10);

    cleanup(&db_path);
    Ok(())
}



fn case_count() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;

    let total = qb.count()?;
    assert_eq!(total, 2);

    cleanup(&db_path);
    Ok(())
}



fn case_update_one() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let rows = qb.where_eq("name", Value::from("Alice")).update_one("age", Value::from(31))?;
    assert_eq!(rows, 1);

    let user = qb.find_by_id(1)?;
    assert_eq!(user.unwrap().age, 31);

    cleanup(&db_path);
    Ok(())
}



fn case_update_model() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let update = TestUser { id: 0, name: "".into(), email: "".into(), age: 35 };
    let rows = qb.where_eq("name", Value::from("Alice")).update_model(&update)?;
    assert_eq!(rows, 1);

    let user = qb.find_by_id(1)?;
    assert_eq!(user.unwrap().age, 35);

    cleanup(&db_path);
    Ok(())
}



fn case_delete() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;

    let rows = qb.where_eq("name", Value::from("Alice")).delete()?;
    assert_eq!(rows, 1);

    let all = qb.find_all()?;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "Bob");

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_commit() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    {
        let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
        tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
        tx.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;
        tx.commit()?;
    }

    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());
    let all = qb.find_all()?;
    assert_eq!(all.len(), 2);

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_rollback() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    {
        let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
        tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
        tx.rollback()?;
    }

    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());
    let all = qb.find_all()?;
    assert_eq!(all.len(), 0);

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_auto_rollback_on_drop() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    {
        let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
        tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    }

    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());
    let all = qb.find_all()?;
    assert_eq!(all.len(), 0);

    cleanup(&db_path);
    Ok(())
}



fn case_find_one() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;

    let user = qb.where_eq("age", Value::from(30)).find_one()?;
    assert!(user.is_some());
    assert_eq!(user.unwrap().name, "Alice");

    let none = qb.where_eq("age", Value::from(99)).find_one()?;
    assert!(none.is_none());

    cleanup(&db_path);
    Ok(())
}



fn case_find_where() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let users = qb.find_where("name", Value::from("Alice"))?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].email, "alice@test.com");

    cleanup(&db_path);
    Ok(())
}



fn case_unique_constraint() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let schema = TestUser::table_schema();
    let email_col = schema.iter().find(|c| c.name == "email").unwrap();
    assert!(email_col.is_unique);

    cleanup(&db_path);
    Ok(())
}



fn case_update_without_where() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    let result = qb.update_one("age", Value::from(99));
    assert!(result.is_err());

    cleanup(&db_path);
    Ok(())
}



fn case_delete_without_where() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    let result = qb.delete();
    assert!(result.is_err());

    cleanup(&db_path);
    Ok(())
}



fn case_multiple_conditions() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Charlie".into(), email: "charlie@test.com".into(), age: 25 })?;

    let users = qb.where_eq("age", Value::from(30)).where_eq("name", Value::from("Alice")).find()?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    cleanup(&db_path);
    Ok(())
}



fn case_chain_reset_after_find() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;

    let users = qb.where_eq("age", Value::from(30)).find()?;
    assert_eq!(users.len(), 1);

    let all = qb.find_all()?;
    assert_eq!(all.len(), 2);

    cleanup(&db_path);
    Ok(())
}



fn case_table_name() -> Result<(), Error> {
    assert_eq!(TestUser::table_name(), "test_users");
    Ok(())
}



fn case_primary_key() -> Result<(), Error> {
    assert_eq!(TestUser::primary_key(), "id");
    Ok(())
}



fn case_columns() -> Result<(), Error> {
    let cols = TestUser::columns();
    assert_eq!(cols, &["id", "name", "email", "age"]);
    Ok(())
}



fn case_to_values_and_from_row() -> Result<(), Error> {
    let user = TestUser { id: 1, name: "Alice".into(), email: "alice@test.com".into(), age: 30 };
    let values = user.to_values();
    assert_eq!(values.len(), 4);

    let restored = TestUser::from_row(&values).unwrap();
    assert_eq!(restored, user);

    Ok(())
}



fn case_error_display() -> Result<(), Error> {
    let err = Error::NotFound("user 1".into());
    assert_eq!(format!("{}", err), "not found: user 1");

    let err = Error::Connection("timeout".into());
    assert_eq!(format!("{}", err), "connection error: timeout");

    let err = Error::Query("syntax error".into());
    assert_eq!(format!("{}", err), "query error: syntax error");

    Ok(())
}



fn case_error_from_string() -> Result<(), Error> {
    let err: Error = "test error".into();
    assert_eq!(format!("{}", err), "protocol error: test error");

    let err: Error = "test error".to_string().into();
    assert_eq!(format!("{}", err), "protocol error: test error");

    Ok(())
}



fn case_error_from_io() -> Result<(), Error> {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err: Error = io_err.into();
    assert!(format!("{}", err).contains("io error"));

    Ok(())
}



fn case_join_sql_generation() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.left_join("orders", "test_users.id = orders.user_id")
        .inner_join("profiles", "test_users.id = profiles.user_id")
        .right_join("scores", "test_users.id = scores.user_id");

    let (sql, _) = qb.build_select_sql();
    assert!(sql.contains("LEFT JOIN"));
    assert!(sql.contains("INNER JOIN"));
    assert!(sql.contains("RIGHT JOIN"));

    cleanup(&db_path);
    Ok(())
}



fn case_value_conversions() -> Result<(), Error> {
    use grorm::{FromSql, ToSql};

    let v = Value::I64(42);
    let i: i64 = FromSql::from_sql(&v).unwrap();
    assert_eq!(i, 42);
    assert_eq!(ToSql::to_sql(&42i64), Value::I64(42));

    let v = Value::String("hello".into());
    let s: String = FromSql::from_sql(&v).unwrap();
    assert_eq!(s, "hello");
    assert_eq!(ToSql::to_sql(&"hello".to_string()), Value::String("hello".into()));

    let v = Value::I32(7);
    let i: i32 = FromSql::from_sql(&v).unwrap();
    assert_eq!(i, 7);
    assert_eq!(ToSql::to_sql(&7i32), Value::I32(7));

    Ok(())
}



fn case_value_from() -> Result<(), Error> {
    let v = Value::from("hello");
    assert_eq!(v, Value::String("hello".into()));

    let v = Value::from(42i64);
    assert_eq!(v, Value::I64(42));

    let v = Value::from(7i32);
    assert_eq!(v, Value::I32(7));

    let v = Value::from(3.14f64);
    assert_eq!(v, Value::F64(3.14));

    Ok(())
}



fn case_connection_config() -> Result<(), Error> {
    let config = ConnectionConfig::sqlite("test.db");
    assert_eq!(config.db_type, "sqlite");
    assert_eq!(config.host, "test.db");

    let config = ConnectionConfig::postgres("localhost", 5432, "testdb", "user", "pass");
    assert_eq!(config.db_type, "postgres");
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 5432);
    assert_eq!(config.database, "testdb");
    assert_eq!(config.username, "user");
    assert_eq!(config.password, "pass");

    let config = ConnectionConfig::mysql("localhost", 3306, "testdb", "user", "pass");
    assert_eq!(config.db_type, "mysql");
    assert_eq!(config.port, 3306);

    Ok(())
}



fn case_pool_get_and_return() -> Result<(), Error> {
    let (pool, db_path) = setup()?;

    let mut conn1 = pool.get()?;
    let mut conn2 = pool.get()?;

    let mut qb1 = QueryBuilder::<TestUser>::new(conn1.driver_mut());
    let count1 = qb1.count()?;
    assert_eq!(count1, 0);

    let mut qb2 = QueryBuilder::<TestUser>::new(conn2.driver_mut());
    let count2 = qb2.count()?;
    assert_eq!(count2, 0);

    cleanup(&db_path);
    Ok(())
}



fn case_insert_multiple() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    let id1 = qb.insert(&TestUser { id: 0, name: "A".into(), email: "a@test.com".into(), age: 20 })?;
    let id2 = qb.insert(&TestUser { id: 0, name: "B".into(), email: "b@test.com".into(), age: 21 })?;
    let id3 = qb.insert(&TestUser { id: 0, name: "C".into(), email: "c@test.com".into(), age: 22 })?;

    assert_eq!(id1, Some(1));
    assert_eq!(id2, Some(2));
    assert_eq!(id3, Some(3));

    let all = qb.find_all()?;
    assert_eq!(all.len(), 3);

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_multiple_ops() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    {
        let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;

        tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
        tx.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;

        tx.where_eq("name", Value::from("Alice")).update_one("age", Value::from(31))?;
        tx.where_eq("name", Value::from("Bob")).delete()?;

        let users = tx.find_all()?;
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "Alice");
        assert_eq!(users[0].age, 31);

        tx.commit()?;
    }

    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());
    let all = qb.find_all()?;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "Alice");
    assert_eq!(all[0].age, 31);

    cleanup(&db_path);
    Ok(())
}



fn case_composite_unique_index() -> Result<(), Error> {
    #[derive(Debug, PartialEq, DeriveModel)]
    #[table = "test_composite"]
    struct CompositeUser {
        id: i64,
        #[unique_index = "uq_name_email"]
        name: String,
        #[unique_index = "uq_name_email"]
        email: String,
    }

    let schema = CompositeUser::table_schema();
    assert_eq!(schema.len(), 3);
    assert_eq!(schema[1].unique_index_name, Some("uq_name_email"));
    assert_eq!(schema[2].unique_index_name, Some("uq_name_email"));

    let db_path = unique_db_path();
    let config = ConnectionConfig::sqlite(&db_path);
    let pool = ConnectionPool::new(SqliteDriverFactory, config, 2);
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<CompositeUser>::new(conn.driver_mut());
    qb.create_table()?;

    qb.insert(&CompositeUser { id: 0, name: "Alice".into(), email: "alice@test.com".into() })?;

    let all = qb.find_all()?;
    assert_eq!(all.len(), 1);

    cleanup(&db_path);
    Ok(())
}



fn case_custom_table_name() -> Result<(), Error> {
    #[derive(Debug, PartialEq, DeriveModel)]
    #[table = "custom_table_name"]
    struct CustomTable { id: i64, name: String }

    assert_eq!(CustomTable::table_name(), "custom_table_name");
    Ok(())
}



fn case_custom_primary_key() -> Result<(), Error> {
    #[derive(Debug, PartialEq, DeriveModel)]
    #[table = "test_custom_pk"]
    #[primary_key = "uuid"]
    struct CustomPk { uuid: String, name: String }

    assert_eq!(CustomPk::primary_key(), "uuid");
    Ok(())
}



fn case_null_value() -> Result<(), Error> {
    use grorm::FromSql;

    let v = Value::Null;
    let result: Result<i64, _> = FromSql::from_sql(&v);
    assert!(result.is_err());

    Ok(())
}



fn case_bool_value() -> Result<(), Error> {
    use grorm::{FromSql, ToSql};

    let v = Value::Bool(true);
    let b: bool = FromSql::from_sql(&v).unwrap();
    assert!(b);

    assert_eq!(ToSql::to_sql(&true), Value::Bool(true));
    assert_eq!(ToSql::to_sql(&false), Value::Bool(false));

    Ok(())
}



fn case_f64_value() -> Result<(), Error> {
    use grorm::{FromSql, ToSql};

    let v = Value::F64(3.14);
    let f: f64 = FromSql::from_sql(&v).unwrap();
    assert!((f - 3.14).abs() < 0.001);

    assert_eq!(ToSql::to_sql(&3.14f64), Value::F64(3.14));

    Ok(())
}



fn case_bytes_value() -> Result<(), Error> {
    use grorm::{FromSql, ToSql};

    let data = vec![1u8, 2, 3];
    let v = Value::Bytes(data.clone());
    let b: Vec<u8> = FromSql::from_sql(&v).unwrap();
    assert_eq!(b, data);

    assert_eq!(ToSql::to_sql(&data), Value::Bytes(data));

    Ok(())
}



fn case_value_clone_and_debug() -> Result<(), Error> {
    let v = Value::String("hello".into());
    let v2 = v.clone();
    assert_eq!(v, v2);
    assert!(format!("{:?}", v).contains("hello"));

    Ok(())
}



fn case_error_debug() -> Result<(), Error> {
    let err = Error::NotFound("test".into());
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("NotFound"));
    assert!(debug_str.contains("test"));

    Ok(())
}



fn case_error_source() -> Result<(), Error> {
    use std::error::Error as StdError;

    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "test");
    let err = Error::Io(io_err);
    assert!(err.source().is_some());

    let err = Error::NotFound("test".into());
    assert!(err.source().is_none());

    Ok(())
}



fn case_join_type_sql() -> Result<(), Error> {
    use grorm::JoinType;

    assert_eq!(JoinType::Left.as_sql(), "LEFT JOIN");
    assert_eq!(JoinType::Inner.as_sql(), "INNER JOIN");
    assert_eq!(JoinType::Right.as_sql(), "RIGHT JOIN");

    Ok(())
}



fn case_query_builder_table_name() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    assert_eq!(qb.table_name(), "test_users");

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_count() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
    tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    tx.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;

    let count = tx.count()?;
    assert_eq!(count, 2);

    tx.commit()?;

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_find_one() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
    tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let user = tx.find_one()?;
    assert!(user.is_some());
    assert_eq!(user.unwrap().name, "Alice");

    tx.commit()?;

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_find_where() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
    tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let users = tx.find_where("name", Value::from("Alice"))?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].email, "alice@test.com");

    tx.commit()?;

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_update_model() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
    tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let update = TestUser { id: 0, name: "".into(), email: "".into(), age: 35 };
    let rows = tx.where_eq("name", Value::from("Alice")).update_model(&update)?;
    assert_eq!(rows, 1);

    let user = tx.find_by_id(1)?;
    assert_eq!(user.unwrap().age, 35);

    tx.commit()?;

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_limit_offset_order() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
    for i in 0..5 {
        tx.insert(&TestUser { id: 0, name: format!("User{}", i), email: format!("user{}@test.com", i), age: 20 + i })?;
    }

    let all = tx.find_all()?;
    assert_eq!(all.len(), 5);

    tx.commit()?;

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_where_in() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
    tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    tx.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;
    tx.insert(&TestUser { id: 0, name: "Charlie".into(), email: "charlie@test.com".into(), age: 35 })?;

    let users = tx.where_in("name", vec![Value::from("Alice"), Value::from("Bob")]).find()?;
    assert_eq!(users.len(), 2);

    tx.commit()?;

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_where_model() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
    tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
    tx.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 25 })?;

    let filter = TestUser { id: 1, name: "".into(), email: "".into(), age: 0 };
    let users = tx.where_model(&filter).find()?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    tx.commit()?;

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_explicit_rollback() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    {
        let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
        tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
        tx.rollback()?;
    }

    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());
    let all = qb.find_all()?;
    assert_eq!(all.len(), 0);

    cleanup(&db_path);
    Ok(())
}



fn case_transaction_commit_then_drop() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;

    {
        let mut tx = Transaction::<TestUser>::begin(conn.driver_mut())?;
        tx.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;
        tx.commit()?;
    }

    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());
    let all = qb.find_all()?;
    assert_eq!(all.len(), 1);

    cleanup(&db_path);
    Ok(())
}



fn case_update_model_no_fields() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let empty_update = TestUser { id: 0, name: "".into(), email: "".into(), age: 0 };
    let result = qb.where_eq("name", Value::from("Alice")).update_model(&empty_update);
    assert!(result.is_err());

    cleanup(&db_path);
    Ok(())
}



fn case_insert_with_explicit_id() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 100, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let all = qb.find_all()?;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "Alice");

    cleanup(&db_path);
    Ok(())
}



fn case_count_empty_table() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    let count = qb.count()?;
    assert_eq!(count, 0);

    cleanup(&db_path);
    Ok(())
}



fn case_find_all_empty() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    let all = qb.find_all()?;
    assert_eq!(all.len(), 0);

    cleanup(&db_path);
    Ok(())
}



fn case_order_multiple_columns() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Bob".into(), email: "bob@test.com".into(), age: 30 })?;
    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let all = qb.find_all()?;
    assert_eq!(all.len(), 2);

    cleanup(&db_path);
    Ok(())
}



fn case_where_in_empty() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let all = qb.find_all()?;
    assert_eq!(all.len(), 1);

    cleanup(&db_path);
    Ok(())
}



fn case_update_one_zero_rows() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    let rows = qb.where_eq("name", Value::from("Nobody")).update_one("age", Value::from(99))?;
    assert_eq!(rows, 0);

    cleanup(&db_path);
    Ok(())
}



fn case_delete_zero_rows() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    let rows = qb.where_eq("name", Value::from("Nobody")).delete()?;
    assert_eq!(rows, 0);

    cleanup(&db_path);
    Ok(())
}



fn case_find_no_results() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    let users = qb.where_eq("name", Value::from("Nobody")).find()?;
    assert_eq!(users.len(), 0);

    cleanup(&db_path);
    Ok(())
}



fn case_limit_zero() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let all = qb.find_all()?;
    assert_eq!(all.len(), 1);

    cleanup(&db_path);
    Ok(())
}



fn case_offset_beyond_data() -> Result<(), Error> {
    let (pool, db_path) = setup()?;
    let mut conn = pool.get()?;
    let mut qb = QueryBuilder::<TestUser>::new(conn.driver_mut());

    qb.insert(&TestUser { id: 0, name: "Alice".into(), email: "alice@test.com".into(), age: 30 })?;

    let all = qb.find_all()?;
    assert_eq!(all.len(), 1);

    cleanup(&db_path);
    Ok(())
}



fn case_connection_config_types() -> Result<(), Error> {
    let sqlite = ConnectionConfig::sqlite("test.db");
    assert_eq!(sqlite.db_type, "sqlite");

    let pg = ConnectionConfig::postgres("localhost", 5432, "testdb", "user", "pass");
    assert_eq!(pg.db_type, "postgres");

    let mysql = ConnectionConfig::mysql("localhost", 3306, "testdb", "user", "pass");
    assert_eq!(mysql.db_type, "mysql");

    Ok(())
}



fn case_column_info_debug() -> Result<(), Error> {
    let schema = TestUser::table_schema();
    let debug_str = format!("{:?}", schema);
    assert!(debug_str.contains("id"));
    assert!(debug_str.contains("name"));
    assert!(debug_str.contains("email"));
    assert!(debug_str.contains("age"));

    Ok(())
}



fn case_join_clause_debug() -> Result<(), Error> {
    use grorm::{JoinClause, JoinType};

    let clause = JoinClause { join_type: JoinType::Left, table: "orders".into(), on_clause: "users.id = orders.user_id".into() };
    let debug_str = format!("{:?}", clause);
    assert!(debug_str.contains("orders"));
    assert!(debug_str.contains("Left"));

    Ok(())
}



fn case_join_type_debug() -> Result<(), Error> {
    use grorm::JoinType;

    assert_eq!(format!("{:?}", JoinType::Left), "Left");
    assert_eq!(format!("{:?}", JoinType::Inner), "Inner");
    assert_eq!(format!("{:?}", JoinType::Right), "Right");

    Ok(())
}



fn case_join_type_clone() -> Result<(), Error> {
    use grorm::JoinType;

    let jt = JoinType::Left;
    let jt2 = jt;
    assert_eq!(jt2, JoinType::Left);

    Ok(())
}



fn case_column_info_clone() -> Result<(), Error> {
    let schema = TestUser::table_schema();
    let cloned = schema[0].clone();
    assert_eq!(cloned.name, "id");

    Ok(())
}



fn case_value_partial_eq() -> Result<(), Error> {
    assert_eq!(Value::I64(1), Value::I64(1));
    assert_ne!(Value::I64(1), Value::I64(2));
    assert_ne!(Value::I64(1), Value::String("1".into()));

    Ok(())
}



fn case_value_default() -> Result<(), Error> {
    let v = Value::Null;
    assert_eq!(v, Value::Null);

    Ok(())
}



fn case_value_from_str() -> Result<(), Error> {
    let v = Value::from("hello");
    assert_eq!(v, Value::String("hello".into()));

    Ok(())
}



fn case_value_from_i64() -> Result<(), Error> {
    let v = Value::from(42i64);
    assert_eq!(v, Value::I64(42));

    Ok(())
}



fn case_value_from_i32() -> Result<(), Error> {
    let v = Value::from(7i32);
    assert_eq!(v, Value::I32(7));

    Ok(())
}



fn case_value_from_f64() -> Result<(), Error> {
    let v = Value::from(3.14);
    assert_eq!(v, Value::F64(3.14));

    Ok(())
}



fn case_value_from_bool() -> Result<(), Error> {
    let v = Value::from(true);
    assert_eq!(v, Value::Bool(true));

    Ok(())
}



fn case_value_from_bytes() -> Result<(), Error> {
    let data = vec![1u8, 2, 3];
    let v = Value::from(data.clone());
    assert_eq!(v, Value::Bytes(data));

    Ok(())
}



fn case_from_sql_i64() -> Result<(), Error> {
    use grorm::FromSql;

    let v = Value::I64(42);
    let i: i64 = FromSql::from_sql(&v).unwrap();
    assert_eq!(i, 42);

    Ok(())
}



fn case_from_sql_i32() -> Result<(), Error> {
    use grorm::FromSql;

    let v = Value::I32(7);
    let i: i32 = FromSql::from_sql(&v).unwrap();
    assert_eq!(i, 7);

    Ok(())
}



fn case_from_sql_string() -> Result<(), Error> {
    use grorm::FromSql;

    let v = Value::String("hello".into());
    let s: String = FromSql::from_sql(&v).unwrap();
    assert_eq!(s, "hello");

    Ok(())
}



fn case_from_sql_f64() -> Result<(), Error> {
    use grorm::FromSql;

    let v = Value::F64(3.14);
    let f: f64 = FromSql::from_sql(&v).unwrap();
    assert!((f - 3.14).abs() < 0.001);

    Ok(())
}



fn case_from_sql_bool() -> Result<(), Error> {
    use grorm::FromSql;

    let v = Value::Bool(true);
    let b: bool = FromSql::from_sql(&v).unwrap();
    assert!(b);

    Ok(())
}



fn case_from_sql_bytes() -> Result<(), Error> {
    use grorm::FromSql;

    let data = vec![1u8, 2, 3];
    let v = Value::Bytes(data.clone());
    let b: Vec<u8> = FromSql::from_sql(&v).unwrap();
    assert_eq!(b, data);

    Ok(())
}



fn case_from_sql_null() -> Result<(), Error> {
    use grorm::FromSql;

    let v = Value::Null;
    let result: Result<i64, _> = FromSql::from_sql(&v);
    assert!(result.is_err());

    Ok(())
}



fn case_to_sql_i64() -> Result<(), Error> {
    use grorm::ToSql;
    assert_eq!(ToSql::to_sql(&42i64), Value::I64(42));
    Ok(())
}



fn case_to_sql_i32() -> Result<(), Error> {
    use grorm::ToSql;
    assert_eq!(ToSql::to_sql(&7i32), Value::I32(7));
    Ok(())
}



fn case_to_sql_string() -> Result<(), Error> {
    use grorm::ToSql;
    assert_eq!(ToSql::to_sql(&"hello".to_string()), Value::String("hello".into()));
    Ok(())
}



fn case_to_sql_f64() -> Result<(), Error> {
    use grorm::ToSql;
    assert_eq!(ToSql::to_sql(&3.14f64), Value::F64(3.14));
    Ok(())
}



fn case_to_sql_bool() -> Result<(), Error> {
    use grorm::ToSql;
    assert_eq!(ToSql::to_sql(&true), Value::Bool(true));
    assert_eq!(ToSql::to_sql(&false), Value::Bool(false));
    Ok(())
}



fn case_to_sql_bytes() -> Result<(), Error> {
    use grorm::ToSql;
    let data = vec![1u8, 2, 3];
    assert_eq!(ToSql::to_sql(&data), Value::Bytes(data));
    Ok(())
}



fn case_to_sql_str() -> Result<(), Error> {
    use grorm::ToSql;
    assert_eq!(ToSql::to_sql(&"hello"), Value::String("hello".into()));
    Ok(())
}



fn case_to_sql_option_some() -> Result<(), Error> {
    use grorm::ToSql;
    let opt: Option<i64> = Some(42);
    assert_eq!(ToSql::to_sql(&opt), Value::I64(42));
    Ok(())
}



fn case_to_sql_option_none() -> Result<(), Error> {
    use grorm::ToSql;
    let opt: Option<i64> = None;
    assert_eq!(ToSql::to_sql(&opt), Value::Null);
    Ok(())
}



fn case_error_from_box_dyn() -> Result<(), Error> {
    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "test");
    let boxed: Box<dyn std::error::Error> = Box::new(io_err);
    let err: Error = boxed.into();
    assert!(format!("{}", err).contains("protocol error"));
    Ok(())
}



fn case_error_from_str() -> Result<(), Error> {
    let err: Error = "test error".into();
    assert_eq!(format!("{}", err), "protocol error: test error");
    Ok(())
}



fn case_error_from_string_type() -> Result<(), Error> {
    let err: Error = "test error".to_string().into();
    assert_eq!(format!("{}", err), "protocol error: test error");
    Ok(())
}



fn case_error_from_io_error() -> Result<(), Error> {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err: Error = io_err.into();
    assert!(format!("{}", err).contains("io error"));
    Ok(())
}



fn case_error_display_all_variants() -> Result<(), Error> {
    assert_eq!(format!("{}", Error::Connection("x".into())), "connection error: x");
    assert_eq!(format!("{}", Error::Query("x".into())), "query error: x");
    assert_eq!(format!("{}", Error::Execute("x".into())), "execute error: x");
    assert_eq!(format!("{}", Error::Protocol("x".into())), "protocol error: x");
    assert_eq!(format!("{}", Error::Model("x".into())), "model error: x");
    assert_eq!(format!("{}", Error::Pool("x".into())), "pool error: x");
    assert_eq!(format!("{}", Error::Config("x".into())), "config error: x");
    assert_eq!(format!("{}", Error::NotFound("x".into())), "not found: x");
    assert_eq!(format!("{}", Error::Transaction("x".into())), "transaction error: x");
    Ok(())
}



fn case_error_debug_all_variants() -> Result<(), Error> {
    let variants = [
        Error::Connection("x".into()),
        Error::Query("x".into()),
        Error::Execute("x".into()),
        Error::Protocol("x".into()),
        Error::Model("x".into()),
        Error::Pool("x".into()),
        Error::Config("x".into()),
        Error::NotFound("x".into()),
        Error::Transaction("x".into()),
    ];
    for err in &variants {
        assert!(!format!("{:?}", err).is_empty());
    }
    Ok(())
}



fn case_error_source_io() -> Result<(), Error> {
    use std::error::Error as StdError;
    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "test");
    let err = Error::Io(io_err);
    assert!(err.source().is_some());
    Ok(())
}



fn case_error_source_none() -> Result<(), Error> {
    use std::error::Error as StdError;
    assert!(Error::NotFound("test".into()).source().is_none());
    assert!(Error::Connection("test".into()).source().is_none());
    Ok(())
}



fn case_result_type_alias() -> Result<(), Error> {
    let r: grorm::Result<i64> = Ok(42);
    assert_eq!(r.unwrap(), 42);

    let r: grorm::Result<i64> = Err(Error::NotFound("test".into()));
    assert!(r.is_err());
    Ok(())
}



fn case_model_trait_methods() -> Result<(), Error> {
    assert_eq!(TestUser::table_name(), "test_users");
    assert_eq!(TestUser::primary_key(), "id");
    assert_eq!(TestUser::columns(), &["id", "name", "email", "age"]);
    assert_eq!(TestUser::table_schema().len(), 4);
    Ok(())
}



fn case_model_from_row_error() -> Result<(), Error> {
    let values = vec![Value::String("not_an_int".into()), Value::String("x".into()), Value::String("x".into()), Value::String("x".into())];
    assert!(TestUser::from_row(&values).is_err());
    Ok(())
}



fn case_model_to_values_roundtrip() -> Result<(), Error> {
    let user = TestUser { id: 42, name: "Test".into(), email: "test@test.com".into(), age: 99 };
    let values = user.to_values();
    let restored = TestUser::from_row(&values).unwrap();
    assert_eq!(restored, user);
    Ok(())
}



fn case_model_empty_string() -> Result<(), Error> {
    let user = TestUser { id: 0, name: "".into(), email: "".into(), age: 0 };
    let values = user.to_values();
    let restored = TestUser::from_row(&values).unwrap();
    assert_eq!(restored, user);
    Ok(())
}



fn case_model_negative_age() -> Result<(), Error> {
    let user = TestUser { id: 0, name: "Test".into(), email: "test@test.com".into(), age: -5 };
    let values = user.to_values();
    let restored = TestUser::from_row(&values).unwrap();
    assert_eq!(restored.age, -5);
    Ok(())
}



fn case_model_large_id() -> Result<(), Error> {
    let user = TestUser { id: i64::MAX, name: "Test".into(), email: "test@test.com".into(), age: 30 };
    let values = user.to_values();
    let restored = TestUser::from_row(&values).unwrap();
    assert_eq!(restored.id, i64::MAX);
    Ok(())
}



fn case_model_special_chars() -> Result<(), Error> {
    let user = TestUser { id: 0, name: "O'Brien".into(), email: "test+tag@test.com".into(), age: 30 };
    let values = user.to_values();
    let restored = TestUser::from_row(&values).unwrap();
    assert_eq!(restored.name, "O'Brien");
    assert_eq!(restored.email, "test+tag@test.com");
    Ok(())
}



fn case_model_unicode() -> Result<(), Error> {
    let user = TestUser { id: 0, name: "测试用户".into(), email: "test@test.com".into(), age: 30 };
    let values = user.to_values();
    let restored = TestUser::from_row(&values).unwrap();
    assert_eq!(restored.name, "测试用户");
    Ok(())
}



fn case_derive_model_attributes() -> Result<(), Error> {
    #[derive(Debug, PartialEq, DeriveModel)]
    #[table = "custom_attrs"]
    #[primary_key = "uuid"]
    struct CustomAttrs {
        uuid: String,
        #[index]
        name: String,
        #[unique]
        email: String,
    }

    assert_eq!(CustomAttrs::table_name(), "custom_attrs");
    assert_eq!(CustomAttrs::primary_key(), "uuid");

    let schema = CustomAttrs::table_schema();
    assert_eq!(schema.len(), 3);
    assert!(schema[1].is_index);
    assert!(schema[2].is_unique);
    Ok(())
}



fn case_derive_model_unique_index() -> Result<(), Error> {
    #[derive(Debug, PartialEq, DeriveModel)]
    #[table = "test_unique_idx"]
    struct UniqueIdxModel {
        id: i64,
        #[unique_index = "uq_a_b"]
        a: String,
        #[unique_index = "uq_a_b"]
        b: String,
    }

    let schema = UniqueIdxModel::table_schema();
    assert_eq!(schema[1].unique_index_name, Some("uq_a_b"));
    assert_eq!(schema[2].unique_index_name, Some("uq_a_b"));
    Ok(())
}



fn case_derive_table_macro() -> Result<(), Error> {
    use grorm::DeriveTable;

    #[derive(Debug, DeriveTable)]
    #[table_name = "my_custom_table"]
    struct MyTable { id: i64 }

    assert_eq!(MyTable::table_name(), "my_custom_table");
    Ok(())
}



fn case_derive_table_default_name() -> Result<(), Error> {
    use grorm::DeriveTable;

    #[derive(Debug, DeriveTable)]
    struct UserProfile {}

    assert_eq!(UserProfile::table_name(), "userprofiles");
    Ok(())
}



fn case_value_clone() -> Result<(), Error> {
    assert_eq!(Value::String("hello".into()).clone(), Value::String("hello".into()));
    assert_eq!(Value::I64(42).clone(), Value::I64(42));
    assert_eq!(Value::Null.clone(), Value::Null);
    Ok(())
}



fn case_value_debug() -> Result<(), Error> {
    assert!(format!("{:?}", Value::I64(42)).contains("42"));
    assert!(format!("{:?}", Value::String("hello".into())).contains("hello"));
    assert!(format!("{:?}", Value::Null).contains("Null"));
    assert!(format!("{:?}", Value::Bool(true)).contains("true"));
    assert!(format!("{:?}", Value::F64(3.14)).contains("3.14"));
    Ok(())
}



fn case_value_display() -> Result<(), Error> {
    assert_eq!(format!("{}", Value::I64(42)), "42");
    assert_eq!(format!("{}", Value::String("hello".into())), "'hello'");
    assert_eq!(format!("{}", Value::Null), "NULL");
    assert_eq!(format!("{}", Value::Bool(true)), "true");
    assert_eq!(format!("{}", Value::F64(3.14)), "3.14");
    Ok(())
}



fn case_value_default_impl() -> Result<(), Error> {
    assert_eq!(Value::Null, Value::Null);
    Ok(())
}



fn case_value_from_impls() -> Result<(), Error> {
    assert_eq!(Value::from("hello"), Value::String("hello".into()));
    assert_eq!(Value::from(42i64), Value::I64(42));
    assert_eq!(Value::from(7i32), Value::I32(7));
    assert_eq!(Value::from(3.14f64), Value::F64(3.14));
    assert_eq!(Value::from(true), Value::Bool(true));
    assert_eq!(Value::from(vec![1u8, 2, 3]), Value::Bytes(vec![1, 2, 3]));
    Ok(())
}



fn case_from_sql_all_types() -> Result<(), Error> {
    use grorm::FromSql;
    assert_eq!(<i64>::from_sql(&Value::I64(42)).unwrap(), 42);
    assert_eq!(<i32>::from_sql(&Value::I32(7)).unwrap(), 7);
    assert_eq!(<String>::from_sql(&Value::String("x".into())).unwrap(), "x");
    assert!((<f64>::from_sql(&Value::F64(3.14)).unwrap() - 3.14).abs() < 0.001);
    assert!(<bool>::from_sql(&Value::Bool(true)).unwrap());
    assert_eq!(<Vec<u8>>::from_sql(&Value::Bytes(vec![1, 2])).unwrap(), vec![1, 2]);
    Ok(())
}



fn case_to_sql_all_types() -> Result<(), Error> {
    use grorm::ToSql;
    assert_eq!(ToSql::to_sql(&42i64), Value::I64(42));
    assert_eq!(ToSql::to_sql(&7i32), Value::I32(7));
    assert_eq!(ToSql::to_sql(&"x".to_string()), Value::String("x".into()));
    assert_eq!(ToSql::to_sql(&3.14f64), Value::F64(3.14));
    assert_eq!(ToSql::to_sql(&true), Value::Bool(true));
    assert_eq!(ToSql::to_sql(&vec![1u8, 2]), Value::Bytes(vec![1, 2]));
    Ok(())
}



fn case_to_sql_str_ref() -> Result<(), Error> {
    use grorm::ToSql;
    assert_eq!(ToSql::to_sql(&"hello"), Value::String("hello".into()));
    Ok(())
}



fn case_to_sql_option() -> Result<(), Error> {
    use grorm::ToSql;
    assert_eq!(ToSql::to_sql(&Some(42i64)), Value::I64(42));
    assert_eq!(ToSql::to_sql(&None::<i64>), Value::Null);
    Ok(())
}



fn case_derive_table_no_attr() -> Result<(), Error> {
    use grorm::DeriveTable;

    #[derive(Debug, DeriveTable)]
    struct FooBar {}

    assert_eq!(FooBar::table_name(), "foobars");
    Ok(())
}



fn case_derive_model_default_table() -> Result<(), Error> {
    #[derive(Debug, PartialEq, DeriveModel)]
    struct DefaultTable { id: i64, name: String }

    assert_eq!(DefaultTable::table_name(), "defaulttables");
    Ok(())
}



fn case_derive_model_default_pk() -> Result<(), Error> {
    #[derive(Debug, PartialEq, DeriveModel)]
    struct DefaultPk { id: i64, name: String }

    assert_eq!(DefaultPk::primary_key(), "id");
    Ok(())
}



fn case_derive_model_non_int_pk() -> Result<(), Error> {
    #[derive(Debug, PartialEq, DeriveModel)]
    #[table = "test_str_pk"]
    #[primary_key = "uuid"]
    struct StrPk { uuid: String, name: String }

    let schema = StrPk::table_schema();
    assert!(schema[0].is_primary_key);
    assert!(!schema[0].is_auto_increment);
    Ok(())
}



fn case_derive_model_int_pk_auto() -> Result<(), Error> {
    #[derive(Debug, PartialEq, DeriveModel)]
    #[table = "test_int_pk"]
    struct IntPk { id: i64, name: String }

    let schema = IntPk::table_schema();
    assert!(schema[0].is_primary_key);
    assert!(schema[0].is_auto_increment);
    Ok(())
}



fn case_derive_model_no_index_no_unique() -> Result<(), Error> {
    #[derive(Debug, PartialEq, DeriveModel)]
    #[table = "test_plain"]
    struct Plain { id: i64, name: String, email: String }

    let schema = Plain::table_schema();
    assert_eq!(schema.len(), 3);
    assert!(!schema[0].is_index);
    assert!(!schema[0].is_unique);
    assert!(!schema[1].is_index);
    assert!(!schema[1].is_unique);
    assert!(!schema[2].is_index);
    assert!(!schema[2].is_unique);
    Ok(())
}
#[test]
#[runtime]
fn run_all_tests() {
    let mut passed = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    macro_rules! run {
        ($f:ident) => {
            match $f() {
                Ok(()) => { passed += 1; }
                Err(e) => {
                    failed += 1;
                    errors.push((stringify!($f), e));
                }
            }
        };
    }

    run!(case_bool_value);
    run!(case_bytes_value);
    run!(case_chain_reset_after_find);
    run!(case_column_info_clone);
    run!(case_column_info_debug);
    run!(case_columns);
    run!(case_composite_unique_index);
    run!(case_connection_config);
    run!(case_connection_config_types);
    run!(case_count);
    run!(case_count_empty_table);
    run!(case_create_table);
    run!(case_custom_primary_key);
    run!(case_custom_table_name);
    run!(case_delete);
    run!(case_delete_without_where);
    run!(case_delete_zero_rows);
    run!(case_derive_model_attributes);
    run!(case_derive_model_default_pk);
    run!(case_derive_model_default_table);
    run!(case_derive_model_int_pk_auto);
    run!(case_derive_model_no_index_no_unique);
    run!(case_derive_model_non_int_pk);
    run!(case_derive_model_unique_index);
    run!(case_derive_table_default_name);
    run!(case_derive_table_macro);
    run!(case_derive_table_no_attr);
    run!(case_error_debug);
    run!(case_error_debug_all_variants);
    run!(case_error_display);
    run!(case_error_display_all_variants);
    run!(case_error_from_box_dyn);
    run!(case_error_from_io);
    run!(case_error_from_io_error);
    run!(case_error_from_str);
    run!(case_error_from_string);
    run!(case_error_from_string_type);
    run!(case_error_source);
    run!(case_error_source_io);
    run!(case_error_source_none);
    run!(case_f64_value);
    run!(case_find_all);
    run!(case_find_all_empty);
    run!(case_find_no_results);
    run!(case_find_one);
    run!(case_find_where);
    run!(case_from_sql_all_types);
    run!(case_from_sql_bool);
    run!(case_from_sql_bytes);
    run!(case_from_sql_f64);
    run!(case_from_sql_i32);
    run!(case_from_sql_i64);
    run!(case_from_sql_null);
    run!(case_from_sql_string);
    run!(case_insert_and_find_by_id);
    run!(case_insert_multiple);
    run!(case_insert_with_explicit_id);
    run!(case_join_clause_debug);
    run!(case_join_sql_generation);
    run!(case_join_type_clone);
    run!(case_join_type_debug);
    run!(case_join_type_sql);
    run!(case_limit_offset_order);
    run!(case_limit_zero);
    run!(case_model_empty_string);
    run!(case_model_from_row_error);
    run!(case_model_large_id);
    run!(case_model_negative_age);
    run!(case_model_special_chars);
    run!(case_model_to_values_roundtrip);
    run!(case_model_trait_methods);
    run!(case_model_unicode);
    run!(case_multiple_conditions);
    run!(case_null_value);
    run!(case_offset_beyond_data);
    run!(case_order_multiple_columns);
    run!(case_pool_get_and_return);
    run!(case_primary_key);
    run!(case_query_builder_table_name);
    run!(case_result_type_alias);
    run!(case_table_name);
    run!(case_to_sql_all_types);
    run!(case_to_sql_bool);
    run!(case_to_sql_bytes);
    run!(case_to_sql_f64);
    run!(case_to_sql_i32);
    run!(case_to_sql_i64);
    run!(case_to_sql_option);
    run!(case_to_sql_option_none);
    run!(case_to_sql_option_some);
    run!(case_to_sql_str);
    run!(case_to_sql_string);
    run!(case_to_sql_str_ref);
    run!(case_to_values_and_from_row);
    run!(case_transaction_auto_rollback_on_drop);
    run!(case_transaction_commit);
    run!(case_transaction_commit_then_drop);
    run!(case_transaction_count);
    run!(case_transaction_explicit_rollback);
    run!(case_transaction_find_one);
    run!(case_transaction_find_where);
    run!(case_transaction_limit_offset_order);
    run!(case_transaction_multiple_ops);
    run!(case_transaction_rollback);
    run!(case_transaction_update_model);
    run!(case_transaction_where_in);
    run!(case_transaction_where_model);
    run!(case_unique_constraint);
    run!(case_update_model);
    run!(case_update_model_no_fields);
    run!(case_update_one);
    run!(case_update_one_zero_rows);
    run!(case_update_without_where);
    run!(case_value_clone);
    run!(case_value_clone_and_debug);
    run!(case_value_conversions);
    run!(case_value_debug);
    run!(case_value_default);
    run!(case_value_default_impl);
    run!(case_value_display);
    run!(case_value_from);
    run!(case_value_from_bool);
    run!(case_value_from_bytes);
    run!(case_value_from_f64);
    run!(case_value_from_i32);
    run!(case_value_from_i64);
    run!(case_value_from_impls);
    run!(case_value_from_str);
    run!(case_value_partial_eq);
    run!(case_where_eq_and_find);
    run!(case_where_in);
    run!(case_where_in_empty);
    run!(case_where_model);

    println!("\n=== Test Results ===");
    println!("Passed: {}", passed);
    println!("Failed: {}", failed);
    for (name, err) in &errors {
        println!("  FAIL {}: {}", name, err);
    }
    if failed > 0 {
        panic!("{} tests failed", failed);
    }
}
