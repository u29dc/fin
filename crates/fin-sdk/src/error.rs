use std::path::PathBuf;

use thiserror::Error;

pub type Result<T, E = FinError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum FinError {
    #[error("config file not found: {path}")]
    ConfigNotFound { path: PathBuf },
    #[error("invalid config at {path}: {message}")]
    ConfigInvalid { path: PathBuf, message: String },
    #[error("rules file not found: {path}")]
    RulesNotFound { path: PathBuf },
    #[error("invalid rules at {path}: {message}")]
    RulesInvalid { path: PathBuf, message: String },
    #[error("i/o error: {message}")]
    Io { message: String },
    #[error("parse error ({context}): {message}")]
    Parse {
        context: &'static str,
        message: String,
    },
    #[error("database error: {message}")]
    Database { message: String },
    #[error("migration error: {message}")]
    Migration { message: String },
    #[error("invalid input ({code}): {message}")]
    InvalidInput { code: &'static str, message: String },
}

impl FinError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::ConfigNotFound { .. } => "NO_CONFIG",
            Self::ConfigInvalid { .. } => "INVALID_CONFIG",
            Self::RulesNotFound { .. } => "NO_RULES",
            Self::RulesInvalid { .. } => "INVALID_RULES",
            Self::Io { .. } => "IO_ERROR",
            Self::Parse { .. } => "PARSE_ERROR",
            Self::Database { .. } => "DB_ERROR",
            Self::Migration { .. } => "MIGRATION_ERROR",
            Self::InvalidInput { code, .. } => code,
        }
    }

    #[must_use]
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::ConfigNotFound { .. } => {
                Some("Copy fin.config.template.toml into your FIN_HOME data directory.")
            }
            Self::ConfigInvalid { .. } => Some("Validate TOML syntax and required sections."),
            Self::RulesNotFound { .. } => {
                Some("Create data/fin.rules.json or run rules migration from TypeScript.")
            }
            Self::RulesInvalid { .. } => Some("Fix fin.rules.json schema fields and retry."),
            Self::Database { .. } => Some("Check DB path, file permissions, and schema version."),
            Self::Migration { .. } => Some("Open DB read/write and retry migration."),
            Self::Io { .. } | Self::Parse { .. } | Self::InvalidInput { .. } => None,
        }
    }
}

impl From<std::io::Error> for FinError {
    fn from(value: std::io::Error) -> Self {
        Self::Io {
            message: value.to_string(),
        }
    }
}

impl From<rusqlite::Error> for FinError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database {
            message: value.to_string(),
        }
    }
}
