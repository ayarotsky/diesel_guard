use thiserror::Error;

#[derive(Error, Debug)]
pub enum DieselGuardError {
    #[error("Failed to parse SQL: {0}")]
    ParseError(String),

    #[error("Failed to read file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Unsafe migration detected: {0}")]
    UnsafeMigration(String),
}

pub type Result<T> = std::result::Result<T, DieselGuardError>;
