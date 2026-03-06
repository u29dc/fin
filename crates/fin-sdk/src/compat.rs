use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value as JsonValue;
use thiserror::Error;

use crate::config::{
    load_config, parse_fin_config, resolve_config_path as resolve_config_path_impl,
};
use crate::error::FinError;
use crate::health::{HealthCheckOptions, HealthReport, run_health_checks};

#[derive(Debug, Error)]
pub enum FinSdkError {
    #[error("config file not found: {path}")]
    ConfigNotFound { path: String },
    #[error("failed to read config file: {path}: {message}")]
    ConfigRead { path: String, message: String },
    #[error("failed to parse config file: {path}: {message}")]
    ConfigParse { path: String, message: String },
    #[error("database error: {message}")]
    Database { message: String },
    #[error("runtime error: {message}")]
    Runtime { message: String },
}

impl From<FinError> for FinSdkError {
    fn from(value: FinError) -> Self {
        match value {
            FinError::ConfigNotFound { path } => Self::ConfigNotFound {
                path: path.display().to_string(),
            },
            FinError::ConfigInvalid { path, message } => Self::ConfigParse {
                path: path.display().to_string(),
                message,
            },
            FinError::Database { message } => Self::Database { message },
            other => Self::Runtime {
                message: other.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMetadata {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub tax_type: String,
    pub expense_reserve_months: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub id: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigShowData {
    pub groups: Vec<GroupMetadata>,
    pub accounts: BTreeMap<String, Vec<AccountSummary>>,
    pub financial: JsonValue,
    pub config_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub config_path: String,
}

pub fn resolve_config_path(explicit_path: Option<&Path>) -> Result<PathBuf, FinSdkError> {
    Ok(resolve_config_path_impl(explicit_path))
}

fn title_case(identifier: &str) -> String {
    identifier
        .split(['-', '_', ' '])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn validate_config(
    explicit_path: Option<&Path>,
) -> Result<ConfigValidationResult, FinSdkError> {
    let config_path = resolve_config_path(explicit_path)?;
    if !config_path.exists() {
        return Err(FinSdkError::ConfigNotFound {
            path: config_path.display().to_string(),
        });
    }

    let raw = fs::read_to_string(&config_path).map_err(|error| FinSdkError::ConfigRead {
        path: config_path.display().to_string(),
        message: error.to_string(),
    })?;

    match parse_fin_config(&raw) {
        Ok(_) => Ok(ConfigValidationResult {
            valid: true,
            errors: vec![],
            config_path: config_path.display().to_string(),
        }),
        Err(error) => Ok(ConfigValidationResult {
            valid: false,
            errors: vec![ValidationError {
                path: "$".to_owned(),
                message: error.to_string(),
            }],
            config_path: config_path.display().to_string(),
        }),
    }
}

pub fn build_config_show(explicit_path: Option<&Path>) -> Result<ConfigShowData, FinSdkError> {
    let loaded = load_config(explicit_path).map_err(FinSdkError::from)?;

    let mut accounts = BTreeMap::<String, Vec<AccountSummary>>::new();
    for account in &loaded.config.accounts {
        accounts
            .entry(account.group.clone())
            .or_default()
            .push(AccountSummary {
                id: account.id.clone(),
                provider: account.provider.clone(),
                label: account.label.clone(),
                subtype: account.subtype.clone(),
            });
    }

    let mut groups = loaded
        .config
        .groups
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|group| GroupMetadata {
            id: group.id,
            label: group.label,
            icon: group.icon,
            tax_type: group.tax_type.unwrap_or_else(|| "none".to_owned()),
            expense_reserve_months: i64::from(group.expense_reserve_months.unwrap_or(3)),
        })
        .collect::<Vec<_>>();

    let existing = groups
        .iter()
        .map(|group| group.id.clone())
        .collect::<BTreeSet<_>>();
    for group_id in loaded.config.group_ids() {
        if existing.contains(&group_id) {
            continue;
        }
        groups.push(GroupMetadata {
            id: group_id.clone(),
            label: title_case(&group_id),
            icon: None,
            tax_type: "none".to_owned(),
            expense_reserve_months: 3,
        });
    }

    let financial = serde_json::to_value(&loaded.config.financial).unwrap_or(JsonValue::Null);
    Ok(ConfigShowData {
        groups,
        accounts,
        financial,
        config_path: loaded.path.display().to_string(),
    })
}

pub fn run_health(
    config_path: Option<&str>,
    db_path: Option<&str>,
) -> Result<HealthReport, FinSdkError> {
    Ok(run_health_checks(HealthCheckOptions {
        config_path: config_path.map(PathBuf::from),
        db_path: db_path.map(PathBuf::from),
        rules_path: None,
        paths_override: None,
    }))
}
