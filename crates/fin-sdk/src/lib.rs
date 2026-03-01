pub mod compat;
pub mod config;
pub mod contracts;
pub mod db;
pub mod error;
pub mod health;
pub mod rules;
pub mod units;

pub use compat::{
    AccountSummary, ConfigShowData, ConfigValidationResult, FinSdkError, GroupMetadata,
    ValidationError, build_config_show, resolve_config_path, run_health, validate_config,
};
pub use db::schema::{REQUIRED_TABLES, SCHEMA_VERSION};
pub use error::{FinError, Result};
pub use health::{
    CheckStatus, HealthCheck, HealthCheckOptions, HealthReport, HealthStatus, HealthSummary,
    Severity, run_health_checks,
};

pub const SDK_NAME: &str = "fin-sdk";
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn sdk_banner() -> String {
    format!("{SDK_NAME} v{SDK_VERSION}")
}
