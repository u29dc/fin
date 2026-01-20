import { getAllTransactionCounts, getAllTransactions, type Transaction } from '@fin/core';
import { getAllGroupMetadata, getGroupIds } from '@fin/core/config';

import { db } from '$lib/server/db';

const VALID_SORT_COLUMNS = ['postedAt', 'cleanDescription', 'pairAccountId', 'amountMinor'] as const;
const VALID_SORT_DIRECTIONS = ['asc', 'desc'] as const;

type SortColumn = (typeof VALID_SORT_COLUMNS)[number];
type SortDirection = (typeof VALID_SORT_DIRECTIONS)[number];

export function load({ url }: { url: URL }) {
	const availableGroups = getGroupIds();
	const groupMetadataList = getAllGroupMetadata();
	const groupMetadata = Object.fromEntries(groupMetadataList.map((m) => [m.id, { label: m.label, icon: m.icon }]));

	// Validate URL params
	const groupParam = url.searchParams.get('group');
	const sortParam = url.searchParams.get('sort') as SortColumn | null;
	const dirParam = url.searchParams.get('dir') as SortDirection | null;

	const initialGroup = groupParam && availableGroups.includes(groupParam) ? groupParam : null;
	const initialSort = sortParam && VALID_SORT_COLUMNS.includes(sortParam) ? sortParam : null;
	const initialDir = dirParam && VALID_SORT_DIRECTIONS.includes(dirParam) ? dirParam : null;

	const transactionLimit = 10_000;

	// Fetch all transactions in a single batched query
	const transactions: Record<string, Transaction[]> = getAllTransactions(db, availableGroups, { limit: transactionLimit });
	const transactionCounts = getAllTransactionCounts(db, availableGroups);

	return {
		transactions,
		transactionCounts,
		transactionLimit,
		availableGroups,
		groupMetadata,
		initialGroup,
		initialSort,
		initialDir,
	};
}
