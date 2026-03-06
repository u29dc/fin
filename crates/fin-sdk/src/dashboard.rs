use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::FinConfig;
use crate::error::Result;
use crate::queries::{MonthlyCashflowPoint, group_monthly_cashflow};
use crate::stats::{mean_i64, median_i64};

const TREND_WINDOW_MONTHS: usize = 3;
const MEDIAN_SPEND_WINDOW_MONTHS: usize = 12;
const ANOMALY_LOOKBACK_MONTHS: usize = 12;
const RECENT_ANOMALY_LIMIT: usize = 3;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShortTermTrend {
    Positive,
    Negative,
    Flat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashflowKpis {
    pub current_month: String,
    pub current_partial_month: Option<MonthlyCashflowPoint>,
    pub last_full_month: Option<MonthlyCashflowPoint>,
    pub previous_full_month: Option<MonthlyCashflowPoint>,
    pub trailing_average_net_minor: Option<i64>,
    pub median_spend_minor: Option<i64>,
    pub short_term_trend: Option<ShortTermTrend>,
    pub anomaly_count_last_12_months: usize,
    pub recent_anomaly_months: Vec<String>,
}

fn take_last_n<T>(values: &[T], count: usize) -> &[T] {
    let start = values.len().saturating_sub(count);
    &values[start..]
}

pub fn current_reporting_month(connection: &Connection) -> Result<String> {
    connection
        .query_row("SELECT strftime('%Y-%m', 'now')", [], |row| row.get(0))
        .map_err(Into::into)
}

#[must_use]
pub fn summarize_cashflow_kpis(
    series: &[MonthlyCashflowPoint],
    current_month: &str,
) -> CashflowKpis {
    let current_partial_month = series
        .iter()
        .find(|point| point.month == current_month)
        .cloned();
    let full_months = series
        .iter()
        .filter(|point| point.month.as_str() < current_month)
        .cloned()
        .collect::<Vec<_>>();

    let last_full_month = full_months.last().cloned();
    let previous_full_month = full_months.iter().rev().nth(1).cloned();

    let trailing_window = take_last_n(&full_months, TREND_WINDOW_MONTHS);
    let trailing_average_net_minor = mean_i64(
        &trailing_window
            .iter()
            .map(|point| point.net_minor)
            .collect::<Vec<_>>(),
    );
    let short_term_trend = trailing_average_net_minor.map(|value| {
        if value > 0 {
            ShortTermTrend::Positive
        } else if value < 0 {
            ShortTermTrend::Negative
        } else {
            ShortTermTrend::Flat
        }
    });

    let median_spend_window = take_last_n(&full_months, MEDIAN_SPEND_WINDOW_MONTHS);
    let median_spend_minor = median_i64(
        &median_spend_window
            .iter()
            .map(|point| point.expense_minor)
            .collect::<Vec<_>>(),
    );

    let anomaly_window = take_last_n(&full_months, ANOMALY_LOOKBACK_MONTHS);
    let recent_anomaly_months = anomaly_window
        .iter()
        .filter(|point| point.is_anomaly)
        .map(|point| point.month.clone())
        .collect::<Vec<_>>();

    CashflowKpis {
        current_month: current_month.to_owned(),
        current_partial_month,
        last_full_month,
        previous_full_month,
        trailing_average_net_minor,
        median_spend_minor,
        short_term_trend,
        anomaly_count_last_12_months: recent_anomaly_months.len(),
        recent_anomaly_months: take_last_n(&recent_anomaly_months, RECENT_ANOMALY_LIMIT).to_vec(),
    }
}

pub fn report_cashflow_kpis(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    months: usize,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<CashflowKpis> {
    let current_month = current_reporting_month(connection)?;
    let series = group_monthly_cashflow(connection, config, group_id, from, to, months)?;
    Ok(summarize_cashflow_kpis(&series, &current_month))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{CashflowKpis, ShortTermTrend, report_cashflow_kpis, summarize_cashflow_kpis};
    use crate::queries::MonthlyCashflowPoint;
    use crate::runtime::{RuntimeContext, RuntimeContextOptions};
    use crate::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

    fn point(
        month: &str,
        income_minor: i64,
        expense_minor: i64,
        rolling_median_expense_minor: Option<i64>,
        expense_deviation_ratio: Option<f64>,
        is_anomaly: bool,
    ) -> MonthlyCashflowPoint {
        MonthlyCashflowPoint {
            month: month.to_owned(),
            income_minor,
            expense_minor,
            net_minor: income_minor - expense_minor,
            savings_rate_pct: Some(
                ((income_minor - expense_minor) as f64 / income_minor as f64) * 100.0,
            ),
            rolling_median_expense_minor,
            expense_deviation_ratio,
            is_anomaly,
        }
    }

    #[test]
    fn kpis_ignore_current_partial_and_future_months() {
        let summary = summarize_cashflow_kpis(
            &[
                point("2026-01", 1_000, 700, Some(650), Some(1.08), false),
                point("2026-02", 1_000, 800, Some(700), Some(1.14), false),
                point("2026-03", 500, 300, Some(750), Some(0.4), true),
                point("2026-04", 1_100, 600, Some(700), Some(0.86), false),
            ],
            "2026-03",
        );

        assert_eq!(
            summary
                .last_full_month
                .as_ref()
                .map(|point| point.month.as_str()),
            Some("2026-02")
        );
        assert_eq!(
            summary
                .current_partial_month
                .as_ref()
                .map(|point| point.month.as_str()),
            Some("2026-03")
        );
        assert_eq!(summary.trailing_average_net_minor, Some(250));
        assert_eq!(summary.median_spend_minor, Some(750));
        assert_eq!(summary.short_term_trend, Some(ShortTermTrend::Positive));
        assert_eq!(summary.anomaly_count_last_12_months, 0);
    }

    #[test]
    fn kpis_handle_sparse_history() {
        let summary: CashflowKpis = summarize_cashflow_kpis(
            &[
                point("2026-01", 1_000, 900, None, None, false),
                point("2026-02", 1_000, 950, None, None, false),
                point("2026-03", 300, 120, None, None, false),
            ],
            "2026-03",
        );

        assert_eq!(
            summary
                .last_full_month
                .as_ref()
                .map(|point| point.month.as_str()),
            Some("2026-02")
        );
        assert_eq!(
            summary
                .previous_full_month
                .as_ref()
                .map(|point| point.month.as_str()),
            Some("2026-01")
        );
        assert_eq!(summary.trailing_average_net_minor, Some(75));
        assert_eq!(summary.median_spend_minor, Some(925));
        assert_eq!(summary.short_term_trend, Some(ShortTermTrend::Positive));
        assert!(summary.recent_anomaly_months.is_empty());
    }

    #[test]
    fn fixture_backed_kpis_surface_recent_anomalies() {
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

        let summary = report_cashflow_kpis(
            runtime.connection(),
            runtime.config(),
            "business",
            36,
            None,
            Some("2026-03-31"),
        )
        .expect("report cashflow kpis");

        assert_eq!(
            summary
                .last_full_month
                .as_ref()
                .map(|point| point.month.as_str()),
            Some("2026-02")
        );
        assert!(summary.trailing_average_net_minor.is_some());
        assert!(summary.median_spend_minor.is_some());
        assert_eq!(summary.short_term_trend, Some(ShortTermTrend::Positive));
        assert!(summary.anomaly_count_last_12_months >= 1);
        assert!(
            summary
                .recent_anomaly_months
                .iter()
                .any(|month| month == "2025-04")
        );
    }
}
