use fin_sdk::config::{LoadedConfig, load_config};
use fin_sdk::db::{OpenDatabaseOptions, open_database, resolve_db_path};
use fin_sdk::{TransactionQueryOptions, report_cashflow, report_summary, view_transactions};
use rusqlite::Connection;

use crate::routes::Route;

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

    pub fn fetch_route(&mut self, route: Route) -> String {
        if let Err(error) = self.ensure_runtime() {
            return format!("Route unavailable: {error}");
        }
        let Some(runtime) = self.runtime.as_ref() else {
            return "Route unavailable: runtime not initialized".to_owned();
        };

        match route {
            Route::Overview => fetch_overview(runtime),
            Route::Transactions => fetch_transactions(runtime),
            Route::Reports => fetch_reports(runtime),
        }
        .unwrap_or_else(|error| format!("Route unavailable: {error}"))
    }

    fn ensure_runtime(&mut self) -> Result<(), String> {
        if self.runtime.is_none() {
            self.runtime = Some(RuntimeContext::open()?);
        }
        Ok(())
    }
}

fn fetch_overview(runtime: &RuntimeContext) -> Result<String, String> {
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

    Ok(lines.join("\n"))
}

fn fetch_transactions(runtime: &RuntimeContext) -> Result<String, String> {
    const TUI_TRANSACTIONS_PREVIEW_LIMIT: usize = 1000;
    let rows = view_transactions(
        &runtime.connection,
        &TransactionQueryOptions {
            limit: TUI_TRANSACTIONS_PREVIEW_LIMIT,
            ..TransactionQueryOptions::default()
        },
    )
    .map_err(|error| error.to_string())?;

    if rows.is_empty() {
        return Ok("Transactions\nNo rows.".to_owned());
    }

    let mut lines = vec![format!("Transactions (latest {})", rows.len())];
    for row in &rows {
        lines.push(format!(
            "{} | {:<30} | {:>10} | {}",
            row.posted_at,
            truncate_account(&row.chart_account_id),
            row.amount_minor,
            truncate_text(&row.clean_description, 36)
        ));
    }
    if rows.len() == TUI_TRANSACTIONS_PREVIEW_LIMIT {
        lines.push(
            "... preview limit reached (use `:fin view transactions --limit N` for larger slices)"
                .to_owned(),
        );
    }
    Ok(lines.join("\n"))
}

fn fetch_reports(runtime: &RuntimeContext) -> Result<String, String> {
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
        return Ok(format!("Cashflow ({selected_group})\nNo series points."));
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
    Ok(lines.join("\n"))
}

fn truncate_account(value: &str) -> String {
    truncate_text(value, 30)
}

fn truncate_text(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    if max <= 3 {
        return value.chars().take(max).collect();
    }
    let mut out = value.chars().take(max - 3).collect::<String>();
    out.push_str("...");
    out
}
