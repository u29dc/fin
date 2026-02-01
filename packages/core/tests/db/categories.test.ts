import { describe, expect, test } from 'bun:test';

import { mapCategoryToAccount, mapToExpenseAccount, mapToIncomeAccount } from '../../src/db/categories';

describe('mapToExpenseAccount', () => {
	test('maps explicit category to account', () => {
		expect(mapToExpenseAccount('groceries')).toBe('Expenses:Food:Groceries');
		expect(mapToExpenseAccount('rent')).toBe('Expenses:Housing:Rent');
		expect(mapToExpenseAccount('software')).toBe('Expenses:Business:Software');
	});

	test('maps category case-insensitively', () => {
		expect(mapToExpenseAccount('GROCERIES')).toBe('Expenses:Food:Groceries');
		expect(mapToExpenseAccount('Groceries')).toBe('Expenses:Food:Groceries');
	});

	test('returns Uncategorized when no category', () => {
		expect(mapToExpenseAccount(null)).toBe('Expenses:Uncategorized');
	});

	test('returns Uncategorized when category unknown', () => {
		expect(mapToExpenseAccount('unknown')).toBe('Expenses:Uncategorized');
	});

	test('maps granular bill categories', () => {
		expect(mapToExpenseAccount('energy')).toBe('Expenses:Bills:Energy');
		expect(mapToExpenseAccount('water')).toBe('Expenses:Bills:Water');
		expect(mapToExpenseAccount('counciltax')).toBe('Expenses:Bills:CouncilTax');
		expect(mapToExpenseAccount('internet')).toBe('Expenses:Bills:Internet');
		expect(mapToExpenseAccount('broadband')).toBe('Expenses:Bills:Internet');
	});

	test('maps bills/directdebit to DirectDebits catch-all', () => {
		expect(mapToExpenseAccount('bills')).toBe('Expenses:Bills:DirectDebits');
		expect(mapToExpenseAccount('directdebit')).toBe('Expenses:Bills:DirectDebits');
	});

	test('maps business expense categories', () => {
		expect(mapToExpenseAccount('tax')).toBe('Expenses:Taxes:VAT');
		expect(mapToExpenseAccount('hmrctax')).toBe('Expenses:Taxes:HMRC');
		expect(mapToExpenseAccount('insurance')).toBe('Expenses:Business:Insurance');
		expect(mapToExpenseAccount('professional')).toBe('Expenses:Business:Services');
	});

	test('maps personal expense categories', () => {
		expect(mapToExpenseAccount('fitness')).toBe('Expenses:Health:Fitness');
		expect(mapToExpenseAccount('health')).toBe('Expenses:Health:Medical');
		expect(mapToExpenseAccount('shopping')).toBe('Expenses:Shopping:Home');
		expect(mapToExpenseAccount('entertainment')).toBe('Expenses:Entertainment:Leisure');
	});
});

describe('mapToIncomeAccount', () => {
	test('maps explicit income category', () => {
		expect(mapToIncomeAccount('salary')).toBe('Income:Salary');
		expect(mapToIncomeAccount('dividends')).toBe('Income:Dividends');
		expect(mapToIncomeAccount('interest')).toBe('Income:Interest');
		expect(mapToIncomeAccount('refund')).toBe('Income:Refunds');
	});

	test('returns Income:Other when no category', () => {
		expect(mapToIncomeAccount(null)).toBe('Income:Other');
	});

	test('returns Income:Other when category unknown', () => {
		expect(mapToIncomeAccount('unknown')).toBe('Income:Other');
	});

	test('rejects non-income categories', () => {
		expect(mapToIncomeAccount('groceries')).toBe('Income:Other');
		expect(mapToIncomeAccount('rent')).toBe('Income:Other');
	});
});

describe('mapCategoryToAccount', () => {
	test('detects transfer category', () => {
		expect(mapCategoryToAccount('transfer', 'some description', false)).toBe('Equity:Transfers');
		expect(mapCategoryToAccount('transfer', 'some description', true)).toBe('Equity:Transfers');
	});

	test('detects transfer patterns in description', () => {
		expect(mapCategoryToAccount(null, 'Pot transfer', false)).toBe('Equity:Transfers');
		expect(mapCategoryToAccount(null, 'Round up savings', true)).toBe('Equity:Transfers');
		expect(mapCategoryToAccount(null, 'Topped up from account', false)).toBe('Equity:Transfers');
	});

	test('routes expense-category inflows to refunds', () => {
		expect(mapCategoryToAccount('groceries', 'Tesco refund', true)).toBe('Income:Refunds');
		expect(mapCategoryToAccount('shopping', 'Amazon return', true)).toBe('Income:Refunds');
		expect(mapCategoryToAccount('bills', 'Direct debit refund', true)).toBe('Income:Refunds');
	});

	test('routes inflows to income mapping', () => {
		expect(mapCategoryToAccount('salary', 'Company Ltd', true)).toBe('Income:Salary');
		expect(mapCategoryToAccount(null, 'Unknown payment', true)).toBe('Income:Other');
	});

	test('routes outflows to expense mapping', () => {
		expect(mapCategoryToAccount('groceries', 'Tesco', false)).toBe('Expenses:Food:Groceries');
		expect(mapCategoryToAccount(null, 'Some merchant', false)).toBe('Expenses:Uncategorized');
	});
});
