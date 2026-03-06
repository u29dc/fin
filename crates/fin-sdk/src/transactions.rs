use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, params_from_iter};
use serde::{Deserialize, Serialize};

use crate::config::FinConfig;
use crate::error::{FinError, Result};

const DEFAULT_PAGE_LIMIT: usize = 50;
const MAX_PAGE_LIMIT: usize = 10_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionSortField {
    PostedAt,
    AmountMinor,
    Description,
    Counterparty,
    AccountId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum TransactionCursorValue {
    Text(String),
    Integer(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionCursor {
    pub sort_field: TransactionSortField,
    pub sort_direction: SortDirection,
    pub sort_value: TransactionCursorValue,
    pub posted_at: String,
    pub journal_entry_id: String,
    pub posting_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionListRow {
    pub posting_id: String,
    pub journal_entry_id: String,
    pub chart_account_id: String,
    pub pair_account_ids: Vec<String>,
    pub posted_at: String,
    pub posted_date: String,
    pub amount_minor: i64,
    pub currency: String,
    pub raw_description: String,
    pub clean_description: String,
    pub counterparty: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionCounterpartyPosting {
    pub posting_id: String,
    pub account_id: String,
    pub amount_minor: i64,
    pub currency: String,
    pub memo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionDetail {
    pub posting_id: String,
    pub journal_entry_id: String,
    pub chart_account_id: String,
    pub posted_at: String,
    pub posted_date: String,
    pub amount_minor: i64,
    pub currency: String,
    pub description: String,
    pub raw_description: Option<String>,
    pub clean_description: Option<String>,
    pub counterparty: Option<String>,
    pub source_file: Option<String>,
    pub is_transfer: bool,
    pub pair_postings: Vec<TransactionCounterpartyPosting>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionPage {
    pub items: Vec<TransactionListRow>,
    pub total_count: usize,
    pub has_more: bool,
    pub next_cursor: Option<TransactionCursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionPageQuery {
    pub group_id: Option<String>,
    pub chart_account_ids: Option<Vec<String>>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub search: Option<String>,
    pub limit: usize,
    pub sort_field: TransactionSortField,
    pub sort_direction: SortDirection,
    pub after: Option<TransactionCursor>,
}

impl Default for TransactionPageQuery {
    fn default() -> Self {
        Self {
            group_id: None,
            chart_account_ids: None,
            from: None,
            to: None,
            search: None,
            limit: DEFAULT_PAGE_LIMIT,
            sort_field: TransactionSortField::PostedAt,
            sort_direction: SortDirection::Desc,
            after: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BaseTransactionRow {
    posting_id: String,
    journal_entry_id: String,
    chart_account_id: String,
    posted_at: String,
    posted_date: String,
    amount_minor: i64,
    currency: String,
    raw_description: String,
    clean_description: String,
    counterparty: Option<String>,
}

#[derive(Debug, Clone)]
struct TransactionSqlPlan {
    items_from_sql: String,
    count_from_sql: String,
    where_sql: String,
    params: Vec<String>,
}

pub fn query_transactions_page(
    connection: &Connection,
    config: &FinConfig,
    query: &TransactionPageQuery,
) -> Result<TransactionPage> {
    let scoped_account_ids = resolve_scoped_account_ids(config, query);
    query_transactions_page_for_accounts(connection, scoped_account_ids.as_deref(), query)
}

pub(crate) fn query_transactions_page_for_accounts(
    connection: &Connection,
    chart_account_ids: Option<&[String]>,
    query: &TransactionPageQuery,
) -> Result<TransactionPage> {
    validate_cursor(query)?;

    let limit = normalize_limit(query.limit);
    let plan = build_transaction_sql_plan(chart_account_ids, query)?;
    let total_count = count_transactions(
        connection,
        &plan.count_from_sql,
        &plan.where_sql,
        &plan.params,
    )?;

    let order_by = order_by_sql(query.sort_field, query.sort_direction);
    let mut params = plan.params.clone();
    params.push((limit + 1).to_string());
    let sql = format!(
        "SELECT p.id,\n                je.id,\n                p.account_id,\n                je.posted_at,\n                je.posted_date,\n                p.amount_minor,\n                p.currency,\n                COALESCE(je.raw_description, je.description),\n                COALESCE(je.clean_description, je.description),\n                je.counterparty\n         {items_from_sql}\n         {where_sql}\n         ORDER BY {order_by}\n         LIMIT ?",
        items_from_sql = plan.items_from_sql,
        where_sql = plan.where_sql,
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        Ok(BaseTransactionRow {
            posting_id: row.get(0)?,
            journal_entry_id: row.get(1)?,
            chart_account_id: row.get(2)?,
            posted_at: row.get(3)?,
            posted_date: row.get(4)?,
            amount_minor: row.get(5)?,
            currency: row.get(6)?,
            raw_description: row.get(7)?,
            clean_description: row.get(8)?,
            counterparty: row.get(9)?,
        })
    })?;

    let mut base_rows = Vec::new();
    for row in rows {
        base_rows.push(row?);
    }

    let has_more = base_rows.len() > limit;
    if has_more {
        base_rows.truncate(limit);
    }

    let pair_accounts = load_pair_account_ids(connection, &base_rows)?;
    let items = base_rows
        .iter()
        .map(|row| TransactionListRow {
            posting_id: row.posting_id.clone(),
            journal_entry_id: row.journal_entry_id.clone(),
            chart_account_id: row.chart_account_id.clone(),
            pair_account_ids: pair_accounts
                .get(&row.posting_id)
                .cloned()
                .unwrap_or_default(),
            posted_at: row.posted_at.clone(),
            posted_date: row.posted_date.clone(),
            amount_minor: row.amount_minor,
            currency: row.currency.clone(),
            raw_description: row.raw_description.clone(),
            clean_description: row.clean_description.clone(),
            counterparty: row.counterparty.clone(),
        })
        .collect::<Vec<_>>();

    let next_cursor = items
        .last()
        .map(|row| cursor_from_row(row, query.sort_field, query.sort_direction));

    Ok(TransactionPage {
        items,
        total_count,
        has_more,
        next_cursor,
    })
}

pub fn load_transaction_detail(
    connection: &Connection,
    posting_id: &str,
) -> Result<Option<TransactionDetail>> {
    let sql = "SELECT p.id,\n                      je.id,\n                      p.account_id,\n                      je.posted_at,\n                      je.posted_date,\n                      p.amount_minor,\n                      p.currency,\n                      je.description,\n                      je.raw_description,\n                      je.clean_description,\n                      je.counterparty,\n                      je.source_file,\n                      je.is_transfer\n               FROM postings p\n               JOIN journal_entries je ON p.journal_entry_id = je.id\n               WHERE p.id = ?";
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query([posting_id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let detail = TransactionDetail {
        posting_id: row.get(0)?,
        journal_entry_id: row.get(1)?,
        chart_account_id: row.get(2)?,
        posted_at: row.get(3)?,
        posted_date: row.get(4)?,
        amount_minor: row.get(5)?,
        currency: row.get(6)?,
        description: row.get(7)?,
        raw_description: row.get(8)?,
        clean_description: row.get(9)?,
        counterparty: row.get(10)?,
        source_file: row.get(11)?,
        is_transfer: row.get::<usize, i64>(12)? == 1,
        pair_postings: load_pair_postings(connection, posting_id)?,
    };
    Ok(Some(detail))
}

fn resolve_scoped_account_ids(
    config: &FinConfig,
    query: &TransactionPageQuery,
) -> Option<Vec<String>> {
    if let Some(chart_account_ids) = &query.chart_account_ids
        && !chart_account_ids.is_empty()
    {
        return Some(
            chart_account_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        );
    }

    query.group_id.as_deref().map(|group_id| {
        config
            .accounts
            .iter()
            .filter(|account| account.group == group_id && account.account_type == "asset")
            .map(|account| account.id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    })
}

fn validate_cursor(query: &TransactionPageQuery) -> Result<()> {
    if let Some(cursor) = &query.after {
        if cursor.sort_field != query.sort_field || cursor.sort_direction != query.sort_direction {
            return Err(FinError::InvalidInput {
                code: "TRANSACTION_CURSOR_MISMATCH",
                message: "cursor sort must match the requested transaction sort".to_owned(),
            });
        }
        if !cursor_value_matches_field(&cursor.sort_value, cursor.sort_field) {
            return Err(FinError::InvalidInput {
                code: "TRANSACTION_CURSOR_INVALID",
                message: "cursor sort value type does not match the requested sort field"
                    .to_owned(),
            });
        }
    }
    Ok(())
}

fn cursor_value_matches_field(
    value: &TransactionCursorValue,
    sort_field: TransactionSortField,
) -> bool {
    matches!(
        (sort_field, value),
        (
            TransactionSortField::AmountMinor,
            TransactionCursorValue::Integer(_)
        ) | (
            TransactionSortField::PostedAt
                | TransactionSortField::Description
                | TransactionSortField::Counterparty
                | TransactionSortField::AccountId,
            TransactionCursorValue::Text(_)
        )
    )
}

fn normalize_limit(limit: usize) -> usize {
    if limit == 0 {
        return DEFAULT_PAGE_LIMIT;
    }
    limit.min(MAX_PAGE_LIMIT)
}

fn count_transactions(
    connection: &Connection,
    from_sql: &str,
    where_sql: &str,
    params: &[String],
) -> Result<usize> {
    let sql = format!("SELECT COUNT(*)\n         {from_sql}\n         {where_sql}");
    let count = connection.query_row(&sql, params_from_iter(params.iter()), |row| {
        row.get::<usize, i64>(0)
    })?;
    Ok(usize::try_from(count).unwrap_or_default())
}

fn build_transaction_sql_plan(
    chart_account_ids: Option<&[String]>,
    query: &TransactionPageQuery,
) -> Result<TransactionSqlPlan> {
    let mut clauses = Vec::new();
    let mut params = Vec::new();
    let scoped_accounts = chart_account_ids.filter(|account_ids| !account_ids.is_empty());

    let (items_from_sql, count_from_sql) = if scoped_accounts.is_some() {
        let count_from_sql =
            "FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id"
                .to_owned();
        let items_from_sql = count_from_sql.clone();
        (items_from_sql, count_from_sql)
    } else {
        let generic_from_sql = "FROM postings p\n         JOIN journal_entries je ON p.journal_entry_id = je.id\n         JOIN chart_of_accounts coa ON p.account_id = coa.id".to_owned();
        clauses.push("coa.account_type = 'asset'".to_owned());
        (generic_from_sql.clone(), generic_from_sql)
    };

    if let Some(chart_account_ids) = scoped_accounts {
        let (account_clause, account_params) = account_match_clause("p", chart_account_ids);
        clauses.push(account_clause);
        params.extend(account_params);
    }

    if let Some(from) = &query.from {
        clauses.push("je.posted_at >= ?".to_owned());
        params.push(format!("{from}T00:00:00"));
    }
    if let Some(to) = &query.to {
        clauses.push("je.posted_at <= ?".to_owned());
        params.push(format!("{to}T23:59:59.999"));
    }
    if let Some(search) = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let pattern = format!("%{}%", search.to_ascii_lowercase());
        clauses.push(
            "(LOWER(COALESCE(je.clean_description, je.description, '')) LIKE ?\n              OR LOWER(COALESCE(je.raw_description, je.description, '')) LIKE ?\n              OR LOWER(COALESCE(je.counterparty, '')) LIKE ?\n              OR LOWER(p.account_id) LIKE ?\n              OR EXISTS (\n                    SELECT 1\n                    FROM postings other\n                    WHERE other.journal_entry_id = je.id\n                      AND other.id != p.id\n                      AND LOWER(other.account_id) LIKE ?\n                ))"
                .to_owned(),
        );
        for _ in 0..5 {
            params.push(pattern.clone());
        }
    }

    if let Some(cursor) = &query.after {
        clauses.push(cursor_clause(
            query.sort_field,
            query.sort_direction,
            cursor,
        )?);
        extend_cursor_params(&mut params, query.sort_field, cursor);
    }

    Ok(TransactionSqlPlan {
        items_from_sql,
        count_from_sql,
        where_sql: format!("WHERE {}", clauses.join(" AND ")),
        params,
    })
}

fn cursor_clause(
    sort_field: TransactionSortField,
    sort_direction: SortDirection,
    cursor: &TransactionCursor,
) -> Result<String> {
    let operator = match sort_direction {
        SortDirection::Asc => ">",
        SortDirection::Desc => "<",
    };
    let clause = match sort_field {
        TransactionSortField::PostedAt => {
            format!("(je.posted_at, je.id, p.id) {operator} (?, ?, ?)")
        }
        TransactionSortField::AmountMinor => {
            format!("(p.amount_minor, je.posted_at, je.id, p.id) {operator} (?, ?, ?, ?)")
        }
        TransactionSortField::Description => format!(
            "(LOWER(COALESCE(je.clean_description, je.description, '')), je.posted_at, je.id, p.id) {operator} (?, ?, ?, ?)"
        ),
        TransactionSortField::Counterparty => format!(
            "(LOWER(COALESCE(je.counterparty, '')), je.posted_at, je.id, p.id) {operator} (?, ?, ?, ?)"
        ),
        TransactionSortField::AccountId => {
            format!("(LOWER(p.account_id), je.posted_at, je.id, p.id) {operator} (?, ?, ?, ?)")
        }
    };
    if !cursor_value_matches_field(&cursor.sort_value, sort_field) {
        return Err(FinError::InvalidInput {
            code: "TRANSACTION_CURSOR_INVALID",
            message: "cursor sort value type does not match the requested sort field".to_owned(),
        });
    }
    Ok(clause)
}

fn extend_cursor_params(
    params: &mut Vec<String>,
    sort_field: TransactionSortField,
    cursor: &TransactionCursor,
) {
    if sort_field != TransactionSortField::PostedAt {
        match &cursor.sort_value {
            TransactionCursorValue::Text(value) => params.push(value.clone()),
            TransactionCursorValue::Integer(value) => params.push(value.to_string()),
        }
    }
    params.push(cursor.posted_at.clone());
    params.push(cursor.journal_entry_id.clone());
    params.push(cursor.posting_id.clone());
}

fn order_by_sql(sort_field: TransactionSortField, sort_direction: SortDirection) -> String {
    let direction = match sort_direction {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };
    match sort_field {
        TransactionSortField::PostedAt => {
            format!("je.posted_at {direction}, je.id {direction}, p.id {direction}")
        }
        TransactionSortField::AmountMinor => format!(
            "p.amount_minor {direction}, je.posted_at {direction}, je.id {direction}, p.id {direction}"
        ),
        TransactionSortField::Description => format!(
            "LOWER(COALESCE(je.clean_description, je.description, '')) {direction}, je.posted_at {direction}, je.id {direction}, p.id {direction}"
        ),
        TransactionSortField::Counterparty => format!(
            "LOWER(COALESCE(je.counterparty, '')) {direction}, je.posted_at {direction}, je.id {direction}, p.id {direction}"
        ),
        TransactionSortField::AccountId => format!(
            "LOWER(p.account_id) {direction}, je.posted_at {direction}, je.id {direction}, p.id {direction}"
        ),
    }
}

fn cursor_from_row(
    row: &TransactionListRow,
    sort_field: TransactionSortField,
    sort_direction: SortDirection,
) -> TransactionCursor {
    let sort_value = match sort_field {
        TransactionSortField::PostedAt => TransactionCursorValue::Text(row.posted_at.clone()),
        TransactionSortField::AmountMinor => TransactionCursorValue::Integer(row.amount_minor),
        TransactionSortField::Description => {
            TransactionCursorValue::Text(row.clean_description.to_ascii_lowercase())
        }
        TransactionSortField::Counterparty => TransactionCursorValue::Text(
            row.counterparty
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase(),
        ),
        TransactionSortField::AccountId => {
            TransactionCursorValue::Text(row.chart_account_id.to_ascii_lowercase())
        }
    };
    TransactionCursor {
        sort_field,
        sort_direction,
        sort_value,
        posted_at: row.posted_at.clone(),
        journal_entry_id: row.journal_entry_id.clone(),
        posting_id: row.posting_id.clone(),
    }
}

fn load_pair_account_ids(
    connection: &Connection,
    rows: &[BaseTransactionRow],
) -> Result<BTreeMap<String, Vec<String>>> {
    if rows.is_empty() {
        return Ok(BTreeMap::new());
    }

    let posting_ids = rows
        .iter()
        .map(|row| row.posting_id.clone())
        .collect::<Vec<_>>();
    let placeholders = placeholders(posting_ids.len());
    let sql = format!(
        "SELECT p.id, other.account_id\n         FROM postings p\n         JOIN postings other ON other.journal_entry_id = p.journal_entry_id\n         WHERE p.id IN ({placeholders})\n           AND other.id != p.id\n         ORDER BY p.id ASC, other.account_id ASC"
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(posting_ids.iter()), |row| {
        Ok((row.get::<usize, String>(0)?, row.get::<usize, String>(1)?))
    })?;

    let mut by_posting = BTreeMap::<String, BTreeSet<String>>::new();
    for row in rows {
        let (posting_id, account_id) = row?;
        by_posting.entry(posting_id).or_default().insert(account_id);
    }

    Ok(by_posting
        .into_iter()
        .map(|(posting_id, account_ids)| (posting_id, account_ids.into_iter().collect()))
        .collect())
}

fn load_pair_postings(
    connection: &Connection,
    posting_id: &str,
) -> Result<Vec<TransactionCounterpartyPosting>> {
    let sql = "SELECT other.id,\n                      other.account_id,\n                      other.amount_minor,\n                      other.currency,\n                      other.memo\n               FROM postings selected\n               JOIN postings other ON other.journal_entry_id = selected.journal_entry_id\n               WHERE selected.id = ?\n                 AND other.id != selected.id\n               ORDER BY other.account_id ASC, other.id ASC";
    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map([posting_id], |row| {
        Ok(TransactionCounterpartyPosting {
            posting_id: row.get(0)?,
            account_id: row.get(1)?,
            amount_minor: row.get(2)?,
            currency: row.get(3)?,
            memo: row.get(4)?,
        })
    })?;

    let mut postings = Vec::new();
    for row in rows {
        postings.push(row?);
    }
    Ok(postings)
}

fn account_match_clause(alias: &str, account_ids: &[String]) -> (String, Vec<String>) {
    (
        format!(
            "{alias}.account_id IN ({})",
            placeholders(account_ids.len())
        ),
        account_ids.to_vec(),
    )
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use tempfile::tempdir;

    use super::{
        SortDirection, TransactionPageQuery, TransactionSortField, load_transaction_detail,
        query_transactions_page,
    };
    use crate::runtime::{RuntimeContext, RuntimeContextOptions};
    use crate::testing::fixture::{FixtureBuildOptions, materialize_fixture_home};

    fn open_fixture() -> RuntimeContext {
        let temp = tempdir().expect("tempdir");
        let fixture = materialize_fixture_home(temp.path(), &FixtureBuildOptions::default())
            .expect("materialize fixture");
        RuntimeContext::open(RuntimeContextOptions {
            config_path: Some(fixture.paths.config_path),
            db_path: Some(fixture.paths.db_path),
            create: false,
            ..RuntimeContextOptions::read_only()
        })
        .expect("open runtime")
    }

    #[test]
    fn paginates_personal_transactions_without_duplicates() {
        let runtime = open_fixture();
        let first_page = query_transactions_page(
            runtime.connection(),
            runtime.config(),
            &TransactionPageQuery {
                group_id: Some("personal".to_owned()),
                limit: 25,
                ..TransactionPageQuery::default()
            },
        )
        .expect("first page");

        assert_eq!(first_page.items.len(), 25);
        assert!(first_page.has_more);
        assert!(first_page.total_count > first_page.items.len());

        let second_page = query_transactions_page(
            runtime.connection(),
            runtime.config(),
            &TransactionPageQuery {
                group_id: Some("personal".to_owned()),
                limit: 25,
                after: first_page.next_cursor.clone(),
                ..TransactionPageQuery::default()
            },
        )
        .expect("second page");

        assert_eq!(second_page.items.len(), 25);
        let first_ids = first_page
            .items
            .iter()
            .map(|row| row.posting_id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(
            second_page
                .items
                .iter()
                .all(|row| !first_ids.contains(row.posting_id.as_str()))
        );
    }

    #[test]
    fn sorts_transactions_by_amount_and_account() {
        let runtime = open_fixture();
        let amount_page = query_transactions_page(
            runtime.connection(),
            runtime.config(),
            &TransactionPageQuery {
                group_id: Some("business".to_owned()),
                limit: 40,
                sort_field: TransactionSortField::AmountMinor,
                sort_direction: SortDirection::Asc,
                ..TransactionPageQuery::default()
            },
        )
        .expect("amount page");

        assert!(
            amount_page
                .items
                .windows(2)
                .all(|window| window[0].amount_minor <= window[1].amount_minor)
        );

        let account_page = query_transactions_page(
            runtime.connection(),
            runtime.config(),
            &TransactionPageQuery {
                group_id: Some("joint".to_owned()),
                limit: 40,
                sort_field: TransactionSortField::AccountId,
                sort_direction: SortDirection::Asc,
                ..TransactionPageQuery::default()
            },
        )
        .expect("account page");

        assert!(
            account_page
                .items
                .windows(2)
                .all(|window| window[0].chart_account_id <= window[1].chart_account_id)
        );
    }

    #[test]
    fn search_filters_by_description_and_counterparty_accounts() {
        let runtime = open_fixture();
        let rent_page = query_transactions_page(
            runtime.connection(),
            runtime.config(),
            &TransactionPageQuery {
                group_id: Some("joint".to_owned()),
                search: Some("rent".to_owned()),
                limit: 20,
                ..TransactionPageQuery::default()
            },
        )
        .expect("rent page");

        assert!(!rent_page.items.is_empty());
        assert!(rent_page.items.iter().all(|row| {
            let haystacks = [
                row.clean_description.to_ascii_lowercase(),
                row.raw_description.to_ascii_lowercase(),
                row.counterparty
                    .as_deref()
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
                row.chart_account_id.to_ascii_lowercase(),
                row.pair_account_ids.join(" ").to_ascii_lowercase(),
            ];
            haystacks.iter().any(|value| value.contains("rent"))
        }));
    }

    #[test]
    fn cursor_replay_is_stable_for_same_request() {
        let runtime = open_fixture();
        let first_page = query_transactions_page(
            runtime.connection(),
            runtime.config(),
            &TransactionPageQuery {
                group_id: Some("business".to_owned()),
                limit: 15,
                sort_field: TransactionSortField::Description,
                sort_direction: SortDirection::Asc,
                ..TransactionPageQuery::default()
            },
        )
        .expect("first page");

        let request = TransactionPageQuery {
            group_id: Some("business".to_owned()),
            limit: 15,
            sort_field: TransactionSortField::Description,
            sort_direction: SortDirection::Asc,
            after: first_page.next_cursor.clone(),
            ..TransactionPageQuery::default()
        };
        let second_page_a =
            query_transactions_page(runtime.connection(), runtime.config(), &request)
                .expect("second page a");
        let second_page_b =
            query_transactions_page(runtime.connection(), runtime.config(), &request)
                .expect("second page b");

        assert_eq!(second_page_a.items, second_page_b.items);
        assert_eq!(second_page_a.next_cursor, second_page_b.next_cursor);
    }

    #[test]
    fn explicit_chart_account_scope_only_returns_requested_account() {
        let runtime = open_fixture();
        let account_id = runtime
            .config()
            .accounts
            .iter()
            .find(|account| account.group == "personal" && account.id == "Assets:Personal:Checking")
            .map(|account| account.id.clone())
            .expect("personal checking");
        let page = query_transactions_page(
            runtime.connection(),
            runtime.config(),
            &TransactionPageQuery {
                chart_account_ids: Some(vec![account_id.clone()]),
                limit: 40,
                ..TransactionPageQuery::default()
            },
        )
        .expect("page");

        assert!(!page.items.is_empty());
        assert!(
            page.items
                .iter()
                .all(|row| row.chart_account_id == account_id)
        );
    }

    #[test]
    fn loads_selected_transaction_detail_with_counterparty_postings() {
        let runtime = open_fixture();
        let page = query_transactions_page(
            runtime.connection(),
            runtime.config(),
            &TransactionPageQuery {
                group_id: Some("personal".to_owned()),
                limit: 10,
                ..TransactionPageQuery::default()
            },
        )
        .expect("page");
        let row = page.items.first().expect("transaction row");

        let detail = load_transaction_detail(runtime.connection(), &row.posting_id)
            .expect("detail query")
            .expect("detail present");

        assert_eq!(detail.posting_id, row.posting_id);
        assert_eq!(detail.journal_entry_id, row.journal_entry_id);
        assert!(!detail.pair_postings.is_empty());
        assert!(
            detail
                .pair_postings
                .iter()
                .all(|posting| posting.posting_id != row.posting_id)
        );
    }
}
