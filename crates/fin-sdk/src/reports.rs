use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::FinConfig;
use crate::error::Result;
use crate::queries::{
    MonthlyCashflowPoint, all_group_ids, consolidated_net_worth_by_group, get_balance_sheet,
    group_monthly_cashflow, view_accounts,
};

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

fn median(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    if sorted.len() % 2 == 1 {
        sorted[sorted.len() / 2]
    } else {
        let left = sorted[(sorted.len() / 2) - 1];
        let right = sorted[sorted.len() / 2];
        (left + right) / 2
    }
}

fn mean(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    values.iter().sum::<i64>() / i64::try_from(values.len()).unwrap_or(1)
}

fn burn_rate(values: &[i64], method: &str) -> i64 {
    if method == "mean" {
        return mean(values);
    }
    median(values)
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

pub fn report_cashflow(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    months: usize,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<(Vec<MonthlyCashflowPoint>, CashflowTotals)> {
    let mut series = group_monthly_cashflow(connection, config, group_id, from, to, months)?;
    if months > 0 && series.len() > months {
        let start = series.len().saturating_sub(months);
        series = series[start..].to_vec();
    }
    let totals = CashflowTotals {
        income_minor: series.iter().map(|point| point.income_minor).sum(),
        expense_minor: series.iter().map(|point| point.expense_minor).sum(),
        net_minor: series.iter().map(|point| point.net_minor).sum(),
    };
    Ok((series, totals))
}

pub fn report_health(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<HealthPoint>> {
    let series = group_monthly_cashflow(connection, config, group_id, from, to, 120)?;
    let reserve_months = i64::from(
        config
            .resolve_group_metadata(group_id)
            .expense_reserve_months,
    );
    let expenses = series
        .iter()
        .map(|point| point.expense_minor)
        .collect::<Vec<_>>();
    let typical_expense = median(&expenses);
    let reserve = typical_expense * reserve_months;
    let mut balance = 0i64;
    let points = series
        .into_iter()
        .map(|point| {
            balance += point.net_minor;
            HealthPoint {
                date: parse_date(&point.month),
                health_minor: balance - reserve,
            }
        })
        .collect();
    Ok(points)
}

pub fn report_runway(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<RunwayPoint>> {
    let series = group_monthly_cashflow(connection, config, group_id, from, to, 120)?;
    let burn_values = series
        .iter()
        .map(|point| point.expense_minor)
        .collect::<Vec<_>>();
    let burn_rate_minor = burn_rate(
        &burn_values,
        config
            .financial
            .get("burn_rate_method")
            .and_then(toml::Value::as_str)
            .unwrap_or("median"),
    );
    let median_expense_minor = median(&burn_values);
    let mut rolling_balance = 0i64;
    let points = series
        .into_iter()
        .map(|point| {
            rolling_balance += point.net_minor;
            let runway_months = if burn_rate_minor <= 0 {
                999.0
            } else {
                (rolling_balance as f64) / (burn_rate_minor as f64)
            };
            RunwayPoint {
                date: parse_date(&point.month),
                runway_months,
                balance_minor: rolling_balance,
                burn_rate_minor,
                median_expense_minor,
            }
        })
        .collect::<Vec<_>>();
    if !points.is_empty() {
        return Ok(points);
    }
    let current_balance = group_total_balance(connection, config, group_id)?;
    let runway_months = if burn_rate_minor <= 0 {
        999.0
    } else {
        (current_balance as f64) / (burn_rate_minor as f64)
    };
    Ok(vec![RunwayPoint {
        date: "now".to_owned(),
        runway_months,
        balance_minor: current_balance,
        burn_rate_minor,
        median_expense_minor,
    }])
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

pub fn report_reserves(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<Vec<ReserveBreakdownPoint>> {
    let series = group_monthly_cashflow(connection, config, group_id, from, to, 120)?;
    let reserve_months = i64::from(
        config
            .resolve_group_metadata(group_id)
            .expense_reserve_months,
    );
    let tax_rate = tax_rate_for_group(config, group_id);
    let expenses = series
        .iter()
        .map(|point| point.expense_minor)
        .collect::<Vec<_>>();
    let typical_expense = median(&expenses);
    let mut ytd_profit = 0i64;
    let mut balance = 0i64;
    let points = series
        .into_iter()
        .map(|point| {
            balance += point.net_minor;
            ytd_profit += point.net_minor;
            let expense_reserve_minor = typical_expense * reserve_months;
            let tax_reserve_minor = ((i64::max(ytd_profit, 0) as f64) * tax_rate).round() as i64;
            let available_minor = balance - expense_reserve_minor - tax_reserve_minor;
            ReserveBreakdownPoint {
                date: parse_date(&point.month),
                balance_minor: balance,
                tax_reserve_minor,
                expense_reserve_minor,
                available_minor,
            }
        })
        .collect::<Vec<_>>();
    Ok(points)
}

pub fn report_summary(
    connection: &Connection,
    config: &FinConfig,
    period_months: usize,
) -> Result<SummaryReport> {
    let mut groups = BTreeMap::new();
    let by_group_net_worth = consolidated_net_worth_by_group(connection, config)?;
    for group_id in all_group_ids(config) {
        let group_label = config.resolve_group_metadata(&group_id).label;
        let runway = report_runway(connection, config, &group_id, None, None)?;
        let health = report_health(connection, config, &group_id, None, None)?;
        let reserves = report_reserves(connection, config, &group_id, None, None)?;
        groups.insert(
            group_id.clone(),
            GroupSummary {
                label: group_label,
                net_worth_minor: by_group_net_worth.get(&group_id).copied().unwrap_or(0),
                latest_runway_months: runway.last().map(|point| point.runway_months),
                latest_health_minor: health.last().map(|point| point.health_minor),
                latest_available_minor: reserves.last().map(|point| point.available_minor),
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
