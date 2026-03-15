use std::collections::BTreeMap;

use rusqlite::{Connection, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::burn::{BurnReportOptions, OwnershipMode, report_burn};
use crate::config::{ExpenseReserveBasis, FinConfig, ReserveMode, ResolvedReservePolicy};
use crate::dashboard::{CashflowKpis, ShortTermTrend, reporting_month, summarize_cashflow_kpis};
use crate::error::Result;
use crate::queries::{
    MonthlyCashflowPoint, all_group_ids, get_balance_sheet, group_asset_account_ids,
    group_monthly_cashflow, view_accounts,
};
use crate::stats::{mean_i64, median_i64};
use crate::timeseries::{BalanceSeriesQueryOptions, group_daily_balance_series};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashflowTotals {
    pub income_minor: i64,
    pub expense_minor: i64,
    pub net_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthPoint {
    pub date: String,
    pub balance_minor: i64,
    pub health_minor: i64,
    pub reserve_mode: ReserveMode,
    pub expense_reserve_basis_kind: ExpenseReserveBasis,
    pub expense_reserve_monthly_basis_minor: i64,
    pub expense_reserve_months: f64,
    pub expense_reserve_factor: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expense_reserve_lookback_months: Option<usize>,
    pub expense_reserve_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunwayPoint {
    pub date: String,
    pub runway_months: f64,
    pub balance_minor: i64,
    pub burn_rate_minor: i64,
    pub median_expense_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReserveBreakdownPoint {
    pub date: String,
    pub reserve_mode: ReserveMode,
    pub expense_reserve_basis_kind: ExpenseReserveBasis,
    pub expense_reserve_monthly_basis_minor: i64,
    pub expense_reserve_months: f64,
    pub expense_reserve_factor: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expense_reserve_lookback_months: Option<usize>,
    pub tax_reserve_basis_kind: TaxReserveBasisKind,
    pub tax_reserve_basis_description: String,
    pub balance_minor: i64,
    pub tax_reserve_minor: i64,
    pub expense_reserve_minor: i64,
    pub available_minor: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaxReserveBasisKind {
    None,
    YtdProfit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSummary {
    pub label: String,
    pub net_worth_minor: i64,
    pub latest_runway_months: Option<f64>,
    pub latest_health_minor: Option<i64>,
    pub latest_available_minor: Option<i64>,
    pub last_full_month_net_minor: Option<i64>,
    pub trailing_average_net_minor: Option<i64>,
    pub median_spend_minor: Option<i64>,
    pub short_term_trend: Option<ShortTermTrend>,
    pub anomaly_count_last_12_months: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryReport {
    pub generated_at: String,
    pub period_months: usize,
    pub groups: BTreeMap<String, GroupSummary>,
    pub consolidated: ConsolidatedSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidatedSummary {
    pub net_worth_minor: i64,
    pub balance_sheet: crate::queries::BalanceSheet,
}

#[derive(Debug, Clone)]
struct GroupReportContext {
    cashflow: Vec<MonthlyCashflowPoint>,
    balance_points: Vec<MonthlyBalancePoint>,
    cashflow_totals: CashflowTotals,
    reserve_policy: ResolvedReservePolicy,
    expense_reserve_monthly_basis_minor: i64,
    expense_reserve_minor: i64,
    tax_reserve_basis_kind: TaxReserveBasisKind,
    tax_reserve_basis_description: String,
    tax_rate: f64,
    tax_year_start_month: u32,
    burn_rate_minor: i64,
    median_expense_minor: i64,
    fallback_balance_minor: Option<i64>,
}

#[derive(Debug, Clone)]
struct MonthlyBalancePoint {
    month: String,
    date: String,
    balance_minor: i64,
}

#[derive(Debug, Clone, Default)]
struct GroupSummaryLatestMetrics {
    latest_runway_months: Option<f64>,
    latest_health_minor: Option<i64>,
    latest_available_minor: Option<i64>,
}

fn burn_rate(values: &[i64], method: &str) -> i64 {
    if method == "mean" {
        return mean_i64(values).unwrap_or(0);
    }
    median_i64(values).unwrap_or(0)
}

fn group_total_balance(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    as_of: Option<&str>,
) -> Result<i64> {
    if as_of.is_none() {
        let accounts = view_accounts(connection, config, Some(group_id))?;
        return Ok(accounts
            .iter()
            .map(|account| account.balance_minor.unwrap_or(0))
            .sum());
    }

    let account_ids = group_asset_account_ids(config, group_id);
    if account_ids.is_empty() {
        return Ok(0);
    }

    let mut account_match_clauses = Vec::new();
    let mut params = Vec::new();
    for account_id in &account_ids {
        account_match_clauses.push("(p.account_id = ? OR p.account_id LIKE ?)".to_owned());
        params.push(account_id.clone());
        params.push(format!("{account_id}:%"));
    }
    params.push(format!(
        "{}T23:59:59.999",
        as_of.expect("as_of already validated")
    ));

    let sql = format!(
        "SELECT COALESCE(SUM(p.amount_minor), 0)\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON p.account_id = coa.id\n         WHERE coa.account_type = 'asset'\n           AND ({})\n           AND je.posted_at <= ?",
        account_match_clauses.join(" OR ")
    );

    connection
        .query_row(&sql, params_from_iter(params.iter()), |row| row.get(0))
        .map_err(Into::into)
}

fn parse_date(month: &str) -> String {
    format!("{month}-01")
}

fn parse_year_month(month: &str) -> Option<(i32, u32)> {
    let mut parts = month.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month_num = parts.next()?.parse::<u32>().ok()?;
    if !(1..=12).contains(&month_num) {
        return None;
    }
    Some((year, month_num))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let is_leap_year = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if is_leap_year { 29 } else { 28 }
        }
        _ => 30,
    }
}

fn cutoff_date_for_month(month: &str, to: Option<&str>) -> String {
    if let Some(to) = to
        && to.starts_with(month)
    {
        return to.to_owned();
    }
    let (year, month_num) = parse_year_month(month).unwrap_or((1970, 1));
    let day = days_in_month(year, month_num);
    format!("{year:04}-{month_num:02}-{day:02}")
}

fn actual_monthly_balance_points(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    cashflow: &[MonthlyCashflowPoint],
    to: Option<&str>,
) -> Result<Vec<MonthlyBalancePoint>> {
    if cashflow.is_empty() {
        return Ok(Vec::new());
    }

    let balance_series = group_daily_balance_series(
        connection,
        config,
        group_id,
        &BalanceSeriesQueryOptions {
            from: Some(parse_date(&cashflow[0].month)),
            to: to.map(ToOwned::to_owned),
            ..BalanceSeriesQueryOptions::default()
        },
    )?;

    let mut cursor = 0usize;
    let mut last_balance_minor = 0i64;
    let mut points = Vec::with_capacity(cashflow.len());
    for point in cashflow {
        let cutoff_date = cutoff_date_for_month(&point.month, to);
        while cursor < balance_series.len() && balance_series[cursor].date <= cutoff_date {
            last_balance_minor = balance_series[cursor].balance_minor;
            cursor += 1;
        }
        points.push(MonthlyBalancePoint {
            month: point.month.clone(),
            date: cutoff_date,
            balance_minor: last_balance_minor,
        });
    }
    Ok(points)
}

fn ytd_profit_by_month(
    cashflow: &[MonthlyCashflowPoint],
    tax_year_start_month: u32,
) -> BTreeMap<String, i64> {
    let mut values = BTreeMap::new();
    let mut running_profit_minor = 0i64;

    for (index, point) in cashflow.iter().enumerate() {
        let month_num = parse_year_month(&point.month).map(|(_, month)| month);
        if index == 0 || month_num == Some(tax_year_start_month) {
            running_profit_minor = 0;
        }
        running_profit_minor += point.net_minor;
        values.insert(point.month.clone(), running_profit_minor);
    }

    values
}

fn runway_months_for_balance(balance_minor: i64, burn_rate_minor: i64) -> f64 {
    if burn_rate_minor <= 0 {
        999.0
    } else {
        (balance_minor as f64) / (burn_rate_minor as f64)
    }
}

fn cashflow_burn_method(config: &FinConfig) -> String {
    config.burn_rate_method()
}

fn tax_rate_for_group(config: &FinConfig, group_id: &str) -> f64 {
    let metadata = config.resolve_group_metadata(group_id);
    match metadata.tax_type.as_str() {
        "corp" => config
            .financial
            .get("corp_tax_rate")
            .and_then(toml::Value::as_float)
            .unwrap_or(0.25),
        "income" => config
            .financial
            .get("personal_income_tax_rate")
            .and_then(toml::Value::as_float)
            .unwrap_or(0.2),
        _ => 0.0,
    }
}

fn tax_reserve_basis_kind_for_rate(tax_rate: f64) -> TaxReserveBasisKind {
    if tax_rate > 0.0 {
        TaxReserveBasisKind::YtdProfit
    } else {
        TaxReserveBasisKind::None
    }
}

fn tax_reserve_basis_description(config: &FinConfig, group_id: &str, tax_rate: f64) -> String {
    let tax_type = config.resolve_group_metadata(group_id).tax_type;
    if tax_rate <= 0.0 {
        return "no tax reserve".to_owned();
    }
    let rate_label = match tax_type.as_str() {
        "corp" => "corp_tax_rate",
        "income" => "personal_income_tax_rate",
        _ => "tax_rate",
    };
    format!("max(ytd_profit_minor, 0) x {rate_label}={tax_rate:.4}")
}

fn recurring_baseline_minor(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    to: Option<&str>,
    lookback_months: usize,
) -> Result<i64> {
    let report = report_burn(
        connection,
        config,
        &[group_id.to_owned()],
        &BurnReportOptions {
            months: lookback_months,
            from: None,
            to,
            ownership_mode: OwnershipMode::Gross,
            include_partial_month: false,
        },
    )?;
    Ok(report.recurring_baseline.monthly_equivalent_minor.max(0))
}

fn expense_reserve_monthly_basis_minor(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    to: Option<&str>,
    policy: &ResolvedReservePolicy,
    median_expense_minor: i64,
) -> Result<i64> {
    match policy.expense_basis {
        ExpenseReserveBasis::HistoricalMedianExpense => Ok(median_expense_minor.max(0)),
        ExpenseReserveBasis::RecurringBaseline => recurring_baseline_minor(
            connection,
            config,
            group_id,
            to,
            policy.lookback_months.unwrap_or(6),
        ),
    }
}

fn build_group_report_context(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
    months: usize,
    reserve_mode: Option<ReserveMode>,
) -> Result<GroupReportContext> {
    let cashflow = group_monthly_cashflow(connection, config, group_id, from, to, months)?;
    let balance_points =
        actual_monthly_balance_points(connection, config, group_id, &cashflow, to)?;
    let burn_rate_method = cashflow_burn_method(config);
    let expenses = cashflow
        .iter()
        .map(|point| point.expense_minor)
        .collect::<Vec<_>>();
    let median_expense_minor = median_i64(&expenses).unwrap_or(0);
    let reserve_policy = config.resolve_reserve_policy(group_id, reserve_mode);
    let expense_reserve_monthly_basis_minor = expense_reserve_monthly_basis_minor(
        connection,
        config,
        group_id,
        to,
        &reserve_policy,
        median_expense_minor,
    )?;
    let expense_reserve_minor = ((expense_reserve_monthly_basis_minor as f64)
        * reserve_policy.expense_months
        * reserve_policy.factor)
        .round() as i64;
    let tax_rate = tax_rate_for_group(config, group_id);
    let cashflow_totals = CashflowTotals {
        income_minor: cashflow.iter().map(|point| point.income_minor).sum(),
        expense_minor: cashflow.iter().map(|point| point.expense_minor).sum(),
        net_minor: cashflow.iter().map(|point| point.net_minor).sum(),
    };

    Ok(GroupReportContext {
        balance_points,
        fallback_balance_minor: if cashflow.is_empty() {
            Some(group_total_balance(connection, config, group_id, to)?)
        } else {
            None
        },
        reserve_policy,
        expense_reserve_monthly_basis_minor,
        expense_reserve_minor: expense_reserve_minor.max(0),
        tax_reserve_basis_kind: tax_reserve_basis_kind_for_rate(tax_rate),
        tax_reserve_basis_description: tax_reserve_basis_description(config, group_id, tax_rate),
        tax_rate,
        tax_year_start_month: config
            .tax_year_start_month(config.resolve_group_metadata(group_id).tax_type.as_str()),
        burn_rate_minor: burn_rate(&expenses, &burn_rate_method),
        median_expense_minor,
        cashflow,
        cashflow_totals,
    })
}

fn derive_health_points(context: &GroupReportContext) -> Vec<HealthPoint> {
    let mut points = Vec::with_capacity(context.balance_points.len());
    for point in &context.balance_points {
        points.push(HealthPoint {
            date: point.date.clone(),
            balance_minor: point.balance_minor,
            health_minor: point.balance_minor - context.expense_reserve_minor,
            reserve_mode: context.reserve_policy.reserve_mode,
            expense_reserve_basis_kind: context.reserve_policy.expense_basis,
            expense_reserve_monthly_basis_minor: context.expense_reserve_monthly_basis_minor,
            expense_reserve_months: context.reserve_policy.expense_months,
            expense_reserve_factor: context.reserve_policy.factor,
            expense_reserve_lookback_months: context.reserve_policy.lookback_months,
            expense_reserve_minor: context.expense_reserve_minor,
        });
    }
    points
}

fn derive_runway_points(context: &GroupReportContext) -> Vec<RunwayPoint> {
    let mut points = Vec::with_capacity(context.balance_points.len().max(1));
    for point in &context.balance_points {
        points.push(RunwayPoint {
            date: point.date.clone(),
            runway_months: runway_months_for_balance(point.balance_minor, context.burn_rate_minor),
            balance_minor: point.balance_minor,
            burn_rate_minor: context.burn_rate_minor,
            median_expense_minor: context.median_expense_minor,
        });
    }
    if points.is_empty() {
        let balance_minor = context.fallback_balance_minor.unwrap_or_default();
        points.push(RunwayPoint {
            date: "now".to_owned(),
            runway_months: runway_months_for_balance(balance_minor, context.burn_rate_minor),
            balance_minor,
            burn_rate_minor: context.burn_rate_minor,
            median_expense_minor: context.median_expense_minor,
        });
    }
    points
}

fn derive_reserve_points(context: &GroupReportContext) -> Vec<ReserveBreakdownPoint> {
    let ytd_profit = ytd_profit_by_month(&context.cashflow, context.tax_year_start_month);
    let mut points = Vec::with_capacity(context.balance_points.len());
    for point in &context.balance_points {
        let tax_reserve_minor = ((i64::max(*ytd_profit.get(&point.month).unwrap_or(&0), 0) as f64)
            * context.tax_rate)
            .round() as i64;
        points.push(ReserveBreakdownPoint {
            date: point.date.clone(),
            reserve_mode: context.reserve_policy.reserve_mode,
            expense_reserve_basis_kind: context.reserve_policy.expense_basis,
            expense_reserve_monthly_basis_minor: context.expense_reserve_monthly_basis_minor,
            expense_reserve_months: context.reserve_policy.expense_months,
            expense_reserve_factor: context.reserve_policy.factor,
            expense_reserve_lookback_months: context.reserve_policy.lookback_months,
            tax_reserve_basis_kind: context.tax_reserve_basis_kind,
            tax_reserve_basis_description: context.tax_reserve_basis_description.clone(),
            balance_minor: point.balance_minor,
            tax_reserve_minor,
            expense_reserve_minor: context.expense_reserve_minor,
            available_minor: point.balance_minor
                - context.expense_reserve_minor
                - tax_reserve_minor,
        });
    }
    points
}

fn summarize_latest_group_metrics(context: &GroupReportContext) -> GroupSummaryLatestMetrics {
    let mut latest = GroupSummaryLatestMetrics::default();
    let ytd_profit = ytd_profit_by_month(&context.cashflow, context.tax_year_start_month);

    for point in &context.balance_points {
        let tax_reserve_minor = ((i64::max(*ytd_profit.get(&point.month).unwrap_or(&0), 0) as f64)
            * context.tax_rate)
            .round() as i64;
        latest.latest_health_minor = Some(point.balance_minor - context.expense_reserve_minor);
        latest.latest_available_minor =
            Some(point.balance_minor - context.expense_reserve_minor - tax_reserve_minor);
        latest.latest_runway_months = Some(runway_months_for_balance(
            point.balance_minor,
            context.burn_rate_minor,
        ));
    }

    if latest.latest_runway_months.is_none() {
        latest.latest_runway_months = Some(runway_months_for_balance(
            context.fallback_balance_minor.unwrap_or_default(),
            context.burn_rate_minor,
        ));
    }

    latest
}

pub fn report_cashflow(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    months: usize,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<(Vec<MonthlyCashflowPoint>, CashflowTotals)> {
    let context = build_group_report_context(connection, config, group_id, from, to, months, None)?;
    Ok((context.cashflow, context.cashflow_totals))
}

pub fn report_health(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<HealthPoint>> {
    report_health_with_mode(connection, config, group_id, from, to, None)
}

pub fn report_health_with_mode(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
    reserve_mode: Option<ReserveMode>,
) -> Result<Vec<HealthPoint>> {
    let context =
        build_group_report_context(connection, config, group_id, from, to, 120, reserve_mode)?;
    Ok(derive_health_points(&context))
}

pub fn report_runway(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<RunwayPoint>> {
    let context = build_group_report_context(connection, config, group_id, from, to, 120, None)?;
    Ok(derive_runway_points(&context))
}

pub fn report_reserves(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<ReserveBreakdownPoint>> {
    report_reserves_with_mode(connection, config, group_id, from, to, None)
}

pub fn report_reserves_with_mode(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
    reserve_mode: Option<ReserveMode>,
) -> Result<Vec<ReserveBreakdownPoint>> {
    let context =
        build_group_report_context(connection, config, group_id, from, to, 120, reserve_mode)?;
    Ok(derive_reserve_points(&context))
}

pub fn report_summary(
    connection: &Connection,
    config: &FinConfig,
    period_months: usize,
    to: Option<&str>,
) -> Result<SummaryReport> {
    let mut groups = BTreeMap::new();
    let current_month = reporting_month(connection, to)?;
    for group_id in all_group_ids(config) {
        let group_label = config.resolve_group_metadata(&group_id).label;
        let context =
            build_group_report_context(connection, config, &group_id, None, to, 120, None)?;
        let latest_metrics = summarize_latest_group_metrics(&context);
        let cashflow_kpis: CashflowKpis =
            summarize_cashflow_kpis(&context.cashflow, &current_month);
        groups.insert(
            group_id.clone(),
            GroupSummary {
                label: group_label,
                net_worth_minor: group_total_balance(connection, config, &group_id, to)?,
                latest_runway_months: latest_metrics.latest_runway_months,
                latest_health_minor: latest_metrics.latest_health_minor,
                latest_available_minor: latest_metrics.latest_available_minor,
                last_full_month_net_minor: cashflow_kpis
                    .last_full_month
                    .as_ref()
                    .map(|point| point.net_minor),
                trailing_average_net_minor: cashflow_kpis.trailing_average_net_minor,
                median_spend_minor: cashflow_kpis.median_spend_minor,
                short_term_trend: cashflow_kpis.short_term_trend,
                anomaly_count_last_12_months: cashflow_kpis.anomaly_count_last_12_months,
            },
        );
    }

    let balance_sheet = get_balance_sheet(connection, to)?;
    let net_worth_minor = groups.values().map(|group| group.net_worth_minor).sum();
    Ok(SummaryReport {
        generated_at: format!("{:?}", std::time::SystemTime::now()),
        period_months,
        groups,
        consolidated: ConsolidatedSummary {
            net_worth_minor,
            balance_sheet,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        report_cashflow, report_health, report_health_with_mode, report_reserves,
        report_reserves_with_mode, report_runway, report_summary,
    };
    use crate::burn::{BurnReportOptions, OwnershipMode, report_burn};
    use crate::config::{ExpenseReserveBasis, ReserveMode};
    use crate::dashboard::{current_reporting_month, summarize_cashflow_kpis};
    use crate::queries::view_accounts;
    use crate::runtime::{RuntimeContext, RuntimeContextOptions};
    use crate::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

    fn open_fixture_runtime() -> RuntimeContext {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");
        RuntimeContext::open(RuntimeContextOptions {
            config_path: Some(fixture.paths.config_path),
            db_path: Some(fixture.paths.db_path),
            create: false,
            ..RuntimeContextOptions::read_only()
        })
        .expect("open runtime")
    }

    fn open_fixture_runtime_with_config_append(suffix: &str) -> RuntimeContext {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");
        let mut raw = fs::read_to_string(&fixture.paths.config_path).expect("read config");
        raw.push('\n');
        raw.push_str(suffix);
        fs::write(&fixture.paths.config_path, raw).expect("write config");
        RuntimeContext::open(RuntimeContextOptions {
            config_path: Some(fixture.paths.config_path),
            db_path: Some(fixture.paths.db_path),
            create: false,
            ..RuntimeContextOptions::read_only()
        })
        .expect("open runtime")
    }

    #[test]
    fn report_cashflow_totals_match_returned_series() {
        let runtime = open_fixture_runtime();
        let (series, totals) = report_cashflow(
            runtime.connection(),
            runtime.config(),
            "business",
            24,
            None,
            Some("2026-03-31"),
        )
        .expect("report cashflow");

        assert_eq!(
            totals.income_minor,
            series.iter().map(|point| point.income_minor).sum::<i64>()
        );
        assert_eq!(
            totals.expense_minor,
            series.iter().map(|point| point.expense_minor).sum::<i64>()
        );
        assert_eq!(
            totals.net_minor,
            series.iter().map(|point| point.net_minor).sum::<i64>()
        );
    }

    #[test]
    fn summary_matches_individual_group_reports() {
        let runtime = open_fixture_runtime();
        let summary = report_summary(runtime.connection(), runtime.config(), 12, None)
            .expect("report summary");
        let current_month = current_reporting_month(runtime.connection()).expect("current month");

        for group_id in runtime.config().group_ids() {
            let group_summary = summary.groups.get(&group_id).expect("group summary");
            let runway = report_runway(
                runtime.connection(),
                runtime.config(),
                &group_id,
                None,
                None,
            )
            .expect("report runway");
            let health = report_health(
                runtime.connection(),
                runtime.config(),
                &group_id,
                None,
                None,
            )
            .expect("report health");
            let reserves = report_reserves(
                runtime.connection(),
                runtime.config(),
                &group_id,
                None,
                None,
            )
            .expect("report reserves");
            let (cashflow, _) = report_cashflow(
                runtime.connection(),
                runtime.config(),
                &group_id,
                120,
                None,
                None,
            )
            .expect("report cashflow");
            let cashflow_kpis = summarize_cashflow_kpis(&cashflow, &current_month);

            assert_eq!(
                group_summary.latest_runway_months,
                runway.last().map(|point| point.runway_months)
            );
            assert_eq!(
                group_summary.latest_health_minor,
                health.last().map(|point| point.health_minor)
            );
            assert_eq!(
                group_summary.latest_available_minor,
                reserves.last().map(|point| point.available_minor)
            );
            assert_eq!(
                group_summary.last_full_month_net_minor,
                cashflow_kpis
                    .last_full_month
                    .as_ref()
                    .map(|point| point.net_minor)
            );
            assert_eq!(
                group_summary.trailing_average_net_minor,
                cashflow_kpis.trailing_average_net_minor
            );
            assert_eq!(
                group_summary.median_spend_minor,
                cashflow_kpis.median_spend_minor
            );
            assert_eq!(
                group_summary.short_term_trend,
                cashflow_kpis.short_term_trend
            );
            assert_eq!(
                group_summary.anomaly_count_last_12_months,
                cashflow_kpis.anomaly_count_last_12_months
            );
        }
    }

    #[test]
    fn reserve_modes_switch_between_historical_and_recurring_baselines() {
        let runtime = open_fixture_runtime_with_config_append(
            r#"
[reserve.modes.recurring]
expense_basis = "recurring_baseline"
expense_months = 6
lookback_months = 6

[reserve.modes.aggressive]
expense_basis = "recurring_baseline"
expense_months = 3
lookback_months = 6

[reserve.groups.business.modes.conservative]
expense_months = 12
"#,
        );
        let group_id = "business";

        let conservative = report_reserves_with_mode(
            runtime.connection(),
            runtime.config(),
            group_id,
            None,
            Some("2026-03-14"),
            Some(ReserveMode::Conservative),
        )
        .expect("conservative reserves");
        let recurring = report_reserves_with_mode(
            runtime.connection(),
            runtime.config(),
            group_id,
            None,
            Some("2026-03-14"),
            Some(ReserveMode::Recurring),
        )
        .expect("recurring reserves");
        let aggressive = report_reserves_with_mode(
            runtime.connection(),
            runtime.config(),
            group_id,
            None,
            Some("2026-03-14"),
            Some(ReserveMode::Aggressive),
        )
        .expect("aggressive reserves");

        let conservative_latest = conservative.last().expect("conservative latest");
        let recurring_latest = recurring.last().expect("recurring latest");
        let aggressive_latest = aggressive.last().expect("aggressive latest");

        let runway = report_runway(
            runtime.connection(),
            runtime.config(),
            group_id,
            None,
            Some("2026-03-14"),
        )
        .expect("runway");
        let burn_lookback = runtime
            .config()
            .resolve_reserve_policy(group_id, Some(ReserveMode::Recurring))
            .lookback_months
            .unwrap_or(6);
        let burn = report_burn(
            runtime.connection(),
            runtime.config(),
            &[group_id.to_owned()],
            &BurnReportOptions {
                months: burn_lookback,
                from: None,
                to: Some("2026-03-14"),
                ownership_mode: OwnershipMode::Gross,
                include_partial_month: false,
            },
        )
        .expect("burn");

        assert_eq!(
            conservative_latest.expense_reserve_basis_kind,
            ExpenseReserveBasis::HistoricalMedianExpense
        );
        assert_eq!(
            conservative_latest.expense_reserve_monthly_basis_minor,
            runway
                .last()
                .map(|point| point.median_expense_minor)
                .unwrap_or_default()
        );
        assert_eq!(
            recurring_latest.expense_reserve_basis_kind,
            ExpenseReserveBasis::RecurringBaseline
        );
        assert_eq!(
            recurring_latest.expense_reserve_monthly_basis_minor,
            burn.recurring_baseline.monthly_equivalent_minor
        );
        assert!(recurring_latest.expense_reserve_minor < conservative_latest.expense_reserve_minor);
        assert!(aggressive_latest.expense_reserve_minor < recurring_latest.expense_reserve_minor);
    }

    #[test]
    fn health_mode_changes_follow_reserve_mode() {
        let runtime = open_fixture_runtime_with_config_append(
            r#"
[reserve.modes.recurring]
expense_basis = "recurring_baseline"
expense_months = 6
lookback_months = 6

[reserve.groups.business.modes.conservative]
expense_months = 12
"#,
        );
        let conservative = report_health_with_mode(
            runtime.connection(),
            runtime.config(),
            "business",
            None,
            Some("2026-03-14"),
            Some(ReserveMode::Conservative),
        )
        .expect("conservative health");
        let recurring = report_health_with_mode(
            runtime.connection(),
            runtime.config(),
            "business",
            None,
            Some("2026-03-14"),
            Some(ReserveMode::Recurring),
        )
        .expect("recurring health");

        let conservative_latest = conservative.last().expect("conservative latest");
        let recurring_latest = recurring.last().expect("recurring latest");

        assert_eq!(conservative_latest.reserve_mode, ReserveMode::Conservative);
        assert_eq!(recurring_latest.reserve_mode, ReserveMode::Recurring);
        assert!(recurring_latest.health_minor > conservative_latest.health_minor);
    }

    #[test]
    fn runway_fallback_uses_current_balance_when_filtered_history_is_empty() {
        let runtime = open_fixture_runtime();
        let group_id = "personal";
        let runway = report_runway(
            runtime.connection(),
            runtime.config(),
            group_id,
            Some("2099-01-01"),
            None,
        )
        .expect("report runway");
        let expected_balance =
            view_accounts(runtime.connection(), runtime.config(), Some(group_id))
                .expect("view accounts")
                .iter()
                .map(|account| account.balance_minor.unwrap_or(0))
                .sum::<i64>();

        assert_eq!(runway.len(), 1);
        assert_eq!(runway[0].date, "now");
        assert_eq!(runway[0].balance_minor, expected_balance);
    }
}
