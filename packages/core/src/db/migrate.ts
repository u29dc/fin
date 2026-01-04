import type { Database } from 'bun:sqlite';

import { SCHEMA_SQL, SCHEMA_VERSION } from './schema';
import { getChartOfAccountsSeeds } from './seed-chart-of-accounts';

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

	if (currentVersion < SCHEMA_VERSION) {
		db.transaction(() => {
			initializeFreshDb(db);
			setUserVersion(db, SCHEMA_VERSION);
		})();
	}
}
