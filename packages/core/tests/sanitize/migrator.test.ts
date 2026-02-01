import { Database } from 'bun:sqlite';
import { beforeEach, describe, expect, test } from 'bun:test';

import { executeMigration, planMigration } from '../../src/sanitize/migrator';
import type { NameMappingConfig } from '../../src/sanitize/types';

const testConfig: NameMappingConfig = {
	rules: [{ patterns: ['AMZN'], target: 'Amazon Shopping', category: 'Shopping' }],
	warnOnUnmapped: false,
	fallbackToRaw: true,
};

function createTestDb(): Database {
	const db = new Database(':memory:');
	db.exec(`
		CREATE TABLE journal_entries (
			id TEXT PRIMARY KEY,
			posted_at TEXT NOT NULL DEFAULT (datetime('now')),
			posted_date TEXT NOT NULL DEFAULT (date('now')),
			description TEXT NOT NULL,
			raw_description TEXT,
			clean_description TEXT,
			counterparty TEXT,
			source_file TEXT,
			created_at TEXT NOT NULL DEFAULT (datetime('now')),
			updated_at TEXT NOT NULL DEFAULT (datetime('now'))
		)
	`);
	return db;
}

function insertEntry(db: Database, id: string, rawDescription: string, cleanDescription: string): void {
	db.prepare(`INSERT INTO journal_entries (id, description, raw_description, clean_description) VALUES (?, ?, ?, ?)`).run(id, cleanDescription, rawDescription, cleanDescription);
}

describe('planMigration', () => {
	let db: Database;

	beforeEach(() => {
		db = createTestDb();
	});

	test('identifies records needing update', () => {
		insertEntry(db, '1', 'AMZN MKTP', 'AMZN MKTP');

		const plan = planMigration(db, testConfig);

		expect(plan.toUpdate.length).toBe(1);
		expect(plan.toUpdate[0]?.proposedClean).toBe('Amazon Shopping');
		expect(plan.alreadyClean).toBe(0);
		expect(plan.noMatch).toBe(0);
	});

	test('skips records already matching target', () => {
		insertEntry(db, '1', 'AMZN MKTP', 'Amazon Shopping');

		const plan = planMigration(db, testConfig);

		expect(plan.toUpdate.length).toBe(0);
		expect(plan.alreadyClean).toBe(1);
	});

	test('skips manually edited records (clean differs from raw)', () => {
		insertEntry(db, '1', 'AMZN MKTP', 'My Custom Name');

		const plan = planMigration(db, testConfig);

		// Name is manually edited (clean !== raw), skip
		expect(plan.toUpdate.length).toBe(0);
		expect(plan.alreadyClean).toBe(1);
	});

	test('counts records with no matching rule', () => {
		insertEntry(db, '1', 'Unknown Merchant', 'Unknown Merchant');

		const plan = planMigration(db, testConfig);

		expect(plan.toUpdate.length).toBe(0);
		expect(plan.noMatch).toBe(1);
	});

	test('handles multiple records correctly', () => {
		insertEntry(db, '1', 'AMZN 1', 'AMZN 1'); // needs update
		insertEntry(db, '2', 'AMZN 2', 'Amazon Shopping'); // already clean
		insertEntry(db, '3', 'Unknown', 'Unknown'); // no match
		insertEntry(db, '4', 'AMZN 3', 'Custom Edit'); // manually edited

		const plan = planMigration(db, testConfig);

		// Record 1: needs update
		// Record 2: already clean
		// Record 3: no match
		// Record 4: manually edited (clean !== raw), skip
		expect(plan.toUpdate.length).toBe(1);
		expect(plan.alreadyClean).toBe(2);
		expect(plan.noMatch).toBe(1);
	});
});

describe('executeMigration', () => {
	test('dry run does not modify database', () => {
		const db = createTestDb();
		insertEntry(db, '1', 'AMZN', 'AMZN');

		const plan = planMigration(db, testConfig);
		const result = executeMigration(db, plan, { dryRun: true });

		expect(result.updated).toBe(1);
		expect(result.skipped).toBe(0);

		const row = db.prepare('SELECT description, clean_description FROM journal_entries').get() as {
			description: string;
			clean_description: string;
		};
		expect(row.clean_description).toBe('AMZN');
	});

	test('updates records when not dry run', () => {
		const db = createTestDb();
		insertEntry(db, '1', 'AMZN', 'AMZN');

		const plan = planMigration(db, testConfig);
		const result = executeMigration(db, plan, { dryRun: false });

		expect(result.updated).toBe(1);

		const row = db.prepare('SELECT description, clean_description FROM journal_entries').get() as {
			description: string;
			clean_description: string;
		};
		expect(row.clean_description).toBe('Amazon Shopping');
		expect(row.description).toBe('Amazon Shopping');
	});

	test('returns correct counts', () => {
		const db = createTestDb();
		insertEntry(db, '1', 'AMZN 1', 'AMZN 1');
		insertEntry(db, '2', 'AMZN 2', 'AMZN 2');
		insertEntry(db, '3', 'Unknown', 'Unknown');

		const plan = planMigration(db, testConfig);
		const result = executeMigration(db, plan);

		expect(result.updated).toBe(2);
		expect(result.skipped).toBe(1);
		expect(result.errors.length).toBe(0);
	});
});
