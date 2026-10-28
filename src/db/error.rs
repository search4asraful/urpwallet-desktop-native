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

impl DbError {
    /// Returns a clean, human-readable, user-friendly explanation of the error.
    /// Never exposes raw driver codes, SQL syntax, or internal technical stack traces.
    pub fn user_friendly_message(&self) -> String {
        match self {
            Self::Sqlite(err) => match err {
                rusqlite::Error::SqliteFailure(code, Some(msg)) => {
                    let msg_lower = msg.to_lowercase();
                    if msg_lower.contains("unique constraint failed: categories.name") {
                        "A category with this name already exists. Please choose a different name.".to_string()
                    } else if msg_lower.contains("unique constraint failed: accounts.name") {
                        "An account with this name already exists. Please choose a different name.".to_string()
                    } else if msg_lower.contains("unique constraint failed") {
                        "A record with this information already exists. Please check for duplicates.".to_string()
                    } else if msg_lower.contains("foreign key constraint failed") {
                        "Cannot complete this action because the record is linked to other financial entries.".to_string()
                    } else if msg_lower.contains("check constraint failed") {
                        "The entered value is outside the allowed range or format.".to_string()
                    } else {
                        match code.code {
                            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                                "The database is currently busy. Please wait a moment and try again.".to_string()
                            }
                            rusqlite::ErrorCode::CannotOpen => {
                                "Unable to access the database file. Please verify that the folder exists and you have write permissions.".to_string()
                            }
                            rusqlite::ErrorCode::ReadOnly => {
                                "The database file is in read-only mode. Please check file permissions.".to_string()
                            }
                            rusqlite::ErrorCode::DiskFull => {
                                "Not enough disk space available to save changes.".to_string()
                            }
                            _ => "A database operation could not be completed. Please try again.".to_string(),
                        }
                    }
                }
                rusqlite::Error::SqliteFailure(code, None) => match code.code {
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                        "The database is currently busy. Please wait a moment and try again.".to_string()
                    }
                    rusqlite::ErrorCode::CannotOpen => {
                        "Unable to open the database file. Please check folder permissions.".to_string()
                    }
                    rusqlite::ErrorCode::ReadOnly => {
                        "The database file is read-only. Please check file permissions.".to_string()
                    }
                    rusqlite::ErrorCode::DiskFull => {
                        "Not enough disk space available to save changes.".to_string()
                    }
                    _ => "A database operation could not be completed. Please try again.".to_string(),
                },
                _ => "A database error occurred. Please try again.".to_string(),
            },
            Self::NotFound(msg) => {
                let msg_lower = msg.to_lowercase();
                if msg_lower.contains("account") {
                    "The selected account could not be found.".to_string()
                } else if msg_lower.contains("category") {
                    "The selected category could not be found.".to_string()
                } else if msg_lower.contains("transaction") {
                    "The selected transaction could not be found.".to_string()
                } else if msg_lower.contains("budget") {
                    "The selected budget could not be found.".to_string()
                } else if msg_lower.contains("bill") {
                    "The selected recurring bill could not be found.".to_string()
                } else if msg_lower.contains("debt") {
                    "The selected debt record could not be found.".to_string()
                } else {
                    "The requested record was not found.".to_string()
                }
            }
            Self::Validation(msg) => msg.clone(),
            Self::ForeignKeyViolation(msg) => {
                if msg.contains("transactions") {
                    "Cannot delete this category because it is currently used by existing transactions. Please delete or reassign those transactions first.".to_string()
                } else {
                    "Cannot delete this record because it is linked to other financial entries.".to_string()
                }
            }
            Self::Security(msg) => {
                let msg_lower = msg.to_lowercase();
                if msg_lower.contains("pin") || msg_lower.contains("password") {
                    "Incorrect PIN or passphrase entered. Please try again.".to_string()
                } else {
                    "Security verification failed. Please try again.".to_string()
                }
            }
            Self::Serialization(_) => {
                "The backup data appears corrupted or has an unsupported format.".to_string()
            }
            Self::Csv(_) => {
                "Unable to process the CSV file. Please verify the file structure.".to_string()
            }
            Self::Io(err) => match err.kind() {
                std::io::ErrorKind::NotFound => "The specified file or folder could not be found.".to_string(),
                std::io::ErrorKind::PermissionDenied => {
                    "Permission denied. The application does not have access to this folder or file.".to_string()
                }
                std::io::ErrorKind::AlreadyExists => "A file with that name already exists.".to_string(),
                _ => "Unable to access the specified file or folder. Please check permissions.".to_string(),
            },
            Self::PasswordHash(_) => {
                "Security processing error. Please ensure your PIN is entered correctly.".to_string()
            }
        }
    }
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
