use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum AppError {
    Validation(String),
    Storage(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(message) => write!(f, "Validation error: {message}"),
            Self::Storage(message) => write!(f, "Storage error: {message}"),
        }
    }
}

impl Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Storage(err.to_string())
    }
}