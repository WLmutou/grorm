## 介绍
名称: grorm
含义: GRoutines + ORM (Goroutine-native ORM for Rust)
标语: "Goroutine-native ORM for Rust - Multi-database support with Go-style concurrency"

目标数据库:
- PostgreSQL 🔄 (已实现)
- MySQL     🔄 (已实现)
- SQLite    🔄 (已实现)


## 1. 项目结构
```bash
grorm/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   ├── driver/          # 数据库驱动抽象层
│   │   ├── mod.rs
│   │   ├── postgres.rs  # PostgreSQL 实现
│   │   ├── mysql.rs     # MySQL 实现 (待开发)
│   │   └── sqlite.rs    # SQLite 实现 (待开发)
│   ├── protocol/        # 数据库协议实现
│   │   ├── mod.rs
│   │   ├── pg.rs
│   │   ├── myproto.rs
│   │   └── sqlite_proto.rs
│   ├── query/           # SQL 构建器
│   │   ├── mod.rs
│   │   ├── select.rs
│   │   ├── insert.rs
│   │   ├── update.rs
│   │   └── delete.rs
│   ├── types/           # 类型映射
│   │   ├── mod.rs
│   │   ├── from_sql.rs
│   │   ├── to_sql.rs
│   │   └── value.rs
│   ├── orm/             # ORM 核心
│   │   ├── mod.rs
│   │   ├── model.rs
│   │   ├── query.rs
│   │   └── transaction.rs
│   └── pool/            # 连接池 (基于 gorust channel)
│       └── mod.rs
├── grorm-macros/        # 过程宏 crate
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs       # #[derive(Model, Table)] 等
└── examples/
    ├── postgres_demo.rs
    ├── mysql_demo.rs
    └── sqlite_demo.rs
```