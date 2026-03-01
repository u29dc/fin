use std::process::Command;

use serde_json::Value;

use crate::routes::Route;

#[derive(Debug, Default)]
pub struct FetchClient;

impl FetchClient {
    pub fn new() -> Self {
        Self
    }

    pub fn fetch_route(&self, route: Route) -> String {
        match route {
            Route::Overview => self.fetch_overview(),
            Route::Transactions => self.fetch_transactions(),
            Route::Reports => self.fetch_reports(),
        }
    }

    fn fetch_overview(&self) -> String {
        match run_cli_json(["report", "summary"]) {
            Ok(payload) => format_overview(payload),
            Err(error) => format!("Overview unavailable: {error}"),
        }
    }

    fn fetch_transactions(&self) -> String {
        match run_cli_json(["view", "transactions", "--limit=20"]) {
            Ok(payload) => format_transactions(payload),
            Err(error) => format!("Transactions unavailable: {error}"),
        }
    }

    fn fetch_reports(&self) -> String {
        match run_cli_json(["report", "cashflow", "--group=personal", "--months=6"]) {
            Ok(payload) => format_cashflow(payload),
            Err(error) => format!("Reports unavailable: {error}"),
        }
    }
}

fn run_cli_json<const N: usize>(args: [&str; N]) -> Result<Value, String> {
    let output = Command::new("bun")
        .arg("run")
        .arg("packages/cli/src/index.ts")
        .args(args)
        .arg("--json")
        .output()
        .map_err(|error| format!("failed to launch bun runtime: {error}"))?;

    if output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "no JSON payload produced (exit {}) stderr: {}",
            output.status.code().unwrap_or(1),
            stderr.trim()
        ));
    }

    serde_json::from_slice(&output.stdout).map_err(|error| {
        format!(
            "failed to parse CLI JSON output: {error}. raw={}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn format_overview(payload: Value) -> String {
    let Some(data) = payload.get("data") else {
        return "Overview payload missing data".to_owned();
    };
    let generated_at = data
        .get("generatedAt")
        .and_then(Value::as_str)
        .unwrap_or("n/a");
    let period = data
        .get("periodMonths")
        .and_then(Value::as_u64)
        .map_or_else(|| "n/a".to_owned(), |value| value.to_string());

    let group_count = data
        .get("groups")
        .and_then(Value::as_object)
        .map_or(0usize, |groups| groups.len());

    let net_worth = data
        .get("consolidated")
        .and_then(|value| value.get("netWorthMinor"))
        .and_then(Value::as_i64)
        .unwrap_or(0);

    format!(
        "Overview\\nGenerated: {generated_at}\\nPeriod (months): {period}\\nGroups: {group_count}\\nConsolidated net worth (minor): {net_worth}"
    )
}

fn format_transactions(payload: Value) -> String {
    let Some(transactions) = payload
        .get("data")
        .and_then(|value| value.get("transactions"))
        .and_then(Value::as_array)
    else {
        return "Transactions payload missing data.transactions".to_owned();
    };

    if transactions.is_empty() {
        return "Transactions\\nNo rows.".to_owned();
    }

    let mut lines = vec![format!("Transactions ({})", transactions.len())];
    for item in transactions.iter().take(12) {
        let date = item.get("date").and_then(Value::as_str).unwrap_or("n/a");
        let account = item.get("account").and_then(Value::as_str).unwrap_or("n/a");
        let amount = item.get("amount").and_then(Value::as_i64).unwrap_or(0);
        let description = item
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("n/a");
        lines.push(format!("{date} | {account} | {amount:>8} | {description}"));
    }
    lines.join("\n")
}

fn format_cashflow(payload: Value) -> String {
    let Some(series) = payload
        .get("data")
        .and_then(|value| value.get("series"))
        .and_then(Value::as_array)
    else {
        return "Reports payload missing data.series".to_owned();
    };

    if series.is_empty() {
        return "Cashflow\\nNo series points.".to_owned();
    }

    let mut lines = vec![format!("Cashflow ({})", series.len())];
    for point in series.iter().take(12) {
        let month = point.get("month").and_then(Value::as_str).unwrap_or("n/a");
        let income = point.get("income").and_then(Value::as_i64).unwrap_or(0);
        let expenses = point.get("expenses").and_then(Value::as_i64).unwrap_or(0);
        let net = point.get("net").and_then(Value::as_i64).unwrap_or(0);
        let savings_rate = point
            .get("savingsRate")
            .and_then(Value::as_f64)
            .map_or_else(|| "n/a".to_owned(), |value| format!("{value:.2}%"));

        lines.push(format!(
            "{month} | income {income:>8} | expenses {expenses:>8} | net {net:>8} | savings {savings_rate}"
        ));
    }
    lines.join("\n")
}
