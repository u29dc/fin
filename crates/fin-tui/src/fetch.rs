use fin_sdk::config::{LoadedConfig, load_config};
use fin_sdk::db::{OpenDatabaseOptions, open_database, resolve_db_path};
use fin_sdk::{TransactionQueryOptions, report_cashflow, report_summary, view_transactions};
use rusqlite::Connection;

use crate::routes::Route;

pub const TUI_TRANSACTIONS_PREVIEW_LIMIT: usize = 1000;

#[derive(Debug, Clone)]
pub struct TransactionTableRow {
    pub posted_at: String,
    pub from_account: String,
    pub to_account: String,
    pub amount_minor: i64,
    pub description: String,
    pub counterparty: String,
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

    pub fn fetch_route(&mut self, route: Route) -> Result<RoutePayload, String> {
        self.ensure_runtime()?;
        let Some(runtime) = self.runtime.as_ref() else {
            return Err("runtime not initialized".to_owned());
        };

        match route {
            Route::Overview => fetch_overview(runtime),
            Route::Transactions => fetch_transactions(runtime),
            Route::Reports => fetch_reports(runtime),
        }
    }

    fn ensure_runtime(&mut self) -> Result<(), String> {
        if self.runtime.is_none() {
            self.runtime = Some(RuntimeContext::open()?);
        }
        Ok(())
    }
}

fn fetch_overview(runtime: &RuntimeContext) -> Result<RoutePayload, String> {
    let summary = report_summary(&runtime.connection, &runtime.loaded.config, 12)
        .map_err(|error| error.to_string())?;

    let mut lines = vec![
        "Overview".to_owned(),
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

fn fetch_reports(runtime: &RuntimeContext) -> Result<RoutePayload, String> {
    let selected_group = runtime
        .loaded
        .config
        .group_ids()
        .into_iter()
        .find(|group| group == "personal")
        .or_else(|| runtime.loaded.config.group_ids().into_iter().next())
        .ok_or_else(|| "No groups configured".to_owned())?;

    let (series, totals) = report_cashflow(
        &runtime.connection,
        &runtime.loaded.config,
        &selected_group,
        6,
        None,
        None,
    )
    .map_err(|error| error.to_string())?;

    if series.is_empty() {
        return Ok(RoutePayload::Text(format!(
            "Cashflow ({selected_group})\nNo series points."
        )));
    }

    let mut lines = vec![
        format!("Cashflow ({selected_group}, {} points)", series.len()),
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
