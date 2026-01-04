export const ACCOUNT_TYPES = ['asset', 'liability', 'equity', 'income', 'expense'] as const;

export type AccountType = (typeof ACCOUNT_TYPES)[number];

export function isAccountType(value: string): value is AccountType {
	return (ACCOUNT_TYPES as readonly string[]).includes(value);
}

export type ChartAccount = {
	id: string;
	name: string;
	accountType: AccountType;
	parentId: string | null;
	currency: string;
	isPlaceholder: boolean;
	active: boolean;
	createdAt: string;
};

export type JournalEntry = {
	id: string;
	postedAt: string;
	description: string;
	rawDescription: string | null;
	cleanDescription: string | null;
	counterparty: string | null;
	sourceFile: string | null;
	createdAt: string;
	updatedAt: string;
};

export type Posting = {
	id: string;
	journalEntryId: string;
	accountId: string;
	amountMinor: number;
	currency: string;
	memo: string | null;
	providerTxnId: string | null;
	providerBalanceMinor: number | null;
	createdAt: string;
};

export type JournalEntryWithPostings = JournalEntry & {
	postings: Posting[];
};

export type NewJournalEntry = {
	id: string;
	postedAt: string;
	description: string;
	rawDescription?: string | null;
	cleanDescription?: string | null;
	counterparty?: string | null;
	sourceFile?: string | null;
	postings: NewPosting[];
};

export type NewPosting = {
	id: string;
	accountId: string;
	amountMinor: number;
	currency?: string;
	memo?: string | null;
	providerTxnId?: string | null;
	providerBalanceMinor?: number | null;
};

export class JournalEntryValidationError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'JournalEntryValidationError';
	}
}

export function validateJournalEntry(entry: NewJournalEntry): void {
	if (entry.postings.length < 2) {
		throw new JournalEntryValidationError('Journal entry must have at least 2 postings');
	}

	const sum = entry.postings.reduce((acc, p) => acc + p.amountMinor, 0);
	if (sum !== 0) {
		throw new JournalEntryValidationError(`Journal entry does not balance: sum is ${sum}, expected 0`);
	}
}

export type BalanceSheet = {
	assets: number;
	liabilities: number;
	equity: number;
	income: number;
	expenses: number;
	netWorth: number;
	netIncome: number;
};

export type MonthlyCashflow = {
	month: string;
	incomeMinor: number;
	expenseMinor: number;
};

export type CategoryBreakdown = {
	accountId: string;
	categoryName: string;
	totalMinor: number;
	transactionCount: number;
};

export type ExpenseNode = {
	accountId: string;
	name: string;
	totalMinor: number;
	children: ExpenseNode[];
};
