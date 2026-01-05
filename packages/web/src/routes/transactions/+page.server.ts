import { getAllTransactions, type Transaction } from '@fin/core';
import { getAllGroupMetadata, getGroupIds } from '@fin/core/config';

import { db } from '$lib/server/db';

export function load() {
	const availableGroups = getGroupIds();
	const groupMetadataList = getAllGroupMetadata();
	const groupMetadata = Object.fromEntries(groupMetadataList.map((m) => [m.id, { label: m.label, icon: m.icon }]));

	// Fetch all transactions in a single batched query
	const transactions: Record<string, Transaction[]> = getAllTransactions(db, availableGroups);

	return { transactions, availableGroups, groupMetadata };
}
