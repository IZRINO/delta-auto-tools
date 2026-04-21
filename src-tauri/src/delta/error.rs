use std::io;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeltaError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("storage failed: {0}")]
    Storage(String),
    #[error("parse failed: {0}")]
    Parse(String),
    #[error("account not found")]
    AccountNotFound,
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl Serialize for DeltaError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl From<reqwest::Error> for DeltaError {
    fn from(value: reqwest::Error) -> Self {
        Self::Request(value.to_string())
    }
}

impl From<rusqlite::Error> for DeltaError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<serde_json::Error> for DeltaError {
    fn from(value: serde_json::Error) -> Self {
        Self::Parse(value.to_string())
    }
}

impl From<url::ParseError> for DeltaError {
    fn from(value: url::ParseError) -> Self {
        Self::Parse(value.to_string())
    }
}

impl From<io::Error> for DeltaError {
    fn from(value: io::Error) -> Self {
        Self::Storage(value.to_string())
    }
}
