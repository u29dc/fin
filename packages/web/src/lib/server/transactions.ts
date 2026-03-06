import {
	createFinApiClient,
	loadShellState,
	type FinApiClient,
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
import {
	TRANSACTION_LIST_LIMIT,
	type TransactionDetail,
	type TransactionDetailPosting,
	type TransactionListItem,
	type TransactionListState,
	type TransactionsSortColumn,
	type TransactionsSortDirection,
} from "$lib/transactions";

export {
	filterTransactionItems,
	TRANSACTION_LIST_LIMIT,
	type TransactionDetail,
	type TransactionDetailPosting,
	type TransactionListItem,
	type TransactionListState,
	type TransactionsSortColumn,
	type TransactionsSortDirection,
} from "$lib/transactions";

const SEARCH_PARAM = "search";
const SELECTED_PARAM = "selected";

const SORT_FIELD_MAP = {
	postedAt: "posted_at",
	cleanDescription: "description",
	pairAccountId: "counterparty",
	amountMinor: "amount_minor",
} as const satisfies Record<TransactionsSortColumn, TransactionSortField>;

export type TransactionsPageData = {
	availableGroups: GroupId[];
	groupMetadata: Record<GroupId, GroupMeta>;
	connection: ConnectionState;
	initialGroup: GroupId;
	initialSort: TransactionsSortColumn;
	initialDir: TransactionsSortDirection;
	searchQuery: string;
	selectedPostingId: string | null;
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

	return {
		availableGroups: shell.availableGroups,
		groupMetadata: shell.groupMetadata,
		connection: shell.connection,
		initialGroup,
		initialSort,
		initialDir,
		searchQuery: normalizeSearch(options.url.searchParams.get(SEARCH_PARAM)),
		selectedPostingId: normalizeSelectedPostingId(options.url.searchParams.get(SELECTED_PARAM)),
	};
}

export async function fetchTransactionsDataset(
	client: FinApiClient,
	options: {
		group: string;
		sort: TransactionsSortColumn;
		direction: TransactionsSortDirection;
	},
): Promise<TransactionListState> {
	const payload = await client.viewTransactions({
		group: options.group,
		limit: TRANSACTION_LIST_LIMIT,
		sortField: SORT_FIELD_MAP[options.sort],
		sortDirection: options.direction,
	});
	return buildListState(payload);
}

export async function fetchTransactionDetail(
	client: FinApiClient,
	postingId: string,
): Promise<TransactionDetail> {
	const payload = await client.viewTransactionDetail(postingId);
	return mapTransactionDetail(payload);
}

function buildListState(page: ViewTransactionsData): TransactionListState {
	const items = page.items.map(mapTransactionListItem);
	return {
		items,
		loadedCount: items.length,
		totalCount: page.totalCount,
		limit: TRANSACTION_LIST_LIMIT,
		truncated: page.totalCount > items.length,
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

function normalizeSearch(value: string | null): string {
	return value?.trim() ?? "";
}

function normalizeSelectedPostingId(value: string | null): string | null {
	const normalized = value?.trim();
	return normalized ? normalized : null;
}
