pub mod config;
pub mod edit;
pub mod health;
pub mod import;
pub mod report;
pub mod rules;
pub mod sanitize;
pub mod start;
pub mod tools;
pub mod version;
pub mod view;

use std::path::PathBuf;

use serde_json::Value as JsonValue;

use fin_sdk::error::FinError;
use fin_sdk::runtime::{RuntimeContext, RuntimeContextOptions};

use crate::envelope::MetaExtras;
use crate::error::{CliError, ErrorCode, ExitCode};

#[derive(Debug, Clone, Default)]
pub struct GlobalOptions {
    pub db: Option<String>,
    #[allow(dead_code)]
    pub format: Option<String>,
}

#[derive(Debug)]
pub struct CommandResult {
    pub tool: &'static str,
    pub data: JsonValue,
    pub text: String,
    pub meta: MetaExtras,
    pub exit_code: ExitCode,
}

#[derive(Debug)]
pub struct CommandFailure {
    pub tool: &'static str,
    pub error: CliError,
}

pub fn map_fin_error(tool: &'static str, error: FinError) -> CommandFailure {
    let cli_error = match error {
        FinError::ConfigNotFound { path } => CliError::new(
            ErrorCode::NoConfig,
            format!("Config file not found: {}", path.display()),
            "Copy fin.config.template.toml into your FIN_HOME data directory",
        ),
        FinError::ConfigInvalid { path, message } => CliError::new(
            ErrorCode::InvalidConfig,
            format!("Invalid config at {}: {message}", path.display()),
            "Validate fin.config.toml and retry",
        ),
        FinError::RulesNotFound { path } => CliError::new(
            ErrorCode::InvalidConfig,
            format!("Rules file not found: {}", path.display()),
            "Create fin.rules.json or run `fin rules migrate-ts`",
        ),
        FinError::RulesInvalid { path, message } => CliError::new(
            ErrorCode::InvalidConfig,
            format!("Invalid rules file at {}: {message}", path.display()),
            "Fix the rules file syntax and required fields",
        ),
        FinError::Database { message } => CliError::new(
            ErrorCode::Db,
            format!("Database error: {message}"),
            "Run `fin health` and verify FIN_HOME/data is writable",
        ),
        FinError::InvalidInput { code, message } => {
            let error_code = if code == "NOT_FOUND" {
                ErrorCode::NotFound
            } else {
                ErrorCode::Runtime
            };
            CliError::new(error_code, message, "Review command arguments and retry")
        }
        other => CliError::new(
            ErrorCode::Runtime,
            format!("{tool} failed: {other}"),
            "Review error details and retry",
        ),
    };

    CommandFailure {
        tool,
        error: cli_error,
    }
}

pub fn open_runtime(
    tool: &'static str,
    explicit_db: Option<&str>,
    readonly: bool,
) -> Result<RuntimeContext, CommandFailure> {
    let mut options = if readonly {
        RuntimeContextOptions::read_only()
    } else {
        RuntimeContextOptions::writable()
    };
    options.db_path = explicit_db.map(PathBuf::from);
    RuntimeContext::open(options).map_err(|error| map_fin_error(tool, error))
}
