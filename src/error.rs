use std::fmt;

/// Convenience type alias for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// Unified error type for all grorm operations.
///
/// Covers connection, query, protocol, model, pool, and transaction errors.
///
/// # Example
///
/// ```rust
/// use grorm::Error;
///
/// fn example() -> Result<(), Error> {
///     Err(Error::NotFound("user not found".into()))
/// }
/// ```
#[derive(Debug)]
pub enum Error {
    /// Database connection errors (auth, network, etc.)
    Connection(String),
    /// SQL query errors (syntax, constraint violations, etc.)
    Query(String),
    /// SQL execution errors (write operations)
    Execute(String),
    /// Low-level protocol errors (wire format, parsing)
    Protocol(String),
    /// Model serialization/deserialization errors
    Model(String),
    /// Connection pool errors (exhausted, closed)
    Pool(String),
    /// Configuration errors (invalid DSN, etc.)
    Config(String),
    /// Wrapped I/O errors
    Io(std::io::Error),
    /// Entity not found (for operations expecting a result)
    NotFound(String),
    /// Transaction errors (begin, commit, rollback)
    Transaction(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Connection(msg) => write!(f, "connection error: {}", msg),
            Error::Query(msg) => write!(f, "query error: {}", msg),
            Error::Execute(msg) => write!(f, "execute error: {}", msg),
            Error::Protocol(msg) => write!(f, "protocol error: {}", msg),
            Error::Model(msg) => write!(f, "model error: {}", msg),
            Error::Pool(msg) => write!(f, "pool error: {}", msg),
            Error::Config(msg) => write!(f, "config error: {}", msg),
            Error::Io(err) => write!(f, "io error: {}", err),
            Error::NotFound(msg) => write!(f, "not found: {}", msg),
            Error::Transaction(msg) => write!(f, "transaction error: {}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error::Protocol(msg)
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Error::Protocol(msg.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for Error {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        Error::Protocol(err.to_string())
    }
}