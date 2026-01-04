import { describe, expect, test } from 'bun:test';

import { type AccountSeriesState, collectSortedDates, mergeBalanceSeriesByDate } from '../../src/queries/groups';

function makeState(points: Array<{ date: string; balance: number }>): AccountSeriesState {
	return {
		points: points.map((p) => ({ date: p.date, balanceMinor: p.balance })),
		index: 0,
		current: null,
	};
}

describe('collectSortedDates', () => {
	test('returns empty for empty input', () => {
		const dates = collectSortedDates([]);
		expect(dates).toEqual([]);
	});

	test('returns empty for empty series', () => {
		const dates = collectSortedDates([makeState([])]);
		expect(dates).toEqual([]);
	});

	test('collects dates from single series', () => {
		const dates = collectSortedDates([
			makeState([
				{ date: '2024-01-01', balance: 100 },
				{ date: '2024-01-02', balance: 200 },
				{ date: '2024-01-03', balance: 300 },
			]),
		]);
		expect(dates).toEqual(['2024-01-01', '2024-01-02', '2024-01-03']);
	});

	test('collects and deduplicates dates from multiple series', () => {
		const dates = collectSortedDates([
			makeState([
				{ date: '2024-01-01', balance: 100 },
				{ date: '2024-01-03', balance: 300 },
			]),
			makeState([
				{ date: '2024-01-02', balance: 200 },
				{ date: '2024-01-03', balance: 400 },
			]),
		]);
		expect(dates).toEqual(['2024-01-01', '2024-01-02', '2024-01-03']);
	});

	test('sorts dates correctly', () => {
		const dates = collectSortedDates([
			makeState([
				{ date: '2024-01-03', balance: 300 },
				{ date: '2024-01-01', balance: 100 },
			]),
		]);
		expect(dates).toEqual(['2024-01-01', '2024-01-03']);
	});
});

describe('mergeBalanceSeriesByDate', () => {
	test('returns empty for empty dates', () => {
		const merged = mergeBalanceSeriesByDate([makeState([{ date: '2024-01-01', balance: 100 }])], []);
		expect(merged).toEqual([]);
	});

	test('passthrough for single account', () => {
		const state = makeState([
			{ date: '2024-01-01', balance: 100 },
			{ date: '2024-01-02', balance: 200 },
			{ date: '2024-01-03', balance: 300 },
		]);

		const merged = mergeBalanceSeriesByDate([state], ['2024-01-01', '2024-01-02', '2024-01-03']);

		expect(merged).toEqual([
			{ date: '2024-01-01', balanceMinor: 100 },
			{ date: '2024-01-02', balanceMinor: 200 },
			{ date: '2024-01-03', balanceMinor: 300 },
		]);
	});

	test('sums balances for two accounts with same dates', () => {
		const state1 = makeState([
			{ date: '2024-01-01', balance: 100 },
			{ date: '2024-01-02', balance: 200 },
		]);
		const state2 = makeState([
			{ date: '2024-01-01', balance: 1000 },
			{ date: '2024-01-02', balance: 2000 },
		]);

		const merged = mergeBalanceSeriesByDate([state1, state2], ['2024-01-01', '2024-01-02']);

		expect(merged).toEqual([
			{ date: '2024-01-01', balanceMinor: 1100 },
			{ date: '2024-01-02', balanceMinor: 2200 },
		]);
	});

	test('carries forward for sparse data', () => {
		const state1 = makeState([
			{ date: '2024-01-01', balance: 100 },
			{ date: '2024-01-03', balance: 300 },
		]);
		const state2 = makeState([
			{ date: '2024-01-02', balance: 1000 },
			{ date: '2024-01-04', balance: 2000 },
		]);

		const merged = mergeBalanceSeriesByDate([state1, state2], ['2024-01-01', '2024-01-02', '2024-01-03', '2024-01-04']);

		expect(merged).toEqual([
			{ date: '2024-01-01', balanceMinor: 100 }, // Only state1 has data
			{ date: '2024-01-02', balanceMinor: 1100 }, // state1 carries forward (100), state2 has 1000
			{ date: '2024-01-03', balanceMinor: 1300 }, // state1 has 300, state2 carries forward (1000)
			{ date: '2024-01-04', balanceMinor: 2300 }, // state1 carries forward (300), state2 has 2000
		]);
	});

	test('handles account with no data on first date', () => {
		const state1 = makeState([{ date: '2024-01-01', balance: 100 }]);
		const state2 = makeState([{ date: '2024-01-02', balance: 1000 }]);

		const merged = mergeBalanceSeriesByDate([state1, state2], ['2024-01-01', '2024-01-02']);

		expect(merged).toEqual([
			{ date: '2024-01-01', balanceMinor: 100 }, // Only state1 has data, state2 is null
			{ date: '2024-01-02', balanceMinor: 1100 }, // state1 carries forward (100), state2 has 1000
		]);
	});

	test('handles negative balances', () => {
		const state1 = makeState([{ date: '2024-01-01', balance: -100 }]);
		const state2 = makeState([{ date: '2024-01-01', balance: 200 }]);

		const merged = mergeBalanceSeriesByDate([state1, state2], ['2024-01-01']);

		expect(merged).toEqual([{ date: '2024-01-01', balanceMinor: 100 }]);
	});

	test('handles all accounts with null initial state', () => {
		const state1 = makeState([{ date: '2024-01-02', balance: 100 }]);
		const state2 = makeState([{ date: '2024-01-02', balance: 200 }]);

		const merged = mergeBalanceSeriesByDate([state1, state2], ['2024-01-01', '2024-01-02']);

		expect(merged).toEqual([
			{ date: '2024-01-01', balanceMinor: 0 }, // Both null, total is 0
			{ date: '2024-01-02', balanceMinor: 300 },
		]);
	});
});
