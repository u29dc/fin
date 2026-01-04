import { describe, expect, test } from 'bun:test';

import { getUnmappedDescriptions, sanitizeBatch, sanitizeDescription } from '../../src/sanitize/matcher';
import type { NameMappingConfig } from '../../src/sanitize/types';

const testConfig: NameMappingConfig = {
	rules: [
		{ patterns: ['AMZN', 'AMAZON'], target: 'Amazon Shopping', category: 'Shopping' },
		{ patterns: ['DBOX'], target: 'Dropbox', category: 'Subscriptions' },
		{ patterns: ['TESCO'], target: 'Tesco', category: 'Groceries' },
		{ patterns: ['NO_CATEGORY'], target: 'No Category' },
		{ patterns: ['^DD\\s+'], target: 'Direct Debit', matchMode: 'regex' },
		{ patterns: ['EXACT_MATCH'], target: 'Exact Match', matchMode: 'exact' },
		{ patterns: ['CASE_SENSITIVE'], target: 'Case Sensitive', caseSensitive: true },
	],
	warnOnUnmapped: true,
	fallbackToRaw: true,
};

describe('sanitizeDescription', () => {
	test('matches contains pattern case-insensitively', () => {
		const result = sanitizeDescription('AMZN MKTP UK*123', testConfig);
		expect(result.cleanDescription).toBe('Amazon Shopping');
		expect(result.matchedRule).not.toBeNull();
		expect(result.wasModified).toBe(true);
	});

	test('matches lowercase input against uppercase pattern', () => {
		const result = sanitizeDescription('amzn marketplace', testConfig);
		expect(result.cleanDescription).toBe('Amazon Shopping');
	});

	test('matches regex pattern', () => {
		const result = sanitizeDescription('DD BRITISH GAS', testConfig);
		expect(result.cleanDescription).toBe('Direct Debit');
	});

	test('returns raw when no match', () => {
		const result = sanitizeDescription('Unknown Merchant', testConfig);
		expect(result.cleanDescription).toBe('Unknown Merchant');
		expect(result.matchedRule).toBeNull();
		expect(result.wasModified).toBe(false);
	});

	test('wasModified is false when already equals target', () => {
		const result = sanitizeDescription('Amazon Shopping', testConfig);
		expect(result.cleanDescription).toBe('Amazon Shopping');
		expect(result.matchedRule).not.toBeNull();
		expect(result.wasModified).toBe(false);
	});

	test('exact match mode requires exact match', () => {
		const result = sanitizeDescription('EXACT_MATCH', testConfig);
		expect(result.cleanDescription).toBe('Exact Match');

		const noMatch = sanitizeDescription('SOME_EXACT_MATCH_HERE', testConfig);
		expect(noMatch.matchedRule).toBeNull();
	});

	test('case sensitive mode respects case', () => {
		const result = sanitizeDescription('CASE_SENSITIVE', testConfig);
		expect(result.cleanDescription).toBe('Case Sensitive');

		const noMatch = sanitizeDescription('case_sensitive', testConfig);
		expect(noMatch.matchedRule).toBeNull();
	});

	test('trims whitespace from input', () => {
		const result = sanitizeDescription('  AMZN MKTP  ', testConfig);
		expect(result.cleanDescription).toBe('Amazon Shopping');
	});

	test('returns category when rule has category', () => {
		const result = sanitizeDescription('TESCO STORES', testConfig);
		expect(result.cleanDescription).toBe('Tesco');
		expect(result.category).toBe('Groceries');
	});

	test('returns null category when rule has no category', () => {
		const result = sanitizeDescription('NO_CATEGORY TEST', testConfig);
		expect(result.cleanDescription).toBe('No Category');
		expect(result.category).toBeNull();
	});

	test('returns null category when no match', () => {
		const result = sanitizeDescription('Unknown Merchant', testConfig);
		expect(result.category).toBeNull();
	});
});

describe('sanitizeBatch', () => {
	test('deduplicates results', () => {
		const inputs = ['AMZN 1', 'AMZN 2', 'AMZN 1', 'DBOX'];
		const results = sanitizeBatch(inputs, testConfig);
		expect(results.size).toBe(3);
	});

	test('maps each unique input to result', () => {
		const inputs = ['AMZN', 'DBOX', 'Unknown'];
		const results = sanitizeBatch(inputs, testConfig);

		expect(results.get('AMZN')?.cleanDescription).toBe('Amazon Shopping');
		expect(results.get('DBOX')?.cleanDescription).toBe('Dropbox');
		expect(results.get('Unknown')?.cleanDescription).toBe('Unknown');
	});
});

describe('getUnmappedDescriptions', () => {
	test('returns only unmatched descriptions', () => {
		const inputs = ['AMZN', 'Unknown', 'DBOX', 'Mystery'];
		const unmapped = getUnmappedDescriptions(inputs, testConfig);
		expect(unmapped).toEqual(['Unknown', 'Mystery']);
	});

	test('returns empty array when all matched', () => {
		const inputs = ['AMZN', 'DBOX'];
		const unmapped = getUnmappedDescriptions(inputs, testConfig);
		expect(unmapped).toEqual([]);
	});

	test('deduplicates inputs', () => {
		const inputs = ['Unknown', 'Unknown', 'Mystery'];
		const unmapped = getUnmappedDescriptions(inputs, testConfig);
		expect(unmapped).toEqual(['Unknown', 'Mystery']);
	});
});
