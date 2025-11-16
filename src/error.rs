use std::fmt;

pub enum Error {
    Io(String),
    Permission(String),
    Command(String),
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(msg) => write!(f, "I/O error: {}", msg),
            Error::Permission(msg) => write!(f, "Permission error: {}", msg),
            Error::Command(msg) => write!(f, "Command error: {}", msg),
        }
    }
}
