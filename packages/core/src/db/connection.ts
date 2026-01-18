import { Database } from 'bun:sqlite';
import { mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

import { migrateToLatest } from './migrate';
import { SCHEMA_VERSION } from './schema';

export type OpenDatabaseOptions = {
	path?: string;
	create?: boolean;
	readonly?: boolean;
	migrate?: boolean;
};

function applyPragmas(db: Database): void {
	db.exec(`
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = -64000;
        PRAGMA temp_store = MEMORY;
    `);
}

function getUserVersion(db: Database): number {
	const row = db.query<{ user_version: number }, []>('PRAGMA user_version').get();
	return row?.user_version ?? 0;
}

export function openDatabase(options: OpenDatabaseOptions = {}): Database {
	const { path = resolve(process.cwd(), 'data/fin.db'), create = true, readonly = false, migrate: shouldMigrate = false } = options;

	if (create) {
		mkdirSync(dirname(path), { recursive: true });
	}

	if (shouldMigrate && readonly) {
		const roDb = new Database(path, { create, readonly: true });
		applyPragmas(roDb);
		const needsMigration = getUserVersion(roDb) < SCHEMA_VERSION;
		roDb.close();

		if (needsMigration) {
			const rwDb = new Database(path, { create, readonly: false });
			applyPragmas(rwDb);
			migrateToLatest(rwDb);
			rwDb.close();
		}

		const db = new Database(path, { create, readonly: true });
		applyPragmas(db);
		return db;
	}

	const db = new Database(path, { create, readonly });
	applyPragmas(db);

	if (shouldMigrate && !readonly) {
		migrateToLatest(db);
	}

	return db;
}
