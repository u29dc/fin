import { describe, expect, test } from 'bun:test';

import { getTypicalMonthlyMinor, mean, median } from '../../src/queries/metrics';

describe('median', () => {
	test('returns 0 for empty array', () => {
		expect(median([])).toBe(0);
	});

	test('returns the value for single element', () => {
		expect(median([100])).toBe(100);
	});

	test('returns average of two middle for even length', () => {
		expect(median([100, 200])).toBe(150);
		expect(median([10, 20, 30, 40])).toBe(25);
	});

	test('returns middle for odd length', () => {
		expect(median([100, 200, 300])).toBe(200);
		expect(median([10, 20, 30, 40, 50])).toBe(30);
	});

	test('handles negative values', () => {
		expect(median([-100, 0, 100])).toBe(0);
		expect(median([-50, -25, 0, 25, 50])).toBe(0);
	});

	test('handles unsorted input', () => {
		expect(median([300, 100, 200])).toBe(200);
		expect(median([50, 10, 30, 40, 20])).toBe(30);
	});

	test('handles all zeros', () => {
		expect(median([0, 0, 0])).toBe(0);
	});

	test('handles large values', () => {
		expect(median([1_000_000, 2_000_000, 3_000_000])).toBe(2_000_000);
	});
});

describe('mean', () => {
	test('returns 0 for empty array', () => {
		expect(mean([])).toBe(0);
	});

	test('returns the value for single element', () => {
		expect(mean([100])).toBe(100);
	});

	test('calculates average correctly', () => {
		expect(mean([100, 200])).toBe(150);
		expect(mean([10, 20, 30])).toBe(20);
	});

	test('rounds to nearest integer', () => {
		expect(mean([1, 2])).toBe(2); // 1.5 rounds to 2
		expect(mean([1, 2, 3, 4])).toBe(3); // 2.5 rounds to 3
	});

	test('handles negative values', () => {
		expect(mean([-100, 100])).toBe(0);
		expect(mean([-50, 0, 50])).toBe(0);
	});

	test('handles all zeros', () => {
		expect(mean([0, 0, 0])).toBe(0);
	});
});

describe('getTypicalMonthlyMinor', () => {
	test('returns 0 for empty array', () => {
		expect(getTypicalMonthlyMinor([])).toBe(0);
	});

	test('uses median when positive', () => {
		// median of [100, 200, 300] = 200
		expect(getTypicalMonthlyMinor([100, 200, 300])).toBe(200);
	});

	test('falls back to mean when median is 0', () => {
		// median of [-100, 0, 100] = 0, mean = 0
		expect(getTypicalMonthlyMinor([-100, 0, 100])).toBe(0);
	});

	test('falls back to mean when median is negative', () => {
		// median of [-300, -200, -100] = -200, mean = -200
		// Since median <= 0, uses mean
		expect(getTypicalMonthlyMinor([-300, -200, -100])).toBe(-200);
	});

	test('uses mean for sparse data with zeros', () => {
		// median of [0, 0, 300] = 0, mean = 100
		expect(getTypicalMonthlyMinor([0, 0, 300])).toBe(100);
	});
});
