pub mod config;
pub mod health;
pub mod tools;
pub mod version;

use serde_json::Value as JsonValue;

use crate::envelope::MetaExtras;
use crate::error::{CliError, ExitCode};

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
