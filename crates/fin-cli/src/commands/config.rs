use std::fs;

use serde::Serialize;
use serde_json::json;

use fin_sdk::config::{parse_fin_config, resolve_config_path};
use fin_sdk::error::FinError;
use fin_sdk::{ConfigShowData, FinSdkError, ReserveMode, build_config_show};

use crate::commands::{CommandFailure, CommandResult};
use crate::envelope::MetaExtras;
use crate::error::{CliError, ErrorCode, ExitCode};

#[derive(Debug, Clone, Serialize)]
struct ValidationError {
    path: String,
    message: String,
}

fn map_fin_error(error: FinError) -> CliError {
    match error {
        FinError::ConfigNotFound { path } => CliError::new(
            ErrorCode::NoConfig,
            format!("Config file not found: {}", path.display()),
            "Copy fin.config.template.toml to data/fin.config.toml (or set FIN_CONFIG_PATH)",
        ),
        FinError::ConfigInvalid { path, message } => CliError::new(
            ErrorCode::InvalidConfig,
            format!("Invalid config at {}: {message}", path.display()),
            "Run `fin config validate` for JSON output or `fin config validate --text` for a readable summary",
        ),
        other => CliError::new(
            ErrorCode::Runtime,
            format!("Failed to read config: {other}"),
            "Verify config path and file permissions, then retry",
        ),
    }
}

fn map_sdk_error(error: FinSdkError) -> CliError {
    match error {
        FinSdkError::ConfigNotFound { path } => CliError::new(
            ErrorCode::NoConfig,
            format!("Config file not found: {path}"),
            "Copy fin.config.template.toml to data/fin.config.toml (or set FIN_CONFIG_PATH)",
        ),
        FinSdkError::ConfigRead { path, message } | FinSdkError::ConfigParse { path, message } => {
            CliError::new(
                ErrorCode::InvalidConfig,
                format!("Invalid config at {path}: {message}"),
                "Run `fin config validate` for JSON output or `fin config validate --text` for a readable summary",
            )
        }
        other => CliError::new(
            ErrorCode::Runtime,
            format!("Failed to read config: {other}"),
            "Verify config path and file permissions, then retry",
        ),
    }
}

fn reserve_mode_label(mode: ReserveMode) -> &'static str {
    match mode {
        ReserveMode::Conservative => "conservative",
        ReserveMode::Recurring => "recurring",
        ReserveMode::Aggressive => "aggressive",
    }
}

fn render_config_show_text(data: &ConfigShowData) -> String {
    let mut lines = vec![
        format!("Config: {}", data.config_path),
        String::new(),
        "Groups:".to_string(),
    ];

    for group in &data.groups {
        lines.push(format!(
            "  {} ({}) -- tax: {}, conservative_reserve: {}mo, default_mode: {}",
            group.id,
            group.label,
            group.tax_type,
            group.expense_reserve_months,
            reserve_mode_label(group.default_reserve_mode),
        ));
        if let Some(group_accounts) = data.accounts.get(group.id.as_str()) {
            for account in group_accounts {
                let label = account
                    .label
                    .as_ref()
                    .map(|value| format!(" \"{value}\""))
                    .unwrap_or_default();
                lines.push(format!(
                    "    {} [{}]{}",
                    account.id, account.provider, label
                ));
            }
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "Reserves: default_mode={}",
        reserve_mode_label(data.reserves.default_mode)
    ));
    for (mode, settings) in &data.reserves.modes {
        lines.push(format!(
            "  {mode}: basis={}, months={}, factor={}, lookback_months={}",
            serde_json::to_string(&settings.expense_basis)
                .unwrap_or_else(|_| "\"unknown\"".to_owned())
                .trim_matches('"'),
            settings.expense_months,
            settings.factor,
            settings
                .lookback_months
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_owned()),
        ));
    }
    for (group_id, group) in &data.reserves.groups {
        lines.push(format!(
            "  {group_id}: default_mode={}",
            reserve_mode_label(group.default_mode)
        ));
        for (mode, settings) in &group.modes {
            lines.push(format!(
                "    {mode}: basis={}, months={}, factor={}, lookback_months={}",
                serde_json::to_string(&settings.expense_basis)
                    .unwrap_or_else(|_| "\"unknown\"".to_owned())
                    .trim_matches('"'),
                settings.expense_months,
                settings.factor,
                settings
                    .lookback_months
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
            ));
        }
    }

    lines.push(String::new());
    lines.push("Financial:".to_string());
    if let Some(financial_table) = data.financial.as_object() {
        for (key, value) in financial_table {
            lines.push(format!("  {key}: {value}"));
        }
    } else {
        lines.push("  <missing>".to_string());
    }

    lines.join("\n")
}

pub fn run_show() -> Result<CommandResult, CommandFailure> {
    let data = build_config_show(None).map_err(|error| CommandFailure {
        tool: "config.show",
        error: map_sdk_error(error),
    })?;
    let text = render_config_show_text(&data);

    Ok(CommandResult {
        tool: "config.show",
        data: json!(data),
        text,
        meta: MetaExtras::default(),
        exit_code: ExitCode::Success,
    })
}

pub fn run_validate() -> Result<CommandResult, CommandFailure> {
    let config_path = resolve_config_path(None);
    if !config_path.exists() {
        return Err(CommandFailure {
            tool: "config.validate",
            error: CliError::new(
                ErrorCode::NoConfig,
                format!("Config file not found: {}", config_path.display()),
                "Copy fin.config.template.toml to data/fin.config.toml (or set FIN_CONFIG_PATH)",
            ),
        });
    }

    let raw = fs::read_to_string(&config_path).map_err(|error| CommandFailure {
        tool: "config.validate",
        error: CliError::new(
            ErrorCode::InvalidConfig,
            format!("Failed to read {}: {error}", config_path.display()),
            "Verify file permissions and TOML file encoding",
        ),
    })?;

    let config_path_str = config_path.display().to_string();
    let parse_result = parse_fin_config(&raw);

    let (valid, errors, exit_code) = match parse_result {
        Ok(_) => (true, Vec::<ValidationError>::new(), ExitCode::Success),
        Err(FinError::InvalidInput { code, message }) => {
            let path = match code {
                "CONFIG_EMPTY_ACCOUNTS" => "accounts",
                "CONFIG_EMPTY_BANKS" => "banks",
                _ => "$",
            };
            (
                false,
                vec![ValidationError {
                    path: path.to_string(),
                    message,
                }],
                ExitCode::Runtime,
            )
        }
        Err(other) => {
            return Err(CommandFailure {
                tool: "config.validate",
                error: map_fin_error(FinError::ConfigInvalid {
                    path: config_path,
                    message: other.to_string(),
                }),
            });
        }
    };

    let text = if valid {
        format!("Config valid: {config_path_str}")
    } else {
        let mut lines = vec![format!("Config invalid: {config_path_str}")];
        for validation_error in &errors {
            lines.push(format!(
                "  {}: {}",
                validation_error.path, validation_error.message
            ));
        }
        lines.join("\n")
    };

    Ok(CommandResult {
        tool: "config.validate",
        data: json!({
            "valid": valid,
            "errors": errors,
            "configPath": config_path_str,
        }),
        text,
        meta: MetaExtras::default(),
        exit_code,
    })
}
