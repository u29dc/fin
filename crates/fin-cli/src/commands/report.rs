use std::collections::BTreeMap;

use serde_json::json;

use fin_sdk::config::load_config;
use fin_sdk::db::{OpenDatabaseOptions, open_database, resolve_db_path};
use fin_sdk::{
    audit_payees, group_category_breakdown, group_category_monthly_median, report_cashflow,
    report_health, report_reserves, report_runway, report_summary, view_accounts,
};

use crate::commands::{CommandFailure, CommandResult, map_fin_error};
use crate::envelope::MetaExtras;
use crate::error::ExitCode;

fn resolve_db(
    tool: &'static str,
    explicit_db: Option<&str>,
) -> Result<(rusqlite::Connection, fin_sdk::config::LoadedConfig), CommandFailure> {
    let loaded = load_config(None).map_err(|error| map_fin_error(tool, error))?;
    let db_path = resolve_db_path(
        explicit_db.map(std::path::Path::new),
        Some(&loaded.config_dir()),
    );
    let connection = open_database(OpenDatabaseOptions {
        path: Some(db_path),
        config_dir: Some(loaded.config_dir()),
        readonly: true,
        create: true,
        migrate: true,
    })
    .map_err(|error| map_fin_error(tool, error))?;
    Ok((connection, loaded))
}

pub fn run_cashflow(
    db: Option<&str>,
    group: &str,
    months: usize,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let (connection, loaded) = resolve_db("report.cashflow", db)?;
    let (series, totals) = report_cashflow(&connection, &loaded.config, group, months, from, to)
        .map_err(|error| map_fin_error("report.cashflow", error))?;
    let rows = series
        .iter()
        .map(|point| {
            json!({
                "month": point.month,
                "income": point.income_minor,
                "expenses": point.expense_minor,
                "net": point.net_minor,
                "savingsRate": point.savings_rate_pct,
            })
        })
        .collect::<Vec<_>>();
    Ok(CommandResult {
        tool: "report.cashflow",
        data: json!({
            "series": rows,
            "totals": {
                "income": totals.income_minor,
                "expenses": totals.expense_minor,
                "net": totals.net_minor,
            }
        }),
        text: format!("{} months | net={}", rows.len(), totals.net_minor),
        meta: MetaExtras {
            count: Some(rows.len()),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_health(
    db: Option<&str>,
    group: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let (connection, loaded) = resolve_db("report.health", db)?;
    let series = report_health(&connection, &loaded.config, group, from, to)
        .map_err(|error| map_fin_error("report.health", error))?;
    let latest = series.last().cloned();
    Ok(CommandResult {
        tool: "report.health",
        data: json!({
            "series": series,
            "latest": latest,
        }),
        text: format!("{} points", series.len()),
        meta: MetaExtras {
            count: Some(series.len()),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_runway(
    db: Option<&str>,
    group: Option<&str>,
    consolidated: bool,
    include: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let (connection, loaded) = resolve_db("report.runway", db)?;
    if consolidated {
        if include.map(str::trim).is_none_or(str::is_empty) {
            return Err(map_fin_error(
                "report.runway",
                fin_sdk::FinError::InvalidInput {
                    code: "INVALID_INCLUDE",
                    message: "Missing include groups for consolidated runway".to_owned(),
                },
            ));
        }
        let include_groups = include
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>();
        let groups = if include_groups.is_empty() {
            loaded.config.group_ids()
        } else {
            include_groups
        };

        let mut merged = BTreeMap::<String, (i64, i64, f64)>::new();
        for group_id in &groups {
            let points = report_runway(&connection, &loaded.config, group_id, from, to)
                .map_err(|error| map_fin_error("report.runway", error))?;
            for point in points {
                let slot = merged.entry(point.date.clone()).or_insert((0, 0, 0.0));
                slot.0 += point.balance_minor;
                slot.1 += point.burn_rate_minor;
                slot.2 = if slot.1 <= 0 {
                    999.0
                } else {
                    (slot.0 as f64) / (slot.1 as f64)
                };
            }
        }
        let series = merged
            .into_iter()
            .map(|(date, (balance_minor, burn_rate_minor, runway_months))| {
                json!({
                    "date": date,
                    "balance": balance_minor,
                    "burnRate": burn_rate_minor,
                    "runway": runway_months,
                })
            })
            .collect::<Vec<_>>();
        let latest = series.last().cloned();
        return Ok(CommandResult {
            tool: "report.runway",
            data: json!({
                "series": series,
                "latest": latest,
                "groups": groups,
            }),
            text: "consolidated runway".to_owned(),
            meta: MetaExtras {
                count: Some(series.len()),
                total: None,
                has_more: None,
            },
            exit_code: ExitCode::Success,
        });
    }

    let group = group.ok_or_else(|| {
        map_fin_error(
            "report.runway",
            fin_sdk::FinError::InvalidInput {
                code: "INVALID_GROUP",
                message: "Missing group".to_owned(),
            },
        )
    })?;
    let raw_series = report_runway(&connection, &loaded.config, group, from, to)
        .map_err(|error| map_fin_error("report.runway", error))?;

    // Preserve legacy semantics: no historic burn/cashflow data means no series points.
    let has_only_placeholder = raw_series.len() == 1
        && raw_series[0].date == "now"
        && raw_series[0].burn_rate_minor == 0
        && raw_series[0].median_expense_minor == 0;

    let series = if has_only_placeholder {
        Vec::new()
    } else {
        raw_series
            .iter()
            .map(|point| {
                json!({
                    "date": point.date,
                    "runway": point.runway_months,
                })
            })
            .collect::<Vec<_>>()
    };
    let latest = series.last().cloned();

    Ok(CommandResult {
        tool: "report.runway",
        data: json!({
            "series": series,
            "latest": latest,
        }),
        text: format!("{} points", series.len()),
        meta: MetaExtras {
            count: Some(series.len()),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_reserves(
    db: Option<&str>,
    group: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let (connection, loaded) = resolve_db("report.reserves", db)?;
    let series = report_reserves(&connection, &loaded.config, group, from, to)
        .map_err(|error| map_fin_error("report.reserves", error))?;
    let latest = series.last().cloned();
    Ok(CommandResult {
        tool: "report.reserves",
        data: json!({
            "series": series,
            "latest": latest,
        }),
        text: format!("{} points", series.len()),
        meta: MetaExtras {
            count: Some(series.len()),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_categories(
    db: Option<&str>,
    group: &str,
    mode: &str,
    months: usize,
    limit: usize,
) -> Result<CommandResult, CommandFailure> {
    let (connection, loaded) = resolve_db("report.categories", db)?;
    if mode == "median" {
        let points =
            group_category_monthly_median(&connection, &loaded.config, group, months, limit)
                .map_err(|error| map_fin_error("report.categories", error))?;
        let categories = points
            .iter()
            .map(|point| {
                json!({
                    "category": point.category,
                    "median": point.monthly_median_minor,
                    "months": point.month_count,
                })
            })
            .collect::<Vec<_>>();
        let estimated_monthly = points
            .iter()
            .map(|point| point.monthly_median_minor)
            .sum::<i64>();
        return Ok(CommandResult {
            tool: "report.categories",
            data: json!({
                "categories": categories,
                "estimatedMonthly": estimated_monthly,
            }),
            text: format!("{} categories", points.len()),
            meta: MetaExtras {
                count: Some(points.len()),
                total: None,
                has_more: None,
            },
            exit_code: ExitCode::Success,
        });
    }

    let points = group_category_breakdown(&connection, &loaded.config, group, months, limit)
        .map_err(|error| map_fin_error("report.categories", error))?;
    let categories = points
        .iter()
        .map(|point| {
            json!({
                "category": point.category,
                "total": point.total_minor,
                "count": point.transaction_count,
            })
        })
        .collect::<Vec<_>>();
    let total = points.iter().map(|point| point.total_minor).sum::<i64>();
    Ok(CommandResult {
        tool: "report.categories",
        data: json!({
            "categories": categories,
            "total": total,
        }),
        text: format!("{} categories", points.len()),
        meta: MetaExtras {
            count: Some(points.len()),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_audit(
    db: Option<&str>,
    account: &str,
    months: usize,
    limit: usize,
) -> Result<CommandResult, CommandFailure> {
    let (connection, _) = resolve_db("report.audit", db)?;
    let points = audit_payees(&connection, account, months, limit)
        .map_err(|error| map_fin_error("report.audit", error))?;
    let total_minor = points.iter().map(|point| point.total_minor).sum::<i64>();
    Ok(CommandResult {
        tool: "report.audit",
        data: json!({
            "payees": points,
            "total": total_minor,
        }),
        text: format!("{} payees", points.len()),
        meta: MetaExtras {
            count: Some(points.len()),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_summary(db: Option<&str>, months: usize) -> Result<CommandResult, CommandFailure> {
    let (connection, loaded) = resolve_db("report.summary", db)?;
    let report = report_summary(&connection, &loaded.config, months)
        .map_err(|error| map_fin_error("report.summary", error))?;

    let mut groups = Vec::new();
    for group_id in loaded.config.group_ids() {
        let summary = report.groups.get(&group_id);
        let accounts = view_accounts(&connection, &loaded.config, Some(&group_id))
            .map_err(|error| map_fin_error("report.summary", error))?;
        let balances = accounts
            .into_iter()
            .map(|account| {
                json!({
                    "account": account.id,
                    "balance": account.balance_minor,
                })
            })
            .collect::<Vec<_>>();
        groups.push(json!({
            "id": group_id,
            "label": summary.map(|value| value.label.clone()).unwrap_or_else(|| loaded.config.resolve_group_metadata(&group_id).label),
            "balances": balances,
            "snapshot": {
                "runway": summary.and_then(|value| value.latest_runway_months),
                "lastMonthNet": null,
                "netWorth": summary.map(|value| value.net_worth_minor).unwrap_or(0),
                "medianSpend": null,
            },
            "assetAllocation": [],
            "lastMonth": null,
            "cashflow": [],
            "incomeSources": [],
            "expenseSinks": [],
            "expenseTree": [],
        }));
    }

    let balance_sheet = &report.consolidated.balance_sheet;
    let generated_at = report.generated_at.clone();

    let text = format!(
        "Summary: {} groups, consolidated net worth {}",
        groups.len(),
        report.consolidated.net_worth_minor
    );

    Ok(CommandResult {
        tool: "report.summary",
        data: json!({
            "generatedAt": generated_at,
            "periodMonths": report.period_months,
            "currency": "GBP",
            "groups": groups,
            "consolidated": {
                "totalBalance": report.consolidated.net_worth_minor,
                "burnRate": null,
                "runway": null,
                "expenseTree": [],
            },
            "balanceSheet": {
                "assets": balance_sheet.assets,
                "liabilities": balance_sheet.liabilities,
                "netWorth": balance_sheet.net_worth,
                "income": balance_sheet.income,
                "expenses": balance_sheet.expenses,
                "netIncome": balance_sheet.net_income,
                "equity": balance_sheet.equity,
            },
        }),
        text,
        meta: MetaExtras::default(),
        exit_code: ExitCode::Success,
    })
}
