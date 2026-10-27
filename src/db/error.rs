use std::fmt;

#[derive(Debug)]
pub enum DbError {
    Sqlite(rusqlite::Error),
    NotFound(String),
    Validation(String),
    ForeignKeyViolation(String),
    Security(String),
    Serialization(serde_json::Error),
    Csv(csv::Error),
    Io(std::io::Error),
    PasswordHash(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "Database error: {e}"),
            Self::NotFound(msg) => write!(f, "Entity not found: {msg}"),
            Self::Validation(msg) => write!(f, "Validation error: {msg}"),
            Self::ForeignKeyViolation(msg) => write!(f, "Foreign key constraint failed: {msg}"),
            Self::Security(msg) => write!(f, "Security / PIN error: {msg}"),
            Self::Serialization(e) => write!(f, "Serialization error: {e}"),
            Self::Csv(e) => write!(f, "CSV error: {e}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::PasswordHash(msg) => write!(f, "Password hash error: {msg}"),
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(e) => Some(e),
            Self::Serialization(e) => Some(e),
            Self::Csv(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<serde_json::Error> for DbError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e)
    }
}

impl From<csv::Error> for DbError {
    fn from(e: csv::Error) -> Self {
        Self::Csv(e)
    }
}

impl From<std::io::Error> for DbError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub type DbResult<T> = Result<T, DbError>;
