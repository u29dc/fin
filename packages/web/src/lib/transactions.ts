export const TRANSACTION_LIST_LIMIT = 10_000;

export type TransactionsSortColumn = "postedAt" | "cleanDescription" | "pairAccountId" | "amountMinor";
export type TransactionsSortDirection = "asc" | "desc";

export type TransactionListItem = {
	postingId: string;
	journalEntryId: string;
	chartAccountId: string;
	pairAccountIds: string[];
	postedAt: string;
	postedDate: string;
	amountMinor: number;
	currency: string;
	rawDescription: string;
	cleanDescription: string;
	counterparty: string | null;
};

export type TransactionDetailPosting = {
	postingId: string;
	accountId: string;
	amountMinor: number;
	currency: string;
	memo: string | null;
};

export type TransactionDetail = {
	postingId: string;
	journalEntryId: string;
	chartAccountId: string;
	postedAt: string;
	postedDate: string;
	amountMinor: number;
	currency: string;
	description: string;
	rawDescription: string | null;
	cleanDescription: string | null;
	counterparty: string | null;
	sourceFile: string | null;
	isTransfer: boolean;
	pairPostings: TransactionDetailPosting[];
};

export type TransactionListState = {
	items: TransactionListItem[];
	loadedCount: number;
	totalCount: number;
	limit: number;
	truncated: boolean;
};

export function filterTransactionItems(items: readonly TransactionListItem[], searchQuery: string): TransactionListItem[] {
	const normalizedQuery = normalizeSearch(searchQuery).toLocaleLowerCase("en-GB");
	if (!normalizedQuery) {
		return [...items];
	}
	const searchTerms = normalizedQuery.split(/\s+/).filter(Boolean);
	if (searchTerms.length === 0) {
		return [...items];
	}

	return items.filter((item) => {
		const haystack = buildTransactionHaystack(item);
		return searchTerms.every((term) => haystack.includes(term));
	});
}

function buildTransactionHaystack(item: TransactionListItem): string {
	return [
		item.cleanDescription,
		item.rawDescription,
		item.counterparty ?? "",
		item.chartAccountId,
		item.pairAccountIds.join(" "),
		item.postedDate,
	]
		.join("\n")
		.toLocaleLowerCase("en-GB");
}

function normalizeSearch(value: string | null): string {
	return value?.trim() ?? "";
}
