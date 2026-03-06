use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use serde::Serialize;
use serde_json::json;

use fin_sdk::config::{GroupMetadata, load_config, parse_fin_config, resolve_config_path};
use fin_sdk::error::FinError;

use crate::commands::{CommandFailure, CommandResult};
use crate::envelope::MetaExtras;
use crate::error::{CliError, ErrorCode, ExitCode};

#[derive(Debug, Clone, Serialize)]
struct AccountSummary {
    id: String,
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subtype: Option<String>,
}

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
            "Run `fin config validate --json` for structured validation output",
        ),
        other => CliError::new(
            ErrorCode::Runtime,
            format!("Failed to read config: {other}"),
            "Verify config path and file permissions, then retry",
        ),
    }
}

fn build_account_map(config: &fin_sdk::config::FinConfig) -> BTreeMap<String, Vec<AccountSummary>> {
    let mut grouped: BTreeMap<String, Vec<AccountSummary>> = BTreeMap::new();
    for account in &config.accounts {
        grouped
            .entry(account.group.clone())
            .or_default()
            .push(AccountSummary {
                id: account.id.clone(),
                provider: account.provider.clone(),
                label: account.label.clone(),
                subtype: account.subtype.clone(),
            });
    }
    grouped
}

fn derive_groups(
    config: &fin_sdk::config::FinConfig,
    configured_groups: Option<Vec<GroupMetadata>>,
    _account_map: &BTreeMap<String, Vec<AccountSummary>>,
) -> Vec<GroupMetadata> {
    if let Some(groups) = configured_groups {
        let existing = groups
            .iter()
            .map(|group| group.id.clone())
            .collect::<BTreeSet<_>>();
        let mut ordered = groups;
        for group_id in config.group_ids() {
            if existing.contains(&group_id) {
                continue;
            }
            ordered.push(GroupMetadata {
                label: group_id.clone(),
                id: group_id,
                icon: None,
                tax_type: None,
                expense_reserve_months: None,
            });
        }
        return ordered;
    }

    config
        .group_ids()
        .into_iter()
        .map(|id| GroupMetadata {
            label: id.clone(),
            id,
            icon: None,
            tax_type: None,
            expense_reserve_months: None,
        })
        .collect()
}

fn render_config_show_text(
    config_path: &str,
    groups: &[GroupMetadata],
    accounts: &BTreeMap<String, Vec<AccountSummary>>,
    financial: &serde_json::Value,
) -> String {
    let mut lines = vec![
        format!("Config: {config_path}"),
        String::new(),
        "Groups:".to_string(),
    ];

    for group in groups {
        let tax_type = group.tax_type.as_deref().unwrap_or("none");
        let reserve = group.expense_reserve_months.unwrap_or(0);
        lines.push(format!(
            "  {} ({}) -- tax: {}, reserve: {}mo",
            group.id, group.label, tax_type, reserve
        ));
        if let Some(group_accounts) = accounts.get(group.id.as_str()) {
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
    lines.push("Financial:".to_string());
    if let Some(financial_table) = financial.as_object() {
        for (key, value) in financial_table {
            lines.push(format!("  {key}: {value}"));
        }
    } else {
        lines.push("  <missing>".to_string());
    }

    lines.join("\n")
}

pub fn run_show() -> Result<CommandResult, CommandFailure> {
    let loaded = load_config(None).map_err(|error| CommandFailure {
        tool: "config.show",
        error: map_fin_error(error),
    })?;

    let account_map = build_account_map(&loaded.config);
    let groups = derive_groups(&loaded.config, loaded.config.groups.clone(), &account_map);
    let financial =
        serde_json::to_value(&loaded.config.financial).unwrap_or(serde_json::Value::Null);
    let config_path = loaded.path.display().to_string();
    let text = render_config_show_text(&config_path, &groups, &account_map, &financial);

    Ok(CommandResult {
        tool: "config.show",
        data: json!({
            "groups": groups,
            "accounts": account_map,
            "financial": financial,
            "configPath": config_path,
        }),
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
