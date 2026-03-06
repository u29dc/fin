import {
	createFinApiClient,
	loadShellState,
	type FinApiClient,
	type ShellState,
	type TransactionCounterpartyPosting,
	type TransactionDetailData,
	type TransactionListRow,
	type TransactionSortField,
	type ViewTransactionsData,
} from "$lib/server/api";
import {
	resolveGroup,
	resolveSort,
	resolveSortDirection,
	type ConnectionState,
	type GroupId,
	type GroupMeta,
} from "$lib/server/skeleton";

const PAGE_SIZE = 100;
const CURSOR_PARAM = "cursor";
const SEARCH_PARAM = "search";
const SELECTED_PARAM = "selected";

const SORT_FIELD_MAP = {
	postedAt: "posted_at",
	cleanDescription: "description",
	pairAccountId: "counterparty",
	amountMinor: "amount_minor",
} as const satisfies Record<TransactionsSortColumn, TransactionSortField>;

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
	count: number;
	totalCount: number;
	hasMore: boolean;
	nextCursorToken: string | null;
	pageSize: number;
	pageNumber: number;
	rangeStart: number;
	rangeEnd: number;
	cursorTrail: string[];
};

export type TransactionsPageData = {
	availableGroups: GroupId[];
	groupMetadata: Record<GroupId, GroupMeta>;
	connection: ConnectionState;
	initialGroup: GroupId;
	initialSort: TransactionsSortColumn;
	initialDir: TransactionsSortDirection;
	searchQuery: string;
	list: TransactionListState;
	selectedPostingId: string | null;
	selectedTransaction: TransactionDetail | null;
};

export async function loadTransactionsPageData(options: {
	url: URL;
	client?: FinApiClient;
}): Promise<TransactionsPageData> {
	const client = options.client ?? createFinApiClient();
	const shell = await loadShellState(client);
	const initialGroup = resolveGroup(options.url, shell.availableGroups);
	const initialSort = resolveSort(options.url);
	const initialDir = resolveSortDirection(options.url);
	const searchQuery = normalizeSearch(options.url.searchParams.get(SEARCH_PARAM));
	const requestedCursorTrail = normalizeCursorTrail(options.url.searchParams.getAll(CURSOR_PARAM));
	const base = createEmptyTransactionsPage(shell, initialGroup, initialSort, initialDir, searchQuery, requestedCursorTrail);

	if (!shell.config) {
		return base;
	}

	const { page, cursorTrail } = await loadTransactionPage(client, {
		group: initialGroup,
		sort: initialSort,
		direction: initialDir,
		searchQuery,
		cursorTrail: requestedCursorTrail,
	});

	if (!page) {
		return {
			...base,
			list: buildListState(null, cursorTrail),
		};
	}

	const items = page.items.map(mapTransactionListItem);
	let selectedPostingId = options.url.searchParams.get(SELECTED_PARAM);
	if (!selectedPostingId || !items.some((item) => item.postingId === selectedPostingId)) {
		selectedPostingId = items[0]?.postingId ?? null;
	}

	let selectedTransaction = await loadSelectedTransaction(client, selectedPostingId);
	if (!selectedTransaction && items.length > 0) {
		selectedPostingId = items[0]?.postingId ?? null;
		selectedTransaction = await loadSelectedTransaction(client, selectedPostingId);
	}

	return {
		...base,
		list: buildListState(page, cursorTrail),
		selectedPostingId,
		selectedTransaction,
	};
}

async function loadTransactionPage(
	client: FinApiClient,
	options: {
		group: string;
		sort: TransactionsSortColumn;
		direction: TransactionsSortDirection;
		searchQuery: string;
		cursorTrail: string[];
	},
): Promise<{ page: ViewTransactionsData | null; cursorTrail: string[] }> {
	const primaryResult = await safeFetch(() =>
		client.viewTransactions({
			group: options.group,
			search: options.searchQuery || undefined,
			limit: PAGE_SIZE,
			sortField: SORT_FIELD_MAP[options.sort],
			sortDirection: options.direction,
			after: options.cursorTrail.at(-1),
		}),
	);
	if (primaryResult || options.cursorTrail.length === 0) {
		return { page: primaryResult, cursorTrail: options.cursorTrail };
	}

	const fallbackResult = await safeFetch(() =>
		client.viewTransactions({
			group: options.group,
			search: options.searchQuery || undefined,
			limit: PAGE_SIZE,
			sortField: SORT_FIELD_MAP[options.sort],
			sortDirection: options.direction,
		}),
	);
	return { page: fallbackResult, cursorTrail: [] };
}

async function loadSelectedTransaction(
	client: FinApiClient,
	selectedPostingId: string | null,
): Promise<TransactionDetail | null> {
	if (!selectedPostingId) {
		return null;
	}
	const detail = await safeFetch(() => client.viewTransactionDetail(selectedPostingId));
	return detail ? mapTransactionDetail(detail) : null;
}

async function safeFetch<T>(fetcher: () => Promise<T>): Promise<T | null> {
	try {
		return await fetcher();
	} catch {
		return null;
	}
}

function createEmptyTransactionsPage(
	shell: ShellState,
	initialGroup: GroupId,
	initialSort: TransactionsSortColumn,
	initialDir: TransactionsSortDirection,
	searchQuery: string,
	cursorTrail: string[],
): TransactionsPageData {
	return {
		availableGroups: shell.availableGroups,
		groupMetadata: shell.groupMetadata,
		connection: shell.connection,
		initialGroup,
		initialSort,
		initialDir,
		searchQuery,
		list: buildListState(null, cursorTrail),
		selectedPostingId: null,
		selectedTransaction: null,
	};
}

function buildListState(page: ViewTransactionsData | null, cursorTrail: string[]): TransactionListState {
	const pageNumber = cursorTrail.length + 1;
	const count = page?.count ?? 0;
	const totalCount = page?.totalCount ?? 0;
	const rangeStart = totalCount === 0 ? 0 : cursorTrail.length * PAGE_SIZE + 1;
	const rangeEnd = totalCount === 0 ? 0 : rangeStart + count - 1;

	return {
		items: page?.items.map(mapTransactionListItem) ?? [],
		count,
		totalCount,
		hasMore: page?.hasMore ?? false,
		nextCursorToken: page?.nextCursorToken ?? null,
		pageSize: PAGE_SIZE,
		pageNumber,
		rangeStart,
		rangeEnd,
		cursorTrail,
	};
}

function mapTransactionListItem(item: TransactionListRow): TransactionListItem {
	return {
		postingId: item.posting_id,
		journalEntryId: item.journal_entry_id,
		chartAccountId: item.chart_account_id,
		pairAccountIds: item.pair_account_ids,
		postedAt: item.posted_at,
		postedDate: item.posted_date,
		amountMinor: item.amount_minor,
		currency: item.currency,
		rawDescription: item.raw_description,
		cleanDescription: item.clean_description,
		counterparty: item.counterparty,
	};
}

function mapTransactionDetail(detail: TransactionDetailData): TransactionDetail {
	return {
		postingId: detail.posting_id,
		journalEntryId: detail.journal_entry_id,
		chartAccountId: detail.chart_account_id,
		postedAt: detail.posted_at,
		postedDate: detail.posted_date,
		amountMinor: detail.amount_minor,
		currency: detail.currency,
		description: detail.description,
		rawDescription: detail.raw_description,
		cleanDescription: detail.clean_description,
		counterparty: detail.counterparty,
		sourceFile: detail.source_file,
		isTransfer: detail.is_transfer,
		pairPostings: detail.pair_postings.map(mapCounterpartyPosting),
	};
}

function mapCounterpartyPosting(posting: TransactionCounterpartyPosting): TransactionDetailPosting {
	return {
		postingId: posting.posting_id,
		accountId: posting.account_id,
		amountMinor: posting.amount_minor,
		currency: posting.currency,
		memo: posting.memo,
	};
}

function normalizeCursorTrail(values: string[]): string[] {
	return values.map((value) => value.trim()).filter(Boolean);
}

function normalizeSearch(value: string | null): string {
	return value?.trim() ?? "";
}
