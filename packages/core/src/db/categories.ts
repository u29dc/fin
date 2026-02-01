// Category mapping for converting legacy transactions to double-entry journal entries

export const EXACT_CATEGORY_TO_ACCOUNT: Record<string, string> = {
	// Internal transfers (not income or expense)
	transfer: 'Equity:Transfers',

	// Income categories
	salary: 'Income:Salary',
	dividends: 'Income:Dividends',
	interest: 'Income:Interest',
	refund: 'Income:Refunds',

	// Expense categories
	food: 'Expenses:Food:Groceries',
	groceries: 'Expenses:Food:Groceries',
	restaurants: 'Expenses:Food:Restaurants',
	transport: 'Expenses:Transport:PublicTransport',
	utilities: 'Expenses:Housing:Utilities',
	rent: 'Expenses:Housing:Rent',
	subscriptions: 'Expenses:Entertainment:Subscriptions',
	businesssubs: 'Expenses:Business:Subscriptions',
	software: 'Expenses:Business:Software',

	// Business expenses
	tax: 'Expenses:Taxes:VAT',
	government: 'Expenses:Taxes:VAT',
	hmrctax: 'Expenses:Taxes:HMRC',
	insurance: 'Expenses:Business:Insurance',
	office: 'Expenses:Business:Equipment',
	vehicle: 'Expenses:Transport:Vehicle',
	professional: 'Expenses:Business:Services',
	contractors: 'Expenses:Business:Contractors',

	// Personal expenses
	fitness: 'Expenses:Health:Fitness',
	healthinsurance: 'Expenses:Health:Insurance',
	supplements: 'Expenses:Food:Supplements',
	health: 'Expenses:Health:Medical',
	shopping: 'Expenses:Shopping:Home',
	entertainment: 'Expenses:Entertainment:Leisure',
	travel: 'Expenses:Transport:Travel',
	charity: 'Expenses:Shopping:Charity',
	cafe: 'Expenses:Food:Restaurants',
	parking: 'Expenses:Transport:Parking',
	fuel: 'Expenses:Transport:Vehicle',

	// Granular bill categories
	energy: 'Expenses:Bills:Energy',
	water: 'Expenses:Bills:Water',
	counciltax: 'Expenses:Bills:CouncilTax',
	internet: 'Expenses:Bills:Internet',
	broadband: 'Expenses:Bills:Internet',

	// Unknown direct debits (catch-all for unidentified recurring payments)
	bills: 'Expenses:Bills:DirectDebits',
	directdebit: 'Expenses:Bills:DirectDebits',

	// Card verification holds (zero-value, not real expenses)
	cardcheck: 'Equity:Transfers',
	'card check': 'Equity:Transfers',

	// Services
	services: 'Expenses:Business:Services',

	// Investments (transfers to investment accounts)
	investment: 'Equity:Investments',

	// Unclear/miscellaneous
	unclear: 'Expenses:Other',

	// Catch-all for unmapped transactions
	other: 'Expenses:Other',
};

export function mapToExpenseAccount(category: string | null): string {
	if (category) {
		const mapped = EXACT_CATEGORY_TO_ACCOUNT[category.toLowerCase()];
		if (mapped) return mapped;
	}
	return 'Expenses:Uncategorized';
}

export function mapToIncomeAccount(category: string | null): string {
	if (category) {
		const mapped = EXACT_CATEGORY_TO_ACCOUNT[category.toLowerCase()];
		if (mapped?.startsWith('Income:')) return mapped;
	}
	return 'Income:Other';
}

// Pattern for detecting internal transfers that shouldn't be income or expense
const TRANSFER_PATTERNS = /\b(pot|round.?up|savings|vault|flex|topped.?up|money.?transfer|internal|transfer)\b/i;

// Expense categories that become refunds when they appear as inflows
// NOTE: "other" is intentionally excluded - it catches unclassified income like client payments
const EXPENSE_CATEGORIES = new Set([
	'groceries',
	'shopping',
	'food',
	'transport',
	'subscriptions',
	'businesssubs',
	'software',
	'utilities',
	'health',
	'personal',
	'entertainment',
	'travel',
	'bills',
	'directdebit',
	'energy',
	'water',
	'counciltax',
	'internet',
	'broadband',
	'fitness',
	'healthinsurance',
	'supplements',
	'insurance',
	'vehicle',
	'tax',
	'government',
	'hmrctax',
	'professional',
	'contractors',
	'charity',
	'cafe',
	'parking',
	'fuel',
	'other',
]);

export function mapCategoryToAccount(category: string | null, description: string, isInflow: boolean): string {
	// Check for transfer category FIRST (before income/expense routing)
	if (category?.toLowerCase() === 'transfer') {
		return 'Equity:Transfers';
	}

	// Check description for internal transfer patterns
	if (TRANSFER_PATTERNS.test(description)) {
		return 'Equity:Transfers';
	}

	// If inflow with expense-type category, it's a refund
	if (isInflow && category && EXPENSE_CATEGORIES.has(category.toLowerCase())) {
		return 'Income:Refunds';
	}

	if (isInflow) {
		return mapToIncomeAccount(category);
	}
	return mapToExpenseAccount(category);
}
