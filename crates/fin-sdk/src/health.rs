use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::config::loader::{LoadedConfig, load_config, resolve_config_path};
use crate::config::paths::{FinPaths, resolve_fin_paths};
use crate::db::connection::resolve_db_path;
use crate::db::migrate::{get_user_version, missing_required_tables};
use crate::db::schema::SCHEMA_VERSION;
use crate::rules::loader::resolve_rules_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Blocking,
    Degraded,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheck {
    pub id: String,
    pub label: String,
    pub status: CheckStatus,
    pub severity: Severity,
    pub detail: Option<String>,
    pub fix: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ready,
    Degraded,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSummary {
    pub ok: usize,
    pub blocking: usize,
    pub degraded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub checks: Vec<HealthCheck>,
    pub summary: HealthSummary,
}

#[derive(Debug, Clone, Default)]
pub struct HealthCheckOptions {
    pub config_path: Option<PathBuf>,
    pub db_path: Option<PathBuf>,
    pub rules_path: Option<PathBuf>,
    pub paths_override: Option<FinPaths>,
}

fn check_config_exists(config_path: &Path) -> HealthCheck {
    if !config_path.exists() {
        return HealthCheck {
            id: "config".to_owned(),
            label: "Configuration".to_owned(),
            status: CheckStatus::Missing,
            severity: Severity::Blocking,
            detail: Some(config_path.display().to_string()),
            fix: Some(vec![format!(
                "cp fin.config.template.toml {}",
                config_path.display()
            )]),
        };
    }
    HealthCheck {
        id: "config".to_owned(),
        label: "Configuration".to_owned(),
        status: CheckStatus::Ok,
        severity: Severity::Info,
        detail: Some(config_path.display().to_string()),
        fix: None,
    }
}

fn check_config_validates(config_path: &Path) -> (HealthCheck, Option<LoadedConfig>) {
    if !config_path.exists() {
        let check = HealthCheck {
            id: "config_valid".to_owned(),
            label: "Configuration validates".to_owned(),
            status: CheckStatus::Missing,
            severity: Severity::Blocking,
            detail: Some("Config file missing, cannot validate".to_owned()),
            fix: Some(vec![format!(
                "cp fin.config.template.toml {}",
                config_path.display()
            )]),
        };
        return (check, None);
    }

    match load_config(Some(config_path)) {
        Ok(loaded) => (
            HealthCheck {
                id: "config_valid".to_owned(),
                label: "Configuration validates".to_owned(),
                status: CheckStatus::Ok,
                severity: Severity::Info,
                detail: Some(config_path.display().to_string()),
                fix: None,
            },
            Some(loaded),
        ),
        Err(error) => (
            HealthCheck {
                id: "config_valid".to_owned(),
                label: "Configuration validates".to_owned(),
                status: CheckStatus::Invalid,
                severity: Severity::Blocking,
                detail: Some(format!("{} -- {}", config_path.display(), error)),
                fix: Some(vec![format!(
                    "cp fin.config.template.toml {}",
                    config_path.display()
                )]),
            },
            None,
        ),
    }
}

fn check_database_exists(db_path: &Path) -> HealthCheck {
    if !db_path.exists() {
        return HealthCheck {
            id: "database".to_owned(),
            label: "Database".to_owned(),
            status: CheckStatus::Missing,
            severity: Severity::Info,
            detail: Some(format!("{} (created on first import)", db_path.display())),
            fix: Some(vec!["fin import".to_owned()]),
        };
    }
    HealthCheck {
        id: "database".to_owned(),
        label: "Database".to_owned(),
        status: CheckStatus::Ok,
        severity: Severity::Info,
        detail: Some(db_path.display().to_string()),
        fix: None,
    }
}

fn check_db_schema(db_path: &Path) -> Option<HealthCheck> {
    if !db_path.exists() {
        return None;
    }

    let connection = match Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(connection) => connection,
        Err(error) => {
            return Some(HealthCheck {
                id: "db_schema".to_owned(),
                label: "Database schema".to_owned(),
                status: CheckStatus::Invalid,
                severity: Severity::Blocking,
                detail: Some(format!("failed to open {}: {error}", db_path.display())),
                fix: Some(vec!["fin import".to_owned()]),
            });
        }
    };

    let version = match get_user_version(&connection) {
        Ok(version) => version,
        Err(error) => {
            return Some(HealthCheck {
                id: "db_schema".to_owned(),
                label: "Database schema".to_owned(),
                status: CheckStatus::Invalid,
                severity: Severity::Blocking,
                detail: Some(format!("failed to read schema version: {error}")),
                fix: Some(vec!["fin import".to_owned()]),
            });
        }
    };
    let missing = match missing_required_tables(&connection) {
        Ok(missing) => missing,
        Err(error) => {
            return Some(HealthCheck {
                id: "db_schema".to_owned(),
                label: "Database schema".to_owned(),
                status: CheckStatus::Invalid,
                severity: Severity::Blocking,
                detail: Some(format!("failed to inspect required tables: {error}")),
                fix: Some(vec!["fin import".to_owned()]),
            });
        }
    };

    if version != SCHEMA_VERSION || !missing.is_empty() {
        let mut details = vec![];
        if version != SCHEMA_VERSION {
            details.push(format!("version {version}, expected {SCHEMA_VERSION}"));
        }
        if !missing.is_empty() {
            details.push(format!("missing tables: {}", missing.join(", ")));
        }
        return Some(HealthCheck {
            id: "db_schema".to_owned(),
            label: "Database schema".to_owned(),
            status: CheckStatus::Invalid,
            severity: Severity::Blocking,
            detail: Some(details.join("; ")),
            fix: Some(vec!["fin import".to_owned()]),
        });
    }

    Some(HealthCheck {
        id: "db_schema".to_owned(),
        label: "Database schema".to_owned(),
        status: CheckStatus::Ok,
        severity: Severity::Info,
        detail: Some(format!(
            "version {version}, {} tables",
            crate::db::schema::REQUIRED_TABLES.len()
        )),
        fix: None,
    })
}

fn check_rules_exists(path: &Path) -> HealthCheck {
    if !path.exists() {
        return HealthCheck {
            id: "rules".to_owned(),
            label: "Rules file".to_owned(),
            status: CheckStatus::Missing,
            severity: Severity::Degraded,
            detail: Some(path.display().to_string()),
            fix: Some(vec![format!(
                "cp fin.rules.template.ts {} && fin rules migrate-ts --source {} --target {}",
                path.with_extension("ts").display(),
                path.with_extension("ts").display(),
                path.display()
            )]),
        };
    }
    HealthCheck {
        id: "rules".to_owned(),
        label: "Rules file".to_owned(),
        status: CheckStatus::Ok,
        severity: Severity::Info,
        detail: Some(path.display().to_string()),
        fix: None,
    }
}

fn check_inbox_exists(path: &Path) -> HealthCheck {
    if !path.exists() {
        return HealthCheck {
            id: "inbox".to_owned(),
            label: "Inbox directory".to_owned(),
            status: CheckStatus::Missing,
            severity: Severity::Info,
            detail: Some(path.display().to_string()),
            fix: Some(vec![format!("mkdir -p {}", path.display())]),
        };
    }
    HealthCheck {
        id: "inbox".to_owned(),
        label: "Inbox directory".to_owned(),
        status: CheckStatus::Ok,
        severity: Severity::Info,
        detail: Some(path.display().to_string()),
        fix: None,
    }
}

fn compute_summary(checks: &[HealthCheck]) -> HealthSummary {
    HealthSummary {
        ok: checks
            .iter()
            .filter(|check| check.status == CheckStatus::Ok)
            .count(),
        blocking: checks
            .iter()
            .filter(|check| check.severity == Severity::Blocking && check.status != CheckStatus::Ok)
            .count(),
        degraded: checks
            .iter()
            .filter(|check| check.severity == Severity::Degraded && check.status != CheckStatus::Ok)
            .count(),
    }
}

pub fn run_health_checks(options: HealthCheckOptions) -> HealthReport {
    let paths = options.paths_override.unwrap_or_else(resolve_fin_paths);
    let config_path = resolve_config_path(options.config_path.as_deref());
    let config_exists = check_config_exists(&config_path);
    let (config_validates, loaded_config) = check_config_validates(&config_path);
    let config_dir = loaded_config.as_ref().map(LoadedConfig::config_dir);
    let db_path = resolve_db_path(options.db_path.as_deref(), config_dir.as_deref());
    let rules_path = resolve_rules_path(options.rules_path.as_deref(), loaded_config.as_ref());

    let mut checks = vec![
        config_exists,
        config_validates,
        check_database_exists(&db_path),
    ];
    if let Some(schema_check) = check_db_schema(&db_path) {
        checks.push(schema_check);
    }
    checks.push(check_rules_exists(&rules_path));
    checks.push(check_inbox_exists(&paths.inbox_dir));

    let has_blocking = checks
        .iter()
        .any(|check| check.severity == Severity::Blocking && check.status != CheckStatus::Ok);
    let has_degraded = checks
        .iter()
        .any(|check| check.severity == Severity::Degraded && check.status != CheckStatus::Ok);

    let status = if has_blocking {
        HealthStatus::Blocked
    } else if has_degraded {
        HealthStatus::Degraded
    } else {
        HealthStatus::Ready
    };
    let summary = compute_summary(&checks);
    HealthReport {
        status,
        checks,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::config::paths::resolve_fin_paths_from_home;
    use crate::db::connection::{OpenDatabaseOptions, open_database};
    use crate::health::{HealthCheckOptions, HealthStatus, run_health_checks};

    #[test]
    fn health_reports_ready_when_core_files_exist() {
        let temp = tempdir().expect("tempdir");
        let paths = resolve_fin_paths_from_home(temp.path().to_path_buf());
        std::fs::create_dir_all(&paths.data_dir).expect("create data dir");
        std::fs::create_dir_all(&paths.inbox_dir).expect("create inbox dir");

        std::fs::write(
            &paths.config_file,
            r#"
[financial]
corp_tax_rate = 0.25

[[accounts]]
id = "Assets:Personal:Monzo"
group = "personal"
type = "asset"
provider = "monzo"

[[banks]]
name = "monzo"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"

[sanitization]
rules = "data/fin.rules.toml"
"#,
        )
        .expect("write config");
        std::fs::write(
            &paths.rules_file,
            r#"
warn_on_unmapped = true
fallback_to_raw = true
"#,
        )
        .expect("write rules");

        let _db = open_database(OpenDatabaseOptions {
            path: Some(paths.db_file.clone()),
            migrate: true,
            ..OpenDatabaseOptions::default()
        })
        .expect("open db");

        let report = run_health_checks(HealthCheckOptions {
            config_path: Some(paths.config_file.clone()),
            db_path: Some(paths.db_file.clone()),
            rules_path: Some(paths.rules_file.clone()),
            paths_override: Some(paths),
        });
        assert_eq!(report.status, HealthStatus::Ready);
    }
}
