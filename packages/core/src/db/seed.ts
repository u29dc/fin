import { getConfig, isConfigInitialized } from '../config/index';
import type { AccountType } from '../types/ledger';

export type ChartAccountSeed = {
	id: string;
	name: string;
	type: AccountType;
	parent: string | null;
	placeholder: boolean;
};

/**
 * Generate asset account seeds from TOML config.
 * Creates placeholder parents and leaf accounts dynamically.
 */
function getAssetAccountSeeds(): ChartAccountSeed[] {
	if (!isConfigInitialized()) {
		// Fallback when config not loaded (shouldn't happen in normal operation)
		return [];
	}

	const accounts = getConfig().accounts.filter((a) => a.type === 'asset');
	const seeds: ChartAccountSeed[] = [];
	const addedParents = new Set<string>();

	// Root Assets placeholder
	seeds.push({ id: 'Assets', name: 'Assets', type: 'asset', parent: null, placeholder: true });
	addedParents.add('Assets');

	for (const account of accounts) {
		// Parse the account ID to get parent hierarchy
		// e.g., "Assets:Business:Wise" -> parent "Assets:Business"
		const parts = account.id.split(':');

		// Add intermediate placeholder parents
		for (let i = 1; i < parts.length; i++) {
			const parentId = parts.slice(0, i + 1).join(':');
			const grandParentId = i === 1 ? 'Assets' : parts.slice(0, i).join(':');

			if (!addedParents.has(parentId) && parentId !== account.id) {
				seeds.push({
					id: parentId,
					name: parts[i] ?? parentId,
					type: 'asset',
					parent: grandParentId,
					placeholder: true,
				});
				addedParents.add(parentId);
			}
		}

		// Add the leaf account
		const parentParts = parts.slice(0, -1);
		const parentId = parentParts.length > 0 ? parentParts.join(':') : null;
		const accountName = account.label ?? parts[parts.length - 1] ?? account.id;

		seeds.push({
			id: account.id,
			name: accountName,
			type: 'asset',
			parent: parentId,
			placeholder: false,
		});
	}

	return seeds;
}

// Static accounts that don't change based on config
const STATIC_SEEDS: ChartAccountSeed[] = [
	// Root accounts (non-asset placeholders)
	{ id: 'Liabilities', name: 'Liabilities', type: 'liability', parent: null, placeholder: true },
	{ id: 'Liabilities:Business', name: 'Business', type: 'liability', parent: 'Liabilities', placeholder: true },
	{ id: 'Liabilities:Business:CorpTaxPayable', name: 'Corp Tax Payable', type: 'liability', parent: 'Liabilities:Business', placeholder: false },
	{ id: 'Liabilities:Business:VATPayable', name: 'VAT Payable', type: 'liability', parent: 'Liabilities:Business', placeholder: false },
	{ id: 'Equity', name: 'Equity', type: 'equity', parent: null, placeholder: true },
	{ id: 'Income', name: 'Income', type: 'income', parent: null, placeholder: true },
	{ id: 'Expenses', name: 'Expenses', type: 'expense', parent: null, placeholder: true },

	// Equity
	{ id: 'Equity:OpeningBalances', name: 'Opening Balances', type: 'equity', parent: 'Equity', placeholder: false },
	{ id: 'Equity:RetainedEarnings', name: 'Retained Earnings', type: 'equity', parent: 'Equity', placeholder: false },
	{ id: 'Equity:Transfers', name: 'Internal Transfers', type: 'equity', parent: 'Equity', placeholder: false },

	// Income hierarchy
	{ id: 'Income:Salary', name: 'Salary', type: 'income', parent: 'Income', placeholder: false },
	{ id: 'Income:Dividends', name: 'Dividends', type: 'income', parent: 'Income', placeholder: false },
	{ id: 'Income:Interest', name: 'Interest', type: 'income', parent: 'Income', placeholder: false },
	{ id: 'Income:Refunds', name: 'Refunds', type: 'income', parent: 'Income', placeholder: false },
	{ id: 'Income:Other', name: 'Other', type: 'income', parent: 'Income', placeholder: false },

	// Expense hierarchy
	{ id: 'Expenses:Food', name: 'Food', type: 'expense', parent: 'Expenses', placeholder: true },
	{ id: 'Expenses:Food:Groceries', name: 'Groceries', type: 'expense', parent: 'Expenses:Food', placeholder: false },
	{ id: 'Expenses:Food:Restaurants', name: 'Restaurants', type: 'expense', parent: 'Expenses:Food', placeholder: false },
	{ id: 'Expenses:Food:Coffee', name: 'Coffee', type: 'expense', parent: 'Expenses:Food', placeholder: false },
	{ id: 'Expenses:Food:Delivery', name: 'Delivery', type: 'expense', parent: 'Expenses:Food', placeholder: false },
	{ id: 'Expenses:Food:Supplements', name: 'Supplements', type: 'expense', parent: 'Expenses:Food', placeholder: false },

	{ id: 'Expenses:Housing', name: 'Housing', type: 'expense', parent: 'Expenses', placeholder: true },
	{ id: 'Expenses:Housing:Rent', name: 'Rent', type: 'expense', parent: 'Expenses:Housing', placeholder: false },
	{ id: 'Expenses:Housing:Utilities', name: 'Utilities', type: 'expense', parent: 'Expenses:Housing', placeholder: false },
	{ id: 'Expenses:Housing:Insurance', name: 'Insurance', type: 'expense', parent: 'Expenses:Housing', placeholder: false },
	{ id: 'Expenses:Housing:Maintenance', name: 'Maintenance', type: 'expense', parent: 'Expenses:Housing', placeholder: false },

	{ id: 'Expenses:Transport', name: 'Transport', type: 'expense', parent: 'Expenses', placeholder: true },
	{ id: 'Expenses:Transport:PublicTransport', name: 'Public Transport', type: 'expense', parent: 'Expenses:Transport', placeholder: false },
	{ id: 'Expenses:Transport:Fuel', name: 'Fuel', type: 'expense', parent: 'Expenses:Transport', placeholder: false },
	{ id: 'Expenses:Transport:Parking', name: 'Parking', type: 'expense', parent: 'Expenses:Transport', placeholder: false },
	{ id: 'Expenses:Transport:Maintenance', name: 'Maintenance', type: 'expense', parent: 'Expenses:Transport', placeholder: false },
	{ id: 'Expenses:Transport:Taxi', name: 'Taxi', type: 'expense', parent: 'Expenses:Transport', placeholder: false },
	{ id: 'Expenses:Transport:Vehicle', name: 'Vehicle', type: 'expense', parent: 'Expenses:Transport', placeholder: false },

	{ id: 'Expenses:Business', name: 'Business', type: 'expense', parent: 'Expenses', placeholder: true },
	{ id: 'Expenses:Business:Software', name: 'Software', type: 'expense', parent: 'Expenses:Business', placeholder: false },
	{ id: 'Expenses:Business:Equipment', name: 'Equipment', type: 'expense', parent: 'Expenses:Business', placeholder: false },
	{ id: 'Expenses:Business:Services', name: 'Services', type: 'expense', parent: 'Expenses:Business', placeholder: false },
	{ id: 'Expenses:Business:BankFees', name: 'Bank Fees', type: 'expense', parent: 'Expenses:Business', placeholder: false },
	{ id: 'Expenses:Business:Legal', name: 'Legal', type: 'expense', parent: 'Expenses:Business', placeholder: false },
	{ id: 'Expenses:Business:Accounting', name: 'Accounting', type: 'expense', parent: 'Expenses:Business', placeholder: false },
	{ id: 'Expenses:Business:Insurance', name: 'Insurance', type: 'expense', parent: 'Expenses:Business', placeholder: false },

	{ id: 'Expenses:Entertainment', name: 'Entertainment', type: 'expense', parent: 'Expenses', placeholder: true },
	{ id: 'Expenses:Entertainment:Subscriptions', name: 'Subscriptions', type: 'expense', parent: 'Expenses:Entertainment', placeholder: false },
	{ id: 'Expenses:Entertainment:Leisure', name: 'Leisure', type: 'expense', parent: 'Expenses:Entertainment', placeholder: false },
	{ id: 'Expenses:Entertainment:Gaming', name: 'Gaming', type: 'expense', parent: 'Expenses:Entertainment', placeholder: false },

	{ id: 'Expenses:Health', name: 'Health', type: 'expense', parent: 'Expenses', placeholder: true },
	{ id: 'Expenses:Health:Medical', name: 'Medical', type: 'expense', parent: 'Expenses:Health', placeholder: false },
	{ id: 'Expenses:Health:Pharmacy', name: 'Pharmacy', type: 'expense', parent: 'Expenses:Health', placeholder: false },
	{ id: 'Expenses:Health:Fitness', name: 'Fitness', type: 'expense', parent: 'Expenses:Health', placeholder: false },
	{ id: 'Expenses:Health:Insurance', name: 'Insurance', type: 'expense', parent: 'Expenses:Health', placeholder: false },

	{ id: 'Expenses:Shopping', name: 'Shopping', type: 'expense', parent: 'Expenses', placeholder: true },
	{ id: 'Expenses:Shopping:Clothing', name: 'Clothing', type: 'expense', parent: 'Expenses:Shopping', placeholder: false },
	{ id: 'Expenses:Shopping:Electronics', name: 'Electronics', type: 'expense', parent: 'Expenses:Shopping', placeholder: false },
	{ id: 'Expenses:Shopping:Home', name: 'Home', type: 'expense', parent: 'Expenses:Shopping', placeholder: false },

	{ id: 'Expenses:Personal', name: 'Personal', type: 'expense', parent: 'Expenses', placeholder: true },
	{ id: 'Expenses:Personal:Gifts', name: 'Gifts', type: 'expense', parent: 'Expenses:Personal', placeholder: false },
	{ id: 'Expenses:Personal:Education', name: 'Education', type: 'expense', parent: 'Expenses:Personal', placeholder: false },
	{ id: 'Expenses:Personal:Charity', name: 'Charity', type: 'expense', parent: 'Expenses:Personal', placeholder: false },

	{ id: 'Expenses:Taxes', name: 'Taxes', type: 'expense', parent: 'Expenses', placeholder: true },
	{ id: 'Expenses:Taxes:IncomeTax', name: 'Income Tax', type: 'expense', parent: 'Expenses:Taxes', placeholder: false },
	{ id: 'Expenses:Taxes:NationalInsurance', name: 'National Insurance', type: 'expense', parent: 'Expenses:Taxes', placeholder: false },
	{ id: 'Expenses:Taxes:VAT', name: 'VAT', type: 'expense', parent: 'Expenses:Taxes', placeholder: false },

	{ id: 'Expenses:Bills', name: 'Bills', type: 'expense', parent: 'Expenses', placeholder: true },
	{ id: 'Expenses:Bills:Energy', name: 'Energy', type: 'expense', parent: 'Expenses:Bills', placeholder: false },
	{ id: 'Expenses:Bills:Water', name: 'Water', type: 'expense', parent: 'Expenses:Bills', placeholder: false },
	{ id: 'Expenses:Bills:CouncilTax', name: 'Council Tax', type: 'expense', parent: 'Expenses:Bills', placeholder: false },
	{ id: 'Expenses:Bills:Internet', name: 'Internet', type: 'expense', parent: 'Expenses:Bills', placeholder: false },
	{ id: 'Expenses:Bills:Insurance', name: 'Insurance', type: 'expense', parent: 'Expenses:Bills', placeholder: false },
	{ id: 'Expenses:Bills:DirectDebits', name: 'Direct Debits', type: 'expense', parent: 'Expenses:Bills', placeholder: false },

	{ id: 'Expenses:Uncategorized', name: 'Uncategorized', type: 'expense', parent: 'Expenses', placeholder: false },
];

/**
 * Get the complete chart of accounts seed data.
 * Combines dynamic asset accounts from config with static expense/income/equity accounts.
 */
export function getChartOfAccountsSeeds(): ChartAccountSeed[] {
	const assetSeeds = getAssetAccountSeeds();
	return [...assetSeeds, ...STATIC_SEEDS];
}
