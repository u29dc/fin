use std::collections::{BTreeSet, HashMap};

use fin_sdk::config::LoadedConfig;
use fin_sdk::runtime::{RuntimeContext, RuntimeContextOptions};
use fin_sdk::{
    DashboardAllocationBasis, MonthlyCashflowPoint, TransactionQueryOptions,
    group_asset_account_ids, group_category_breakdown, report_cashflow, report_cashflow_kpis,
    report_group_allocation, report_reserves, report_runway, report_summary, view_accounts,
    view_transactions,
};
use rusqlite::{Connection, params_from_iter};

use crate::routes::Route;

use super::context::{
    CASHFLOW_LOOKBACK_MONTHS, CashflowRouteState, CategoriesRouteState, FetchContext,
    OverviewRouteState, OverviewScope, ReportsRouteState, SummaryRouteState,
    TransactionsRouteState,
};
use super::models::{
    AccountFreshnessRow, CashflowDashboardPayload, CashflowPoint, CategoriesDashboardPayload,
    CategoryParetoPoint, CategoryStabilityRow, OverviewDashboardPayload, RoutePayload,
    SummaryAllocation, SummaryAllocationSegment, SummaryDashboardPayload, SummaryGroupPanel,
    SummaryMonthSnapshot, TransactionTableRow, TransactionsPayload, UncategorizedLeakage,
};

#[derive(Debug, Default)]
pub struct FetchClient {
    runtime: Option<RuntimeContext>,
}

impl FetchClient {
    pub fn new() -> Self {
        Self { runtime: None }
    }

    pub fn available_groups(&mut self) -> Result<Vec<String>, String> {
        self.ensure_runtime()?;
        let Some(runtime) = self.runtime.as_ref() else {
            return Ok(vec![]);
        };
        Ok(runtime.config().group_ids())
    }

    pub fn fetch_route(
        &mut self,
        route: Route,
        context: &FetchContext,
    ) -> Result<RoutePayload, String> {
        self.ensure_runtime()?;
        let Some(runtime) = self.runtime.as_ref() else {
            return Err("runtime not initialized".to_owned());
        };

        match route {
            Route::Summary => fetch_summary_dashboard(runtime, &context.summary),
            Route::Transactions => fetch_transactions(runtime, &context.transactions),
            Route::Cashflow => fetch_cashflow_dashboard(runtime, &context.cashflow),
            Route::Overview => fetch_overview_dashboard(runtime, &context.overview),
            Route::Categories => fetch_categories_dashboard(runtime, &context.categories),
            Route::Reports => fetch_reports(runtime, &context.reports),
        }
    }

    fn ensure_runtime(&mut self) -> Result<(), String> {
        if self.runtime.is_none() {
            self.runtime = Some(
                RuntimeContext::open(RuntimeContextOptions {
                    create: false,
                    ..RuntimeContextOptions::read_only()
                })
                .map_err(|error| error.to_string())?,
            );
        }
        Ok(())
    }
}

fn fetch_summary_dashboard(
    runtime: &RuntimeContext,
    context: &SummaryRouteState,
) -> Result<RoutePayload, String> {
    let summary = report_summary(
        runtime.connection(),
        runtime.config(),
        context.trailing_months,
    )
    .map_err(|error| error.to_string())?;
    let mut groups = Vec::new();

    for group_id in runtime.config().group_ids() {
        let group = summary.groups.get(&group_id);
        let cashflow_kpis = report_cashflow_kpis(
            runtime.connection(),
            runtime.config(),
            &group_id,
            CASHFLOW_LOOKBACK_MONTHS,
            None,
            None,
        )
        .map_err(|error| error.to_string())?;
        let allocation = report_group_allocation(runtime.connection(), runtime.config(), &group_id)
            .map_err(|error| error.to_string())?;
        let dashboard = allocation.dashboard;

        let last_month = cashflow_kpis
            .last_full_month
            .as_ref()
            .map(|point| SummaryMonthSnapshot {
                month: point.month.clone(),
                income_minor: point.income_minor,
                expense_minor: point.expense_minor,
                net_minor: point.net_minor,
                savings_rate_pct: point.savings_rate_pct,
                income_change_pct: pct_change(
                    point.income_minor,
                    cashflow_kpis
                        .previous_full_month
                        .as_ref()
                        .map(|value| value.income_minor),
                ),
                expense_change_pct: pct_change(
                    point.expense_minor,
                    cashflow_kpis
                        .previous_full_month
                        .as_ref()
                        .map(|value| value.expense_minor),
                ),
                net_change_pct: pct_change(
                    point.net_minor,
                    cashflow_kpis
                        .previous_full_month
                        .as_ref()
                        .map(|value| value.net_minor),
                ),
            });

        groups.push(SummaryGroupPanel {
            group_id: group_id.clone(),
            label: group
                .map(|value| value.label.clone())
                .unwrap_or_else(|| group_id.clone()),
            net_worth_minor: group.map_or(0, |value| value.net_worth_minor),
            runway_months: group.and_then(|value| value.latest_runway_months),
            available_minor: group
                .and_then(|value| value.latest_available_minor)
                .or(Some(dashboard.available_minor)),
            last_full_month_net_minor: group.and_then(|value| value.last_full_month_net_minor),
            avg_six_month_net_minor: group.and_then(|value| value.trailing_average_net_minor),
            median_spend_minor: group.and_then(|value| value.median_spend_minor),
            short_term_trend: group.and_then(|value| value.short_term_trend),
            anomaly_count_last_12_months: group
                .map_or(0, |value| value.anomaly_count_last_12_months),
            recent_anomaly_months: cashflow_kpis.recent_anomaly_months,
            allocation: SummaryAllocation {
                basis_label: match dashboard.basis {
                    DashboardAllocationBasis::PersonalBuffer => "personal buffer".to_owned(),
                    DashboardAllocationBasis::ReserveComposition => {
                        "reserve composition".to_owned()
                    }
                },
                balance_basis_minor: dashboard.balance_basis_minor,
                display_total_minor: dashboard.display_total_minor,
                available_minor: dashboard.available_minor,
                expense_reserve_minor: dashboard.expense_reserve_minor,
                expense_reserve_display_minor: dashboard.expense_reserve_display_minor,
                tax_reserve_minor: dashboard.tax_reserve_minor,
                emergency_fund_minor: dashboard.emergency_fund_minor,
                savings_minor: dashboard.savings_minor,
                investment_minor: dashboard.investment_minor,
                shortfall_minor: dashboard.shortfall_minor,
                under_reserved: dashboard.under_reserved,
                segments: dashboard
                    .segments
                    .into_iter()
                    .map(|segment| SummaryAllocationSegment {
                        bucket: segment.bucket,
                        label: segment.label,
                        amount_minor: segment.amount_minor,
                        share_pct: segment.share_pct,
                    })
                    .collect(),
            },
            last_month,
        });
    }

    Ok(RoutePayload::SummaryDashboard(SummaryDashboardPayload {
        generated_at: summary.generated_at,
        consolidated_net_worth_minor: summary.consolidated.net_worth_minor,
        groups,
    }))
}

fn pct_change(current: i64, previous: Option<i64>) -> Option<f64> {
    let previous = previous?;
    if previous == 0 {
        return None;
    }
    Some(((current - previous) as f64 / previous.abs() as f64) * 100.0)
}

fn fetch_transactions(
    runtime: &RuntimeContext,
    context: &TransactionsRouteState,
) -> Result<RoutePayload, String> {
    let chart_account_ids = context
        .group_id
        .as_deref()
        .map(|group_id| group_asset_account_ids(runtime.config(), group_id));
    let rows = view_transactions(
        runtime.connection(),
        &TransactionQueryOptions {
            chart_account_ids,
            limit: context.page_limit,
            ..TransactionQueryOptions::default()
        },
    )
    .map_err(|error| error.to_string())?;

    let has_more = rows.len() == context.page_limit;
    let mapped = rows
        .into_iter()
        .map(|row| {
            let primary = summarize_accounts(&row.chart_account_id);
            let pair = summarize_accounts(&row.pair_account_id);
            let (from_account, to_account) = if row.amount_minor < 0 {
                (primary, pair)
            } else {
                (pair, primary)
            };

            TransactionTableRow {
                posted_at: row.posted_at,
                from_account,
                to_account,
                amount_minor: row.amount_minor,
                description: if row.clean_description.trim().is_empty() {
                    row.raw_description
                } else {
                    row.clean_description
                },
                counterparty: row.counterparty.unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();

    Ok(RoutePayload::Transactions(TransactionsPayload {
        rows: mapped,
        limit: context.page_limit,
        has_more,
    }))
}

fn fetch_cashflow_dashboard(
    runtime: &RuntimeContext,
    context: &CashflowRouteState,
) -> Result<RoutePayload, String> {
    let group_id = resolve_group(runtime, &context.group_id);
    let current = current_month(runtime.connection())?;

    let (series, _) = report_cashflow(
        runtime.connection(),
        runtime.config(),
        &group_id,
        CASHFLOW_LOOKBACK_MONTHS,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;

    let full_months = full_month_series(&series, &current);
    if full_months.is_empty() {
        return Ok(RoutePayload::Text(format!(
            "Cashflow ({group_id})\nNo full-month points yet."
        )));
    }

    let points = full_months
        .iter()
        .rev()
        .take(context.view_months)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|point| CashflowPoint {
            month: point.month,
            income_minor: point.income_minor,
            expense_minor: point.expense_minor,
            net_minor: point.net_minor,
        })
        .collect::<Vec<_>>();

    let latest_full_month = points.last().cloned();
    let avg_six_month_income_minor = average_last_n(full_months, 6, |point| point.income_minor);
    let avg_six_month_expense_minor = average_last_n(full_months, 6, |point| point.expense_minor);
    let avg_six_month_net_minor = average_last_n(full_months, 6, |point| point.net_minor);

    let runway = report_runway(
        runtime.connection(),
        runtime.config(),
        &group_id,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;
    let runway_months = runway.last().map(|point| point.runway_months);

    let reserves = report_reserves(
        runtime.connection(),
        runtime.config(),
        &group_id,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;
    let latest_reserve = reserves.last();

    Ok(RoutePayload::CashflowDashboard(CashflowDashboardPayload {
        group_id,
        points,
        latest_full_month,
        avg_six_month_income_minor,
        avg_six_month_expense_minor,
        avg_six_month_net_minor,
        runway_months,
        available_minor: latest_reserve.map(|point| point.available_minor),
        expense_reserve_minor: latest_reserve.map(|point| point.expense_reserve_minor),
        tax_reserve_minor: latest_reserve.map(|point| point.tax_reserve_minor),
    }))
}

fn fetch_overview_dashboard(
    runtime: &RuntimeContext,
    context: &OverviewRouteState,
) -> Result<RoutePayload, String> {
    let scope = &context.scope;
    let group_filter = match scope {
        OverviewScope::All => None,
        OverviewScope::Group(group) => Some(group.as_str()),
    };

    let mut accounts = view_accounts(runtime.connection(), runtime.config(), group_filter)
        .map_err(|error| error.to_string())?;
    if accounts.is_empty() {
        return Ok(RoutePayload::Text(format!(
            "Overview ({})\nNo asset accounts match current scope.",
            scope.label()
        )));
    }

    accounts.sort_by(|left, right| {
        right
            .balance_minor
            .unwrap_or_default()
            .abs()
            .cmp(&left.balance_minor.unwrap_or_default().abs())
            .then(left.id.cmp(&right.id))
    });

    if accounts.len() > context.max_accounts {
        accounts.truncate(context.max_accounts);
    }

    let rows = accounts
        .into_iter()
        .map(|account| {
            let stale_days = account
                .updated_at
                .as_deref()
                .and_then(|value| days_since(runtime.connection(), value).ok());
            AccountFreshnessRow {
                label: account.name,
                balance_minor: account.balance_minor.unwrap_or(0),
                updated_at: account.updated_at,
                stale_days,
            }
        })
        .collect::<Vec<_>>();

    let total_balance_minor = rows.iter().map(|row| row.balance_minor).sum::<i64>();

    Ok(RoutePayload::OverviewDashboard(OverviewDashboardPayload {
        scope_label: scope.label(),
        total_balance_minor,
        accounts: rows,
    }))
}

fn fetch_categories_dashboard(
    runtime: &RuntimeContext,
    context: &CategoriesRouteState,
) -> Result<RoutePayload, String> {
    let group_id = resolve_group(runtime, &context.group_id);
    let current = current_month(runtime.connection())?;

    let breakdown = group_category_breakdown(
        runtime.connection(),
        runtime.config(),
        &group_id,
        context.months,
        context.pareto_limit,
    )
    .map_err(|error| error.to_string())?;
    if breakdown.is_empty() {
        return Ok(RoutePayload::Text(format!(
            "Categories ({group_id})\nNo category data."
        )));
    }

    let leakage = load_uncategorized_leakage(
        runtime.connection(),
        runtime.loaded_config(),
        &group_id,
        context.months,
        &current,
    )?;

    let total_expense_minor = if leakage.total_expense_minor > 0 {
        leakage.total_expense_minor
    } else {
        breakdown.iter().map(|point| point.total_minor).sum::<i64>()
    };

    let pareto = breakdown
        .into_iter()
        .map(|point| {
            let share_pct = if total_expense_minor <= 0 {
                0.0
            } else {
                (point.total_minor as f64 / total_expense_minor as f64) * 100.0
            };
            CategoryParetoPoint {
                category: point.category,
                total_minor: point.total_minor,
                transaction_count: point.transaction_count,
                share_pct,
            }
        })
        .collect::<Vec<_>>();

    let (months, stability) = load_category_stability(
        runtime.connection(),
        runtime.loaded_config(),
        &group_id,
        context.months,
        context.stability_limit,
        &current,
    )?;

    Ok(RoutePayload::CategoriesDashboard(
        CategoriesDashboardPayload {
            group_id,
            pareto,
            months,
            stability,
            leakage,
        },
    ))
}

fn fetch_reports(
    runtime: &RuntimeContext,
    context: &ReportsRouteState,
) -> Result<RoutePayload, String> {
    let group_id = resolve_group(runtime, &context.group_id);
    let current = current_month(runtime.connection())?;

    let (series, totals) = report_cashflow(
        runtime.connection(),
        runtime.config(),
        &group_id,
        CASHFLOW_LOOKBACK_MONTHS,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;
    let full_months = full_month_series(&series, &current);

    let runway = report_runway(
        runtime.connection(),
        runtime.config(),
        &group_id,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;
    let reserves = report_reserves(
        runtime.connection(),
        runtime.config(),
        &group_id,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;

    let latest_net = full_months.last().map_or(0, |point| point.net_minor);
    let avg_six_net = average_last_n(full_months, 6, |point| point.net_minor).unwrap_or(0);
    let latest_runway = runway
        .last()
        .map(|point| point.runway_months)
        .unwrap_or(0.0);
    let latest_reserve = reserves.last();
    let available = latest_reserve.map_or(0, |point| point.available_minor);

    let mut lines = vec![
        format!("Reports ({group_id})"),
        format!(
            "Totals (all months) | income {:>12} | expenses {:>12} | net {:>12}",
            totals.income_minor, totals.expense_minor, totals.net_minor
        ),
        format!(
            "Latest full month net {:>12} | 6m avg net {:>12}",
            latest_net, avg_six_net
        ),
        format!(
            "Runway {:>8.2} months | Available {:>12}",
            latest_runway, available
        ),
        String::new(),
        "Recent full months".to_owned(),
    ];

    for point in full_months
        .iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let savings = point
            .savings_rate_pct
            .map_or_else(|| "n/a".to_owned(), |value| format!("{value:.1}%"));
        lines.push(format!(
            "{} | income {:>10} | expenses {:>10} | net {:>10} | savings {:>7}",
            point.month, point.income_minor, point.expense_minor, point.net_minor, savings
        ));
    }

    Ok(RoutePayload::Text(lines.join("\n")))
}

fn resolve_group(runtime: &RuntimeContext, requested_group: &str) -> String {
    let groups = runtime.config().group_ids();
    if groups.iter().any(|group| group == requested_group) {
        return requested_group.to_owned();
    }
    groups
        .first()
        .cloned()
        .unwrap_or_else(|| requested_group.to_owned())
}

fn full_month_series<'a>(
    points: &'a [MonthlyCashflowPoint],
    current_month: &str,
) -> &'a [MonthlyCashflowPoint] {
    if points.is_empty() {
        return points;
    }
    let split = points
        .iter()
        .position(|point| point.month == current_month)
        .unwrap_or(points.len());
    &points[..split]
}

fn average_last_n(
    points: &[MonthlyCashflowPoint],
    n: usize,
    selector: impl Fn(&MonthlyCashflowPoint) -> i64,
) -> Option<i64> {
    if points.is_empty() || n == 0 {
        return None;
    }
    let sample = points.iter().rev().take(n).collect::<Vec<_>>();
    if sample.is_empty() {
        return None;
    }
    let total = sample.iter().map(|point| selector(point)).sum::<i64>();
    Some(total / i64::try_from(sample.len()).unwrap_or(1))
}

fn current_month(connection: &Connection) -> Result<String, String> {
    connection
        .query_row("SELECT strftime('%Y-%m', 'now', 'localtime')", [], |row| {
            row.get::<usize, String>(0)
        })
        .map_err(|error| error.to_string())
}

fn days_since(connection: &Connection, timestamp: &str) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT CAST(julianday('now', 'localtime') - julianday(?1) AS INTEGER)",
            [timestamp],
            |row| row.get::<usize, i64>(0),
        )
        .map_err(|error| error.to_string())
}

fn load_category_stability(
    connection: &Connection,
    loaded: &LoadedConfig,
    group_id: &str,
    months: usize,
    limit: usize,
    current_month: &str,
) -> Result<(Vec<String>, Vec<CategoryStabilityRow>), String> {
    let account_ids = loaded
        .config
        .accounts
        .iter()
        .filter(|account| account.group == group_id && account.account_type == "asset")
        .map(|account| account.id.clone())
        .collect::<Vec<_>>();
    if account_ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let placeholders_sql = sql_placeholders(account_ids.len());
    let sql = format!(
        "SELECT strftime('%Y-%m', je.posted_at) AS month,\n                coa.name AS category,\n                COALESCE(SUM(p.amount_minor), 0) AS month_total\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON p.account_id = coa.id\n         WHERE coa.account_type = 'expense'\n           AND je.posted_at >= date('now', 'start of month', '-' || ? || ' months')\n           AND strftime('%Y-%m', je.posted_at) < ?\n           AND EXISTS (\n             SELECT 1\n             FROM postings asset\n             WHERE asset.journal_entry_id = p.journal_entry_id\n               AND asset.account_id IN ({placeholders_sql})\n           )\n         GROUP BY month, coa.name\n         ORDER BY month ASC, month_total DESC"
    );

    let mut params = vec![months.to_string(), current_month.to_owned()];
    params.extend(account_ids);

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query(params_from_iter(params.iter()))
        .map_err(|error| error.to_string())?;

    let mut month_set = BTreeSet::new();
    let mut month_totals = HashMap::<String, HashMap<String, i64>>::new();
    let mut category_totals = HashMap::<String, i64>::new();

    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let month: String = row.get(0).map_err(|error| error.to_string())?;
        let category: String = row.get(1).map_err(|error| error.to_string())?;
        let total_minor: i64 = row.get(2).map_err(|error| error.to_string())?;

        month_set.insert(month.clone());
        month_totals
            .entry(category.clone())
            .or_default()
            .insert(month, total_minor);
        category_totals
            .entry(category)
            .and_modify(|current| *current += total_minor)
            .or_insert(total_minor);
    }

    let months = month_set.into_iter().collect::<Vec<_>>();
    if months.is_empty() {
        return Ok((months, Vec::new()));
    }

    let mut ranked = category_totals.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));

    let selected = ranked
        .into_iter()
        .take(limit)
        .map(|(category, _)| {
            let values = months
                .iter()
                .map(|month| {
                    month_totals
                        .get(&category)
                        .and_then(|totals| totals.get(month))
                        .copied()
                        .unwrap_or(0)
                })
                .collect::<Vec<_>>();
            let total_minor = values.iter().sum::<i64>();
            CategoryStabilityRow {
                category,
                month_values_minor: values,
                total_minor,
            }
        })
        .collect::<Vec<_>>();

    Ok((months, selected))
}

fn load_uncategorized_leakage(
    connection: &Connection,
    loaded: &LoadedConfig,
    group_id: &str,
    months: usize,
    current_month: &str,
) -> Result<UncategorizedLeakage, String> {
    let account_ids = loaded
        .config
        .accounts
        .iter()
        .filter(|account| account.group == group_id && account.account_type == "asset")
        .map(|account| account.id.clone())
        .collect::<Vec<_>>();
    if account_ids.is_empty() {
        return Ok(UncategorizedLeakage {
            total_expense_minor: 0,
            uncategorized_minor: 0,
            uncategorized_count: 0,
            leakage_pct: 0.0,
        });
    }

    let placeholders_sql = sql_placeholders(account_ids.len());
    let sql = format!(
        "SELECT COALESCE(SUM(CASE WHEN coa.name = 'Uncategorized' THEN p.amount_minor ELSE 0 END), 0) AS uncategorized_minor,\n                COALESCE(COUNT(CASE WHEN coa.name = 'Uncategorized' THEN 1 END), 0) AS uncategorized_count,\n                COALESCE(SUM(p.amount_minor), 0) AS total_expense_minor\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON p.account_id = coa.id\n         WHERE coa.account_type = 'expense'\n           AND je.posted_at >= date('now', 'start of month', '-' || ? || ' months')\n           AND strftime('%Y-%m', je.posted_at) < ?\n           AND EXISTS (\n             SELECT 1\n             FROM postings asset\n             WHERE asset.journal_entry_id = p.journal_entry_id\n               AND asset.account_id IN ({placeholders_sql})\n           )"
    );

    let mut params = vec![months.to_string(), current_month.to_owned()];
    params.extend(account_ids);

    connection
        .query_row(&sql, params_from_iter(params.iter()), |row| {
            let uncategorized_minor = row.get::<usize, i64>(0)?;
            let uncategorized_count = row.get::<usize, i64>(1)?;
            let total_expense_minor = row.get::<usize, i64>(2)?;
            let leakage_pct = if total_expense_minor <= 0 {
                0.0
            } else {
                (uncategorized_minor as f64 / total_expense_minor as f64) * 100.0
            };
            Ok(UncategorizedLeakage {
                total_expense_minor,
                uncategorized_minor,
                uncategorized_count,
                leakage_pct,
            })
        })
        .map_err(|error| error.to_string())
}

fn sql_placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

fn summarize_accounts(accounts: &str) -> String {
    let parts = accounts
        .split(',')
        .filter_map(|value| value.rsplit(':').next())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return "n/a".to_owned();
    }
    if parts.len() == 1 {
        return parts[0].to_owned();
    }
    format!("{} (+{})", parts[0], parts.len() - 1)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use fin_sdk::runtime::{RuntimeContext, RuntimeContextOptions};
    use fin_sdk::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

    use super::{FetchClient, FetchContext, RoutePayload};
    use crate::routes::Route;

    #[test]
    fn fetch_client_uses_shared_runtime_context() {
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

        let mut client = FetchClient {
            runtime: Some(runtime),
        };

        assert_eq!(
            client.available_groups().expect("available groups"),
            vec![
                "business".to_owned(),
                "joint".to_owned(),
                "personal".to_owned()
            ]
        );

        let payload = client
            .fetch_route(Route::Summary, &FetchContext::default())
            .expect("summary payload");
        match payload {
            RoutePayload::SummaryDashboard(payload) => {
                assert_eq!(payload.groups.len(), 3);
                assert!(
                    payload
                        .groups
                        .iter()
                        .all(|group| !group.allocation.segments.is_empty())
                );
                assert!(
                    payload
                        .groups
                        .iter()
                        .all(|group| group.last_month.as_ref().is_some())
                );
                assert!(
                    payload
                        .groups
                        .iter()
                        .all(|group| group.avg_six_month_net_minor.is_some())
                );
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }
}
