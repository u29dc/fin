use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::config::FinConfig;
use crate::error::Result;
use crate::queries::group_asset_account_ids;

const DEFAULT_POINT_LIMIT: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DailyBalancePoint {
    pub date: String,
    pub balance_minor: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContributionPoint {
    pub date: String,
    pub contributions_minor: i64,
}

/// Shared daily-series options.
///
/// `from` and `to` filter on `journal_entries.posted_date`.
/// `limit` applies after any optional downsampling and keeps the most recent points.
/// `downsample_min_step_days` is opt-in and never applies unless explicitly set.
#[derive(Debug, Clone, Default)]
pub struct BalanceSeriesQueryOptions {
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: usize,
    pub downsample_min_step_days: Option<u32>,
}

#[derive(Debug)]
struct AccountMappingSql {
    case_clause: String,
    or_clause: String,
    case_params: Vec<String>,
    or_params: Vec<String>,
}

#[derive(Debug)]
struct DailyBalanceRow {
    chart_account_id: String,
    date: String,
    daily_amount: i64,
}

#[derive(Debug)]
struct StartingBalanceRow {
    chart_account_id: String,
    balance: i64,
}

#[derive(Debug)]
struct ContributionPostingRow {
    date: String,
    amount_minor: i64,
}

/// Return a sparse daily running-balance series for one account id, including descendant accounts.
pub fn account_daily_balance_series(
    connection: &Connection,
    account_id: &str,
    options: &BalanceSeriesQueryOptions,
) -> Result<Vec<DailyBalancePoint>> {
    Ok(
        all_accounts_daily_balance_series(connection, &[account_id.to_owned()], options)?
            .remove(account_id)
            .unwrap_or_default(),
    )
}

/// Return sparse daily running-balance series for multiple accounts in one batched query.
pub fn all_accounts_daily_balance_series(
    connection: &Connection,
    account_ids: &[String],
    options: &BalanceSeriesQueryOptions,
) -> Result<BTreeMap<String, Vec<DailyBalancePoint>>> {
    let mut series = all_accounts_daily_balance_series_raw(connection, account_ids, options)?;
    for points in series.values_mut() {
        *points = finalize_series(points, options);
    }
    Ok(series)
}

/// Return a merged daily balance timeline by forward-filling each account on the union of observed dates.
pub fn merged_accounts_daily_balance_series(
    connection: &Connection,
    account_ids: &[String],
    options: &BalanceSeriesQueryOptions,
) -> Result<Vec<DailyBalancePoint>> {
    let raw = all_accounts_daily_balance_series_raw(connection, account_ids, options)?;
    Ok(finalize_series(
        &merge_balance_series_by_date(&raw),
        options,
    ))
}

/// Return sparse per-account daily balance series for all asset accounts in a group.
pub fn group_account_balance_series(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    options: &BalanceSeriesQueryOptions,
) -> Result<BTreeMap<String, Vec<DailyBalancePoint>>> {
    let account_ids = group_asset_account_ids(config, group_id);
    all_accounts_daily_balance_series(connection, &account_ids, options)
}

/// Return a merged daily balance timeline for all asset accounts in a group.
pub fn group_daily_balance_series(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    options: &BalanceSeriesQueryOptions,
) -> Result<Vec<DailyBalancePoint>> {
    let account_ids = group_asset_account_ids(config, group_id);
    merged_accounts_daily_balance_series(connection, &account_ids, options)
}

#[must_use]
pub fn merge_balance_series_by_date(
    series_by_account: &BTreeMap<String, Vec<DailyBalancePoint>>,
) -> Vec<DailyBalancePoint> {
    if series_by_account.is_empty() {
        return Vec::new();
    }

    let sorted_dates = collect_sorted_dates(series_by_account);
    let mut indices = BTreeMap::<String, usize>::new();
    let mut current = BTreeMap::<String, Option<i64>>::new();
    for account_id in series_by_account.keys() {
        indices.insert(account_id.clone(), 0);
        current.insert(account_id.clone(), None);
    }

    let mut merged = Vec::with_capacity(sorted_dates.len());
    for date in sorted_dates {
        let mut total = 0i64;
        for (account_id, points) in series_by_account {
            let index = indices.entry(account_id.clone()).or_insert(0);
            while *index < points.len() && points[*index].date == date {
                current.insert(account_id.clone(), Some(points[*index].balance_minor));
                *index += 1;
            }
            if let Some(balance_minor) = current.get(account_id).copied().flatten() {
                total += balance_minor;
            }
        }
        merged.push(DailyBalancePoint {
            date,
            balance_minor: total,
        });
    }
    merged
}

/// Return a cumulative posting-based contribution series for one account id, including descendants.
pub fn cumulative_contribution_series(
    connection: &Connection,
    account_id: &str,
    options: &BalanceSeriesQueryOptions,
) -> Result<Vec<ContributionPoint>> {
    let mut clauses = vec!["(p.account_id = ?1 OR p.account_id LIKE ?2)".to_owned()];
    let mut params = vec![account_id.to_owned(), format!("{account_id}:%")];

    if let Some(from) = &options.from {
        clauses.push("je.posted_date >= ?".to_owned());
        params.push(from.clone());
    }
    if let Some(to) = &options.to {
        clauses.push("je.posted_date <= ?".to_owned());
        params.push(to.clone());
    }

    let sql = format!(
        "SELECT je.posted_date AS date,\n                p.amount_minor\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         WHERE {}\n         ORDER BY je.posted_date ASC, je.posted_at ASC, p.id ASC",
        clauses.join(" AND ")
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok(ContributionPostingRow {
            date: row.get(0)?,
            amount_minor: row.get(1)?,
        })
    })?;

    let mut running = 0i64;
    let mut series = Vec::<ContributionPoint>::new();
    for row in rows {
        let row = row?;
        running += row.amount_minor;
        if let Some(last) = series.last_mut()
            && last.date == row.date
        {
            last.contributions_minor = running;
            continue;
        }
        series.push(ContributionPoint {
            date: row.date,
            contributions_minor: running,
        });
    }

    Ok(finalize_series(&series, options))
}

fn all_accounts_daily_balance_series_raw(
    connection: &Connection,
    account_ids: &[String],
    options: &BalanceSeriesQueryOptions,
) -> Result<BTreeMap<String, Vec<DailyBalancePoint>>> {
    let mut result = BTreeMap::<String, Vec<DailyBalancePoint>>::new();
    if account_ids.is_empty() {
        return Ok(result);
    }

    let mapping = build_account_mapping_sql(account_ids);
    let mut params = mapping.case_params.clone();
    params.extend(mapping.or_params.clone());

    let mut clauses = vec![format!("({})", mapping.or_clause)];
    if let Some(from) = &options.from {
        clauses.push("je.posted_date >= ?".to_owned());
        params.push(from.clone());
    }
    if let Some(to) = &options.to {
        clauses.push("je.posted_date <= ?".to_owned());
        params.push(to.clone());
    }

    let sql = format!(
        "SELECT {} AS chart_account_id,\n                je.posted_date AS date,\n                COALESCE(SUM(p.amount_minor), 0) AS daily_amount\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         WHERE {}\n         GROUP BY chart_account_id, je.posted_date\n         ORDER BY chart_account_id ASC, date ASC",
        mapping.case_clause,
        clauses.join(" AND ")
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok(DailyBalanceRow {
            chart_account_id: row.get(0)?,
            date: row.get(1)?,
            daily_amount: row.get(2)?,
        })
    })?;

    let starting_balances = if let Some(from) = options.from.as_deref() {
        fetch_starting_balances(connection, &mapping, from)?
    } else {
        BTreeMap::new()
    };
    let mut running_balances = BTreeMap::<String, i64>::new();

    for account_id in account_ids {
        result.insert(account_id.clone(), Vec::new());
        running_balances.insert(
            account_id.clone(),
            starting_balances.get(account_id).copied().unwrap_or(0),
        );
    }

    for row in rows {
        let row = row?;
        let running_balance = running_balances
            .entry(row.chart_account_id.clone())
            .or_insert(0);
        *running_balance += row.daily_amount;
        result
            .entry(row.chart_account_id)
            .or_default()
            .push(DailyBalancePoint {
                date: row.date,
                balance_minor: *running_balance,
            });
    }

    Ok(result)
}

fn build_account_mapping_sql(account_ids: &[String]) -> AccountMappingSql {
    let mut case_expressions = Vec::new();
    let mut case_params = Vec::new();
    let mut or_conditions = Vec::new();
    let mut or_params = Vec::new();

    for account_id in account_ids {
        case_expressions.push("WHEN p.account_id = ? OR p.account_id LIKE ? THEN ?".to_owned());
        case_params.push(account_id.clone());
        case_params.push(format!("{account_id}:%"));
        case_params.push(account_id.clone());

        or_conditions.push("(p.account_id = ? OR p.account_id LIKE ?)".to_owned());
        or_params.push(account_id.clone());
        or_params.push(format!("{account_id}:%"));
    }

    AccountMappingSql {
        case_clause: format!("CASE {} END", case_expressions.join(" ")),
        or_clause: or_conditions.join(" OR "),
        case_params,
        or_params,
    }
}

fn fetch_starting_balances(
    connection: &Connection,
    mapping: &AccountMappingSql,
    from_date: &str,
) -> Result<BTreeMap<String, i64>> {
    let mut params = mapping.case_params.clone();
    params.extend(mapping.or_params.clone());
    params.push(from_date.to_owned());

    let sql = format!(
        "SELECT {} AS chart_account_id,\n                COALESCE(SUM(p.amount_minor), 0) AS balance\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         WHERE ({}) AND je.posted_date < ?\n         GROUP BY chart_account_id",
        mapping.case_clause, mapping.or_clause
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok(StartingBalanceRow {
            chart_account_id: row.get(0)?,
            balance: row.get(1)?,
        })
    })?;

    let mut balances = BTreeMap::new();
    for row in rows {
        let row = row?;
        balances.insert(row.chart_account_id, row.balance);
    }
    Ok(balances)
}

fn finalize_series<T>(points: &[T], options: &BalanceSeriesQueryOptions) -> Vec<T>
where
    T: Clone + HasDate,
{
    let mut finalized = if let Some(step_days) = options.downsample_min_step_days {
        downsample_daily_series(points, step_days)
    } else {
        points.to_vec()
    };
    let limit = normalize_limit(options.limit);
    if finalized.len() > limit {
        let start = finalized.len().saturating_sub(limit);
        finalized = finalized[start..].to_vec();
    }
    finalized
}

fn normalize_limit(limit: usize) -> usize {
    if limit == 0 {
        DEFAULT_POINT_LIMIT
    } else {
        limit
    }
}

fn collect_sorted_dates(
    series_by_account: &BTreeMap<String, Vec<DailyBalancePoint>>,
) -> Vec<String> {
    let mut dates = BTreeSet::new();
    for points in series_by_account.values() {
        for point in points {
            dates.insert(point.date.clone());
        }
    }
    dates.into_iter().collect()
}

trait HasDate {
    fn date(&self) -> &str;
}

impl HasDate for DailyBalancePoint {
    fn date(&self) -> &str {
        &self.date
    }
}

impl HasDate for ContributionPoint {
    fn date(&self) -> &str {
        &self.date
    }
}

fn downsample_daily_series<T>(points: &[T], step_days: u32) -> Vec<T>
where
    T: Clone + HasDate,
{
    if points.len() <= 2 || step_days <= 1 {
        return points.to_vec();
    }

    let mut downsampled = Vec::new();
    let mut last_kept_day = None;
    for (index, point) in points.iter().enumerate() {
        if index == 0 || index + 1 == points.len() {
            if last_kept_day != day_index(point.date()) {
                last_kept_day = day_index(point.date());
                downsampled.push(point.clone());
            }
            continue;
        }
        let Some(current_day) = day_index(point.date()) else {
            return points.to_vec();
        };
        let should_keep = match last_kept_day {
            Some(previous_day) => {
                current_day - previous_day >= i32::try_from(step_days).unwrap_or(1)
            }
            None => true,
        };
        if should_keep {
            last_kept_day = Some(current_day);
            downsampled.push(point.clone());
        }
    }
    downsampled
}

fn day_index(date: &str) -> Option<i32> {
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let adjusted_year = year - i32::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - (era * 400);
    let month_index = i32::try_from(month).ok()? + if month > 2 { -3 } else { 9 };
    let day_of_year = ((153 * month_index) + 2) / 5 + i32::try_from(day).ok()? - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some((era * 146_097) + day_of_era - 719_468)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::tempdir;

    use super::{
        BalanceSeriesQueryOptions, DailyBalancePoint, account_daily_balance_series,
        cumulative_contribution_series, downsample_daily_series, group_account_balance_series,
        group_daily_balance_series, merge_balance_series_by_date,
    };
    use crate::queries::view_accounts;
    use crate::runtime::{RuntimeContext, RuntimeContextOptions};
    use crate::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

    #[test]
    fn merge_forward_fills_sparse_series_on_union_of_dates() {
        let merged = merge_balance_series_by_date(&BTreeMap::from([
            (
                "a".to_owned(),
                vec![
                    DailyBalancePoint {
                        date: "2024-01-01".to_owned(),
                        balance_minor: 100,
                    },
                    DailyBalancePoint {
                        date: "2024-01-03".to_owned(),
                        balance_minor: 140,
                    },
                ],
            ),
            (
                "b".to_owned(),
                vec![
                    DailyBalancePoint {
                        date: "2024-01-02".to_owned(),
                        balance_minor: 50,
                    },
                    DailyBalancePoint {
                        date: "2024-01-03".to_owned(),
                        balance_minor: 65,
                    },
                ],
            ),
        ]));

        assert_eq!(
            merged,
            vec![
                DailyBalancePoint {
                    date: "2024-01-01".to_owned(),
                    balance_minor: 100,
                },
                DailyBalancePoint {
                    date: "2024-01-02".to_owned(),
                    balance_minor: 150,
                },
                DailyBalancePoint {
                    date: "2024-01-03".to_owned(),
                    balance_minor: 205,
                },
            ]
        );
    }

    #[test]
    fn downsampling_keeps_first_last_and_step_boundaries() {
        let series = vec![
            DailyBalancePoint {
                date: "2024-01-01".to_owned(),
                balance_minor: 10,
            },
            DailyBalancePoint {
                date: "2024-01-02".to_owned(),
                balance_minor: 20,
            },
            DailyBalancePoint {
                date: "2024-01-03".to_owned(),
                balance_minor: 30,
            },
            DailyBalancePoint {
                date: "2024-01-04".to_owned(),
                balance_minor: 40,
            },
            DailyBalancePoint {
                date: "2024-01-05".to_owned(),
                balance_minor: 50,
            },
        ];

        let downsampled = downsample_daily_series(&series, 2);

        assert_eq!(
            downsampled,
            vec![
                DailyBalancePoint {
                    date: "2024-01-01".to_owned(),
                    balance_minor: 10,
                },
                DailyBalancePoint {
                    date: "2024-01-03".to_owned(),
                    balance_minor: 30,
                },
                DailyBalancePoint {
                    date: "2024-01-05".to_owned(),
                    balance_minor: 50,
                },
            ]
        );
    }

    #[test]
    fn filtered_account_series_preserves_starting_balance_before_window() {
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

        let full = account_daily_balance_series(
            runtime.connection(),
            "Assets:Personal:Checking",
            &BalanceSeriesQueryOptions::default(),
        )
        .expect("full series");
        let filtered = account_daily_balance_series(
            runtime.connection(),
            "Assets:Personal:Checking",
            &BalanceSeriesQueryOptions {
                from: Some("2024-06-01".to_owned()),
                ..BalanceSeriesQueryOptions::default()
            },
        )
        .expect("filtered series");

        let expected = full
            .iter()
            .find(|point| point.date.as_str() >= "2024-06-01")
            .cloned()
            .expect("matching point in full series");
        assert_eq!(filtered.first(), Some(&expected));
    }

    #[test]
    fn group_daily_balance_series_matches_latest_group_total() {
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

        let merged = group_daily_balance_series(
            runtime.connection(),
            runtime.config(),
            "personal",
            &BalanceSeriesQueryOptions::default(),
        )
        .expect("group daily balance");
        let balances = view_accounts(runtime.connection(), runtime.config(), Some("personal"))
            .expect("view accounts");
        let expected_total = balances
            .iter()
            .map(|account| account.balance_minor.unwrap_or(0))
            .sum::<i64>();

        assert_eq!(
            merged.last().map(|point| point.balance_minor),
            Some(expected_total)
        );
    }

    #[test]
    fn group_account_series_returns_sparse_per_account_histories() {
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

        let series = group_account_balance_series(
            runtime.connection(),
            runtime.config(),
            "business",
            &BalanceSeriesQueryOptions {
                downsample_min_step_days: Some(7),
                ..BalanceSeriesQueryOptions::default()
            },
        )
        .expect("group account series");

        assert!(series.contains_key("Assets:Business:Operating"));
        assert!(series.contains_key("Assets:Business:TaxReserve"));
        assert!(series.values().all(|points| !points.is_empty()));
    }

    #[test]
    fn contribution_series_is_cumulative_and_sorted() {
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

        let series = cumulative_contribution_series(
            runtime.connection(),
            "Assets:Personal:Investments",
            &BalanceSeriesQueryOptions::default(),
        )
        .expect("contribution series");

        assert!(!series.is_empty());
        assert!(series.windows(2).all(|window| {
            let [left, right] = window else {
                return true;
            };
            left.date <= right.date && left.contributions_minor <= right.contributions_minor
        }));
    }
}
