import type { Database } from 'bun:sqlite';

import { SCHEMA_SQL, SCHEMA_VERSION } from './schema';
import { getChartOfAccountsSeeds } from './seed';

type UserVersionRow = {
	user_version: number;
};

function getUserVersion(db: Database): number {
	const row = db.query<UserVersionRow, []>('PRAGMA user_version').get();
	return row?.user_version ?? 0;
}

function setUserVersion(db: Database, version: number): void {
	db.exec(`PRAGMA user_version = ${version}`);
}

type CountRow = { count: number };

type ColumnInfoRow = { name: string };

function columnExists(db: Database, tableName: string, columnName: string): boolean {
	const rows = db.query<ColumnInfoRow, []>(`PRAGMA table_info(${tableName})`).all();
	return rows.some((row) => row.name === columnName);
}

function initializeFreshDb(db: Database): void {
	db.exec(SCHEMA_SQL);

	// Skip seeding if chart_of_accounts already has data
	const row = db.query<CountRow, []>('SELECT COUNT(*) as count FROM chart_of_accounts').get();
	if (row && row.count > 0) {
		return;
	}

	const stmt = db.prepare(`
		INSERT INTO chart_of_accounts (id, name, account_type, parent_id, is_placeholder)
		VALUES (?, ?, ?, ?, ?)
	`);

	for (const account of getChartOfAccountsSeeds()) {
		stmt.run(account.id, account.name, account.type, account.parent, account.placeholder ? 1 : 0);
	}
}

function migrateToV2(db: Database, currentVersion: number): void {
	if (currentVersion >= 2) {
		return;
	}

	db.exec(`
		DROP INDEX IF EXISTS idx_postings_provider_txn;
		CREATE UNIQUE INDEX IF NOT EXISTS idx_postings_provider_txn
			ON postings(provider_txn_id, account_id)
			WHERE provider_txn_id IS NOT NULL;
	`);
}

function migrateToV3(db: Database, currentVersion: number): void {
	if (currentVersion >= 3) {
		return;
	}

	const billAccounts = [
		{ id: 'Expenses:Bills:Energy', name: 'Energy', parent: 'Expenses:Bills' },
		{ id: 'Expenses:Bills:Water', name: 'Water', parent: 'Expenses:Bills' },
		{ id: 'Expenses:Bills:CouncilTax', name: 'Council Tax', parent: 'Expenses:Bills' },
		{ id: 'Expenses:Bills:Internet', name: 'Internet', parent: 'Expenses:Bills' },
		{ id: 'Expenses:Bills:Insurance', name: 'Insurance', parent: 'Expenses:Bills' },
	];

	const insertStmt = db.prepare(`
		INSERT OR IGNORE INTO chart_of_accounts (id, name, account_type, parent_id, is_placeholder)
		VALUES (?, ?, 'expense', ?, 0)
	`);

	for (const account of billAccounts) {
		insertStmt.run(account.id, account.name, account.parent);
	}
}

function migrateToV4(db: Database, currentVersion: number): void {
	if (currentVersion >= 4) {
		return;
	}

	if (!columnExists(db, 'journal_entries', 'posted_date')) {
		db.exec(`ALTER TABLE journal_entries ADD COLUMN posted_date TEXT NOT NULL DEFAULT ''`);
	}
	db.exec(`UPDATE journal_entries SET posted_date = date(posted_at) WHERE posted_date IS NULL OR posted_date = ''`);
	db.exec(`CREATE INDEX IF NOT EXISTS idx_journal_entries_posted_date ON journal_entries(posted_date)`);
	db.exec(`CREATE INDEX IF NOT EXISTS idx_postings_journal_entry_account ON postings(journal_entry_id, account_id)`);
	db.exec(`CREATE INDEX IF NOT EXISTS idx_journal_entries_source_file ON journal_entries(source_file)`);
}

function migrateToV5(db: Database, currentVersion: number): void {
	if (currentVersion >= 5) {
		return;
	}

	if (!columnExists(db, 'journal_entries', 'is_transfer')) {
		db.exec(`ALTER TABLE journal_entries ADD COLUMN is_transfer INTEGER NOT NULL DEFAULT 0`);
	}
	db.exec(`
		UPDATE journal_entries
		SET is_transfer = 1
		WHERE id IN (
			SELECT p.journal_entry_id
			FROM postings p
			INNER JOIN chart_of_accounts coa ON p.account_id = coa.id
			GROUP BY p.journal_entry_id
			HAVING COUNT(*) = 2
				AND SUM(CASE WHEN coa.account_type = 'asset' THEN 1 ELSE 0 END) = 2
		)
	`);
	db.exec(`CREATE INDEX IF NOT EXISTS idx_journal_entries_is_transfer_posted ON journal_entries(is_transfer, posted_at)`);
}

export function migrateToLatest(db: Database): void {
	const currentVersion = getUserVersion(db);
	if (currentVersion >= SCHEMA_VERSION) {
		return;
	}

	db.transaction(() => {
		if (currentVersion === 0) {
			initializeFreshDb(db);
		}

		migrateToV2(db, currentVersion);
		migrateToV3(db, currentVersion);
		migrateToV4(db, currentVersion);
		migrateToV5(db, currentVersion);

		setUserVersion(db, SCHEMA_VERSION);
	})();
}
