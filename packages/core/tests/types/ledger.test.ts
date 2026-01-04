import { describe, expect, test } from 'bun:test';

import { JournalEntryValidationError, type NewJournalEntry, type NewPosting, validateJournalEntry } from '../../src/types/ledger';

function makePosting(accountId: string, amountMinor: number): NewPosting {
	return {
		id: `p_${crypto.randomUUID().slice(0, 8)}`,
		accountId,
		amountMinor,
	};
}

function makeEntry(postings: NewPosting[]): NewJournalEntry {
	return {
		id: `je_${crypto.randomUUID().slice(0, 8)}`,
		postedAt: '2024-01-01T10:00:00Z',
		description: 'Test entry',
		postings,
	};
}

describe('validateJournalEntry', () => {
	test('accepts balanced entry with two postings', () => {
		const entry = makeEntry([makePosting('Assets:Personal:Monzo', -1000), makePosting('Expenses:Food:Groceries', 1000)]);
		expect(() => validateJournalEntry(entry)).not.toThrow();
	});

	test('accepts balanced entry with three postings', () => {
		const entry = makeEntry([makePosting('Assets:Personal:Monzo', -1000), makePosting('Expenses:Food:Groceries', 700), makePosting('Expenses:Food:Coffee', 300)]);
		expect(() => validateJournalEntry(entry)).not.toThrow();
	});

	test('rejects unbalanced entry', () => {
		const entry = makeEntry([makePosting('Assets:Personal:Monzo', -1000), makePosting('Expenses:Food:Groceries', 999)]);
		expect(() => validateJournalEntry(entry)).toThrow(JournalEntryValidationError);
		expect(() => validateJournalEntry(entry)).toThrow('does not balance');
	});

	test('rejects single posting', () => {
		const entry = makeEntry([makePosting('Assets:Personal:Monzo', -1000)]);
		expect(() => validateJournalEntry(entry)).toThrow(JournalEntryValidationError);
		expect(() => validateJournalEntry(entry)).toThrow('at least 2 postings');
	});

	test('rejects empty postings', () => {
		const entry = makeEntry([]);
		expect(() => validateJournalEntry(entry)).toThrow(JournalEntryValidationError);
		expect(() => validateJournalEntry(entry)).toThrow('at least 2 postings');
	});

	test('accepts transfer entry between asset accounts', () => {
		const entry = makeEntry([makePosting('Assets:Personal:Monzo', -100000), makePosting('Assets:Personal:Savings', 100000)]);
		expect(() => validateJournalEntry(entry)).not.toThrow();
	});

	test('accepts income entry', () => {
		const entry = makeEntry([makePosting('Assets:Business:Wise', 500000), makePosting('Income:Salary', -500000)]);
		expect(() => validateJournalEntry(entry)).not.toThrow();
	});

	test('handles zero-sum with positive and negative', () => {
		const entry = makeEntry([makePosting('Assets:Personal:Monzo', 0), makePosting('Expenses:Uncategorized', 0)]);
		expect(() => validateJournalEntry(entry)).not.toThrow();
	});
});
