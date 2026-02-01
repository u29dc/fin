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

export function migrateToLatest(db: Database): void {
	const currentVersion = getUserVersion(db);
	if (currentVersion >= SCHEMA_VERSION) {
		return;
	}

	db.transaction(() => {
		if (currentVersion === 0) {
			initializeFreshDb(db);
		}

		if (currentVersion < 2) {
			db.exec(`
                DROP INDEX IF EXISTS idx_postings_provider_txn;
                CREATE UNIQUE INDEX IF NOT EXISTS idx_postings_provider_txn
                    ON postings(provider_txn_id, account_id)
                    WHERE provider_txn_id IS NOT NULL;
            `);
		}

		if (currentVersion < 3) {
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

		setUserVersion(db, SCHEMA_VERSION);
	})();
}
