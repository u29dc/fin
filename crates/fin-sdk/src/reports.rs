use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::FinConfig;
use crate::dashboard::{
    CashflowKpis, ShortTermTrend, current_reporting_month, summarize_cashflow_kpis,
};
use crate::error::Result;
use crate::queries::{
    MonthlyCashflowPoint, all_group_ids, consolidated_net_worth_by_group, get_balance_sheet,
    group_monthly_cashflow, view_accounts,
};
use crate::stats::{mean_i64, median_i64};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashflowTotals {
    pub income_minor: i64,
    pub expense_minor: i64,
    pub net_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthPoint {
    pub date: String,
    pub health_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunwayPoint {
    pub date: String,
    pub runway_months: f64,
    pub balance_minor: i64,
    pub burn_rate_minor: i64,
    pub median_expense_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveBreakdownPoint {
    pub date: String,
    pub balance_minor: i64,
    pub tax_reserve_minor: i64,
    pub expense_reserve_minor: i64,
    pub available_minor: i64,
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
    cashflow_totals: CashflowTotals,
    expense_reserve_minor: i64,
    tax_rate: f64,
    burn_rate_minor: i64,
    median_expense_minor: i64,
    fallback_balance_minor: Option<i64>,
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

fn group_total_balance(connection: &Connection, config: &FinConfig, group_id: &str) -> Result<i64> {
    let accounts = view_accounts(connection, config, Some(group_id))?;
    Ok(accounts
        .iter()
        .map(|account| account.balance_minor.unwrap_or(0))
        .sum())
}

fn parse_date(month: &str) -> String {
    format!("{month}-01")
}

fn runway_months_for_balance(balance_minor: i64, burn_rate_minor: i64) -> f64 {
    if burn_rate_minor <= 0 {
        999.0
    } else {
        (balance_minor as f64) / (burn_rate_minor as f64)
    }
}

fn cashflow_burn_method(config: &FinConfig) -> &str {
    config
        .financial
        .get("burn_rate_method")
        .and_then(toml::Value::as_str)
        .unwrap_or("median")
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

fn build_group_report_context(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
    months: usize,
) -> Result<GroupReportContext> {
    let cashflow = group_monthly_cashflow(connection, config, group_id, from, to, months)?;
    let reserve_months = i64::from(
        config
            .resolve_group_metadata(group_id)
            .expense_reserve_months,
    );
    let expenses = cashflow
        .iter()
        .map(|point| point.expense_minor)
        .collect::<Vec<_>>();
    let median_expense_minor = median_i64(&expenses).unwrap_or(0);
    let cashflow_totals = CashflowTotals {
        income_minor: cashflow.iter().map(|point| point.income_minor).sum(),
        expense_minor: cashflow.iter().map(|point| point.expense_minor).sum(),
        net_minor: cashflow.iter().map(|point| point.net_minor).sum(),
    };

    Ok(GroupReportContext {
        fallback_balance_minor: if cashflow.is_empty() {
            Some(group_total_balance(connection, config, group_id)?)
        } else {
            None
        },
        expense_reserve_minor: median_expense_minor * reserve_months,
        tax_rate: tax_rate_for_group(config, group_id),
        burn_rate_minor: burn_rate(&expenses, cashflow_burn_method(config)),
        median_expense_minor,
        cashflow,
        cashflow_totals,
    })
}

fn derive_health_points(context: &GroupReportContext) -> Vec<HealthPoint> {
    let mut balance = 0i64;
    let mut points = Vec::with_capacity(context.cashflow.len());
    for point in &context.cashflow {
        balance += point.net_minor;
        points.push(HealthPoint {
            date: parse_date(&point.month),
            health_minor: balance - context.expense_reserve_minor,
        });
    }
    points
}

fn derive_runway_points(context: &GroupReportContext) -> Vec<RunwayPoint> {
    let mut balance = 0i64;
    let mut points = Vec::with_capacity(context.cashflow.len().max(1));
    for point in &context.cashflow {
        balance += point.net_minor;
        points.push(RunwayPoint {
            date: parse_date(&point.month),
            runway_months: runway_months_for_balance(balance, context.burn_rate_minor),
            balance_minor: balance,
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
    let mut balance = 0i64;
    let mut ytd_profit = 0i64;
    let mut points = Vec::with_capacity(context.cashflow.len());
    for point in &context.cashflow {
        balance += point.net_minor;
        ytd_profit += point.net_minor;
        let tax_reserve_minor =
            ((i64::max(ytd_profit, 0) as f64) * context.tax_rate).round() as i64;
        points.push(ReserveBreakdownPoint {
            date: parse_date(&point.month),
            balance_minor: balance,
            tax_reserve_minor,
            expense_reserve_minor: context.expense_reserve_minor,
            available_minor: balance - context.expense_reserve_minor - tax_reserve_minor,
        });
    }
    points
}

fn summarize_latest_group_metrics(context: &GroupReportContext) -> GroupSummaryLatestMetrics {
    let mut balance = 0i64;
    let mut ytd_profit = 0i64;
    let mut latest = GroupSummaryLatestMetrics::default();

    for point in &context.cashflow {
        balance += point.net_minor;
        ytd_profit += point.net_minor;
        let tax_reserve_minor =
            ((i64::max(ytd_profit, 0) as f64) * context.tax_rate).round() as i64;
        latest.latest_health_minor = Some(balance - context.expense_reserve_minor);
        latest.latest_available_minor =
            Some(balance - context.expense_reserve_minor - tax_reserve_minor);
        latest.latest_runway_months =
            Some(runway_months_for_balance(balance, context.burn_rate_minor));
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
    let context = build_group_report_context(connection, config, group_id, from, to, months)?;
    Ok((context.cashflow, context.cashflow_totals))
}

pub fn report_health(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<HealthPoint>> {
    let context = build_group_report_context(connection, config, group_id, from, to, 120)?;
    Ok(derive_health_points(&context))
}

pub fn report_runway(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<RunwayPoint>> {
    let context = build_group_report_context(connection, config, group_id, from, to, 120)?;
    Ok(derive_runway_points(&context))
}

pub fn report_reserves(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<ReserveBreakdownPoint>> {
    let context = build_group_report_context(connection, config, group_id, from, to, 120)?;
    Ok(derive_reserve_points(&context))
}

pub fn report_summary(
    connection: &Connection,
    config: &FinConfig,
    period_months: usize,
) -> Result<SummaryReport> {
    let mut groups = BTreeMap::new();
    let by_group_net_worth = consolidated_net_worth_by_group(connection, config)?;
    let current_month = current_reporting_month(connection)?;
    for group_id in all_group_ids(config) {
        let group_label = config.resolve_group_metadata(&group_id).label;
        let context = build_group_report_context(connection, config, &group_id, None, None, 120)?;
        let latest_metrics = summarize_latest_group_metrics(&context);
        let cashflow_kpis: CashflowKpis =
            summarize_cashflow_kpis(&context.cashflow, &current_month);
        groups.insert(
            group_id.clone(),
            GroupSummary {
                label: group_label,
                net_worth_minor: by_group_net_worth.get(&group_id).copied().unwrap_or(0),
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

    let balance_sheet = get_balance_sheet(connection, None)?;
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
    use tempfile::tempdir;

    use super::{report_cashflow, report_health, report_reserves, report_runway, report_summary};
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
        let summary =
            report_summary(runtime.connection(), runtime.config(), 12).expect("report summary");
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
