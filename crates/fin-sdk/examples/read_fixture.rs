use std::collections::BTreeMap;
use std::path::PathBuf;

use fin_sdk::config::load_config;
use fin_sdk::db::{OpenDatabaseOptions, open_database};
use fin_sdk::queries::{
    LedgerQueryOptions, TransactionQueryOptions, group_asset_account_ids, view_accounts,
    view_ledger, view_transactions,
};
use fin_sdk::reports::{report_cashflow, report_reserves, report_runway, report_summary};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct SummaryDashboardPayload {
    generated_at: String,
    consolidated_net_worth_minor: i64,
    groups: Vec<SummaryDashboardRow>,
}

#[derive(Debug, Serialize)]
struct SummaryDashboardRow {
    group_id: String,
    net_worth_minor: i64,
    latest_runway_months: Option<f64>,
    latest_available_minor: Option<i64>,
    latest_account_count: usize,
}

fn run_iterations<T, F>(iterations: usize, mut operation: F) -> T
where
    F: FnMut() -> T,
{
    let iterations = iterations.max(1);
    let mut result = operation();
    for _ in 1..iterations {
        result = operation();
    }
    result
}

fn main() {
    let mut args = std::env::args().skip(1);
    let command = args
        .next()
        .unwrap_or_else(|| "summary-dashboard".to_owned());
    let home_dir = args
        .next()
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("FIN_HOME").map(PathBuf::from))
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("target")
                .join("bench-fixtures")
                .join("benchmark-runtime")
        });
    let iterations = std::env::var("READ_FIXTURE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1);

    let config_path = home_dir.join("data/fin.config.toml");
    let data_dir = home_dir.join("data");
    let db_path = data_dir.join("fin.db");
    let loaded =
        load_config(Some(&config_path)).unwrap_or_else(|error| panic!("load config: {error}"));
    let connection = open_database(OpenDatabaseOptions {
        path: Some(db_path),
        config_dir: Some(data_dir),
        create: false,
        readonly: true,
        migrate: true,
    })
    .unwrap_or_else(|error| panic!("open db: {error}"));

    match command.as_str() {
        "summary-dashboard" => {
            let payload = run_iterations(iterations, || {
                let summary =
                    report_summary(&connection, &loaded.config, 12).expect("summary report");
                let mut groups = Vec::new();
                for group_id in loaded.config.group_ids() {
                    let runway = report_runway(&connection, &loaded.config, &group_id, None, None)
                        .expect("runway");
                    let reserves =
                        report_reserves(&connection, &loaded.config, &group_id, None, None)
                            .expect("reserves");
                    let accounts = view_accounts(&connection, &loaded.config, Some(&group_id))
                        .expect("accounts");
                    let group = summary.groups.get(&group_id).expect("group summary");
                    groups.push(SummaryDashboardRow {
                        group_id,
                        net_worth_minor: group.net_worth_minor,
                        latest_runway_months: runway.last().map(|point| point.runway_months),
                        latest_available_minor: reserves.last().map(|point| point.available_minor),
                        latest_account_count: accounts.len(),
                    });
                }
                SummaryDashboardPayload {
                    generated_at: summary.generated_at,
                    consolidated_net_worth_minor: summary.consolidated.net_worth_minor,
                    groups,
                }
            });
            serde_json::to_writer(std::io::stdout(), &payload)
                .expect("write summary dashboard payload");
        }
        "transactions-personal" => {
            let payload = run_iterations(iterations, || {
                view_transactions(
                    &connection,
                    &TransactionQueryOptions {
                        chart_account_ids: Some(group_asset_account_ids(
                            &loaded.config,
                            "personal",
                        )),
                        limit: 1_000,
                        ..TransactionQueryOptions::default()
                    },
                )
                .expect("transactions")
            });
            serde_json::to_writer(std::io::stdout(), &payload).expect("write transactions");
        }
        "cashflow-business" => {
            let (series, totals) = run_iterations(iterations, || {
                report_cashflow(&connection, &loaded.config, "business", 24, None, None)
                    .expect("cashflow")
            });
            serde_json::to_writer(
                std::io::stdout(),
                &BTreeMap::from([
                    (
                        "series",
                        serde_json::to_value(series).expect("series value"),
                    ),
                    (
                        "totals",
                        serde_json::to_value(totals).expect("totals value"),
                    ),
                ]),
            )
            .expect("write cashflow");
        }
        "accounts" => {
            let group_id = args.next();
            let payload = run_iterations(iterations, || {
                view_accounts(&connection, &loaded.config, group_id.as_deref()).expect("accounts")
            });
            serde_json::to_writer(std::io::stdout(), &payload).expect("write accounts");
        }
        "ledger" => {
            let limit = args
                .next()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(500);
            let account_id = args.next();
            let payload = run_iterations(iterations, || {
                view_ledger(
                    &connection,
                    &LedgerQueryOptions {
                        account_id: account_id.clone(),
                        limit,
                        ..LedgerQueryOptions::default()
                    },
                )
                .expect("ledger")
            });
            serde_json::to_writer(std::io::stdout(), &payload).expect("write ledger");
        }
        other => {
            eprintln!("unsupported fixture read command: {other}");
            std::process::exit(1);
        }
    }
}
