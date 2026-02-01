import { Database } from 'bun:sqlite';
import { describe, expect, test } from 'bun:test';
import { SCHEMA_SQL } from '../../src/db/schema';
import { getExpensesByCategoryForAccounts } from '../../src/queries/ledger';

type AccountType = 'asset' | 'expense' | 'income' | 'equity';

function createTestDb(): Database {
	const db = new Database(':memory:');
	db.exec('PRAGMA foreign_keys = ON;');
	db.exec(SCHEMA_SQL);
	return db;
}

function insertAccount(db: Database, id: string, type: AccountType): void {
	db.prepare('INSERT INTO chart_of_accounts (id, name, account_type) VALUES (?, ?, ?)').run(id, id, type);
}

function insertEntry(db: Database, id: string, postedAt: string, assetAccountId: string, expenseAccountId: string, amountMinor: number): void {
	db.prepare('INSERT INTO journal_entries (id, posted_at, description) VALUES (?, ?, ?)').run(id, postedAt, 'Test');
	db.prepare('INSERT INTO postings (id, journal_entry_id, account_id, amount_minor) VALUES (?, ?, ?, ?)').run(`${id}_asset`, id, assetAccountId, -amountMinor);
	db.prepare('INSERT INTO postings (id, journal_entry_id, account_id, amount_minor) VALUES (?, ?, ?, ?)').run(`${id}_expense`, id, expenseAccountId, amountMinor);
}

describe('getExpensesByCategoryForAccounts', () => {
	test('filters categories by asset accounts involved in entries', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		insertAccount(db, 'Assets:Business:Wise', 'asset');
		insertAccount(db, 'Expenses:Food:Groceries', 'expense');
		insertAccount(db, 'Expenses:Business:Software', 'expense');

		insertEntry(db, 'je1', '2020-01-01T00:00:00', 'Assets:Personal:Monzo', 'Expenses:Food:Groceries', 10000);
		insertEntry(db, 'je2', '2020-01-02T00:00:00', 'Assets:Business:Wise', 'Expenses:Business:Software', 20000);

		const personal = getExpensesByCategoryForAccounts(db, ['Assets:Personal:Monzo'], 120);
		const business = getExpensesByCategoryForAccounts(db, ['Assets:Business:Wise'], 120);

		expect(personal).toHaveLength(1);
		expect(personal[0]?.accountId).toBe('Expenses:Food:Groceries');
		expect(personal[0]?.totalMinor).toBe(10000);

		expect(business).toHaveLength(1);
		expect(business[0]?.accountId).toBe('Expenses:Business:Software');
		expect(business[0]?.totalMinor).toBe(20000);
	});
});
