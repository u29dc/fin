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
	software: 'Expenses:Business:Software',

	// Business expenses
	tax: 'Expenses:Taxes:VAT',
	government: 'Expenses:Taxes:VAT',
	insurance: 'Expenses:Business:Insurance',
	office: 'Expenses:Business:Equipment',
	vehicle: 'Expenses:Transport:Vehicle',

	// Personal expenses
	healthinsurance: 'Expenses:Health:Insurance',
	supplements: 'Expenses:Food:Supplements',
	health: 'Expenses:Health:Medical',
	shopping: 'Expenses:Shopping:Home',
	entertainment: 'Expenses:Entertainment:Leisure',

	// Unknown direct debits (catch-all for unidentified recurring payments)
	bills: 'Expenses:Bills:DirectDebits',
	directdebit: 'Expenses:Bills:DirectDebits',
};

export const DESCRIPTION_PATTERNS: Array<{ pattern: RegExp; accountId: string }> = [
	// Internal transfers (pots, savings, round-ups)
	{ pattern: /\b(pot|round.?up|savings|vault|flex|topped.?up)\b/i, accountId: 'Equity:Transfers' },

	// Groceries
	{ pattern: /\b(tesco|sainsbury'?s?|asda|morrisons|waitrose|aldi|lidl|co-op|marks.?spencer|m&s)\b/i, accountId: 'Expenses:Food:Groceries' },

	// Restaurants & delivery
	{ pattern: /\b(uber.?eats|deliveroo|just.?eat|dominos|pizza|mcdonalds|kfc|nandos|wagamama|pret)\b/i, accountId: 'Expenses:Food:Delivery' },
	{ pattern: /\b(restaurant|cafe|starbucks|costa|nero|coffee)\b/i, accountId: 'Expenses:Food:Coffee' },

	// Transport
	{ pattern: /\b(tfl|oyster|trainline|national.?rail|uber(?!.?eats)|bolt|lyft)\b/i, accountId: 'Expenses:Transport:PublicTransport' },
	{ pattern: /\b(shell|bp|esso|texaco|jet|petrol|diesel)\b/i, accountId: 'Expenses:Transport:Fuel' },
	{ pattern: /\b(ncp|parking|parkopedia)\b/i, accountId: 'Expenses:Transport:Parking' },

	// Subscriptions
	{ pattern: /\b(spotify|netflix|youtube|disney|amazon.?prime|apple.?tv|hbo|paramount)\b/i, accountId: 'Expenses:Entertainment:Subscriptions' },
	{ pattern: /\b(apple\.com\/bill|google.?play|app.?store)\b/i, accountId: 'Expenses:Entertainment:Subscriptions' },

	// Software & business
	{ pattern: /\b(aws|amazon.?web|digitalocean|linode|vultr|hetzner)\b/i, accountId: 'Expenses:Business:Software' },
	{ pattern: /\b(github|gitlab|bitbucket|vercel|netlify|cloudflare|heroku)\b/i, accountId: 'Expenses:Business:Software' },
	{ pattern: /\b(notion|figma|slack|zoom|microsoft|google.?workspace|dropbox)\b/i, accountId: 'Expenses:Business:Software' },
	{ pattern: /\b(openai|anthropic|claude)\b/i, accountId: 'Expenses:Business:Software' },

	// Utilities
	{ pattern: /\b(british.?gas|edf|octopus|bulb|ovo|eon|thames.?water|electric|gas.?bill)\b/i, accountId: 'Expenses:Housing:Utilities' },
	{ pattern: /\b(council.?tax|tv.?licence)\b/i, accountId: 'Expenses:Housing:Utilities' },
	{ pattern: /\b(virgin.?media|bt|sky|plusnet|ee|vodafone|three|o2)\b/i, accountId: 'Expenses:Housing:Utilities' },

	// Health & fitness
	{ pattern: /\b(pharmacy|boots|superdrug|lloyds.?pharmacy)\b/i, accountId: 'Expenses:Health:Pharmacy' },
	{ pattern: /\b(gym|puregym|david.?lloyd|nuffield|virgin.?active)\b/i, accountId: 'Expenses:Health:Fitness' },
	{ pattern: /\b(nhs|doctor|dentist|hospital|clinic)\b/i, accountId: 'Expenses:Health:Medical' },

	// Shopping
	{ pattern: /\b(amazon(?!.?web|.?prime)|ebay|etsy|aliexpress)\b/i, accountId: 'Expenses:Shopping:Home' },
	{ pattern: /\b(apple|currys|argos|john.?lewis|asos|zara|h&m|uniqlo|next)\b/i, accountId: 'Expenses:Shopping:Clothing' },
	{ pattern: /\b(ikea|habitat|made\.com|wayfair)\b/i, accountId: 'Expenses:Shopping:Home' },

	// Bank fees
	{ pattern: /\b(bank.?fee|account.?fee|monthly.?fee|overdraft|interest.?charge)\b/i, accountId: 'Expenses:Business:BankFees' },

	// Tax
	{ pattern: /\bHMRC.*VAT\b/i, accountId: 'Expenses:Taxes:VAT' },

	// Health insurance
	{ pattern: /\b(health.?insurance|private.?health|medical.?insurance)\b/i, accountId: 'Expenses:Health:Insurance' },

	// Income patterns
	{ pattern: /\b(salary|payroll|wages)\b/i, accountId: 'Income:Salary' },
	{ pattern: /\b(dividend|distribution)\b/i, accountId: 'Income:Dividends' },
	{ pattern: /\b(interest.?paid|savings.?interest)\b/i, accountId: 'Income:Interest' },
	{ pattern: /\b(refund|rebate|cashback)\b/i, accountId: 'Income:Refunds' },
];

export function mapToExpenseAccount(category: string | null, description: string): string {
	// First try explicit category
	if (category) {
		const lowerCategory = category.toLowerCase();
		const mapped = EXACT_CATEGORY_TO_ACCOUNT[lowerCategory];
		if (mapped) {
			return mapped;
		}
	}

	// Then try description patterns
	for (const { pattern, accountId } of DESCRIPTION_PATTERNS) {
		if (pattern.test(description)) {
			return accountId;
		}
	}

	// Default
	return 'Expenses:Uncategorized';
}

export function mapToIncomeAccount(category: string | null, description: string): string {
	// First try explicit category
	if (category) {
		const lowerCategory = category.toLowerCase();
		const mapped = EXACT_CATEGORY_TO_ACCOUNT[lowerCategory];
		if (mapped?.startsWith('Income:')) {
			return mapped;
		}
	}

	// Then try description patterns
	for (const { pattern, accountId } of DESCRIPTION_PATTERNS) {
		if (pattern.test(description) && accountId.startsWith('Income:')) {
			return accountId;
		}
	}

	// Default
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
	'software',
	'utilities',
	'health',
	'personal',
	'entertainment',
	'travel',
	'bills',
	'directdebit',
	'healthinsurance',
	'supplements',
	'insurance',
	'vehicle',
	'tax',
	'government',
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
		return mapToIncomeAccount(category, description);
	}
	return mapToExpenseAccount(category, description);
}
