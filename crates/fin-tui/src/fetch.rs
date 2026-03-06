use std::collections::{BTreeSet, HashMap};

use fin_sdk::config::LoadedConfig;
use fin_sdk::runtime::{RuntimeContext, RuntimeContextOptions};
use fin_sdk::{
    MonthlyCashflowPoint, SortDirection, TransactionQueryOptions, TransactionSortField,
    group_asset_account_ids, group_category_breakdown, report_cashflow, report_reserves,
    report_runway, report_summary, view_accounts, view_transactions,
};
use rusqlite::{Connection, params_from_iter};

use crate::cache::{RouteCacheKey, RouteViewKey};
use crate::routes::Route;

pub const TUI_TRANSACTIONS_PREVIEW_LIMIT: usize = 1000;
const CASHFLOW_LOOKBACK_MONTHS: usize = 120;
const CASHFLOW_VIEW_MONTHS: usize = 12;
const CATEGORY_MONTHS: usize = 6;
const CATEGORY_LIMIT: usize = 12;
const CATEGORY_STABILITY_LIMIT: usize = 6;
const OVERVIEW_MAX_ACCOUNTS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverviewScope {
    All,
    Group(String),
}

impl OverviewScope {
    #[must_use]
    pub fn id(&self) -> String {
        match self {
            Self::All => "all".to_owned(),
            Self::Group(group) => group.clone(),
        }
    }

    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::All => "all accounts".to_owned(),
            Self::Group(group) => format!("{group} accounts"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryRouteState {
    pub trailing_months: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionsRouteState {
    pub group_id: Option<String>,
    pub search_query: String,
    pub search_active: bool,
    pub sort_field: TransactionSortField,
    pub sort_direction: SortDirection,
    pub window_months: Option<usize>,
    pub page_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashflowRouteState {
    pub group_id: String,
    pub view_months: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewRouteState {
    pub scope: OverviewScope,
    pub max_accounts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoriesRouteState {
    pub group_id: String,
    pub months: usize,
    pub pareto_limit: usize,
    pub stability_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportsRouteState {
    pub group_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchContext {
    pub summary: SummaryRouteState,
    pub transactions: TransactionsRouteState,
    pub cashflow: CashflowRouteState,
    pub overview: OverviewRouteState,
    pub categories: CategoriesRouteState,
    pub reports: ReportsRouteState,
}

impl Default for FetchContext {
    fn default() -> Self {
        Self {
            summary: SummaryRouteState {
                trailing_months: CASHFLOW_VIEW_MONTHS,
            },
            transactions: TransactionsRouteState {
                group_id: None,
                search_query: String::new(),
                search_active: false,
                sort_field: TransactionSortField::PostedAt,
                sort_direction: SortDirection::Desc,
                window_months: None,
                page_limit: TUI_TRANSACTIONS_PREVIEW_LIMIT,
            },
            cashflow: CashflowRouteState {
                group_id: "business".to_owned(),
                view_months: CASHFLOW_VIEW_MONTHS,
            },
            overview: OverviewRouteState {
                scope: OverviewScope::All,
                max_accounts: OVERVIEW_MAX_ACCOUNTS,
            },
            categories: CategoriesRouteState {
                group_id: "business".to_owned(),
                months: CATEGORY_MONTHS,
                pareto_limit: CATEGORY_LIMIT,
                stability_limit: CATEGORY_STABILITY_LIMIT,
            },
            reports: ReportsRouteState {
                group_id: "business".to_owned(),
            },
        }
    }
}

impl FetchContext {
    #[must_use]
    pub fn route_cache_key(&self, route: Route) -> RouteCacheKey {
        RouteCacheKey::new(route, self.route_cache_fingerprint(route))
    }

    #[must_use]
    pub fn route_view_key(&self, route: Route) -> RouteViewKey {
        RouteViewKey::new(route, self.route_view_fingerprint(route))
    }

    #[must_use]
    pub fn route_context(&self, route: Route) -> String {
        match route {
            Route::Summary => format!("finance/summary/{}m", self.summary.trailing_months),
            Route::Transactions => {
                let group = self.transactions.group_id.as_deref().unwrap_or("all");
                let window = self
                    .transactions
                    .window_months
                    .map(|months| format!("{months}m"))
                    .unwrap_or_else(|| "all".to_owned());
                format!(
                    "finance/transactions/{group}/{}-{}/{}",
                    transaction_sort_id(self.transactions.sort_field),
                    sort_direction_id(self.transactions.sort_direction),
                    window
                )
            }
            Route::Cashflow => format!(
                "finance/cashflow/{}/{}m",
                self.cashflow.group_id, self.cashflow.view_months
            ),
            Route::Overview => format!(
                "finance/overview/{}/max-{}",
                self.overview.scope.id(),
                self.overview.max_accounts
            ),
            Route::Categories => format!(
                "finance/categories/{}/{}m",
                self.categories.group_id, self.categories.months
            ),
            Route::Reports => format!("finance/reports/{}", self.reports.group_id),
        }
    }

    fn route_cache_fingerprint(&self, route: Route) -> String {
        match route {
            Route::Summary => format!("trailing-months={}", self.summary.trailing_months),
            Route::Transactions => {
                let group = self.transactions.group_id.as_deref().unwrap_or("all");
                let window = self
                    .transactions
                    .window_months
                    .map_or_else(|| "all".to_owned(), |months| months.to_string());
                format!(
                    "group={group}|sort={}-{}|window={window}|limit={}",
                    transaction_sort_id(self.transactions.sort_field),
                    sort_direction_id(self.transactions.sort_direction),
                    self.transactions.page_limit
                )
            }
            Route::Cashflow => format!(
                "group={}|months={}",
                self.cashflow.group_id, self.cashflow.view_months
            ),
            Route::Overview => format!(
                "scope={}|max-accounts={}",
                self.overview.scope.id(),
                self.overview.max_accounts
            ),
            Route::Categories => format!(
                "group={}|months={}|pareto={}|stability={}",
                self.categories.group_id,
                self.categories.months,
                self.categories.pareto_limit,
                self.categories.stability_limit
            ),
            Route::Reports => format!("group={}", self.reports.group_id),
        }
    }

    fn route_view_fingerprint(&self, route: Route) -> String {
        match route {
            Route::Transactions => format!(
                "{}|search={}|active={}",
                self.route_cache_fingerprint(route),
                encode_context_fragment(&self.transactions.search_query),
                self.transactions.search_active
            ),
            _ => self.route_cache_fingerprint(route),
        }
    }
}

fn encode_context_fragment(value: &str) -> String {
    value.replace('|', "%7C")
}

fn transaction_sort_id(sort_field: TransactionSortField) -> &'static str {
    match sort_field {
        TransactionSortField::PostedAt => "posted_at",
        TransactionSortField::AmountMinor => "amount_minor",
        TransactionSortField::Description => "description",
        TransactionSortField::Counterparty => "counterparty",
        TransactionSortField::AccountId => "account_id",
    }
}

fn sort_direction_id(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Asc => "asc",
        SortDirection::Desc => "desc",
    }
}

#[derive(Debug, Clone)]
pub struct TransactionTableRow {
    pub posted_at: String,
    pub from_account: String,
    pub to_account: String,
    pub amount_minor: i64,
    pub description: String,
    pub counterparty: String,
}

pub fn transaction_matches_query(row: &TransactionTableRow, query: &str) -> bool {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return true;
    }

    row.posted_at.to_ascii_lowercase().contains(&needle)
        || row.from_account.to_ascii_lowercase().contains(&needle)
        || row.to_account.to_ascii_lowercase().contains(&needle)
        || row.amount_minor.to_string().contains(&needle)
        || row.description.to_ascii_lowercase().contains(&needle)
        || row.counterparty.to_ascii_lowercase().contains(&needle)
}

#[derive(Debug, Clone)]
pub struct TransactionsPayload {
    pub rows: Vec<TransactionTableRow>,
    pub limit: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone)]
pub struct GroupKpiRow {
    pub group_id: String,
    pub net_worth_minor: i64,
    pub runway_months: Option<f64>,
    pub available_minor: Option<i64>,
    pub last_full_month_net_minor: Option<i64>,
    pub avg_six_month_net_minor: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ReserveGaugeRow {
    pub group_id: String,
    pub runway_months: Option<f64>,
    pub available_minor: i64,
    pub target_minor: i64,
    pub expense_reserve_minor: i64,
    pub tax_reserve_minor: i64,
}

#[derive(Debug, Clone)]
pub struct SummaryDashboardPayload {
    pub generated_at: String,
    pub consolidated_net_worth_minor: i64,
    pub group_rows: Vec<GroupKpiRow>,
    pub reserve_rows: Vec<ReserveGaugeRow>,
}

#[derive(Debug, Clone)]
pub struct CashflowPoint {
    pub month: String,
    pub income_minor: i64,
    pub expense_minor: i64,
    pub net_minor: i64,
}

#[derive(Debug, Clone)]
pub struct CashflowDashboardPayload {
    pub group_id: String,
    pub points: Vec<CashflowPoint>,
    pub latest_full_month: Option<CashflowPoint>,
    pub avg_six_month_income_minor: Option<i64>,
    pub avg_six_month_expense_minor: Option<i64>,
    pub avg_six_month_net_minor: Option<i64>,
    pub runway_months: Option<f64>,
    pub available_minor: Option<i64>,
    pub expense_reserve_minor: Option<i64>,
    pub tax_reserve_minor: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CategoryParetoPoint {
    pub category: String,
    pub total_minor: i64,
    pub transaction_count: i64,
    pub share_pct: f64,
}

#[derive(Debug, Clone)]
pub struct CategoryStabilityRow {
    pub category: String,
    pub month_values_minor: Vec<i64>,
    pub total_minor: i64,
}

#[derive(Debug, Clone)]
pub struct UncategorizedLeakage {
    pub total_expense_minor: i64,
    pub uncategorized_minor: i64,
    pub uncategorized_count: i64,
    pub leakage_pct: f64,
}

#[derive(Debug, Clone)]
pub struct CategoriesDashboardPayload {
    pub group_id: String,
    pub pareto: Vec<CategoryParetoPoint>,
    pub months: Vec<String>,
    pub stability: Vec<CategoryStabilityRow>,
    pub leakage: UncategorizedLeakage,
}

#[derive(Debug, Clone)]
pub struct AccountFreshnessRow {
    pub label: String,
    pub balance_minor: i64,
    pub updated_at: Option<String>,
    pub stale_days: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct OverviewDashboardPayload {
    pub scope_label: String,
    pub total_balance_minor: i64,
    pub accounts: Vec<AccountFreshnessRow>,
}

#[derive(Debug, Clone)]
pub enum RoutePayload {
    Text(String),
    Transactions(TransactionsPayload),
    SummaryDashboard(SummaryDashboardPayload),
    CashflowDashboard(CashflowDashboardPayload),
    OverviewDashboard(OverviewDashboardPayload),
    CategoriesDashboard(CategoriesDashboardPayload),
}

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
    let current_month = current_month(runtime.connection())?;

    let mut group_rows = Vec::new();
    let mut reserve_rows = Vec::new();

    for group_id in runtime.config().group_ids() {
        let group = summary.groups.get(&group_id);
        let (series, _) = report_cashflow(
            runtime.connection(),
            runtime.config(),
            &group_id,
            CASHFLOW_LOOKBACK_MONTHS,
            None,
            None,
        )
        .map_err(|error| error.to_string())?;
        let full_months = full_month_series(&series, &current_month);
        let last_full_month_net_minor = full_months.last().map(|point| point.net_minor);
        let avg_six_month_net_minor = average_last_n(full_months, 6, |point| point.net_minor);

        group_rows.push(GroupKpiRow {
            group_id: group_id.clone(),
            net_worth_minor: group.map_or(0, |value| value.net_worth_minor),
            runway_months: group.and_then(|value| value.latest_runway_months),
            available_minor: group.and_then(|value| value.latest_available_minor),
            last_full_month_net_minor,
            avg_six_month_net_minor,
        });

        let reserves = report_reserves(
            runtime.connection(),
            runtime.config(),
            &group_id,
            None,
            None,
        )
        .map_err(|error| error.to_string())?;
        let latest_reserve = reserves.last();
        let expense_reserve_minor = latest_reserve.map_or(0, |value| value.expense_reserve_minor);
        let tax_reserve_minor = latest_reserve.map_or(0, |value| value.tax_reserve_minor);
        let available_minor = latest_reserve.map_or(0, |value| value.available_minor);

        reserve_rows.push(ReserveGaugeRow {
            group_id: group_id.clone(),
            runway_months: group.and_then(|value| value.latest_runway_months),
            available_minor,
            target_minor: expense_reserve_minor + tax_reserve_minor,
            expense_reserve_minor,
            tax_reserve_minor,
        });
    }

    Ok(RoutePayload::SummaryDashboard(SummaryDashboardPayload {
        generated_at: summary.generated_at,
        consolidated_net_worth_minor: summary.consolidated.net_worth_minor,
        group_rows,
        reserve_rows,
    }))
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
                        .and_then(|by_month| by_month.get(month))
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
        "SELECT COALESCE(SUM(CASE WHEN p.account_id = 'Expenses:Uncategorized' THEN p.amount_minor ELSE 0 END), 0) AS uncategorized_minor,\n                COALESCE(SUM(CASE WHEN p.account_id = 'Expenses:Uncategorized' THEN 1 ELSE 0 END), 0) AS uncategorized_count,\n                COALESCE(SUM(p.amount_minor), 0) AS total_expense_minor\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON coa.id = p.account_id\n         WHERE coa.account_type = 'expense'\n           AND je.posted_at >= date('now', 'start of month', '-' || ? || ' months')\n           AND strftime('%Y-%m', je.posted_at) < ?\n           AND EXISTS (\n             SELECT 1\n             FROM postings asset\n             WHERE asset.journal_entry_id = p.journal_entry_id\n               AND asset.account_id IN ({placeholders_sql})\n           )"
    );

    let mut params = vec![months.to_string(), current_month.to_owned()];
    params.extend(account_ids);

    let values = connection
        .query_row(&sql, params_from_iter(params.iter()), |row| {
            let uncategorized_minor = row.get::<usize, i64>(0)?;
            let uncategorized_count = row.get::<usize, i64>(1)?;
            let total_expense_minor = row.get::<usize, i64>(2)?;
            Ok((
                uncategorized_minor,
                uncategorized_count,
                total_expense_minor,
            ))
        })
        .map_err(|error| error.to_string())?;

    let leakage_pct = if values.2 <= 0 {
        0.0
    } else {
        (values.0 as f64 / values.2 as f64) * 100.0
    };

    Ok(UncategorizedLeakage {
        total_expense_minor: values.2,
        uncategorized_minor: values.0,
        uncategorized_count: values.1,
        leakage_pct,
    })
}

fn sql_placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

fn summarize_accounts(accounts: &str) -> String {
    let parts = accounts
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
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
                assert_eq!(payload.group_rows.len(), 3);
                assert_eq!(payload.reserve_rows.len(), 3);
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    }

    #[test]
    fn fetch_context_uses_distinct_cache_and_view_keys() {
        let mut context = FetchContext::default();
        let base_cache_key = context.route_cache_key(Route::Transactions);
        let base_view_key = context.route_view_key(Route::Transactions);

        context.transactions.search_query = "rent".to_owned();
        context.transactions.search_active = true;

        assert_eq!(base_cache_key, context.route_cache_key(Route::Transactions));
        assert_ne!(base_view_key, context.route_view_key(Route::Transactions));

        context.cashflow.group_id = "joint".to_owned();
        assert_ne!(
            context.route_cache_key(Route::Cashflow),
            FetchContext::default().route_cache_key(Route::Cashflow)
        );
    }
}
