import type { Database } from 'bun:sqlite';

import { type AssetAccountId, getGroupChartAccounts } from '../types/chart-account-ids';

// ============================================
// TRANSACTION TYPES
// ============================================

type JournalEntryRow = {
	id: string;
	chart_account_id: string;
	pair_account_id: string;
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
			p2.account_id as pair_account_id,
			je.posted_at,
			p.amount_minor,
			p.currency,
			COALESCE(je.raw_description, je.description) as raw_description,
			COALESCE(je.clean_description, je.description) as clean_description,
			je.counterparty
		FROM journal_entries je
		JOIN postings p ON p.journal_entry_id = je.id
		JOIN postings p2 ON p2.journal_entry_id = je.id AND p2.id != p.id
		${whereClause}
		ORDER BY je.posted_at DESC
		LIMIT ?;
	`;
	params.push(limit);

	const rows = db.query<JournalEntryRow, (string | number)[]>(sql).all(...params);

	return rows.map((row: JournalEntryRow) => ({
		id: row.id,
		chartAccountId: row.chart_account_id,
		pairAccountId: row.pair_account_id,
		postedAt: row.posted_at,
		amountMinor: row.amount_minor,
		currency: row.currency,
		rawDescription: row.raw_description,
		cleanDescription: row.clean_description,
		counterparty: row.counterparty,
	}));
}

// ============================================
// BATCHED TRANSACTION QUERIES
// ============================================

export type AllTransactionsOptions = {
	limit?: number;
};

/**
 * Fetch transactions for all groups in a single query.
 * Returns transactions grouped by group ID.
 */
export function getAllTransactions(db: Database, groupIds: string[], options: AllTransactionsOptions = {}): Record<string, Transaction[]> {
	const { limit = 10_000 } = options;

	// Collect all chart account IDs from all groups
	const groupAccounts = getGroupChartAccounts();
	const allChartAccountIds = groupIds.flatMap((gid) => groupAccounts[gid] ?? []);

	if (allChartAccountIds.length === 0) {
		const result: Record<string, Transaction[]> = {};
		for (const gid of groupIds) {
			result[gid] = [];
		}
		return result;
	}

	// Single query for all chart accounts with self-join for pair account
	const placeholders = allChartAccountIds.map(() => '?').join(', ');
	const sql = `
		SELECT
			je.id,
			p.account_id as chart_account_id,
			p2.account_id as pair_account_id,
			je.posted_at,
			p.amount_minor,
			p.currency,
			COALESCE(je.raw_description, je.description) as raw_description,
			COALESCE(je.clean_description, je.description) as clean_description,
			je.counterparty
		FROM journal_entries je
		JOIN postings p ON p.journal_entry_id = je.id
		JOIN postings p2 ON p2.journal_entry_id = je.id AND p2.id != p.id
		WHERE p.account_id IN (${placeholders})
		ORDER BY je.posted_at DESC
		LIMIT ?;
	`;

	const rows = db.query<JournalEntryRow, (string | number)[]>(sql).all(...allChartAccountIds, limit);

	// Build reverse lookup: chart account ID -> group ID
	const chartAccountToGroup = new Map<string, string>();
	for (const gid of groupIds) {
		const accountIds = groupAccounts[gid];
		if (accountIds) {
			for (const chartAccountId of accountIds) {
				chartAccountToGroup.set(chartAccountId, gid);
			}
		}
	}

	// Initialize result with empty arrays
	const result: Record<string, Transaction[]> = {};
	for (const gid of groupIds) {
		result[gid] = [];
	}

	// Group transactions by their group ID
	for (const row of rows) {
		const groupId = chartAccountToGroup.get(row.chart_account_id);
		const groupResult = groupId ? result[groupId] : undefined;
		if (groupResult) {
			groupResult.push({
				id: row.id,
				chartAccountId: row.chart_account_id,
				pairAccountId: row.pair_account_id,
				postedAt: row.posted_at,
				amountMinor: row.amount_minor,
				currency: row.currency,
				rawDescription: row.raw_description,
				cleanDescription: row.clean_description,
				counterparty: row.counterparty,
			});
		}
	}

	return result;
}
