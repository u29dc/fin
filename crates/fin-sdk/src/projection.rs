use std::collections::BTreeSet;

use rusqlite::{Connection, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::config::FinConfig;
use crate::error::Result;
use crate::stats::{mean_i64, median_i64};

const DEFAULT_PROJECTION_MONTHS: usize = 24;
const DEFAULT_TRAILING_WINDOW_MONTHS: usize = 12;
const MEDIAN_REFERENCE_MONTHS: usize = 12;
const DEFAULT_MINIMUM_BURN_RATIO: f64 = 0.6;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionScopeKind {
    Group,
    Consolidated,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionScenarioKind {
    CurrentBurn,
    MinimumBurn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunwayProjectionOptions {
    pub months: usize,
    pub minimum_burn_ratio: f64,
    pub as_of: Option<String>,
    pub trailing_outflow_window_months: Option<usize>,
}

impl Default for RunwayProjectionOptions {
    fn default() -> Self {
        Self {
            months: DEFAULT_PROJECTION_MONTHS,
            minimum_burn_ratio: DEFAULT_MINIMUM_BURN_RATIO,
            as_of: None,
            trailing_outflow_window_months: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunwayThresholds {
    pub warning_minor: Option<i64>,
    pub threshold_minor: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunwayThresholdCrossing {
    pub month_index: usize,
    pub date: String,
    pub balance_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunwayProjectionPoint {
    pub month_index: usize,
    pub date: String,
    pub balance_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunwayProjectionScenario {
    pub kind: ProjectionScenarioKind,
    pub label: String,
    pub burn_rate_minor: i64,
    pub is_net_positive: bool,
    pub zero_balance_crossing: Option<RunwayThresholdCrossing>,
    pub warning_crossing: Option<RunwayThresholdCrossing>,
    pub threshold_crossing: Option<RunwayThresholdCrossing>,
    pub points: Vec<RunwayProjectionPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunwayProjectionAssumptions {
    pub as_of_date: String,
    pub projection_months: usize,
    pub trailing_outflow_window_months: usize,
    pub burn_rate_method: String,
    pub minimum_burn_ratio: f64,
    pub full_months_only: bool,
    pub include_as_of_month_in_history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunwayProjectionReport {
    pub scope_kind: ProjectionScopeKind,
    pub scope_id: String,
    pub liquid_balance_minor: i64,
    pub current_burn_minor: i64,
    pub minimum_burn_minor: i64,
    pub median_monthly_expense_minor: i64,
    pub thresholds: RunwayThresholds,
    pub assumptions: RunwayProjectionAssumptions,
    pub scenarios: Vec<RunwayProjectionScenario>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MonthlyOutflowPoint {
    month: String,
    expense_minor: i64,
}

#[derive(Debug, Clone)]
struct ProjectionInputs {
    liquid_balance_minor: i64,
    current_burn_minor: i64,
    median_monthly_expense_minor: i64,
    thresholds: RunwayThresholds,
    assumptions: RunwayProjectionAssumptions,
}

/// Build forward runway projections for a single group.
pub fn project_group_runway(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    options: &RunwayProjectionOptions,
) -> Result<RunwayProjectionReport> {
    let scope_group_ids = [group_id.to_owned()];
    let asset_account_ids = scoped_asset_account_ids(config, &scope_group_ids);
    let runway_balance_account_ids = scoped_runway_balance_account_ids(config, &scope_group_ids);
    let inputs = projection_inputs(
        connection,
        config,
        &asset_account_ids,
        &runway_balance_account_ids,
        options,
    )?;
    Ok(build_projection_report(
        ProjectionScopeKind::Group,
        group_id.to_owned(),
        inputs,
    ))
}

/// Build forward runway projections across multiple groups.
pub fn project_consolidated_runway(
    connection: &Connection,
    config: &FinConfig,
    group_ids: &[String],
    options: &RunwayProjectionOptions,
) -> Result<RunwayProjectionReport> {
    let asset_account_ids = scoped_asset_account_ids(config, group_ids);
    let runway_balance_account_ids = scoped_runway_balance_account_ids(config, group_ids);
    let inputs = projection_inputs(
        connection,
        config,
        &asset_account_ids,
        &runway_balance_account_ids,
        options,
    )?;
    Ok(build_projection_report(
        ProjectionScopeKind::Consolidated,
        "consolidated".to_owned(),
        inputs,
    ))
}

fn build_projection_report(
    scope_kind: ProjectionScopeKind,
    scope_id: String,
    inputs: ProjectionInputs,
) -> RunwayProjectionReport {
    let current_burn_minor = inputs.current_burn_minor.max(0);
    let minimum_burn_minor =
        ((current_burn_minor as f64) * inputs.assumptions.minimum_burn_ratio).round() as i64;

    let scenarios = vec![
        build_projection_scenario(
            ProjectionScenarioKind::CurrentBurn,
            "Current burn",
            inputs.liquid_balance_minor,
            current_burn_minor,
            inputs.assumptions.projection_months,
            &inputs.assumptions.as_of_date,
            &inputs.thresholds,
        ),
        build_projection_scenario(
            ProjectionScenarioKind::MinimumBurn,
            "Minimum burn",
            inputs.liquid_balance_minor,
            minimum_burn_minor.max(0),
            inputs.assumptions.projection_months,
            &inputs.assumptions.as_of_date,
            &inputs.thresholds,
        ),
    ];

    RunwayProjectionReport {
        scope_kind,
        scope_id,
        liquid_balance_minor: inputs.liquid_balance_minor,
        current_burn_minor,
        minimum_burn_minor: minimum_burn_minor.max(0),
        median_monthly_expense_minor: inputs.median_monthly_expense_minor.max(0),
        thresholds: inputs.thresholds,
        assumptions: inputs.assumptions,
        scenarios,
    }
}

fn projection_inputs(
    connection: &Connection,
    config: &FinConfig,
    asset_account_ids: &[String],
    liquid_account_ids: &[String],
    options: &RunwayProjectionOptions,
) -> Result<ProjectionInputs> {
    let as_of_date = resolved_as_of_date(connection, options.as_of.as_deref())?;
    let trailing_outflow_window_months = options
        .trailing_outflow_window_months
        .unwrap_or_else(|| {
            financial_usize(config, "trailing_expense_window_months")
                .unwrap_or(DEFAULT_TRAILING_WINDOW_MONTHS)
        })
        .max(1);
    let burn_rate_method = burn_rate_method(config);
    let include_as_of_month_in_history = is_end_of_month(&as_of_date);
    let monthly_outflow = scoped_monthly_outflow(
        connection,
        config,
        asset_account_ids,
        &as_of_date,
        trailing_outflow_window_months.max(MEDIAN_REFERENCE_MONTHS) + 1,
    )?;
    let full_month_history = monthly_outflow
        .into_iter()
        .filter(|point| {
            let cutoff_month = year_month(&as_of_date);
            if include_as_of_month_in_history {
                point.month <= cutoff_month
            } else {
                point.month < cutoff_month
            }
        })
        .collect::<Vec<_>>();

    let trailing_window = tail_values(&full_month_history, trailing_outflow_window_months)
        .iter()
        .map(|point| point.expense_minor)
        .collect::<Vec<_>>();
    let median_reference = tail_values(&full_month_history, MEDIAN_REFERENCE_MONTHS)
        .iter()
        .map(|point| point.expense_minor)
        .collect::<Vec<_>>();
    let current_burn_minor = match burn_rate_method.as_str() {
        "mean" => mean_i64(&trailing_window).unwrap_or(0),
        _ => median_i64(&trailing_window).unwrap_or(0),
    }
    .max(0);
    let median_monthly_expense_minor = median_i64(&median_reference).unwrap_or(0).max(0);
    let liquid_balance_minor = scoped_balance_as_of(connection, liquid_account_ids, &as_of_date)?;
    let thresholds = RunwayThresholds {
        warning_minor: financial_i64(config, "runway_warning_minor"),
        threshold_minor: financial_i64(config, "runway_threshold_minor"),
    };

    Ok(ProjectionInputs {
        liquid_balance_minor,
        current_burn_minor,
        median_monthly_expense_minor,
        thresholds,
        assumptions: RunwayProjectionAssumptions {
            as_of_date,
            projection_months: options.months.max(1),
            trailing_outflow_window_months,
            burn_rate_method,
            minimum_burn_ratio: clamp_minimum_burn_ratio(options.minimum_burn_ratio),
            full_months_only: true,
            include_as_of_month_in_history,
        },
    })
}

fn build_projection_scenario(
    kind: ProjectionScenarioKind,
    label: &str,
    starting_balance_minor: i64,
    burn_rate_minor: i64,
    months: usize,
    as_of_date: &str,
    thresholds: &RunwayThresholds,
) -> RunwayProjectionScenario {
    let points = (0..=months)
        .map(|month_index| {
            let projected_balance_minor = if burn_rate_minor <= 0 {
                starting_balance_minor
            } else {
                starting_balance_minor
                    .saturating_sub(burn_rate_minor * i64::try_from(month_index).unwrap_or(0))
            }
            .max(0);
            RunwayProjectionPoint {
                month_index,
                date: add_months(as_of_date, month_index),
                balance_minor: projected_balance_minor,
            }
        })
        .collect::<Vec<_>>();

    RunwayProjectionScenario {
        kind,
        label: label.to_owned(),
        burn_rate_minor,
        is_net_positive: burn_rate_minor <= 0,
        zero_balance_crossing: find_first_crossing(&points, 0),
        warning_crossing: thresholds
            .warning_minor
            .and_then(|warning_minor| find_first_crossing(&points, warning_minor)),
        threshold_crossing: thresholds
            .threshold_minor
            .and_then(|threshold_minor| find_first_crossing(&points, threshold_minor)),
        points,
    }
}

fn find_first_crossing(
    points: &[RunwayProjectionPoint],
    threshold_minor: i64,
) -> Option<RunwayThresholdCrossing> {
    points
        .iter()
        .find(|point| point.balance_minor <= threshold_minor)
        .map(|point| RunwayThresholdCrossing {
            month_index: point.month_index,
            date: point.date.clone(),
            balance_minor: point.balance_minor,
        })
}

fn scoped_asset_account_ids(config: &FinConfig, group_ids: &[String]) -> Vec<String> {
    let scope = group_ids.iter().collect::<BTreeSet<_>>();
    config
        .accounts
        .iter()
        .filter(|account| account.account_type == "asset" && scope.contains(&account.group))
        .map(|account| account.id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn scoped_runway_balance_account_ids(config: &FinConfig, group_ids: &[String]) -> Vec<String> {
    let scope = group_ids.iter().collect::<BTreeSet<_>>();
    let excluded_prefixes = runway_balance_exclude_prefixes(config);
    config
        .accounts
        .iter()
        .filter(|account| account.account_type == "asset" && scope.contains(&account.group))
        .filter(|account| !matches_any_account_prefix(&account.id, &excluded_prefixes))
        .map(|account| account.id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn scoped_balance_as_of(
    connection: &Connection,
    account_ids: &[String],
    as_of_date: &str,
) -> Result<i64> {
    if account_ids.is_empty() {
        return Ok(0);
    }

    let (account_clause, mut params) = account_match_clause("p", account_ids);
    params.push(format!("{as_of_date}T23:59:59.999"));
    let sql = format!(
        "SELECT COALESCE(SUM(p.amount_minor), 0)\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON p.account_id = coa.id\n         WHERE coa.account_type = 'asset'\n           AND {}\n           AND je.posted_at <= ?",
        account_clause
    );

    connection
        .query_row(&sql, params_from_iter(params.iter()), |row| row.get(0))
        .map_err(Into::into)
}

fn scoped_monthly_outflow(
    connection: &Connection,
    config: &FinConfig,
    account_ids: &[String],
    as_of_date: &str,
    month_limit: usize,
) -> Result<Vec<MonthlyOutflowPoint>> {
    if account_ids.is_empty() {
        return Ok(Vec::new());
    }

    let (asset_outflow_clause, mut params) = account_match_clause("asset_out", account_ids);
    let (asset_inflow_clause, inflow_params) = account_match_clause("asset_in", account_ids);
    params.extend(inflow_params);
    let excluded_prefixes = burn_rate_exclude_prefixes(config);
    let exclude_sql = if excluded_prefixes.is_empty() {
        String::new()
    } else {
        let (exclude_clause, exclude_params) = exclusion_clause("p", &excluded_prefixes);
        params.extend(exclude_params);
        format!("\n               AND {exclude_clause}")
    };

    params.push(format!("{as_of_date}T23:59:59.999"));
    params.push(month_limit.max(1).to_string());
    let sql = format!(
        "SELECT month, expense_minor\n         FROM (\n             SELECT strftime('%Y-%m', je.posted_at) AS month,\n                    COALESCE(SUM(p.amount_minor), 0) AS expense_minor\n             FROM journal_entries je\n             JOIN postings p ON p.journal_entry_id = je.id\n             JOIN chart_of_accounts coa ON p.account_id = coa.id\n             WHERE coa.account_type = 'expense'\n               AND EXISTS (\n                 SELECT 1\n                 FROM postings asset_out\n                 WHERE asset_out.journal_entry_id = je.id\n                   AND asset_out.amount_minor < 0\n                   AND {}\n               )\n               AND NOT EXISTS (\n                 SELECT 1\n                 FROM postings asset_in\n                 WHERE asset_in.journal_entry_id = je.id\n                   AND asset_in.amount_minor > 0\n                   AND {}\n               ){}\n               AND je.posted_at <= ?\n             GROUP BY month\n             ORDER BY month DESC\n             LIMIT ?\n         ) recent\n         ORDER BY month ASC",
        asset_outflow_clause, asset_inflow_clause, exclude_sql
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok(MonthlyOutflowPoint {
            month: row.get(0)?,
            expense_minor: row.get(1)?,
        })
    })?;

    let mut points = Vec::new();
    for row in rows {
        points.push(row?);
    }
    Ok(points)
}

fn account_match_clause(alias: &str, account_ids: &[String]) -> (String, Vec<String>) {
    let mut clauses = Vec::new();
    let mut params = Vec::new();
    for account_id in account_ids {
        clauses.push(format!(
            "({alias}.account_id = ? OR {alias}.account_id LIKE ?)"
        ));
        params.push(account_id.clone());
        params.push(format!("{account_id}:%"));
    }
    (format!("({})", clauses.join(" OR ")), params)
}

fn exclusion_clause(alias: &str, account_prefixes: &[String]) -> (String, Vec<String>) {
    let mut clauses = Vec::new();
    let mut params = Vec::new();
    for account_prefix in account_prefixes {
        clauses.push(format!(
            "NOT ({alias}.account_id = ? OR {alias}.account_id LIKE ?)"
        ));
        params.push(account_prefix.clone());
        params.push(format!("{account_prefix}:%"));
    }
    (clauses.join(" AND "), params)
}

fn tail_values<T>(values: &[T], count: usize) -> &[T] {
    let start = values.len().saturating_sub(count.max(1));
    &values[start..]
}

fn burn_rate_exclude_prefixes(config: &FinConfig) -> Vec<String> {
    config
        .financial
        .get("burn_rate_exclude_accounts")
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn runway_balance_exclude_prefixes(config: &FinConfig) -> Vec<String> {
    config
        .financial
        .get("runway_balance_exclude_accounts")
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn matches_any_account_prefix(account_id: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|prefix| account_id == prefix || account_id.starts_with(&format!("{prefix}:")))
}

fn burn_rate_method(config: &FinConfig) -> String {
    config
        .financial
        .get("burn_rate_method")
        .and_then(toml::Value::as_str)
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "mean" | "median"))
        .unwrap_or_else(|| "median".to_owned())
}

fn financial_usize(config: &FinConfig, key: &str) -> Option<usize> {
    financial_i64(config, key).and_then(|value| usize::try_from(value).ok())
}

fn financial_i64(config: &FinConfig, key: &str) -> Option<i64> {
    match config.financial.get(key) {
        Some(toml::Value::Integer(value)) => Some(*value),
        Some(toml::Value::Float(value)) => Some(value.round() as i64),
        _ => None,
    }
}

fn resolved_as_of_date(connection: &Connection, as_of: Option<&str>) -> Result<String> {
    if let Some(as_of) = as_of {
        return Ok(as_of.to_owned());
    }
    connection
        .query_row(
            "SELECT COALESCE(MAX(date(posted_at)), date('now')) FROM journal_entries",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn clamp_minimum_burn_ratio(value: f64) -> f64 {
    if !value.is_finite() {
        return DEFAULT_MINIMUM_BURN_RATIO;
    }
    value.clamp(0.0, 1.0)
}

fn year_month(date: &str) -> String {
    date.chars().take(7).collect()
}

fn is_end_of_month(date: &str) -> bool {
    let Some((year, month, day)) = parse_iso_date(date) else {
        return false;
    };
    day == days_in_month(year, month)
}

fn add_months(date: &str, months: usize) -> String {
    let Some((year, month, day)) = parse_iso_date(date) else {
        return date.to_owned();
    };
    let total_months = usize::try_from(month.saturating_sub(1)).unwrap_or(0) + months;
    let year_offset = i32::try_from(total_months / 12).unwrap_or(0);
    let next_year = year + year_offset;
    let next_month = u32::try_from((total_months % 12) + 1).unwrap_or(1);
    let next_day = day.min(days_in_month(next_year, next_month));
    format!("{next_year:04}-{next_month:02}-{next_day:02}")
}

fn parse_iso_date(date: &str) -> Option<(i32, u32, u32)> {
    let mut parts = date.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    Some((year, month, day))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 30,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::{
        ProjectionScenarioKind, ProjectionScopeKind, RunwayProjectionOptions, RunwayThresholds,
        add_months, build_projection_scenario, clamp_minimum_burn_ratio,
        matches_any_account_prefix, project_consolidated_runway, project_group_runway,
        runway_balance_exclude_prefixes, scoped_monthly_outflow, scoped_runway_balance_account_ids,
    };
    use crate::config::parse_fin_config;
    use crate::db::migrate::migrate_to_latest;
    use crate::runtime::{RuntimeContext, RuntimeContextOptions};
    use crate::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

    #[test]
    fn add_months_clamps_to_shorter_months() {
        assert_eq!(add_months("2026-01-31", 1), "2026-02-28");
        assert_eq!(add_months("2024-01-31", 1), "2024-02-29");
        assert_eq!(add_months("2026-11-30", 3), "2027-02-28");
    }

    #[test]
    fn clamp_minimum_burn_ratio_stays_in_range() {
        assert_eq!(clamp_minimum_burn_ratio(-1.0), 0.0);
        assert_eq!(clamp_minimum_burn_ratio(0.6), 0.6);
        assert_eq!(clamp_minimum_burn_ratio(2.0), 1.0);
        assert_eq!(clamp_minimum_burn_ratio(f64::NAN), 0.6);
    }

    #[test]
    fn projection_scenario_tracks_warning_threshold_and_zero_crossings() {
        let scenario = build_projection_scenario(
            ProjectionScenarioKind::CurrentBurn,
            "Current burn",
            100_000,
            30_000,
            4,
            "2026-03-31",
            &RunwayThresholds {
                warning_minor: Some(60_000),
                threshold_minor: Some(40_000),
            },
        );

        assert_eq!(scenario.points.len(), 5);
        assert_eq!(
            scenario
                .warning_crossing
                .as_ref()
                .map(|point| point.month_index),
            Some(2)
        );
        assert_eq!(
            scenario
                .threshold_crossing
                .as_ref()
                .map(|point| point.month_index),
            Some(2)
        );
        assert_eq!(
            scenario
                .zero_balance_crossing
                .as_ref()
                .map(|point| point.month_index),
            Some(4)
        );
    }

    #[test]
    fn projection_scenario_marks_net_positive_without_breaches() {
        let scenario = build_projection_scenario(
            ProjectionScenarioKind::MinimumBurn,
            "Minimum burn",
            250_000,
            0,
            6,
            "2026-03-31",
            &RunwayThresholds {
                warning_minor: Some(50_000),
                threshold_minor: Some(25_000),
            },
        );

        assert!(scenario.is_net_positive);
        assert!(scenario.zero_balance_crossing.is_none());
        assert!(scenario.warning_crossing.is_none());
        assert!(scenario.threshold_crossing.is_none());
        assert!(
            scenario
                .points
                .iter()
                .all(|point| point.balance_minor == 250_000)
        );
    }

    #[test]
    fn fixture_group_projection_returns_two_scenarios_with_positive_runway_balance() {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");
        let runtime = RuntimeContext::open(RuntimeContextOptions {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            create: false,
            ..RuntimeContextOptions::read_only()
        })
        .expect("open runtime");

        let mut config = runtime.config().clone();
        config.financial.insert(
            "runway_warning_minor".to_owned(),
            toml::Value::Integer(2_500_000),
        );
        config.financial.insert(
            "runway_threshold_minor".to_owned(),
            toml::Value::Integer(1_500_000),
        );
        config.financial.insert(
            "trailing_expense_window_months".to_owned(),
            toml::Value::Integer(6),
        );

        let report = project_group_runway(
            runtime.connection(),
            &config,
            "personal",
            &RunwayProjectionOptions {
                months: 12,
                minimum_burn_ratio: 0.6,
                as_of: Some("2026-03-31".to_owned()),
                trailing_outflow_window_months: None,
            },
        )
        .expect("group projection");

        assert_eq!(report.scope_kind, ProjectionScopeKind::Group);
        assert_eq!(report.scope_id, "personal");
        assert_eq!(report.scenarios.len(), 2);
        assert_eq!(report.assumptions.as_of_date, "2026-03-31");
        assert_eq!(report.assumptions.projection_months, 12);
        assert_eq!(report.assumptions.trailing_outflow_window_months, 6);
        assert!(report.current_burn_minor >= report.minimum_burn_minor);
        assert!(report.current_burn_minor > 0);
        assert!(report.median_monthly_expense_minor > 0);
        assert!(report.liquid_balance_minor > 0);
        assert!(
            report
                .scenarios
                .iter()
                .all(|scenario| scenario.points.len() == 13)
        );
        assert!(report.scenarios.iter().any(|scenario| {
            scenario.kind == ProjectionScenarioKind::CurrentBurn
                && scenario.burn_rate_minor == report.current_burn_minor
        }));
        assert!(report.scenarios.iter().any(|scenario| {
            scenario.kind == ProjectionScenarioKind::MinimumBurn
                && scenario.burn_rate_minor == report.minimum_burn_minor
        }));
    }

    #[test]
    fn scoped_runway_balance_includes_investments_by_default_and_honors_excludes() {
        let config = parse_fin_config(
            r#"
[financial]
runway_balance_exclude_accounts = ["Assets:Personal:Savings"]

[[groups]]
id = "personal"
label = "Personal"

[[accounts]]
id = "Assets:Personal:Monzo"
group = "personal"
type = "asset"
provider = "monzo"

[[accounts]]
id = "Assets:Personal:Vanguard"
group = "personal"
type = "asset"
provider = "vanguard"
subtype = "investment"

[[accounts]]
id = "Assets:Personal:Savings"
group = "personal"
type = "asset"
provider = "monzo"

[[banks]]
name = "monzo"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"

[[banks]]
name = "vanguard"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"
"#,
        )
        .expect("config parses");

        assert_eq!(
            runway_balance_exclude_prefixes(&config),
            vec!["Assets:Personal:Savings".to_owned()]
        );
        assert!(matches_any_account_prefix(
            "Assets:Personal:Savings:Pot",
            &runway_balance_exclude_prefixes(&config)
        ));

        let ids = scoped_runway_balance_account_ids(&config, &["personal".to_owned()]);
        assert_eq!(
            ids,
            vec![
                "Assets:Personal:Monzo".to_owned(),
                "Assets:Personal:Vanguard".to_owned(),
            ]
        );
    }

    #[test]
    fn scoped_monthly_outflow_ignores_internal_selected_scope_transfers() {
        let mut connection = Connection::open_in_memory().expect("open sqlite");
        migrate_to_latest(&mut connection).expect("migrate schema");
        let config = parse_fin_config(
            r#"
[financial]

[[groups]]
id = "business"
label = "Business"

[[groups]]
id = "personal"
label = "Personal"

[[accounts]]
id = "Assets:Business:Monzo"
group = "business"
type = "asset"
provider = "monzo"

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
"#,
        )
        .expect("config parses");

        connection
            .execute_batch(
                r#"
INSERT INTO chart_of_accounts (id, name, account_type, parent_id) VALUES
    ('Assets', 'Assets', 'asset', NULL),
    ('Assets:Business:Monzo', 'Business Monzo', 'asset', 'Assets'),
    ('Assets:Personal:Monzo', 'Personal Monzo', 'asset', 'Assets'),
    ('Expenses', 'Expenses', 'expense', NULL),
    ('Expenses:Business:Operations', 'Operations', 'expense', 'Expenses'),
    ('Expenses:Business:Salary', 'Salary', 'expense', 'Expenses'),
    ('Income', 'Income', 'income', NULL),
    ('Income:Personal:Salary', 'Salary', 'income', 'Income'),
    ('Equity', 'Equity', 'equity', NULL),
    ('Equity:OpeningBalances', 'Opening Balances', 'equity', 'Equity');

INSERT INTO journal_entries (id, posted_at, posted_date, description, counterparty, source_file) VALUES
    ('je-opening-business', '2026-02-01T08:00:00Z', '2026-02-01', 'Opening business', 'Fixture', 'fixture.csv'),
    ('je-opening-personal', '2026-02-01T08:05:00Z', '2026-02-01', 'Opening personal', 'Fixture', 'fixture.csv'),
    ('je-rent', '2026-02-10T09:00:00Z', '2026-02-10', 'Office rent', 'Landlord', 'fixture.csv'),
    ('je-draw', '2026-02-12T09:00:00Z', '2026-02-12', 'Owner draw', 'Owner', 'fixture.csv');

INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo) VALUES
    ('po-open-business-asset', 'je-opening-business', 'Assets:Business:Monzo', 500000, 'GBP', NULL),
    ('po-open-business-equity', 'je-opening-business', 'Equity:OpeningBalances', -500000, 'GBP', NULL),
    ('po-open-personal-asset', 'je-opening-personal', 'Assets:Personal:Monzo', 250000, 'GBP', NULL),
    ('po-open-personal-equity', 'je-opening-personal', 'Equity:OpeningBalances', -250000, 'GBP', NULL),
    ('po-rent-expense', 'je-rent', 'Expenses:Business:Operations', 100000, 'GBP', NULL),
    ('po-rent-asset', 'je-rent', 'Assets:Business:Monzo', -100000, 'GBP', NULL),
    ('po-draw-expense', 'je-draw', 'Expenses:Business:Salary', 200000, 'GBP', NULL),
    ('po-draw-business', 'je-draw', 'Assets:Business:Monzo', -200000, 'GBP', NULL),
    ('po-draw-personal', 'je-draw', 'Assets:Personal:Monzo', 200000, 'GBP', NULL),
    ('po-draw-income', 'je-draw', 'Income:Personal:Salary', -200000, 'GBP', NULL);
"#,
            )
            .expect("seed ledger");

        let outflow = scoped_monthly_outflow(
            &connection,
            &config,
            &[
                "Assets:Business:Monzo".to_owned(),
                "Assets:Personal:Monzo".to_owned(),
            ],
            "2026-02-28",
            2,
        )
        .expect("outflow series");

        assert_eq!(outflow.len(), 1);
        assert_eq!(outflow[0].month, "2026-02");
        assert_eq!(outflow[0].expense_minor, 100_000);
    }

    #[test]
    fn fixture_consolidated_projection_combines_scope() {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");
        let runtime = RuntimeContext::open(RuntimeContextOptions {
            config_path: Some(fixture.paths.config_path.clone()),
            db_path: Some(fixture.paths.db_path.clone()),
            create: false,
            ..RuntimeContextOptions::read_only()
        })
        .expect("open runtime");

        let group_ids = vec![
            "business".to_owned(),
            "personal".to_owned(),
            "joint".to_owned(),
        ];
        let report = project_consolidated_runway(
            runtime.connection(),
            runtime.config(),
            &group_ids,
            &RunwayProjectionOptions {
                months: 6,
                minimum_burn_ratio: 0.5,
                as_of: Some("2026-03-31".to_owned()),
                trailing_outflow_window_months: Some(6),
            },
        )
        .expect("consolidated projection");

        assert_eq!(report.scope_kind, ProjectionScopeKind::Consolidated);
        assert_eq!(report.scope_id, "consolidated");
        assert_eq!(report.assumptions.minimum_burn_ratio, 0.5);
        assert!(report.liquid_balance_minor > 0);
        assert!(report.current_burn_minor > 0);
        assert_eq!(report.scenarios.len(), 2);
    }
}
