import { Database } from 'bun:sqlite';
import { describe, expect, test } from 'bun:test';
import { SCHEMA_SQL } from '../../src/db/schema';
import { createJournalEntriesFromTransactions } from '../../src/import/journal';
import type { CanonicalTransaction } from '../../src/import/transactions';

function createTestDb(): Database {
	const db = new Database(':memory:');
	db.exec('PRAGMA foreign_keys = ON;');
	db.exec(SCHEMA_SQL);
	return db;
}

function insertAccount(db: Database, id: string, type: 'asset' | 'expense' | 'income' | 'equity'): void {
	db.prepare('INSERT INTO chart_of_accounts (id, name, account_type) VALUES (?, ?, ?)').run(id, id, type);
}

function createTxn(overrides: Partial<CanonicalTransaction> = {}): CanonicalTransaction {
	return {
		id: overrides.id ?? crypto.randomUUID(),
		chartAccountId: overrides.chartAccountId ?? 'Assets:Personal:Monzo',
		postedAt: overrides.postedAt ?? '2024-01-01T00:00:00',
		amountMinor: overrides.amountMinor ?? -1234,
		currency: overrides.currency ?? 'GBP',
		rawDescription: overrides.rawDescription ?? 'Test',
		cleanDescription: overrides.cleanDescription ?? 'Test',
		counterparty: overrides.counterparty ?? null,
		category: overrides.category ?? null,
		providerTxnId: overrides.providerTxnId ?? 'txn-1',
		balanceMinor: overrides.balanceMinor ?? null,
		sourceFile: overrides.sourceFile ?? 'test.csv',
	};
}

describe('createJournalEntriesFromTransactions', () => {
	test('rolls back failed entry to avoid partial inserts', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');

		const txn = createTxn({ providerTxnId: 'txn-fk' });
		const result = createJournalEntriesFromTransactions(db, [txn]);

		expect(result.errors.length).toBe(1);
		expect(result.journalEntriesCreated).toBe(0);

		const journalCount = db.query<{ count: number }, []>('SELECT COUNT(*) as count FROM journal_entries').get()?.count ?? 0;
		const postingsCount = db.query<{ count: number }, []>('SELECT COUNT(*) as count FROM postings').get()?.count ?? 0;

		expect(journalCount).toBe(0);
		expect(postingsCount).toBe(0);
	});

	test('dedupes duplicate provider_txn_id within a batch', () => {
		const db = createTestDb();
		insertAccount(db, 'Assets:Personal:Monzo', 'asset');
		insertAccount(db, 'Expenses:Uncategorized', 'expense');

		const txnA = createTxn({ providerTxnId: 'dup-1' });
		const txnB = createTxn({ providerTxnId: 'dup-1' });

		const result = createJournalEntriesFromTransactions(db, [txnA, txnB]);

		expect(result.uniqueTransactions).toBe(1);
		expect(result.duplicateTransactions).toBe(1);
		expect(result.journalEntriesCreated).toBe(1);
		expect(result.errors.length).toBe(0);

		const journalCount = db.query<{ count: number }, []>('SELECT COUNT(*) as count FROM journal_entries').get()?.count ?? 0;
		const postingsCount = db.query<{ count: number }, []>('SELECT COUNT(*) as count FROM postings').get()?.count ?? 0;

		expect(journalCount).toBe(1);
		expect(postingsCount).toBe(2);
	});
});
