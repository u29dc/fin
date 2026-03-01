use std::path::{Path, PathBuf};

use serde_json::json;

use fin_sdk::config::{load_config, resolve_fin_paths};
use fin_sdk::error::FinError;
use fin_sdk::rules::{load_rules, migrate_ts_rules_file};

use crate::commands::{CommandFailure, CommandResult};
use crate::envelope::MetaExtras;
use crate::error::{CliError, ErrorCode, ExitCode};

fn map_fin_error(tool: &'static str, error: FinError) -> CommandFailure {
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
            "Create fin.rules.toml or run `fin rules migrate-ts`",
        ),
        FinError::RulesInvalid { path, message } => CliError::new(
            ErrorCode::InvalidConfig,
            format!("Invalid rules file at {}: {message}", path.display()),
            "Fix the rules file syntax and required fields",
        ),
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

fn as_opt_path(path: Option<&str>) -> Option<PathBuf> {
    path.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    })
}

fn render_rules_summary(path: &Path, external_loaded: bool, rule_count: usize) -> String {
    let origin = if external_loaded {
        "external"
    } else {
        "default"
    };
    format!(
        "Rules: {}\nOrigin: {origin}\nRule count: {rule_count}",
        path.display()
    )
}

pub fn run_show(path: Option<&str>) -> Result<CommandResult, CommandFailure> {
    let explicit_path = as_opt_path(path);
    let loaded_config = load_config(None).ok();
    let loaded = load_rules(explicit_path.as_deref(), loaded_config.as_ref(), None)
        .map_err(|error| map_fin_error("rules.show", error))?;

    let rule_count = loaded.config.rules.len();
    let text = render_rules_summary(&loaded.resolved_path, loaded.external_loaded, rule_count);

    Ok(CommandResult {
        tool: "rules.show",
        data: json!({
            "rulesPath": loaded.resolved_path,
            "externalLoaded": loaded.external_loaded,
            "ruleCount": rule_count,
            "warnOnUnmapped": loaded.config.warn_on_unmapped,
            "fallbackToRaw": loaded.config.fallback_to_raw,
        }),
        text,
        meta: MetaExtras::default(),
        exit_code: ExitCode::Success,
    })
}

pub fn run_validate(path: Option<&str>) -> Result<CommandResult, CommandFailure> {
    let explicit_path = as_opt_path(path);
    let loaded_config = load_config(None).ok();
    let resolved_path =
        fin_sdk::rules::resolve_rules_path(explicit_path.as_deref(), loaded_config.as_ref());

    match load_rules(explicit_path.as_deref(), loaded_config.as_ref(), None) {
        Ok(loaded) => {
            let rule_count = loaded.config.rules.len();
            Ok(CommandResult {
                tool: "rules.validate",
                data: json!({
                    "valid": true,
                    "errors": [],
                    "rulesPath": resolved_path,
                    "externalLoaded": loaded.external_loaded,
                    "ruleCount": rule_count,
                }),
                text: format!("Rules valid: {}", loaded.resolved_path.display()),
                meta: MetaExtras::default(),
                exit_code: ExitCode::Success,
            })
        }
        Err(error) => {
            let message = error.to_string();
            Ok(CommandResult {
                tool: "rules.validate",
                data: json!({
                    "valid": false,
                    "errors": [{ "path": "$", "message": message }],
                    "rulesPath": resolved_path,
                    "externalLoaded": false,
                    "ruleCount": 0,
                }),
                text: format!(
                    "Rules invalid: {}\n  $: {}",
                    resolved_path.display(),
                    message
                ),
                meta: MetaExtras::default(),
                exit_code: ExitCode::Runtime,
            })
        }
    }
}

pub fn run_migrate_ts(
    source_path: Option<&str>,
    target_path: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let paths = resolve_fin_paths();
    let source = as_opt_path(source_path).unwrap_or(paths.legacy_rules_file);
    let target = as_opt_path(target_path).unwrap_or(paths.rules_file);

    let summary = migrate_ts_rules_file(&source, &target)
        .map_err(|error| map_fin_error("rules.migrate_ts", error))?;

    let text = format!(
        "Rules migrated:\n  source: {}\n  target: {}\n  rules: {}",
        summary.source_path.display(),
        summary.target_path.display(),
        summary.rule_count
    );

    Ok(CommandResult {
        tool: "rules.migrate_ts",
        data: json!({
            "sourcePath": summary.source_path,
            "targetPath": summary.target_path,
            "ruleCount": summary.rule_count,
            "warnOnUnmapped": summary.warn_on_unmapped,
            "fallbackToRaw": summary.fallback_to_raw,
        }),
        text,
        meta: MetaExtras::default(),
        exit_code: ExitCode::Success,
    })
}
