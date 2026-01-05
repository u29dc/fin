import { join } from 'node:path';
import { migrateToLatest, openDatabase } from '@fin/core';
import { findMonorepoRoot, initConfig } from '@fin/core/config';

// Initialize config before database operations (required for SSR build)
initConfig();

function getDbPath(): string {
	if (process.env['DB_PATH']) {
		return process.env['DB_PATH'];
	}
	const root = findMonorepoRoot(process.cwd());
	if (root) {
		return join(root, 'data', 'fin.db');
	}
	return join(process.cwd(), 'data', 'fin.db');
}

const dbPath = getDbPath();
export const db = openDatabase({ path: dbPath, migrate: false });
migrateToLatest(db);
