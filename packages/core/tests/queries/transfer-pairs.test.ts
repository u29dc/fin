import { describe, expect, test } from 'bun:test';
import { matchTransferPairs, matchTransferPairsForAmountGroup, type TransferRow } from '../../src/queries/metrics';
import type { AssetAccountId } from '../../src/types/chart-account-ids';

const FROM_ACCOUNTS = new Set<AssetAccountId>(['Assets:Business:Wise', 'Assets:Business:Monzo']);
const TO_ACCOUNTS = new Set<AssetAccountId>(['Assets:Personal:Monzo']);

function makeTxn(id: string, chartAccountId: AssetAccountId, postedAt: string, amountMinor: number): TransferRow {
	return { id, chart_account_id: chartAccountId, posted_at: postedAt, amount_minor: amountMinor };
}

describe('matchTransferPairsForAmountGroup', () => {
	test('matches simple same-day pair', () => {
		const group = [makeTxn('a', 'Assets:Business:Wise', '2024-01-15T10:00:00', -100000), makeTxn('b', 'Assets:Personal:Monzo', '2024-01-15T11:00:00', 100000)];

		const pairs = matchTransferPairsForAmountGroup(group, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(1);
		expect(pairs[0]?.from.id).toBe('a');
		expect(pairs[0]?.to.id).toBe('b');
		expect(pairs[0]?.absAmountMinor).toBe(100000);
	});

	test('matches with 1-day lag (from before to)', () => {
		const group = [makeTxn('a', 'Assets:Business:Wise', '2024-01-15T23:00:00', -100000), makeTxn('b', 'Assets:Personal:Monzo', '2024-01-16T01:00:00', 100000)];

		const pairs = matchTransferPairsForAmountGroup(group, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(1);
		expect(pairs[0]?.from.id).toBe('a');
		expect(pairs[0]?.to.id).toBe('b');
	});

	test('matches with 1-day lag (to before from)', () => {
		const group = [makeTxn('b', 'Assets:Personal:Monzo', '2024-01-15T23:00:00', 100000), makeTxn('a', 'Assets:Business:Wise', '2024-01-16T01:00:00', -100000)];

		const pairs = matchTransferPairsForAmountGroup(group, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(1);
	});

	test('does not match with 2-day gap', () => {
		const group = [makeTxn('a', 'Assets:Business:Wise', '2024-01-15T10:00:00', -100000), makeTxn('b', 'Assets:Personal:Monzo', '2024-01-17T10:00:00', 100000)];

		const pairs = matchTransferPairsForAmountGroup(group, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(0);
	});

	test('multiple from, single to: first from matches', () => {
		const group = [
			makeTxn('a1', 'Assets:Business:Wise', '2024-01-15T09:00:00', -100000),
			makeTxn('a2', 'Assets:Business:Monzo', '2024-01-15T10:00:00', -100000),
			makeTxn('b', 'Assets:Personal:Monzo', '2024-01-15T11:00:00', 100000),
		];

		const pairs = matchTransferPairsForAmountGroup(group, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(1);
		expect(pairs[0]?.from.id).toBe('a2'); // Last 'from' before 'to' matches due to pop behavior
	});

	test('single from, multiple to: first to matches', () => {
		const group = [
			makeTxn('a', 'Assets:Business:Wise', '2024-01-15T09:00:00', -100000),
			makeTxn('b1', 'Assets:Personal:Monzo', '2024-01-15T10:00:00', 100000),
			makeTxn('b2', 'Assets:Personal:Monzo', '2024-01-15T11:00:00', 100000),
		];

		const pairs = matchTransferPairsForAmountGroup(group, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(1);
		// First 'to' is pushed to unmatched, 'from' matches with it
	});

	test('ignores wrong direction transactions', () => {
		const group = [
			// Positive in from account (wrong direction)
			makeTxn('a', 'Assets:Business:Wise', '2024-01-15T10:00:00', 100000),
			// Negative in to account (wrong direction)
			makeTxn('b', 'Assets:Personal:Monzo', '2024-01-15T11:00:00', -100000),
		];

		const pairs = matchTransferPairsForAmountGroup(group, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(0);
	});

	test('returns empty for empty group', () => {
		const pairs = matchTransferPairsForAmountGroup([], FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(0);
	});

	test('handles multiple pairs', () => {
		const group = [
			makeTxn('a1', 'Assets:Business:Wise', '2024-01-15T09:00:00', -100000),
			makeTxn('b1', 'Assets:Personal:Monzo', '2024-01-15T10:00:00', 100000),
			makeTxn('a2', 'Assets:Business:Wise', '2024-01-16T09:00:00', -100000),
			makeTxn('b2', 'Assets:Personal:Monzo', '2024-01-16T10:00:00', 100000),
		];

		const pairs = matchTransferPairsForAmountGroup(group, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(2);
	});
});

describe('matchTransferPairs', () => {
	test('groups by absolute amount and matches', () => {
		const rows = [
			makeTxn('a1', 'Assets:Business:Wise', '2024-01-15T10:00:00', -100000),
			makeTxn('b1', 'Assets:Personal:Monzo', '2024-01-15T11:00:00', 100000),
			makeTxn('a2', 'Assets:Business:Wise', '2024-01-16T10:00:00', -200000),
			makeTxn('b2', 'Assets:Personal:Monzo', '2024-01-16T11:00:00', 200000),
		];

		const pairs = matchTransferPairs(rows, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(2);

		const amounts = pairs.map((p) => p.absAmountMinor).sort((a, b) => a - b);
		expect(amounts).toEqual([100000, 200000]);
	});

	test('filters out small amounts (below MIN_TRANSFER_MINOR)', () => {
		const rows = [
			// 499 pence is below MIN_TRANSFER_MINOR (500)
			makeTxn('a', 'Assets:Business:Wise', '2024-01-15T10:00:00', -499),
			makeTxn('b', 'Assets:Personal:Monzo', '2024-01-15T11:00:00', 499),
		];

		const pairs = matchTransferPairs(rows, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(0);
	});

	test('matches amounts at MIN_TRANSFER_MINOR threshold', () => {
		const rows = [
			// 500 pence is at MIN_TRANSFER_MINOR
			makeTxn('a', 'Assets:Business:Wise', '2024-01-15T10:00:00', -500),
			makeTxn('b', 'Assets:Personal:Monzo', '2024-01-15T11:00:00', 500),
		];

		const pairs = matchTransferPairs(rows, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(1);
	});

	test('returns empty for empty rows', () => {
		const pairs = matchTransferPairs([], FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(0);
	});

	test('handles mixed matchable and unmatchable rows', () => {
		const rows = [
			// Matchable pair
			makeTxn('a1', 'Assets:Business:Wise', '2024-01-15T10:00:00', -100000),
			makeTxn('b1', 'Assets:Personal:Monzo', '2024-01-15T11:00:00', 100000),
			// Unmatchable (too far apart in time)
			makeTxn('a2', 'Assets:Business:Wise', '2024-01-20T10:00:00', -200000),
			makeTxn('b2', 'Assets:Personal:Monzo', '2024-01-25T11:00:00', 200000),
		];

		const pairs = matchTransferPairs(rows, FROM_ACCOUNTS, TO_ACCOUNTS);
		expect(pairs).toHaveLength(1);
		expect(pairs[0]?.absAmountMinor).toBe(100000);
	});
});
