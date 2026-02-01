import type { Database } from 'bun:sqlite';

import { type AssetAccountId, getGroupChartAccounts } from '../types/accounts';

// ============================================
// TRANSACTION TYPES
// ============================================

type JournalEntryRow = {
	id: string;
	chart_account_id: string;
	pair_account_id: string | null;
	posted_at: string;
	amount_minor: number;
	currency: string;
	raw_description: string;
	clean_description: string;
	counterparty: string | null;
};

export type Transaction = {
	id: string;
	chartAccountId: string;
	pairAccountId: string;
	postedAt: string;
	amountMinor: number;
	currency: string;
	rawDescription: string;
	cleanDescription: string;
	counterparty: string | null;
};

export type TransactionQueryOptions = {
	chartAccountId?: AssetAccountId;
	chartAccountIds?: string[];
	from?: string;
	to?: string;
	limit?: number;
};

// ============================================
// TRANSACTION QUERIES
// ============================================

export function getTransactions(db: Database, options: TransactionQueryOptions = {}): Transaction[] {
	const { chartAccountId, chartAccountIds, from, to, limit = 10_000 } = options;

	const where: string[] = [];
	const params: (string | number)[] = [];

	// Support multiple chart account IDs (takes precedence over single chartAccountId)
	if (chartAccountIds && chartAccountIds.length > 0) {
		const placeholders = chartAccountIds.map(() => '?').join(', ');
		where.push(`p.account_id IN (${placeholders})`);
		params.push(...chartAccountIds);
	} else if (chartAccountId) {
		where.push('p.account_id = ?');
		params.push(chartAccountId);
	}
	if (from) {
		where.push('je.posted_at >= ?');
		params.push(`${from}T00:00:00`);
	}
	if (to) {
		where.push('je.posted_at <= ?');
		params.push(`${to}T23:59:59.999`);
	}

	const whereClause = where.length > 0 ? `WHERE ${where.join(' AND ')}` : '';

	const sql = `
		SELECT
			je.id,
			p.account_id as chart_account_id,
			GROUP_CONCAT(DISTINCT p2.account_id) as pair_account_id,
			je.posted_at,
			p.amount_minor,
			p.currency,
			COALESCE(je.raw_description, je.description) as raw_description,
			COALESCE(je.clean_description, je.description) as clean_description,
			je.counterparty
		FROM journal_entries je
		JOIN postings p ON p.journal_entry_id = je.id
		LEFT JOIN postings p2 ON p2.journal_entry_id = je.id AND p2.id != p.id
		${whereClause}
		GROUP BY je.id, p.id
		ORDER BY je.posted_at DESC
		LIMIT ?;
	`;
	params.push(limit);

	const rows = db.query<JournalEntryRow, (string | number)[]>(sql).all(...params);

	return rows.map(mapRowToTransaction);
}

// ============================================
// BATCHED TRANSACTION QUERIES
// ============================================

export type AllTransactionsOptions = {
	limit?: number;
};

function mapRowToTransaction(row: JournalEntryRow): Transaction {
	return {
		id: row.id,
		chartAccountId: row.chart_account_id,
		pairAccountId: row.pair_account_id ?? '',
		postedAt: row.posted_at,
		amountMinor: row.amount_minor,
		currency: row.currency,
		rawDescription: row.raw_description,
		cleanDescription: row.clean_description,
		counterparty: row.counterparty,
	};
}

function initEmptyGroupResult(groupIds: string[]): Record<string, Transaction[]> {
	const result: Record<string, Transaction[]> = {};
	for (const gid of groupIds) {
		result[gid] = [];
	}
	return result;
}

/**
 * Fetch transactions for all groups in a single query.
 * Returns transactions grouped by group ID.
 */
export function getAllTransactions(db: Database, groupIds: string[], options: AllTransactionsOptions = {}): Record<string, Transaction[]> {
	const { limit = 10_000 } = options;
	const groupAccounts = getGroupChartAccounts();
	const result = initEmptyGroupResult(groupIds);

	for (const gid of groupIds) {
		const chartAccountIds = groupAccounts[gid] ?? [];
		if (chartAccountIds.length === 0) {
			continue;
		}
		result[gid] = getTransactions(db, { chartAccountIds, limit });
	}

	return result;
}

export type TransactionCounts = Record<string, number>;

export function getAllTransactionCounts(db: Database, groupIds: string[]): TransactionCounts {
	const groupAccounts = getGroupChartAccounts();
	const allChartAccountIds = groupIds.flatMap((gid) => groupAccounts[gid] ?? []);

	const result: TransactionCounts = {};
	for (const gid of groupIds) {
		result[gid] = 0;
	}

	if (allChartAccountIds.length === 0) {
		return result;
	}

	const placeholders = allChartAccountIds.map(() => '?').join(', ');
	type CountRow = { chart_account_id: string; count: number };
	const sql = `
		SELECT p.account_id as chart_account_id, COUNT(*) as count
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		WHERE p.account_id IN (${placeholders})
		GROUP BY p.account_id
	`;
	const rows = db.query<CountRow, (string | number)[]>(sql).all(...allChartAccountIds);

	const chartAccountToGroup = new Map<string, string>();
	for (const gid of groupIds) {
		const accountIds = groupAccounts[gid];
		if (accountIds) {
			for (const chartAccountId of accountIds) {
				chartAccountToGroup.set(chartAccountId, gid);
			}
		}
	}

	for (const row of rows) {
		const groupId = chartAccountToGroup.get(row.chart_account_id);
		if (!groupId) {
			continue;
		}
		result[groupId] = (result[groupId] ?? 0) + row.count;
	}

	return result;
}
