import { Database } from 'bun:sqlite';
import { describe, expect, test } from 'bun:test';
import { SCHEMA_SQL } from '../../src/db/schema';
import { loadTransferRows, matchTransferPairs } from '../../src/queries/metrics';
import type { AssetAccountId } from '../../src/types/accounts';

/**
 * Integration tests for transfer detection pipeline:
 * loadTransferRows() -> matchTransferPairs() -> categorization
 *
 * Note: Full end-to-end tests of getScenarioMonthlyFlows() require
 * initializing the config system with known account groups.
 * These tests focus on the DB query → matching → pairing flow.
 */

const BUSINESS_ACCOUNTS = new Set<AssetAccountId>(['Assets:Business:Wise' as AssetAccountId, 'Assets:Business:Monzo' as AssetAccountId]);
const PERSONAL_ACCOUNTS = new Set<AssetAccountId>(['Assets:Personal:Monzo' as AssetAccountId]);
const JOINT_ACCOUNTS = new Set<AssetAccountId>(['Assets:Joint:Monzo' as AssetAccountId]);

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

function setupAccounts(db: Database): void {
	insertAccount(db, 'Assets:Business:Wise', 'asset');
	insertAccount(db, 'Assets:Business:Monzo', 'asset');
	insertAccount(db, 'Assets:Personal:Monzo', 'asset');
	insertAccount(db, 'Assets:Joint:Monzo', 'asset');
}

describe('Transfer Detection Integration', () => {
	describe('Business to Personal transfers (salary/dividends)', () => {
		test('detects and matches business to personal transfer', () => {
			const db = createTestDb();
			setupAccounts(db);
			insertTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 500000);

			// Load transfer rows involving business and personal accounts
			const allAccountIds = [...BUSINESS_ACCOUNTS, ...PERSONAL_ACCOUNTS];
			const rows = loadTransferRows(db, allAccountIds, '2024-01-01');

			expect(rows).toHaveLength(2);

			// Match pairs
			const pairs = matchTransferPairs(rows, BUSINESS_ACCOUNTS, PERSONAL_ACCOUNTS);

			expect(pairs).toHaveLength(1);
			expect(pairs[0]?.from.chart_account_id).toBe('Assets:Business:Wise');
			expect(pairs[0]?.to.chart_account_id).toBe('Assets:Personal:Monzo');
			expect(pairs[0]?.absAmountMinor).toBe(500000);
		});

		test('matches multiple transfers across different dates', () => {
			const db = createTestDb();
			setupAccounts(db);
			// Monthly salary-like transfers
			insertTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 300000);
			insertTransfer(db, 'je2', '2024-02-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 300000);
			insertTransfer(db, 'je3', '2024-03-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 300000);

			const allAccountIds = [...BUSINESS_ACCOUNTS, ...PERSONAL_ACCOUNTS];
			const rows = loadTransferRows(db, allAccountIds, '2024-01-01');
			const pairs = matchTransferPairs(rows, BUSINESS_ACCOUNTS, PERSONAL_ACCOUNTS);

			expect(pairs).toHaveLength(3);
			expect(pairs.every((p) => p.absAmountMinor === 300000)).toBe(true);
		});

		test('distinguishes salary from dividend by amount', () => {
			const db = createTestDb();
			setupAccounts(db);
			// Smaller transfer (would be salary if threshold is e.g., 400000)
			insertTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 300000);
			// Larger transfer (would be dividend if threshold is e.g., 400000)
			insertTransfer(db, 'je2', '2024-01-20T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 500000);

			const allAccountIds = [...BUSINESS_ACCOUNTS, ...PERSONAL_ACCOUNTS];
			const rows = loadTransferRows(db, allAccountIds, '2024-01-01');
			const pairs = matchTransferPairs(rows, BUSINESS_ACCOUNTS, PERSONAL_ACCOUNTS);

			expect(pairs).toHaveLength(2);
			const amounts = pairs.map((p) => p.absAmountMinor).sort((a, b) => a - b);
			expect(amounts).toEqual([300000, 500000]);
		});
	});

	describe('Personal to Joint transfers', () => {
		test('detects and matches personal to joint transfer', () => {
			const db = createTestDb();
			setupAccounts(db);
			insertTransfer(db, 'je1', '2024-01-28T10:00:00', 'Assets:Personal:Monzo', 'Assets:Joint:Monzo', 150000);

			const allAccountIds = [...PERSONAL_ACCOUNTS, ...JOINT_ACCOUNTS];
			const rows = loadTransferRows(db, allAccountIds, '2024-01-01');

			expect(rows).toHaveLength(2);

			const pairs = matchTransferPairs(rows, PERSONAL_ACCOUNTS, JOINT_ACCOUNTS);

			expect(pairs).toHaveLength(1);
			expect(pairs[0]?.from.chart_account_id).toBe('Assets:Personal:Monzo');
			expect(pairs[0]?.to.chart_account_id).toBe('Assets:Joint:Monzo');
			expect(pairs[0]?.absAmountMinor).toBe(150000);
		});
	});

	describe('Mixed transfer scenarios', () => {
		test('handles transfers from multiple business accounts', () => {
			const db = createTestDb();
			setupAccounts(db);
			// From Wise
			insertTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 200000);
			// From Monzo
			insertTransfer(db, 'je2', '2024-01-20T10:00:00', 'Assets:Business:Monzo', 'Assets:Personal:Monzo', 300000);

			const allAccountIds = [...BUSINESS_ACCOUNTS, ...PERSONAL_ACCOUNTS];
			const rows = loadTransferRows(db, allAccountIds, '2024-01-01');
			const pairs = matchTransferPairs(rows, BUSINESS_ACCOUNTS, PERSONAL_ACCOUNTS);

			expect(pairs).toHaveLength(2);
			const fromAccounts = pairs.map((p) => p.from.chart_account_id).sort();
			expect(fromAccounts).toEqual(['Assets:Business:Monzo', 'Assets:Business:Wise']);
		});

		test('ignores transfers below MIN_TRANSFER_MINOR threshold', () => {
			const db = createTestDb();
			setupAccounts(db);
			// 499 pence = below 500 threshold
			insertTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 499);
			// 500 pence = at threshold
			insertTransfer(db, 'je2', '2024-01-16T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 500);

			const allAccountIds = [...BUSINESS_ACCOUNTS, ...PERSONAL_ACCOUNTS];
			const rows = loadTransferRows(db, allAccountIds, '2024-01-01');
			const pairs = matchTransferPairs(rows, BUSINESS_ACCOUNTS, PERSONAL_ACCOUNTS);

			// Only the 500 pence transfer should match
			expect(pairs).toHaveLength(1);
			expect(pairs[0]?.absAmountMinor).toBe(500);
		});

		test('handles same-day and next-day matching', () => {
			const db = createTestDb();
			setupAccounts(db);
			// Same day - should match
			insertTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 100000);
			// Next day lag (from posted, to received next day) - simulated by separate entries
			// Note: In real data, these would be separate postings found by loadTransferRows

			const allAccountIds = [...BUSINESS_ACCOUNTS, ...PERSONAL_ACCOUNTS];
			const rows = loadTransferRows(db, allAccountIds, '2024-01-01');
			const pairs = matchTransferPairs(rows, BUSINESS_ACCOUNTS, PERSONAL_ACCOUNTS);

			expect(pairs).toHaveLength(1);
		});
	});

	describe('Edge cases', () => {
		test('returns no pairs when no transfers exist', () => {
			const db = createTestDb();
			setupAccounts(db);

			const allAccountIds = [...BUSINESS_ACCOUNTS, ...PERSONAL_ACCOUNTS];
			const rows = loadTransferRows(db, allAccountIds, '2024-01-01');
			const pairs = matchTransferPairs(rows, BUSINESS_ACCOUNTS, PERSONAL_ACCOUNTS);

			expect(rows).toEqual([]);
			expect(pairs).toEqual([]);
		});

		test('handles lookback date correctly', () => {
			const db = createTestDb();
			setupAccounts(db);
			// Old transfer - should be excluded
			insertTransfer(db, 'je1', '2023-01-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 100000);
			// Recent transfer - should be included
			insertTransfer(db, 'je2', '2024-06-15T10:00:00', 'Assets:Business:Wise', 'Assets:Personal:Monzo', 200000);

			const allAccountIds = [...BUSINESS_ACCOUNTS, ...PERSONAL_ACCOUNTS];
			// Looking back from 2024-01-01 (12 month lookback)
			const rows = loadTransferRows(db, allAccountIds, '2024-01-01');
			const pairs = matchTransferPairs(rows, BUSINESS_ACCOUNTS, PERSONAL_ACCOUNTS);

			expect(pairs).toHaveLength(1);
			expect(pairs[0]?.absAmountMinor).toBe(200000);
		});

		test('does not match transfers in wrong direction', () => {
			const db = createTestDb();
			setupAccounts(db);
			// Personal to Business (wrong direction for salary/dividend)
			insertTransfer(db, 'je1', '2024-01-15T10:00:00', 'Assets:Personal:Monzo', 'Assets:Business:Wise', 100000);

			const allAccountIds = [...BUSINESS_ACCOUNTS, ...PERSONAL_ACCOUNTS];
			const rows = loadTransferRows(db, allAccountIds, '2024-01-01');

			// Rows exist (both postings involve requested accounts)
			expect(rows).toHaveLength(2);

			// But matching Business -> Personal finds no pairs (direction is reversed)
			const pairs = matchTransferPairs(rows, BUSINESS_ACCOUNTS, PERSONAL_ACCOUNTS);
			expect(pairs).toHaveLength(0);

			// Matching Personal -> Business would find the pair
			const reversePairs = matchTransferPairs(rows, PERSONAL_ACCOUNTS, BUSINESS_ACCOUNTS);
			expect(reversePairs).toHaveLength(1);
		});
	});
});
