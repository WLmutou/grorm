use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Connection(String),
    Query(String),
    Execute(String),
    Protocol(String),
    Model(String),
    Pool(String),
    Config(String),
    Io(std::io::Error),
    NotFound(String),
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