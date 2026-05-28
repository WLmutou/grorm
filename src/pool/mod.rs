use crate::driver::default::DefaultDriverFactory;
use crate::driver::{ConnectionConfig, DatabaseDriver, DriverFactory};
use crate::error::Error;
use gorust::channel::{self, Receiver, Sender};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

type PooledConnection = Box<dyn DatabaseDriver>;

struct PoolInner {
    connections: Mutex<Vec<Option<PooledConnection>>>,
    available: Mutex<VecDeque<usize>>,
    notify_tx: Sender<()>,
    notify_rx: Receiver<()>,
    config: ConnectionConfig,
    factory: Box<dyn DriverFactory>,
    max_size: usize,
    current_size: AtomicUsize,
}

#[derive(Clone)]
pub struct ConnectionPool {
    inner: Arc<PoolInner>,
}

impl Default for ConnectionPool {
    fn default() -> Self {
        let (notify_tx, notify_rx) = channel::new();

        let inner = PoolInner {
            connections: Mutex::new(Vec::new()),
            available: Mutex::new(VecDeque::new()),
            notify_tx,
            notify_rx,
            config: ConnectionConfig::default(), // 需要实现 Default
            factory: Box::new(DefaultDriverFactory), // 需要具体类型
            max_size: 10,                        // 默认最大连接数
            current_size: AtomicUsize::new(0),
        };

        ConnectionPool {
            inner: Arc::new(inner),
        }
    }
}

impl ConnectionPool {
    pub fn new<F>(factory: F, config: ConnectionConfig, max_size: usize) -> Self
    where
        F: DriverFactory + 'static,
    {
        let (notify_tx, notify_rx) = channel::new();

        let inner = Arc::new(PoolInner {
            connections: Mutex::new(Vec::with_capacity(max_size)),
            available: Mutex::new(VecDeque::with_capacity(max_size)),
            notify_tx,
            notify_rx,
            config,
            factory: Box::new(factory),
            max_size,
            current_size: AtomicUsize::new(0),
        });

        ConnectionPool { inner }
    }

    pub fn get(&self) -> Result<PoolConnection, Error> {
        {
            let mut available = self.inner.available.lock();
            if let Some(idx) = available.pop_front() {
                let mut connections = self.inner.connections.lock();
                if let Some(Some(conn)) = connections.get(idx) {
                    if conn.is_connected() {
                        let conn = connections[idx].take().unwrap();
                        return Ok(PoolConnection {
                            pool: self.inner.clone(),
                            index: idx,
                            conn: Some(conn),
                        });
                    }
                }
            }
        }

        let current = self.inner.current_size.load(Ordering::Acquire);
        if current < self.inner.max_size {
            let mut conn = self.inner.factory.create();
            conn.connect(&self.inner.config)?;

            let mut connections = self.inner.connections.lock();
            let idx = connections.len();
            connections.push(None);
            self.inner.current_size.fetch_add(1, Ordering::Release);
            drop(connections);

            return Ok(PoolConnection {
                pool: self.inner.clone(),
                index: idx,
                conn: Some(conn),
            });
        }

        let _ = self.inner.notify_rx.recv();

        let mut available = self.inner.available.lock();
        if let Some(idx) = available.pop_front() {
            let mut connections = self.inner.connections.lock();
            if let Some(Some(_)) = connections.get(idx) {
                let conn = connections[idx].take().unwrap();
                return Ok(PoolConnection {
                    pool: self.inner.clone(),
                    index: idx,
                    conn: Some(conn),
                });
            }
        }

        Err("Pool closed".into())
    }

    pub fn close(&self) {
        let mut connections = self.inner.connections.lock();
        for conn in connections.iter_mut() {
            if let Some(mut c) = conn.take() {
                let _ = c.close();
            }
        }
    }
}

pub struct PoolConnection {
    pool: Arc<PoolInner>,
    index: usize,
    conn: Option<PooledConnection>,
}

impl PoolConnection {
    pub fn driver(&self) -> &dyn DatabaseDriver {
        if let Some(ref conn) = self.conn {
            return conn.as_ref();
        }
        panic!("Connection not available");
    }

    pub fn driver_mut(&mut self) -> &mut dyn DatabaseDriver {
        if let Some(ref mut conn) = self.conn {
            return conn.as_mut();
        }
        panic!("Connection not available");
    }
}

impl Drop for PoolConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            let mut connections = self.pool.connections.lock();
            connections[self.index] = Some(conn);
        }
        self.pool.available.lock().push_back(self.index);
        // 使用 try_send 避免在 Drop 中阻塞导致死锁
        let _ = self.pool.notify_tx.try_send(());
    }
}
