import { Database } from 'bun:sqlite';
import { describe, expect, test } from 'bun:test';
import { SCHEMA_SQL } from '../../src/db/schema';
import { loadTransferRows } from '../../src/queries/metrics';
import type { AssetAccountId } from '../../src/types/chart-account-ids';

function createTestDb(): Database {
	const db = new Database(':memory:');
	db.exec(SCHEMA_SQL);
	return db;
}

function insertAccount(db: Database, id: string, type: 'asset' | 'expense' | 'income' | 'equity'): void {
	db.prepare(`INSERT INTO chart_of_accounts (id, name, account_type) VALUES (?, ?, ?)`).run(id, id, type);
}

function insertTransfer(db: Database, journalId: string, postedAt: string, fromAccount: string, toAccount: string, amountMinor: number): void {
	db.prepare(`INSERT INTO journal_entries (id, posted_at, description) VALUES (?, ?, 'Transfer')`).run(journalId, postedAt);
	db.prepare(`INSERT INTO postings (id, journal_entry_id, account_id, amount_minor) VALUES (?, ?, ?, ?)`).run(`${journalId}_from`, journalId, fromAccount, -amountMinor);
	db.prepare(`INSERT INTO postings (id, journal_entry_id, account_id, amount_minor) VALUES (?, ?, ?, ?)`).run(`${journalId}_to`, journalId, toAccount, amountMinor);
}

function insertNonTransfer(db: Database, journalId: string, postedAt: string, assetAccount: string, counterAccount: string, amountMinor: number): void {
	db.prepare(`INSERT INTO journal_entries (id, posted_at, description) VALUES (?, ?, 'Payment')`).run(journalId, postedAt);
	db.prepare(`INSERT INTO postings (id, journal_entry_id, account_id, amount_minor) VALUES (?, ?, ?, ?)`).run(`${journalId}_asset`, journalId, assetAccount, amountMinor);
	db.prepare(`INSERT INTO postings (id, journal_entry_id, account_id, amount_minor) VALUES (?, ?, ?, ?)`).run(`${journalId}_counter`, journalId, counterAccount, -amountMinor);
}

function insertThreePostingEntry(db: Database, journalId: string, postedAt: string, account1: string, account2: string, account3: string): void {
	db.prepare(`INSERT INTO journal_entries (id, posted_at, description) VALUES (?, ?, 'Split')`).run(journalId, postedAt);
	db.prepare(`INSERT INTO postings (id, journal_entry_id, account_id, amount_minor) VALUES (?, ?, ?, ?)`).run(`${journalId}_1`, journalId, account1, -100000);
	db.prepare(`INSERT INTO postings (id, journal_entry_id, account_id, amount_minor) VALUES (?, ?, ?, ?)`).run(`${journalId}_2`, journalId, account2, 50000);
	db.prepare(`INSERT INTO postings (id, journal_entry_id, account_id, amount_minor) VALUES (?, ?, ?, ?)`).run(`${journalId}_3`, journalId, account3, 50000);
}

describe('loadTransferRows', () => {
	test('returns empty array for empty account list', () => {
		const db = createTestDb();
		const rows = loadTransferRows(db, [], '2024-01-01');
		expect(rows).toEqual([]);
	});

	test('returns empty array when no transfers exist', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Business:Wise', 'asset');
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');

		const rows = loadTransferRows(db, ['Assets:Business:Wise' as AssetAccountId], '2024-01-01');
		expect(rows).toEqual([]);
	});

	test('returns empty array for transfers before fromDate', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Business:Wise', 'asset');
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		insertTransfer(db, 'je1', '2023-12-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 100000);

		const rows = loadTransferRows(db, ['Assets:Business:Wise' as AssetAccountId], '2024-01-01');
		expect(rows).toEqual([]);
	});

	test('loads single transfer pair', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Business:Wise', 'asset');
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		insertTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 100000);

		const rows = loadTransferRows(db, ['Assets:Business:Wise' as AssetAccountId, 'Assets:Personal:Monzo' as AssetAccountId], '2024-01-01');

		expect(rows).toHaveLength(2);
		const fromRow = rows.find((r) => r.amount_minor < 0);
		const toRow = rows.find((r) => r.amount_minor > 0);

		expect(fromRow?.chart_account_id).toBe('Assets:Business:Wise');
		expect(fromRow?.amount_minor).toBe(-100000);
		expect(toRow?.chart_account_id).toBe('Assets:Personal:Monzo');
		expect(toRow?.amount_minor).toBe(100000);
	});

	test('filters by account - returns only requested side', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Business:Wise', 'asset');
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		insertTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 100000);

		// Only request the business account
		const rows = loadTransferRows(db, ['Assets:Business:Wise' as AssetAccountId], '2024-01-01');

		expect(rows).toHaveLength(1);
		expect(rows[0]?.chart_account_id).toBe('Assets:Business:Wise');
		expect(rows[0]?.amount_minor).toBe(-100000);
	});

	test('excludes entries with 3+ postings', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Business:Wise', 'asset');
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		insertAccount(db, 'Assets:Personal:Savings', 'asset');
		insertThreePostingEntry(db, 'je1', '2024-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 'Assets:Personal:Savings');

		const rows = loadTransferRows(db, ['Assets:Business:Wise' as AssetAccountId, 'Assets:Personal:Monzo' as AssetAccountId], '2024-01-01');

		expect(rows).toEqual([]);
	});

	test('excludes non-transfer entries (asset + expense)', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		insertAccount(db, 'Expenses:Food', 'expense');
		insertNonTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Personal:Monzo', 'Expenses:Food', -5000);

		const rows = loadTransferRows(db, ['Assets:Personal:Monzo' as AssetAccountId], '2024-01-01');

		expect(rows).toEqual([]);
	});

	test('excludes non-transfer entries (asset + income)', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		insertAccount(db, 'Income:Salary', 'income');
		insertNonTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Personal:Monzo', 'Income:Salary', 300000);

		const rows = loadTransferRows(db, ['Assets:Personal:Monzo' as AssetAccountId], '2024-01-01');

		expect(rows).toEqual([]);
	});

	test('loads multiple transfers', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Business:Wise', 'asset');
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		insertTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 100000);
		insertTransfer(db, 'je2', '2024-02-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 200000);

		const rows = loadTransferRows(db, ['Assets:Business:Wise' as AssetAccountId, 'Assets:Personal:Monzo' as AssetAccountId], '2024-01-01');

		expect(rows).toHaveLength(4);
		const amounts = rows.map((r) => r.amount_minor).sort((a, b) => a - b);
		expect(amounts).toEqual([-200000, -100000, 100000, 200000]);
	});

	test('handles date boundary correctly', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Business:Wise', 'asset');
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		// Exactly at fromDate
		insertTransfer(db, 'je1', '2024-01-01T00:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 100000);
		// Before fromDate
		insertTransfer(db, 'je2', '2023-12-31T23:59:59', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 50000);

		const rows = loadTransferRows(db, ['Assets:Business:Wise' as AssetAccountId, 'Assets:Personal:Monzo' as AssetAccountId], '2024-01-01');

		expect(rows).toHaveLength(2);
		expect(rows.every((r) => Math.abs(r.amount_minor) === 100000)).toBe(true);
	});

	test('preserves posted_at timestamp', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Business:Wise', 'asset');
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		insertTransfer(db, 'je1', '2024-01-15T14:30:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 100000);

		const rows = loadTransferRows(db, ['Assets:Business:Wise' as AssetAccountId, 'Assets:Personal:Monzo' as AssetAccountId], '2024-01-01');

		expect(rows[0]?.posted_at).toBe('2024-01-15T14:30:00');
	});
});
