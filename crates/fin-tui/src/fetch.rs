use std::collections::{BTreeMap, BTreeSet};

use fin_sdk::config::{LoadedConfig, load_config};
use fin_sdk::db::{OpenDatabaseOptions, open_database, resolve_db_path};
use fin_sdk::{
    TransactionQueryOptions, group_category_breakdown, report_cashflow, report_summary,
    view_transactions,
};
use rusqlite::{Connection, params_from_iter};

use crate::routes::Route;

pub const TUI_TRANSACTIONS_PREVIEW_LIMIT: usize = 1000;
const CASHFLOW_MONTHS: usize = 48;
const CATEGORY_MONTHS: usize = 6;
const CATEGORY_LIMIT: usize = 12;
const CHART_MAX_POINTS: usize = 160;
const OVERVIEW_MAX_SERIES: usize = 8;

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

    #[must_use]
    fn allows_group(&self, group: &str) -> bool {
        match self {
            Self::All => true,
            Self::Group(selected) => selected == group,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchContext {
    pub cashflow_group: String,
    pub overview_scope: OverviewScope,
}

impl Default for FetchContext {
    fn default() -> Self {
        Self {
            cashflow_group: "business".to_owned(),
            overview_scope: OverviewScope::All,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartTone {
    Accent1,
    Accent2,
    Accent3,
    Accent4,
}

#[derive(Debug, Clone)]
pub struct LineChartSeries {
    pub label: String,
    pub tone: ChartTone,
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct LineChartPayload {
    pub title: String,
    pub subtitle: String,
    pub x_labels: Vec<String>,
    pub series: Vec<LineChartSeries>,
}

#[derive(Debug, Clone)]
pub struct CategoryBarPoint {
    pub label: String,
    pub total_major: u64,
}

#[derive(Debug, Clone)]
pub struct CategoryBarsPayload {
    pub title: String,
    pub subtitle: String,
    pub points: Vec<CategoryBarPoint>,
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
pub enum RoutePayload {
    Text(String),
    Transactions(TransactionsPayload),
    LineChart(LineChartPayload),
    CategoryBars(CategoryBarsPayload),
}

#[derive(Debug)]
struct RuntimeContext {
    connection: Connection,
    loaded: LoadedConfig,
}

impl RuntimeContext {
    fn open() -> Result<Self, String> {
        let loaded = load_config(None).map_err(|error| error.to_string())?;
        let db_path = resolve_db_path(None, Some(&loaded.config_dir()));
        let connection = open_database(OpenDatabaseOptions {
            path: Some(db_path),
            config_dir: Some(loaded.config_dir()),
            readonly: true,
            create: false,
            migrate: true,
        })
        .map_err(|error| error.to_string())?;
        Ok(Self { connection, loaded })
    }
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
        Ok(runtime.loaded.config.group_ids())
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
            Route::Summary => fetch_summary(runtime),
            Route::Transactions => fetch_transactions(runtime),
            Route::Cashflow => fetch_cashflow_chart(runtime, &context.cashflow_group),
            Route::Overview => fetch_overview_chart(runtime, &context.overview_scope),
            Route::Categories => fetch_category_bars(runtime, &context.cashflow_group),
            Route::Reports => fetch_reports(runtime, &context.cashflow_group),
        }
    }

    fn ensure_runtime(&mut self) -> Result<(), String> {
        if self.runtime.is_none() {
            self.runtime = Some(RuntimeContext::open()?);
        }
        Ok(())
    }
}

fn fetch_summary(runtime: &RuntimeContext) -> Result<RoutePayload, String> {
    let summary = report_summary(&runtime.connection, &runtime.loaded.config, 12)
        .map_err(|error| error.to_string())?;

    let mut lines = vec![
        "Summary".to_owned(),
        format!("Generated: {}", summary.generated_at),
        format!("Period (months): {}", summary.period_months),
        format!(
            "Consolidated net worth (minor): {}",
            summary.consolidated.net_worth_minor
        ),
        String::new(),
        "Group snapshots".to_owned(),
    ];

    for (group_id, group) in &summary.groups {
        let runway = group
            .latest_runway_months
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "n/a".to_owned());
        let health = group
            .latest_health_minor
            .map_or_else(|| "n/a".to_owned(), |value| value.to_string());
        let available = group
            .latest_available_minor
            .map_or_else(|| "n/a".to_owned(), |value| value.to_string());
        lines.push(format!(
            "{group_id:>9} | nw {net:>12} | runway {runway:>7} | health {health:>12} | available {available:>12}",
            net = group.net_worth_minor,
        ));
    }

    Ok(RoutePayload::Text(lines.join("\n")))
}

fn fetch_transactions(runtime: &RuntimeContext) -> Result<RoutePayload, String> {
    let rows = view_transactions(
        &runtime.connection,
        &TransactionQueryOptions {
            limit: TUI_TRANSACTIONS_PREVIEW_LIMIT,
            ..TransactionQueryOptions::default()
        },
    )
    .map_err(|error| error.to_string())?;

    let has_more = rows.len() == TUI_TRANSACTIONS_PREVIEW_LIMIT;
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
        limit: TUI_TRANSACTIONS_PREVIEW_LIMIT,
        has_more,
    }))
}

fn fetch_cashflow_chart(
    runtime: &RuntimeContext,
    requested_group: &str,
) -> Result<RoutePayload, String> {
    let group_id = resolve_group(runtime, requested_group);
    let (points, _totals) = report_cashflow(
        &runtime.connection,
        &runtime.loaded.config,
        &group_id,
        CASHFLOW_MONTHS,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;
    if points.is_empty() {
        return Ok(RoutePayload::Text(format!(
            "Cashflow ({group_id})\nNo series points."
        )));
    }

    let indices = downsample_indices(points.len(), CHART_MAX_POINTS);
    let x_labels = indices
        .iter()
        .filter_map(|index| points.get(*index).map(|point| point.month.clone()))
        .collect::<Vec<_>>();

    let mut income = Vec::with_capacity(indices.len());
    let mut expense = Vec::with_capacity(indices.len());
    let mut net = Vec::with_capacity(indices.len());

    for (x, source_index) in indices.iter().enumerate() {
        let Some(point) = points.get(*source_index) else {
            continue;
        };
        let x_value = x as f64;
        income.push((x_value, point.income_minor as f64 / 100.0));
        expense.push((x_value, point.expense_minor as f64 / 100.0));
        net.push((x_value, point.net_minor as f64 / 100.0));
    }

    Ok(RoutePayload::LineChart(LineChartPayload {
        title: "Cashflow".to_owned(),
        subtitle: format!("{group_id} | monthly income, expense, net"),
        x_labels,
        series: vec![
            LineChartSeries {
                label: "income".to_owned(),
                tone: ChartTone::Accent1,
                points: income,
            },
            LineChartSeries {
                label: "expense".to_owned(),
                tone: ChartTone::Accent3,
                points: expense,
            },
            LineChartSeries {
                label: "net".to_owned(),
                tone: ChartTone::Accent2,
                points: net,
            },
        ],
    }))
}

fn fetch_overview_chart(
    runtime: &RuntimeContext,
    scope: &OverviewScope,
) -> Result<RoutePayload, String> {
    let assets = runtime
        .loaded
        .config
        .accounts
        .iter()
        .filter(|account| account.account_type == "asset" && scope.allows_group(&account.group))
        .map(|account| {
            (
                account.id.clone(),
                account.label.clone().unwrap_or_else(|| {
                    account
                        .id
                        .rsplit(':')
                        .next()
                        .unwrap_or(&account.id)
                        .to_owned()
                }),
            )
        })
        .collect::<Vec<_>>();

    if assets.is_empty() {
        return Ok(RoutePayload::Text(format!(
            "Overview ({})\nNo asset accounts match current scope.",
            scope.label()
        )));
    }

    let account_ids = assets.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
    let labels_by_id = assets.into_iter().collect::<BTreeMap<_, _>>();

    let timeline = load_account_balance_timeline(&runtime.connection, &account_ids)?;
    if timeline.dates.is_empty() {
        return Ok(RoutePayload::Text(format!(
            "Overview ({})\nNo balance history yet.",
            scope.label()
        )));
    }

    let selected_series = select_top_account_series(
        &timeline.series_by_account,
        OVERVIEW_MAX_SERIES,
        "Other accounts",
    );
    if selected_series.is_empty() {
        return Ok(RoutePayload::Text(format!(
            "Overview ({})\nNo balance data found.",
            scope.label()
        )));
    }

    let indices = downsample_indices(timeline.dates.len(), CHART_MAX_POINTS);
    let x_labels = indices
        .iter()
        .filter_map(|index| timeline.dates.get(*index).cloned())
        .collect::<Vec<_>>();

    let tones = [
        ChartTone::Accent1,
        ChartTone::Accent2,
        ChartTone::Accent3,
        ChartTone::Accent4,
    ];

    let series = selected_series
        .iter()
        .enumerate()
        .map(|(series_index, (account_id, balances))| {
            let points = indices
                .iter()
                .enumerate()
                .filter_map(|(x, source_index)| {
                    balances
                        .get(*source_index)
                        .map(|balance_minor| (x as f64, *balance_minor as f64 / 100.0))
                })
                .collect::<Vec<_>>();

            let label = if account_id == "__other__" {
                "Other accounts".to_owned()
            } else {
                labels_by_id.get(account_id).cloned().unwrap_or_else(|| {
                    account_id
                        .rsplit(':')
                        .next()
                        .unwrap_or(account_id)
                        .to_owned()
                })
            };

            LineChartSeries {
                label,
                tone: tones[series_index % tones.len()],
                points,
            }
        })
        .collect::<Vec<_>>();

    Ok(RoutePayload::LineChart(LineChartPayload {
        title: "Overview".to_owned(),
        subtitle: format!("{} | account balances over time", scope.label()),
        x_labels,
        series,
    }))
}

fn fetch_category_bars(
    runtime: &RuntimeContext,
    requested_group: &str,
) -> Result<RoutePayload, String> {
    let group_id = resolve_group(runtime, requested_group);
    let points = group_category_breakdown(
        &runtime.connection,
        &runtime.loaded.config,
        &group_id,
        CATEGORY_MONTHS,
        CATEGORY_LIMIT,
    )
    .map_err(|error| error.to_string())?;

    if points.is_empty() {
        return Ok(RoutePayload::Text(format!(
            "Categories ({group_id})\nNo category data."
        )));
    }

    let bars = points
        .into_iter()
        .map(|point| CategoryBarPoint {
            label: point.category,
            total_major: (point.total_minor.abs() / 100).max(0) as u64,
        })
        .collect::<Vec<_>>();

    Ok(RoutePayload::CategoryBars(CategoryBarsPayload {
        title: "Categories".to_owned(),
        subtitle: format!("{group_id} | median spend distribution ({CATEGORY_MONTHS}mo)"),
        points: bars,
    }))
}

fn fetch_reports(runtime: &RuntimeContext, requested_group: &str) -> Result<RoutePayload, String> {
    let group_id = resolve_group(runtime, requested_group);
    let (series, totals) = report_cashflow(
        &runtime.connection,
        &runtime.loaded.config,
        &group_id,
        6,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;

    if series.is_empty() {
        return Ok(RoutePayload::Text(format!(
            "Reports ({group_id})\nNo series points."
        )));
    }

    let mut lines = vec![
        format!("Reports ({group_id}, {} points)", series.len()),
        format!(
            "Totals | income {:>10} | expenses {:>10} | net {:>10}",
            totals.income_minor, totals.expense_minor, totals.net_minor
        ),
    ];
    for point in series.iter().take(12) {
        let savings = point
            .savings_rate_pct
            .map_or_else(|| "n/a".to_owned(), |value| format!("{value:.2}%"));
        lines.push(format!(
            "{} | income {:>10} | expenses {:>10} | net {:>10} | savings {:>8}",
            point.month, point.income_minor, point.expense_minor, point.net_minor, savings
        ));
    }
    Ok(RoutePayload::Text(lines.join("\n")))
}

fn resolve_group(runtime: &RuntimeContext, requested_group: &str) -> String {
    let groups = runtime.loaded.config.group_ids();
    if groups.iter().any(|group| group == requested_group) {
        return requested_group.to_owned();
    }
    groups
        .first()
        .cloned()
        .unwrap_or_else(|| requested_group.to_owned())
}

#[derive(Debug, Clone)]
struct BalanceTimeline {
    dates: Vec<String>,
    series_by_account: BTreeMap<String, Vec<i64>>,
}

fn load_account_balance_timeline(
    connection: &Connection,
    account_ids: &[String],
) -> Result<BalanceTimeline, String> {
    if account_ids.is_empty() {
        return Ok(BalanceTimeline {
            dates: Vec::new(),
            series_by_account: BTreeMap::new(),
        });
    }

    let placeholders = sql_placeholders(account_ids.len());
    let sql = format!(
        "SELECT je.posted_date,\n                p.account_id,\n                SUM(p.amount_minor) AS delta_minor\n         FROM postings p\n         JOIN journal_entries je ON je.id = p.journal_entry_id\n         WHERE p.account_id IN ({placeholders})\n         GROUP BY je.posted_date, p.account_id\n         ORDER BY je.posted_date ASC"
    );

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let mut rows = statement
        .query(params_from_iter(account_ids.iter()))
        .map_err(|error| error.to_string())?;

    let mut dates = BTreeSet::new();
    let mut deltas = BTreeMap::<String, BTreeMap<String, i64>>::new();

    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let date: String = row.get(0).map_err(|error| error.to_string())?;
        let account_id: String = row.get(1).map_err(|error| error.to_string())?;
        let delta_minor: i64 = row.get(2).map_err(|error| error.to_string())?;
        dates.insert(date.clone());
        deltas
            .entry(date)
            .or_default()
            .entry(account_id)
            .and_modify(|current| *current += delta_minor)
            .or_insert(delta_minor);
    }

    let ordered_dates = dates.into_iter().collect::<Vec<_>>();
    let mut running = account_ids
        .iter()
        .map(|id| (id.clone(), 0_i64))
        .collect::<BTreeMap<_, _>>();
    let mut series_by_account = account_ids
        .iter()
        .map(|id| (id.clone(), Vec::<i64>::with_capacity(ordered_dates.len())))
        .collect::<BTreeMap<_, _>>();

    for date in &ordered_dates {
        if let Some(date_deltas) = deltas.get(date) {
            for (account_id, delta_minor) in date_deltas {
                if let Some(current) = running.get_mut(account_id) {
                    *current += *delta_minor;
                }
            }
        }

        for account_id in account_ids {
            let value = *running.get(account_id).unwrap_or(&0_i64);
            if let Some(series) = series_by_account.get_mut(account_id) {
                series.push(value);
            }
        }
    }

    Ok(BalanceTimeline {
        dates: ordered_dates,
        series_by_account,
    })
}

fn select_top_account_series(
    input: &BTreeMap<String, Vec<i64>>,
    max_series: usize,
    other_id: &str,
) -> BTreeMap<String, Vec<i64>> {
    if input.is_empty() || max_series == 0 {
        return BTreeMap::new();
    }

    let mut ranked = input
        .iter()
        .map(|(account_id, balances)| {
            let latest = balances.last().copied().unwrap_or(0_i64).abs();
            (account_id.clone(), latest)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));

    let mut selected_ids = ranked
        .iter()
        .take(max_series)
        .map(|(account_id, _)| account_id.clone())
        .collect::<Vec<_>>();
    selected_ids.sort();

    let mut output = BTreeMap::new();
    for account_id in &selected_ids {
        if let Some(series) = input.get(account_id) {
            output.insert(account_id.clone(), series.clone());
        }
    }

    if ranked.len() > max_series {
        let remainder = ranked
            .iter()
            .skip(max_series)
            .map(|(account_id, _)| account_id.clone())
            .collect::<Vec<_>>();
        let Some(len) = input.values().next().map(Vec::len) else {
            return output;
        };
        let mut merged = vec![0_i64; len];
        for account_id in remainder {
            if let Some(series) = input.get(&account_id) {
                for (index, value) in series.iter().enumerate() {
                    merged[index] += value;
                }
            }
        }
        output.insert(other_id.to_owned(), merged);
    }

    output
}

fn downsample_indices(total: usize, max_points: usize) -> Vec<usize> {
    if total == 0 || max_points == 0 {
        return Vec::new();
    }
    if total <= max_points {
        return (0..total).collect();
    }

    let step = total.div_ceil(max_points);
    let mut indices = (0..total).step_by(step).collect::<Vec<_>>();
    if indices.last().copied() != Some(total - 1) {
        indices.push(total - 1);
    }
    indices
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
