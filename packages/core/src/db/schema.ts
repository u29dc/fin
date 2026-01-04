export const SCHEMA_VERSION = 1;

export const SCHEMA_SQL = `
-- Chart of Accounts: hierarchical account structure
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

-- Journal Entries: the transaction header
CREATE TABLE IF NOT EXISTS journal_entries (
	id TEXT PRIMARY KEY,
	posted_at TEXT NOT NULL,
	description TEXT NOT NULL,
	raw_description TEXT,
	clean_description TEXT,
	counterparty TEXT,
	source_file TEXT,
	created_at TEXT NOT NULL DEFAULT (datetime('now')),
	updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Postings: the individual debit/credit lines
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

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_postings_journal_entry ON postings(journal_entry_id);
CREATE INDEX IF NOT EXISTS idx_postings_account ON postings(account_id);
CREATE INDEX IF NOT EXISTS idx_journal_entries_posted ON journal_entries(posted_at);
CREATE INDEX IF NOT EXISTS idx_chart_of_accounts_type ON chart_of_accounts(account_type);
CREATE INDEX IF NOT EXISTS idx_chart_of_accounts_parent ON chart_of_accounts(parent_id);

-- Unique index on provider_txn_id for deduplication
CREATE UNIQUE INDEX IF NOT EXISTS idx_postings_provider_txn
	ON postings(provider_txn_id)
	WHERE provider_txn_id IS NOT NULL;
`;
