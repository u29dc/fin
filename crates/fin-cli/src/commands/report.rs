use std::collections::BTreeMap;

use serde_json::json;

use fin_sdk::{
    BurnReportOptions, OwnershipMode, ReserveMode, TwoPoolRunwayOptions, TwoPoolScenarioKind,
    audit_payees, group_category_breakdown, group_category_monthly_median, report_burn,
    report_cashflow, report_health_with_mode, report_reserves_with_mode, report_runway,
    report_summary, report_two_pool_runway, view_accounts,
};

use crate::commands::{CommandFailure, CommandResult, map_fin_error, open_runtime};
use crate::envelope::MetaExtras;
use crate::error::ExitCode;

pub struct RunwayCommandArgs<'a> {
    pub db: Option<&'a str>,
    pub group: Option<&'a str>,
    pub consolidated: bool,
    pub include: Option<&'a str>,
    pub months: usize,
    pub mode: &'a str,
    pub scenario: &'a str,
    pub ownership_mode: &'a str,
    pub reserve_mode: Option<&'a str>,
    pub salary_monthly_minor: Option<i64>,
    pub dividends_monthly_minor: Option<i64>,
    pub include_joint_expenses: Option<bool>,
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
}

fn parse_reserve_mode(
    tool: &'static str,
    reserve_mode: Option<&str>,
) -> Result<Option<ReserveMode>, CommandFailure> {
    reserve_mode
        .map(|value| {
            value.parse::<ReserveMode>().map_err(|message| {
                map_fin_error(
                    tool,
                    fin_sdk::FinError::InvalidInput {
                        code: "INVALID_RESERVE_MODE",
                        message,
                    },
                )
            })
        })
        .transpose()
}

pub fn run_cashflow(
    db: Option<&str>,
    group: &str,
    months: usize,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("report.cashflow", db, true)?;
    let (series, totals) = report_cashflow(
        runtime.connection(),
        runtime.config(),
        group,
        months,
        from,
        to,
    )
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

pub fn run_burn(
    db: Option<&str>,
    include: Option<&str>,
    months: usize,
    from: Option<&str>,
    to: Option<&str>,
    include_partial_month: bool,
    ownership_mode: &str,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("report.burn", db, true)?;
    let groups = include
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let ownership_mode = ownership_mode.parse::<OwnershipMode>().map_err(|message| {
        map_fin_error(
            "report.burn",
            fin_sdk::FinError::InvalidInput {
                code: "INVALID_OWNERSHIP_MODE",
                message,
            },
        )
    })?;
    let report = report_burn(
        runtime.connection(),
        runtime.config(),
        &groups,
        &BurnReportOptions {
            months,
            from,
            to,
            ownership_mode,
            include_partial_month,
        },
    )
    .map_err(|error| map_fin_error("report.burn", error))?;

    Ok(CommandResult {
        tool: "report.burn",
        data: json!({
            "fromDate": report.from_date,
            "toDate": report.to_date,
            "requestedToDate": report.requested_to_date,
            "windowMode": report.window_mode,
            "includesPartialMonth": report.includes_partial_month,
            "ownershipMode": report.ownership_mode,
            "groups": report.groups,
            "groupTotals": report.group_totals,
            "recurringBaseline": report.recurring_baseline,
            "periodicObligations": report.periodic_obligations,
            "nonRecurring": report.non_recurring,
            "vatPassThrough": report.vat_pass_through,
            "transfersExcluded": report.transfers_excluded,
            "periodicItems": report.periodic_items,
            "nonRecurringItems": report.non_recurring_items,
            "monthlySeries": report.monthly_series,
            "confidence": report.confidence,
        }),
        text: format!(
            "burn | baseline={} periodic={} non_recurring={} vat={} transfers_excluded={}",
            report.recurring_baseline.monthly_equivalent_minor,
            report.periodic_obligations.monthly_equivalent_minor,
            report.non_recurring.monthly_equivalent_minor,
            report.vat_pass_through.monthly_equivalent_minor,
            report.transfers_excluded.monthly_equivalent_minor,
        ),
        meta: MetaExtras::default(),
        exit_code: ExitCode::Success,
    })
}

pub fn run_health(
    db: Option<&str>,
    group: &str,
    reserve_mode: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("report.health", db, true)?;
    let reserve_mode = parse_reserve_mode("report.health", reserve_mode)?;
    let series = report_health_with_mode(
        runtime.connection(),
        runtime.config(),
        group,
        from,
        to,
        reserve_mode,
    )
    .map_err(|error| map_fin_error("report.health", error))?;
    let reserve_series = report_reserves_with_mode(
        runtime.connection(),
        runtime.config(),
        group,
        from,
        to,
        reserve_mode,
    )
    .map_err(|error| map_fin_error("report.health", error))?;
    let latest = series.last().cloned();
    let reserve_latest = reserve_series.last().cloned();
    Ok(CommandResult {
        tool: "report.health",
        data: json!({
            "reserveMode": latest.as_ref().map(|value| value.reserve_mode),
            "expenseReserveBasisKind": latest.as_ref().map(|value| value.expense_reserve_basis_kind),
            "expenseReserveMonthlyBasisMinor": latest.as_ref().map(|value| value.expense_reserve_monthly_basis_minor),
            "expenseReserveMonths": latest.as_ref().map(|value| value.expense_reserve_months),
            "expenseReserveFactor": latest.as_ref().map(|value| value.expense_reserve_factor),
            "expenseReserveLookbackMonths": latest.as_ref().map(|value| value.expense_reserve_lookback_months),
            "taxReserveBasisKind": reserve_latest.as_ref().map(|value| value.tax_reserve_basis_kind),
            "taxReserveBasisDescription": reserve_latest.as_ref().map(|value| value.tax_reserve_basis_description.clone()),
            "reserveLatest": reserve_latest,
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

pub fn run_runway(args: RunwayCommandArgs<'_>) -> Result<CommandResult, CommandFailure> {
    let RunwayCommandArgs {
        db,
        group,
        consolidated,
        include,
        months,
        mode,
        scenario,
        ownership_mode,
        reserve_mode,
        salary_monthly_minor,
        dividends_monthly_minor,
        include_joint_expenses,
        from,
        to,
    } = args;

    let runtime = open_runtime("report.runway", db, true)?;
    let normalized_mode = mode.trim().to_ascii_lowercase();

    if matches!(normalized_mode.as_str(), "two-pool" | "two_pool") {
        if consolidated || group.is_some() || include.is_some() || from.is_some() {
            return Err(map_fin_error(
                "report.runway",
                fin_sdk::FinError::InvalidInput {
                    code: "INVALID_INPUT",
                    message: "two-pool runway does not accept --group, --consolidated, --include, or --from".to_owned(),
                },
            ));
        }
        let ownership_mode = ownership_mode.parse::<OwnershipMode>().map_err(|message| {
            map_fin_error(
                "report.runway",
                fin_sdk::FinError::InvalidInput {
                    code: "INVALID_OWNERSHIP_MODE",
                    message,
                },
            )
        })?;
        let scenario = scenario.parse::<TwoPoolScenarioKind>().map_err(|message| {
            map_fin_error(
                "report.runway",
                fin_sdk::FinError::InvalidInput {
                    code: "INVALID_SCENARIO",
                    message,
                },
            )
        })?;
        let reserve_mode = parse_reserve_mode("report.runway", reserve_mode)?;
        let report = report_two_pool_runway(
            runtime.connection(),
            runtime.config(),
            &TwoPoolRunwayOptions {
                months,
                to,
                reserve_mode,
                ownership_mode,
                scenario,
                salary_monthly_minor,
                dividends_monthly_minor,
                include_joint_expenses,
            },
        )
        .map_err(|error| map_fin_error("report.runway", error))?;
        let scenario_name = match report.scenario {
            TwoPoolScenarioKind::Config => "config",
            TwoPoolScenarioKind::TaxEfficient => "tax-efficient",
            TwoPoolScenarioKind::Custom => "custom",
        };

        return Ok(CommandResult {
            tool: "report.runway",
            data: json!({
                "mode": "two_pool",
                "reserveMode": report.reserve_mode,
                "ownershipMode": report.ownership_mode,
                "groups": ["business", "personal", "joint"],
                "series": [],
                "latest": null,
                "twoPool": report,
            }),
            text: format!(
                "two-pool runway | scenario={} source={} constraint={} months={:.2}",
                scenario_name,
                report.scenario_source,
                report.constraint_pool,
                report.constraint_months,
            ),
            meta: MetaExtras {
                count: Some(0),
                total: None,
                has_more: None,
            },
            exit_code: ExitCode::Success,
        });
    }

    if normalized_mode != "historical" {
        return Err(map_fin_error(
            "report.runway",
            fin_sdk::FinError::InvalidInput {
                code: "INVALID_MODE",
                message: format!("unsupported runway mode: {mode}"),
            },
        ));
    }

    if reserve_mode.is_some()
        || salary_monthly_minor.is_some()
        || dividends_monthly_minor.is_some()
        || include_joint_expenses.is_some()
        || scenario != "tax-efficient"
    {
        return Err(map_fin_error(
            "report.runway",
            fin_sdk::FinError::InvalidInput {
                code: "INVALID_INPUT",
                message:
                    "historical runway does not accept reserve-mode, scenario, or two-pool override flags"
                        .to_owned(),
            },
        ));
    }

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
            runtime.config().group_ids()
        } else {
            include_groups
        };

        let mut merged = BTreeMap::<String, (i64, i64, f64)>::new();
        for group_id in &groups {
            let points = report_runway(runtime.connection(), runtime.config(), group_id, from, to)
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
                "mode": "historical",
                "reserveMode": null,
                "ownershipMode": null,
                "series": series,
                "latest": latest,
                "groups": groups,
                "twoPool": null,
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
    let raw_series = report_runway(runtime.connection(), runtime.config(), group, from, to)
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
                    "balance": point.balance_minor,
                    "burnRate": point.burn_rate_minor,
                    "medianExpense": point.median_expense_minor,
                    "runway": point.runway_months,
                })
            })
            .collect::<Vec<_>>()
    };
    let latest = series.last().cloned();

    Ok(CommandResult {
        tool: "report.runway",
        data: json!({
            "mode": "historical",
            "reserveMode": null,
            "ownershipMode": null,
            "series": series,
            "latest": latest,
            "groups": [group],
            "twoPool": null,
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
    reserve_mode: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("report.reserves", db, true)?;
    let reserve_mode = parse_reserve_mode("report.reserves", reserve_mode)?;
    let series = report_reserves_with_mode(
        runtime.connection(),
        runtime.config(),
        group,
        from,
        to,
        reserve_mode,
    )
    .map_err(|error| map_fin_error("report.reserves", error))?;
    let latest = series.last().cloned();
    Ok(CommandResult {
        tool: "report.reserves",
        data: json!({
            "reserveMode": latest.as_ref().map(|value| value.reserve_mode),
            "expenseReserveBasisKind": latest.as_ref().map(|value| value.expense_reserve_basis_kind),
            "expenseReserveMonthlyBasisMinor": latest.as_ref().map(|value| value.expense_reserve_monthly_basis_minor),
            "expenseReserveMonths": latest.as_ref().map(|value| value.expense_reserve_months),
            "expenseReserveFactor": latest.as_ref().map(|value| value.expense_reserve_factor),
            "expenseReserveLookbackMonths": latest.as_ref().map(|value| value.expense_reserve_lookback_months),
            "taxReserveBasisKind": latest.as_ref().map(|value| value.tax_reserve_basis_kind),
            "taxReserveBasisDescription": latest.as_ref().map(|value| value.tax_reserve_basis_description.clone()),
            "series": series,
            "latest": latest,
        }),
        text: latest
            .as_ref()
            .map(|point| {
                format!(
                    "{} points | mode={} available={}",
                    series.len(),
                    point.reserve_mode.as_str(),
                    point.available_minor,
                )
            })
            .unwrap_or_else(|| format!("{} points", series.len())),
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
    to: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("report.categories", db, true)?;
    if mode == "median" {
        let points = group_category_monthly_median(
            runtime.connection(),
            runtime.config(),
            group,
            months,
            limit,
            to,
        )
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

    let points = group_category_breakdown(
        runtime.connection(),
        runtime.config(),
        group,
        months,
        limit,
        to,
    )
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
    to: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("report.audit", db, true)?;
    let points = audit_payees(runtime.connection(), account, months, limit, to)
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

pub fn run_summary(
    db: Option<&str>,
    months: usize,
    to: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("report.summary", db, true)?;
    let report = report_summary(runtime.connection(), runtime.config(), months, to)
        .map_err(|error| map_fin_error("report.summary", error))?;

    let mut groups = Vec::new();
    for group_id in runtime.config().group_ids() {
        let summary = report.groups.get(&group_id);
        let accounts = view_accounts(runtime.connection(), runtime.config(), Some(&group_id))
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
            "label": summary.map(|value| value.label.clone()).unwrap_or_else(|| runtime.config().resolve_group_metadata(&group_id).label),
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
