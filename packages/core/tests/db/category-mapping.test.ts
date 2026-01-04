import { describe, expect, test } from 'bun:test';

import { mapCategoryToAccount, mapToExpenseAccount, mapToIncomeAccount } from '../../src/db/category-mapping';

describe('mapToExpenseAccount', () => {
	test('maps explicit category to account', () => {
		expect(mapToExpenseAccount('groceries', 'some description')).toBe('Expenses:Food:Groceries');
		expect(mapToExpenseAccount('rent', 'monthly payment')).toBe('Expenses:Housing:Rent');
		expect(mapToExpenseAccount('software', 'subscription')).toBe('Expenses:Business:Software');
	});

	test('maps category case-insensitively', () => {
		expect(mapToExpenseAccount('GROCERIES', 'test')).toBe('Expenses:Food:Groceries');
		expect(mapToExpenseAccount('Groceries', 'test')).toBe('Expenses:Food:Groceries');
	});

	test('falls back to description pattern when no category', () => {
		expect(mapToExpenseAccount(null, 'Tesco Express')).toBe('Expenses:Food:Groceries');
		expect(mapToExpenseAccount(null, 'SAINSBURYS')).toBe('Expenses:Food:Groceries');
		expect(mapToExpenseAccount(null, 'Uber Eats order')).toBe('Expenses:Food:Delivery');
	});

	test('falls back to description pattern when category unknown', () => {
		expect(mapToExpenseAccount('unknown-category', 'TFL.GOV.UK')).toBe('Expenses:Transport:PublicTransport');
		expect(mapToExpenseAccount('misc', 'Shell Petrol Station')).toBe('Expenses:Transport:Fuel');
	});

	test('returns Uncategorized when no match', () => {
		expect(mapToExpenseAccount(null, 'random merchant xyz')).toBe('Expenses:Uncategorized');
		expect(mapToExpenseAccount('unknown', 'another merchant')).toBe('Expenses:Uncategorized');
	});

	test('matches software subscriptions', () => {
		expect(mapToExpenseAccount(null, 'GITHUB.COM')).toBe('Expenses:Business:Software');
		expect(mapToExpenseAccount(null, 'VERCEL INC')).toBe('Expenses:Business:Software');
		expect(mapToExpenseAccount(null, 'AWS EMEA')).toBe('Expenses:Business:Software');
		expect(mapToExpenseAccount(null, 'OPENAI API')).toBe('Expenses:Business:Software');
	});

	test('matches entertainment subscriptions', () => {
		expect(mapToExpenseAccount(null, 'SPOTIFY')).toBe('Expenses:Entertainment:Subscriptions');
		expect(mapToExpenseAccount(null, 'NETFLIX.COM')).toBe('Expenses:Entertainment:Subscriptions');
		expect(mapToExpenseAccount(null, 'APPLE.COM/BILL')).toBe('Expenses:Entertainment:Subscriptions');
	});

	test('matches utility providers', () => {
		expect(mapToExpenseAccount(null, 'OCTOPUS ENERGY')).toBe('Expenses:Housing:Utilities');
		expect(mapToExpenseAccount(null, 'BRITISH GAS')).toBe('Expenses:Housing:Utilities');
		expect(mapToExpenseAccount(null, 'COUNCIL TAX')).toBe('Expenses:Housing:Utilities');
	});

	test('matches health expenses', () => {
		expect(mapToExpenseAccount(null, 'BOOTS PHARMACY')).toBe('Expenses:Health:Pharmacy');
		expect(mapToExpenseAccount(null, 'PUREGYM')).toBe('Expenses:Health:Fitness');
	});
});

describe('mapToIncomeAccount', () => {
	test('maps explicit income category', () => {
		expect(mapToIncomeAccount('salary', 'Company Ltd')).toBe('Income:Salary');
		expect(mapToIncomeAccount('dividends', 'Dividend payment')).toBe('Income:Dividends');
		expect(mapToIncomeAccount('interest', 'Savings interest')).toBe('Income:Interest');
	});

	test('falls back to description pattern', () => {
		expect(mapToIncomeAccount(null, 'SALARY PAYMENT')).toBe('Income:Salary');
		expect(mapToIncomeAccount(null, 'DIVIDEND FROM XYZ')).toBe('Income:Dividends');
		expect(mapToIncomeAccount(null, 'REFUND FROM AMAZON')).toBe('Income:Refunds');
	});

	test('returns Income:Other when no match', () => {
		expect(mapToIncomeAccount(null, 'some random income')).toBe('Income:Other');
		expect(mapToIncomeAccount('unknown', 'payment received')).toBe('Income:Other');
	});
});

describe('mapCategoryToAccount', () => {
	test('uses expense mapping for outflows', () => {
		expect(mapCategoryToAccount('groceries', 'Tesco', false)).toBe('Expenses:Food:Groceries');
		expect(mapCategoryToAccount(null, 'SHELL', false)).toBe('Expenses:Transport:Fuel');
	});

	test('uses income mapping for inflows', () => {
		expect(mapCategoryToAccount('salary', 'Company Ltd', true)).toBe('Income:Salary');
		expect(mapCategoryToAccount(null, 'REFUND', true)).toBe('Income:Refunds');
	});
});
