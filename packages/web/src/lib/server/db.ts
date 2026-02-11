import { migrateToLatest, openDatabase } from '@fin/core';
import { initConfig, resolveFinPaths } from '@fin/core/config';

// Initialize config before database operations (required for SSR build)
initConfig();

function getDbPath(): string {
	if (process.env['DB_PATH']) {
		return process.env['DB_PATH'];
	}
	return resolveFinPaths().dbFile;
}

const dbPath = getDbPath();
export const db = openDatabase({ path: dbPath, migrate: false });
migrateToLatest(db);
