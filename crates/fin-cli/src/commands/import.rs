use serde_json::json;

use fin_sdk::{ImportInboxOptions, ImportMode, import_inbox};

use crate::commands::{CommandFailure, CommandResult, map_fin_error};
use crate::envelope::MetaExtras;
use crate::error::ExitCode;

pub fn run(
    inbox: Option<&str>,
    db: Option<&str>,
    full_export: bool,
) -> Result<CommandResult, CommandFailure> {
    let mode = if full_export {
        ImportMode::FullExport
    } else {
        ImportMode::Append
    };
    let result = import_inbox(ImportInboxOptions {
        inbox_dir: inbox.map(std::path::PathBuf::from),
        db_path: db.map(std::path::PathBuf::from),
        migrate: true,
        mode,
        ..ImportInboxOptions::default()
    })
    .map_err(|error| map_fin_error("import", error))?;

    let text = format!(
        "Results:\n  Mode: {:?}\n  Transactions parsed: {}\n  Duplicates skipped: {}\n  Replaced provider transactions: {}\n  Journal entries created: {}\n  Transfer pairs created: {}\n  Entry errors: {}\n  Archived files: {}",
        result.mode,
        result.total_transactions,
        result.duplicate_transactions,
        result.replaced_provider_transactions,
        result.journal_entries_created,
        result.transfer_pairs_created,
        result.entry_errors.len(),
        result.archived_files.len(),
    );

    Ok(CommandResult {
        tool: "import",
        data: json!({
            "mode": result.mode,
            "processedFiles": result.processed_files,
            "archivedFiles": result.archived_files,
            "skippedFiles": result.skipped_files,
            "totalTransactions": result.total_transactions,
            "uniqueTransactions": result.unique_transactions,
            "duplicateTransactions": result.duplicate_transactions,
            "journalEntriesAttempted": result.journal_entries_attempted,
            "journalEntriesCreated": result.journal_entries_created,
            "transferPairsCreated": result.transfer_pairs_created,
            "replacedProviderTransactions": result.replaced_provider_transactions,
            "replacedJournalEntries": result.replaced_journal_entries,
            "entryErrors": result.entry_errors,
            "accountsTouched": result.accounts_touched,
            "unmappedDescriptions": result.unmapped_descriptions,
        }),
        text,
        meta: MetaExtras {
            count: Some(result.journal_entries_created),
            total: None,
            has_more: None,
        },
        exit_code: ExitCode::Success,
    })
}
