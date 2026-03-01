use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{FinError, Result};
use crate::sanitize::ensure_account_exists;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditChange {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EditChanges {
    pub description: Option<EditChange>,
    pub account: Option<EditChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditTransactionPreview {
    pub entry_id: String,
    pub dry_run: bool,
    pub account_created: bool,
    pub changes: EditChanges,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoidPreview {
    pub original_entry_id: String,
    pub void_entry_id: Option<String>,
    pub postings_reversed: usize,
    pub dry_run: bool,
}

#[derive(Debug)]
struct EntryRow {
    id: String,
    is_transfer: i64,
    description: String,
}

#[derive(Debug)]
struct PostingRow {
    id: String,
    account_id: String,
    amount_minor: i64,
    currency: String,
    memo: Option<String>,
}

fn load_entry(connection: &Connection, id: &str) -> Result<EntryRow> {
    let entry = connection
        .query_row(
            "SELECT id, is_transfer, description\n             FROM journal_entries\n             WHERE id = ?1",
            [id],
            |row| {
                Ok(EntryRow {
                    id: row.get(0)?,
                    is_transfer: row.get(1)?,
                    description: row.get(2)?,
                })
            },
        )
        .optional()?;
    entry.ok_or_else(|| FinError::InvalidInput {
        code: "NOT_FOUND",
        message: format!("Journal entry not found: {id}"),
    })
}

fn load_postings(connection: &Connection, journal_entry_id: &str) -> Result<Vec<PostingRow>> {
    let mut statement = connection.prepare(
        "SELECT id, account_id, amount_minor, currency, memo\n         FROM postings\n         WHERE journal_entry_id = ?1\n         ORDER BY id ASC",
    )?;
    let mut rows = statement.query([journal_entry_id])?;
    let mut postings = Vec::new();
    while let Some(row) = rows.next()? {
        postings.push(PostingRow {
            id: row.get(0)?,
            account_id: row.get(1)?,
            amount_minor: row.get(2)?,
            currency: row.get(3)?,
            memo: row.get(4)?,
        });
    }
    Ok(postings)
}

fn non_asset_posting(postings: &[PostingRow]) -> Result<&PostingRow> {
    let non_asset = postings
        .iter()
        .filter(|posting| !posting.account_id.starts_with("Assets:"))
        .collect::<Vec<_>>();
    if non_asset.len() != 1 {
        return Err(FinError::InvalidInput {
            code: "AMBIGUOUS_POSTING",
            message: format!(
                "Expected exactly one non-asset posting, found {}",
                non_asset.len()
            ),
        });
    }
    Ok(non_asset[0])
}

pub fn edit_transaction(
    connection: &mut Connection,
    id: &str,
    description: Option<&str>,
    account_id: Option<&str>,
    dry_run: bool,
) -> Result<EditTransactionPreview> {
    if description.is_none() && account_id.is_none() {
        return Err(FinError::InvalidInput {
            code: "INVALID_INPUT",
            message: "At least one of description/account is required.".to_owned(),
        });
    }
    let entry = load_entry(connection, id)?;
    let postings = load_postings(connection, id)?;
    let mut changes = EditChanges::default();
    let mut account_created = false;

    if let Some(description) = description
        && description != entry.description
    {
        changes.description = Some(EditChange {
            from: entry.description.clone(),
            to: description.to_owned(),
        });
    }

    if let Some(account_id) = account_id {
        let target = non_asset_posting(&postings)?;
        if account_id != target.account_id {
            changes.account = Some(EditChange {
                from: target.account_id.clone(),
                to: account_id.to_owned(),
            });
        }
        if !dry_run {
            account_created = ensure_account_exists(connection, account_id)?;
        } else {
            let exists = connection
                .query_row(
                    "SELECT id FROM chart_of_accounts WHERE id = ?1",
                    [account_id],
                    |row| row.get::<usize, String>(0),
                )
                .optional()?
                .is_some();
            account_created = !exists;
        }
    }

    if dry_run {
        return Ok(EditTransactionPreview {
            entry_id: entry.id,
            dry_run: true,
            account_created,
            changes,
        });
    }

    let tx = connection.transaction()?;
    if let Some(change) = &changes.description {
        tx.execute(
            "UPDATE journal_entries\n             SET description = ?1,\n                 clean_description = ?1,\n                 updated_at = datetime('now')\n             WHERE id = ?2",
            params![change.to, id],
        )?;
    }
    if let Some(change) = &changes.account {
        let target = non_asset_posting(&postings)?;
        tx.execute(
            "UPDATE postings SET account_id = ?1 WHERE id = ?2",
            params![change.to, target.id],
        )?;
    }
    tx.commit()?;

    Ok(EditTransactionPreview {
        entry_id: entry.id,
        dry_run: false,
        account_created,
        changes,
    })
}

pub fn void_entry(connection: &mut Connection, id: &str, dry_run: bool) -> Result<VoidPreview> {
    let entry = load_entry(connection, id)?;
    let postings = load_postings(connection, id)?;
    if dry_run {
        return Ok(VoidPreview {
            original_entry_id: entry.id,
            void_entry_id: None,
            postings_reversed: postings.len(),
            dry_run: true,
        });
    }
    let void_entry_id = format!("je_{}", Uuid::new_v4().simple());
    let description = format!("VOID: {}", entry.description);
    let tx = connection.transaction()?;
    tx.execute(
        "INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, raw_description, clean_description, counterparty, source_file)\n         VALUES (?1, datetime('now'), date('now'), ?2, ?3, NULL, ?3, NULL, NULL)",
        params![void_entry_id, entry.is_transfer, description],
    )?;
    for posting in postings {
        tx.execute(
            "INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo, provider_txn_id, provider_balance_minor)\n             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL)",
            params![
                format!("p_{}", Uuid::new_v4().simple()),
                void_entry_id,
                posting.account_id,
                -posting.amount_minor,
                posting.currency,
                format!("VOID: {}", posting.memo.unwrap_or_default()),
            ],
        )?;
    }
    tx.commit()?;
    Ok(VoidPreview {
        original_entry_id: entry.id,
        void_entry_id: Some(void_entry_id),
        postings_reversed: load_postings(connection, id)?.len(),
        dry_run: false,
    })
}
