import { Database } from 'bun:sqlite';
import { mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

import { migrateToLatest } from './migrate';

export type OpenDatabaseOptions = {
	path?: string;
	create?: boolean;
	readonly?: boolean;
	migrate?: boolean;
};

export function openDatabase(options: OpenDatabaseOptions = {}): Database {
	const { path = resolve(process.cwd(), 'data/fin.db'), create = true, readonly = false, migrate: shouldMigrate = false } = options;

	if (create) {
		mkdirSync(dirname(path), { recursive: true });
	}

	const db = new Database(path, { create, readonly });

	// Performance pragmas
	db.exec(`
		PRAGMA foreign_keys = ON;
		PRAGMA journal_mode = WAL;
		PRAGMA synchronous = NORMAL;
		PRAGMA cache_size = -64000;
		PRAGMA temp_store = MEMORY;
	`);

	if (shouldMigrate) {
		migrateToLatest(db);
	}

	return db;
}
