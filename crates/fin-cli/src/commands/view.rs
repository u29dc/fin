use serde_json::json;

use fin_sdk::{
    LedgerQueryOptions, TransactionQueryOptions, edit_transaction, get_balance_sheet,
    ledger_entry_count, view_accounts, view_ledger, view_transactions, void_entry,
};

use crate::commands::{CommandFailure, CommandResult, map_fin_error, open_runtime};
use crate::envelope::MetaExtras;
use crate::error::ExitCode;

pub fn run_accounts(
    db: Option<&str>,
    group: Option<&str>,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("view.accounts", db, true)?;
    let accounts = view_accounts(runtime.connection(), runtime.config(), group)
        .map_err(|error| map_fin_error("view.accounts", error))?;
    let total = accounts
        .iter()
        .map(|account| account.balance_minor.unwrap_or(0))
        .sum::<i64>();
    let rows = accounts
        .iter()
        .map(|account| {
            json!({
                "id": account.id,
                "name": account.name,
                "type": account.account_type,
                "balance": account.balance_minor,
                "updated": account.updated_at,
            })
        })
        .collect::<Vec<_>>();
    Ok(CommandResult {
        tool: "view.accounts",
        data: json!({
            "accounts": rows,
            "total": total,
        }),
        text: format!("{} accounts | total={total}", accounts.len()),
        meta: MetaExtras {
            count: Some(accounts.len()),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_transactions(
    db: Option<&str>,
    account: Option<&str>,
    group: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
    search: Option<&str>,
    limit: usize,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("view.transactions", db, true)?;
    let chart_account_ids = if let Some(group) = group {
        Some(fin_sdk::group_asset_account_ids(runtime.config(), group))
    } else if let Some(account) = account {
        Some(vec![account.to_owned()])
    } else {
        None
    };
    let options = TransactionQueryOptions {
        chart_account_ids,
        from: from.map(std::string::ToString::to_string),
        to: to.map(std::string::ToString::to_string),
        search: search.map(std::string::ToString::to_string),
        limit,
    };
    let transactions = view_transactions(runtime.connection(), &options)
        .map_err(|error| map_fin_error("view.transactions", error))?;
    let rows = transactions
        .iter()
        .map(|row| {
            json!({
                "date": row.posted_at,
                "account": row.chart_account_id,
                "amount": row.amount_minor,
                "description": row.clean_description,
                "id": row.id,
            })
        })
        .collect::<Vec<_>>();
    Ok(CommandResult {
        tool: "view.transactions",
        data: json!({
            "transactions": rows,
            "count": rows.len(),
        }),
        text: format!("{} transactions", rows.len()),
        meta: MetaExtras {
            count: Some(rows.len()),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_ledger(
    db: Option<&str>,
    account: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
    limit: usize,
) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("view.ledger", db, true)?;
    let options = LedgerQueryOptions {
        account_id: account.map(std::string::ToString::to_string),
        from: from.map(std::string::ToString::to_string),
        to: to.map(std::string::ToString::to_string),
        limit,
    };
    let entries = view_ledger(runtime.connection(), &options)
        .map_err(|error| map_fin_error("view.ledger", error))?;
    let total = ledger_entry_count(runtime.connection(), account)
        .map_err(|error| map_fin_error("view.ledger", error))?;
    Ok(CommandResult {
        tool: "view.ledger",
        data: json!({
            "entries": entries,
            "count": entries.len(),
            "total": total,
        }),
        text: format!("{} entries (of {total})", entries.len()),
        meta: MetaExtras {
            count: Some(entries.len()),
            total: usize::try_from(total).ok(),
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}

pub fn run_balance(db: Option<&str>, as_of: Option<&str>) -> Result<CommandResult, CommandFailure> {
    let runtime = open_runtime("view.balance", db, true)?;
    let sheet = get_balance_sheet(runtime.connection(), as_of)
        .map_err(|error| map_fin_error("view.balance", error))?;
    Ok(CommandResult {
        tool: "view.balance",
        data: json!({
            "assets": sheet.assets,
            "liabilities": sheet.liabilities,
            "equity": sheet.equity,
            "income": sheet.income,
            "expenses": sheet.expenses,
            "netWorth": sheet.net_worth,
            "netIncome": sheet.net_income,
        }),
        text: format!(
            "Assets={} Liabilities={} NetWorth={}",
            sheet.assets, sheet.liabilities, sheet.net_worth
        ),
        meta: MetaExtras::default(),
        exit_code: ExitCode::Success,
    })
}

pub fn run_void(
    db: Option<&str>,
    id: &str,
    dry_run: bool,
) -> Result<CommandResult, CommandFailure> {
    let mut runtime = open_runtime("view.void", db, dry_run)?;
    let preview = void_entry(runtime.connection_mut(), id, dry_run)
        .map_err(|error| map_fin_error("view.void", error))?;
    Ok(CommandResult {
        tool: "view.void",
        data: json!({
            "originalEntryId": preview.original_entry_id,
            "voidEntryId": preview.void_entry_id,
            "postingsReversed": preview.postings_reversed,
            "dryRun": preview.dry_run,
        }),
        text: format!(
            "void {} | postings={} | dry-run={}",
            preview.original_entry_id, preview.postings_reversed, preview.dry_run
        ),
        meta: MetaExtras::default(),
        exit_code: ExitCode::Success,
    })
}

pub fn run_edit_transaction(
    db: Option<&str>,
    id: &str,
    description: Option<&str>,
    account: Option<&str>,
    dry_run: bool,
) -> Result<CommandResult, CommandFailure> {
    let mut runtime = open_runtime("edit.transaction", db, dry_run)?;
    let result = edit_transaction(runtime.connection_mut(), id, description, account, dry_run)
        .map_err(|error| map_fin_error("edit.transaction", error))?;
    Ok(CommandResult {
        tool: "edit.transaction",
        data: json!({
            "entryId": result.entry_id,
            "dryRun": result.dry_run,
            "accountCreated": result.account_created,
            "changes": result.changes,
        }),
        text: format!("edited {} (dry-run={})", result.entry_id, result.dry_run),
        meta: MetaExtras::default(),
        exit_code: ExitCode::Success,
    })
}
