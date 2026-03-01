#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationMetadata {
    pub from_version: i32,
    pub to_version: i32,
    pub name: &'static str,
    pub parity_source: &'static str,
    pub notes: &'static str,
}

pub const SCHEMA_VERSION: i32 = 5;

pub const REQUIRED_TABLES: [&str; 3] = ["chart_of_accounts", "journal_entries", "postings"];

pub const MIGRATION_METADATA: [MigrationMetadata; 5] = [
    MigrationMetadata {
        from_version: 0,
        to_version: 1,
        name: "init-schema",
        parity_source: "packages/core/src/db/schema.ts",
        notes: "Create baseline tables and indexes.",
    },
    MigrationMetadata {
        from_version: 1,
        to_version: 2,
        name: "dedupe-index",
        parity_source: "packages/core/src/db/migrate.ts#migrateToV2",
        notes: "Provider transaction dedupe unique index scoped to account.",
    },
    MigrationMetadata {
        from_version: 2,
        to_version: 3,
        name: "bill-accounts",
        parity_source: "packages/core/src/db/migrate.ts#migrateToV3",
        notes: "Add bill category leaf accounts.",
    },
    MigrationMetadata {
        from_version: 3,
        to_version: 4,
        name: "posted-date-indexes",
        parity_source: "packages/core/src/db/migrate.ts#migrateToV4",
        notes: "Add posted_date and import query indexes.",
    },
    MigrationMetadata {
        from_version: 4,
        to_version: 5,
        name: "transfer-flag",
        parity_source: "packages/core/src/db/migrate.ts#migrateToV5",
        notes: "Add is_transfer marker and transfer index.",
    },
];

pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS chart_of_accounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    account_type TEXT NOT NULL CHECK (account_type IN ('asset', 'liability', 'equity', 'income', 'expense')),
    parent_id TEXT REFERENCES chart_of_accounts(id),
    currency TEXT DEFAULT 'GBP',
    is_placeholder INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS journal_entries (
    id TEXT PRIMARY KEY,
    posted_at TEXT NOT NULL,
    posted_date TEXT NOT NULL,
    is_transfer INTEGER NOT NULL DEFAULT 0,
    description TEXT NOT NULL,
    raw_description TEXT,
    clean_description TEXT,
    counterparty TEXT,
    source_file TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS postings (
    id TEXT PRIMARY KEY,
    journal_entry_id TEXT NOT NULL REFERENCES journal_entries(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES chart_of_accounts(id),
    amount_minor INTEGER NOT NULL,
    currency TEXT NOT NULL DEFAULT 'GBP',
    memo TEXT,
    provider_txn_id TEXT,
    provider_balance_minor INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_postings_journal_entry ON postings(journal_entry_id);
CREATE INDEX IF NOT EXISTS idx_postings_account ON postings(account_id);
CREATE INDEX IF NOT EXISTS idx_postings_journal_entry_account ON postings(journal_entry_id, account_id);
CREATE INDEX IF NOT EXISTS idx_journal_entries_posted ON journal_entries(posted_at);
CREATE INDEX IF NOT EXISTS idx_journal_entries_posted_date ON journal_entries(posted_date);
CREATE INDEX IF NOT EXISTS idx_journal_entries_is_transfer_posted ON journal_entries(is_transfer, posted_at);
CREATE INDEX IF NOT EXISTS idx_journal_entries_source_file ON journal_entries(source_file);
CREATE INDEX IF NOT EXISTS idx_chart_of_accounts_type ON chart_of_accounts(account_type);
CREATE INDEX IF NOT EXISTS idx_chart_of_accounts_parent ON chart_of_accounts(parent_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_postings_provider_txn
    ON postings(provider_txn_id, account_id)
    WHERE provider_txn_id IS NOT NULL;
"#;
