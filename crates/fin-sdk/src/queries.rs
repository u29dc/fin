use std::collections::{BTreeMap, BTreeSet, HashMap};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::config::FinConfig;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalanceRow {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub balance_minor: Option<i64>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRow {
    pub id: String,
    pub chart_account_id: String,
    pub pair_account_id: String,
    pub posted_at: String,
    pub amount_minor: i64,
    pub currency: String,
    pub raw_description: String,
    pub clean_description: String,
    pub counterparty: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostingRow {
    pub id: String,
    pub journal_entry_id: String,
    pub account_id: String,
    pub amount_minor: i64,
    pub currency: String,
    pub memo: Option<String>,
    pub provider_txn_id: Option<String>,
    pub provider_balance_minor: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntryRow {
    pub id: String,
    pub posted_at: String,
    pub posted_date: String,
    pub is_transfer: bool,
    pub description: String,
    pub raw_description: Option<String>,
    pub clean_description: Option<String>,
    pub counterparty: Option<String>,
    pub source_file: Option<String>,
    pub postings: Vec<PostingRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BalanceSheet {
    pub assets: i64,
    pub liabilities: i64,
    pub equity: i64,
    pub income: i64,
    pub expenses: i64,
    pub net_worth: i64,
    pub net_income: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyCashflowPoint {
    pub month: String,
    pub income_minor: i64,
    pub expense_minor: i64,
    pub net_minor: i64,
    pub savings_rate_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryBreakdownPoint {
    pub category: String,
    pub total_minor: i64,
    pub transaction_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryMedianPoint {
    pub category: String,
    pub monthly_median_minor: i64,
    pub month_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPayeePoint {
    pub description: String,
    pub transaction_count: i64,
    pub total_minor: i64,
    pub latest_posted_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct TransactionQueryOptions {
    pub chart_account_ids: Option<Vec<String>>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub search: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Default)]
pub struct LedgerQueryOptions {
    pub account_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: usize,
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

fn collect_group_account_ids(config: &FinConfig, group_id: &str) -> Vec<String> {
    config
        .accounts
        .iter()
        .filter(|account| account.group == group_id && account.account_type == "asset")
        .map(|account| account.id.clone())
        .collect()
}

pub fn group_asset_account_ids(config: &FinConfig, group_id: &str) -> Vec<String> {
    collect_group_account_ids(config, group_id)
}

pub fn all_group_ids(config: &FinConfig) -> Vec<String> {
    config.group_ids()
}

pub fn view_accounts(
    connection: &Connection,
    config: &FinConfig,
    group_filter: Option<&str>,
) -> Result<Vec<AccountBalanceRow>> {
    let mut asset_accounts = config
        .accounts
        .iter()
        .filter(|account| account.account_type == "asset")
        .cloned()
        .collect::<Vec<_>>();
    if let Some(group_filter) = group_filter {
        asset_accounts.retain(|account| account.group == group_filter);
    }

    let mut rows = Vec::new();
    for account in asset_accounts {
        let balance_minor = connection.query_row(
            "SELECT SUM(p.amount_minor)\n             FROM postings p\n             WHERE p.account_id = ?1",
            [&account.id],
            |row| row.get::<usize, Option<i64>>(0),
        )?;
        let updated_at = connection
            .query_row(
                "SELECT je.posted_at\n                 FROM postings p\n                 JOIN journal_entries je ON p.journal_entry_id = je.id\n                 WHERE p.account_id = ?1\n                 ORDER BY je.posted_at DESC\n                 LIMIT 1",
                [&account.id],
                |row| row.get::<usize, String>(0),
            )
            .optional()?;

        rows.push(AccountBalanceRow {
            id: account.id,
            name: account.label.unwrap_or_else(|| "Account".to_owned()),
            account_type: account.account_type,
            balance_minor,
            updated_at,
        });
    }
    Ok(rows)
}

pub fn view_transactions(
    connection: &Connection,
    options: &TransactionQueryOptions,
) -> Result<Vec<TransactionRow>> {
    let mut where_clauses = Vec::new();
    let mut params: Vec<String> = Vec::new();
    if let Some(account_ids) = &options.chart_account_ids
        && !account_ids.is_empty()
    {
        where_clauses.push(format!(
            "p.account_id IN ({})",
            placeholders(account_ids.len())
        ));
        params.extend(account_ids.clone());
    }
    if let Some(from) = &options.from {
        where_clauses.push("je.posted_at >= ?".to_owned());
        params.push(format!("{from}T00:00:00"));
    }
    if let Some(to) = &options.to {
        where_clauses.push("je.posted_at <= ?".to_owned());
        params.push(format!("{to}T23:59:59.999"));
    }
    if let Some(search) = &options.search {
        where_clauses.push(
            "(COALESCE(je.clean_description, '') LIKE ? OR COALESCE(je.raw_description, '') LIKE ? OR COALESCE(je.counterparty, '') LIKE ?)"
                .to_owned(),
        );
        let pattern = format!("%{search}%");
        params.push(pattern.clone());
        params.push(pattern.clone());
        params.push(pattern);
    }
    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };
    let limit = if options.limit == 0 {
        50
    } else {
        options.limit
    };
    let sql = format!(
        "SELECT je.id,\n                p.account_id,\n                GROUP_CONCAT(DISTINCT p2.account_id),\n                je.posted_at,\n                p.amount_minor,\n                p.currency,\n                COALESCE(je.raw_description, je.description),\n                COALESCE(je.clean_description, je.description),\n                je.counterparty\n         FROM journal_entries je\n         JOIN postings p ON p.journal_entry_id = je.id\n         LEFT JOIN postings p2 ON p2.journal_entry_id = je.id AND p2.id != p.id\n         {where_sql}\n         GROUP BY je.id, p.id\n         ORDER BY je.posted_at DESC\n         LIMIT ?"
    );
    params.push(limit.to_string());
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(rusqlite::params_from_iter(params.iter()))?;
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(TransactionRow {
            id: row.get(0)?,
            chart_account_id: row.get(1)?,
            pair_account_id: row.get::<usize, Option<String>>(2)?.unwrap_or_default(),
            posted_at: row.get(3)?,
            amount_minor: row.get(4)?,
            currency: row.get(5)?,
            raw_description: row.get(6)?,
            clean_description: row.get(7)?,
            counterparty: row.get(8)?,
        });
    }
    Ok(result)
}

pub fn view_ledger(
    connection: &Connection,
    options: &LedgerQueryOptions,
) -> Result<Vec<JournalEntryRow>> {
    let mut clauses = Vec::new();
    let mut params: Vec<String> = Vec::new();
    if let Some(account_id) = &options.account_id {
        clauses
            .push("id IN (SELECT journal_entry_id FROM postings WHERE account_id = ?)".to_owned());
        params.push(account_id.clone());
    }
    if let Some(from) = &options.from {
        clauses.push("posted_at >= ?".to_owned());
        params.push(format!("{from}T00:00:00"));
    }
    if let Some(to) = &options.to {
        clauses.push("posted_at <= ?".to_owned());
        params.push(format!("{to}T23:59:59.999"));
    }
    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };
    let limit = if options.limit == 0 {
        50
    } else {
        options.limit
    };
    params.push(limit.to_string());
    let sql = format!(
        "SELECT id,\n                posted_at,\n                posted_date,\n                is_transfer,\n                description,\n                raw_description,\n                clean_description,\n                counterparty,\n                source_file\n         FROM journal_entries\n         {where_sql}\n         ORDER BY posted_at DESC\n         LIMIT ?"
    );
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(rusqlite::params_from_iter(params.iter()))?;
    let mut entries = Vec::new();
    while let Some(row) = rows.next()? {
        let journal_id: String = row.get(0)?;
        let postings = load_postings(connection, &journal_id)?;
        entries.push(JournalEntryRow {
            id: journal_id,
            posted_at: row.get(1)?,
            posted_date: row.get(2)?,
            is_transfer: row.get::<usize, i64>(3)? == 1,
            description: row.get(4)?,
            raw_description: row.get(5)?,
            clean_description: row.get(6)?,
            counterparty: row.get(7)?,
            source_file: row.get(8)?,
            postings,
        });
    }
    Ok(entries)
}

fn load_postings(connection: &Connection, journal_id: &str) -> Result<Vec<PostingRow>> {
    let mut statement = connection.prepare(
        "SELECT id,\n                journal_entry_id,\n                account_id,\n                amount_minor,\n                currency,\n                memo,\n                provider_txn_id,\n                provider_balance_minor\n         FROM postings\n         WHERE journal_entry_id = ?1\n         ORDER BY id ASC",
    )?;
    let mut rows = statement.query([journal_id])?;
    let mut postings = Vec::new();
    while let Some(row) = rows.next()? {
        postings.push(PostingRow {
            id: row.get(0)?,
            journal_entry_id: row.get(1)?,
            account_id: row.get(2)?,
            amount_minor: row.get(3)?,
            currency: row.get(4)?,
            memo: row.get(5)?,
            provider_txn_id: row.get(6)?,
            provider_balance_minor: row.get(7)?,
        });
    }
    Ok(postings)
}

pub fn ledger_entry_count(connection: &Connection, account_id: Option<&str>) -> Result<i64> {
    if let Some(account_id) = account_id {
        let count = connection.query_row(
            "SELECT COUNT(*)\n             FROM journal_entries\n             WHERE id IN (SELECT journal_entry_id FROM postings WHERE account_id = ?1)",
            [account_id],
            |row| row.get::<usize, i64>(0),
        )?;
        return Ok(count);
    }
    let count = connection.query_row("SELECT COUNT(*) FROM journal_entries", [], |row| {
        row.get::<usize, i64>(0)
    })?;
    Ok(count)
}

pub fn get_balance_sheet(connection: &Connection, as_of: Option<&str>) -> Result<BalanceSheet> {
    let mut clauses = vec!["1=1".to_owned()];
    let mut params = Vec::new();
    if let Some(as_of) = as_of {
        clauses.push("je.posted_at <= ?".to_owned());
        params.push(format!("{as_of}T23:59:59.999"));
    }
    let sql = format!(
        "SELECT coa.account_type, COALESCE(SUM(p.amount_minor), 0)\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON p.account_id = coa.id\n         WHERE {}\n         GROUP BY coa.account_type",
        clauses.join(" AND ")
    );
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(rusqlite::params_from_iter(params.iter()))?;
    let mut sheet = BalanceSheet::default();
    while let Some(row) = rows.next()? {
        let account_type: String = row.get(0)?;
        let total: i64 = row.get(1)?;
        match account_type.as_str() {
            "asset" => sheet.assets += total,
            "liability" => sheet.liabilities += total,
            "equity" => sheet.equity += total,
            "income" => sheet.income += -total,
            "expense" => sheet.expenses += total,
            _ => {}
        }
    }
    sheet.net_worth = sheet.assets - sheet.liabilities;
    sheet.net_income = sheet.income - sheet.expenses;
    Ok(sheet)
}

pub fn group_monthly_cashflow(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    from: Option<&str>,
    to: Option<&str>,
    limit: usize,
) -> Result<Vec<MonthlyCashflowPoint>> {
    let account_ids = collect_group_account_ids(config, group_id);
    if account_ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders_sql = placeholders(account_ids.len());
    let mut params = account_ids;
    let mut extra_conditions = Vec::new();
    if let Some(from) = from {
        extra_conditions.push("je.posted_at >= ?".to_owned());
        params.push(format!("{from}T00:00:00"));
    }
    if let Some(to) = to {
        extra_conditions.push("je.posted_at <= ?".to_owned());
        params.push(format!("{to}T23:59:59.999"));
    }
    let extra_sql = if extra_conditions.is_empty() {
        String::new()
    } else {
        format!(" AND {}", extra_conditions.join(" AND "))
    };
    let sql = format!(
        "SELECT month,\n                income_minor,\n                expense_minor\n         FROM (\n             SELECT strftime('%Y-%m', je.posted_at) AS month,\n                    SUM(CASE WHEN coa.account_type = 'income' THEN -p.amount_minor ELSE 0 END) AS income_minor,\n                    SUM(CASE WHEN coa.account_type = 'expense' THEN p.amount_minor ELSE 0 END) AS expense_minor\n             FROM journal_entries je\n             JOIN postings p ON p.journal_entry_id = je.id\n             JOIN chart_of_accounts coa ON coa.id = p.account_id\n             WHERE coa.account_type IN ('income', 'expense')\n               AND EXISTS (\n                 SELECT 1\n                 FROM postings asset\n                 WHERE asset.journal_entry_id = je.id\n                   AND asset.account_id IN ({placeholders_sql})\n               )\n               {extra_sql}\n             GROUP BY month\n             ORDER BY month DESC\n             LIMIT ?\n         ) recent\n         ORDER BY month ASC"
    );

    let limit_value = if limit == 0 { 120 } else { limit };
    params.push(limit_value.to_string());
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(rusqlite::params_from_iter(params.iter()))?;
    let mut points = Vec::new();
    while let Some(row) = rows.next()? {
        let income_minor: i64 = row.get(1)?;
        let expense_minor: i64 = row.get(2)?;
        let net_minor = income_minor - expense_minor;
        let savings_rate_pct = if income_minor > 0 {
            Some(((net_minor as f64) / (income_minor as f64)) * 100.0)
        } else {
            None
        };
        points.push(MonthlyCashflowPoint {
            month: row.get(0)?,
            income_minor,
            expense_minor,
            net_minor,
            savings_rate_pct,
        });
    }
    Ok(points)
}

pub fn group_category_breakdown(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    months: usize,
    limit: usize,
) -> Result<Vec<CategoryBreakdownPoint>> {
    let account_ids = collect_group_account_ids(config, group_id);
    if account_ids.is_empty() {
        return Ok(vec![]);
    }
    let months = if months == 0 { 3 } else { months };
    let placeholders_sql = placeholders(account_ids.len());
    let sql = format!(
        "SELECT coa.name,\n                COALESCE(SUM(p.amount_minor), 0) AS total_minor,\n                COUNT(*) AS transaction_count\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON coa.id = p.account_id\n         WHERE coa.account_type = 'expense'\n           AND je.posted_at >= date('now', '-' || ? || ' months')\n           AND EXISTS (\n             SELECT 1\n             FROM postings asset\n             WHERE asset.journal_entry_id = p.journal_entry_id\n               AND asset.account_id IN ({placeholders_sql})\n           )\n         GROUP BY coa.id, coa.name\n         ORDER BY total_minor DESC\n         LIMIT ?"
    );
    let mut params = vec![months.to_string()];
    params.extend(account_ids);
    params.push(if limit == 0 { 100 } else { limit }.to_string());
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(rusqlite::params_from_iter(params.iter()))?;
    let mut points = Vec::new();
    while let Some(row) = rows.next()? {
        points.push(CategoryBreakdownPoint {
            category: row.get(0)?,
            total_minor: row.get(1)?,
            transaction_count: row.get(2)?,
        });
    }
    Ok(points)
}

pub fn group_category_monthly_median(
    connection: &Connection,
    config: &FinConfig,
    group_id: &str,
    months: usize,
    limit: usize,
) -> Result<Vec<CategoryMedianPoint>> {
    let account_ids = collect_group_account_ids(config, group_id);
    if account_ids.is_empty() {
        return Ok(vec![]);
    }
    let months = if months == 0 { 6 } else { months };
    let placeholders_sql = placeholders(account_ids.len());
    let sql = format!(
        "SELECT p.account_id,\n                coa.name,\n                strftime('%Y-%m', je.posted_at) AS month,\n                COALESCE(SUM(p.amount_minor), 0) AS month_total\n         FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON p.account_id = coa.id\n         WHERE coa.account_type = 'expense'\n           AND je.posted_at >= date('now', '-' || ? || ' months')\n           AND EXISTS (\n             SELECT 1\n             FROM postings asset\n             WHERE asset.journal_entry_id = p.journal_entry_id\n               AND asset.account_id IN ({placeholders_sql})\n           )\n         GROUP BY p.account_id, coa.name, month\n         ORDER BY p.account_id, month"
    );
    let mut params = vec![months.to_string()];
    params.extend(account_ids);

    let mut by_account = HashMap::<String, (String, Vec<i64>)>::new();
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(rusqlite::params_from_iter(params.iter()))?;
    while let Some(row) = rows.next()? {
        let account_id: String = row.get(0)?;
        let category_name: String = row.get(1)?;
        let month_total: i64 = row.get(3)?;
        let entry = by_account
            .entry(account_id)
            .or_insert_with(|| (category_name, Vec::new()));
        entry.1.push(month_total);
    }

    let mut medians = by_account
        .into_values()
        .map(|(category, mut values)| {
            values.sort_unstable();
            let month_count = values.len();
            let median = if month_count == 0 {
                0
            } else if month_count % 2 == 1 {
                values[month_count / 2]
            } else {
                let left = values[(month_count / 2) - 1];
                let right = values[month_count / 2];
                (left + right) / 2
            };
            CategoryMedianPoint {
                category,
                monthly_median_minor: median,
                month_count,
            }
        })
        .collect::<Vec<_>>();
    medians.sort_by(|left, right| {
        right
            .monthly_median_minor
            .cmp(&left.monthly_median_minor)
            .then(left.category.cmp(&right.category))
    });
    if limit > 0 && medians.len() > limit {
        medians.truncate(limit);
    }
    Ok(medians)
}

pub fn audit_payees(
    connection: &Connection,
    account_id: &str,
    months: usize,
    limit: usize,
) -> Result<Vec<AuditPayeePoint>> {
    let months = if months == 0 { 6 } else { months };
    let cap = if limit == 0 { 50 } else { limit };
    let mut statement = connection.prepare(
        "SELECT COALESCE(je.clean_description, je.raw_description, je.description) AS description,\n                COUNT(*) AS transaction_count,\n                COALESCE(SUM(p.amount_minor), 0) AS total_minor,\n                MAX(je.posted_at) AS latest_posted_at\n         FROM postings p\n         JOIN journal_entries je ON je.id = p.journal_entry_id\n         WHERE p.account_id = ?1\n           AND je.posted_at >= date('now', '-' || ?2 || ' months')\n         GROUP BY description\n         ORDER BY ABS(total_minor) DESC\n         LIMIT ?3",
    )?;
    let mut rows = statement.query(params![
        account_id,
        i64::try_from(months).unwrap_or(6),
        i64::try_from(cap).unwrap_or(50)
    ])?;
    let mut points = Vec::new();
    while let Some(row) = rows.next()? {
        points.push(AuditPayeePoint {
            description: row.get(0)?,
            transaction_count: row.get(1)?,
            total_minor: row.get(2)?,
            latest_posted_at: row.get(3)?,
        });
    }
    Ok(points)
}

pub fn consolidated_net_worth_by_group(
    connection: &Connection,
    config: &FinConfig,
) -> Result<BTreeMap<String, i64>> {
    let mut by_group = BTreeMap::new();
    for group in all_group_ids(config) {
        let account_ids = collect_group_account_ids(config, &group);
        let value = if account_ids.is_empty() {
            0
        } else {
            let sql = format!(
                "SELECT COALESCE(SUM(p.amount_minor), 0)\n                 FROM postings p\n                 WHERE p.account_id IN ({})",
                placeholders(account_ids.len())
            );
            connection.query_row(
                &sql,
                rusqlite::params_from_iter(account_ids.iter()),
                |row| row.get::<usize, i64>(0),
            )?
        };
        by_group.insert(group, value);
    }
    Ok(by_group)
}

pub fn transaction_counts_by_group(
    connection: &Connection,
    config: &FinConfig,
) -> Result<BTreeMap<String, i64>> {
    let mut map = BTreeMap::new();
    for group in all_group_ids(config) {
        let account_ids = collect_group_account_ids(config, &group);
        if account_ids.is_empty() {
            map.insert(group, 0);
            continue;
        }
        let sql = format!(
            "SELECT COUNT(*)\n             FROM postings p\n             WHERE p.account_id IN ({})",
            placeholders(account_ids.len())
        );
        let count = connection.query_row(
            &sql,
            rusqlite::params_from_iter(account_ids.iter()),
            |row| row.get::<usize, i64>(0),
        )?;
        map.insert(group, count);
    }
    Ok(map)
}

pub fn unique_months_from_cashflow(points: &[MonthlyCashflowPoint]) -> Vec<String> {
    let mut months = BTreeSet::new();
    for point in points {
        months.insert(point.month.clone());
    }
    months.into_iter().collect()
}
