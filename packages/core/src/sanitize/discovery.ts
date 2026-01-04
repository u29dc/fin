import type { Database } from 'bun:sqlite';

import { sanitizeDescription } from './matcher';
import type { DescriptionSummary, DiscoveryOptions, NameMappingConfig } from './types';

type DescriptionRow = {
	raw_description: string;
	occurrences: number;
	total_amount: number;
	chart_account_ids: string;
	first_seen: string;
	last_seen: string;
};

/**
 * Query unique raw_description values from journal_entries
 * to help build initial mapping rules.
 */
export function discoverDescriptions(db: Database, options: DiscoveryOptions = {}): DescriptionSummary[] {
	const { minOccurrences = 1, chartAccountId, limit = 500, sortBy = 'occurrences' } = options;

	const orderClause = {
		occurrences: 'occurrences DESC',
		amount: 'ABS(total_amount) DESC',
		recent: 'last_seen DESC',
	}[sortBy];

	// Query from journal_entries with postings join for account filtering and amounts
	let sql: string;
	let params: (string | number)[];

	if (chartAccountId) {
		sql = `
			SELECT
				je.raw_description,
				COUNT(DISTINCT je.id) as occurrences,
				SUM(p.amount_minor) as total_amount,
				GROUP_CONCAT(DISTINCT p.account_id) as chart_account_ids,
				MIN(je.posted_at) as first_seen,
				MAX(je.posted_at) as last_seen
			FROM journal_entries je
			JOIN postings p ON p.journal_entry_id = je.id
			WHERE je.raw_description IS NOT NULL
				AND p.account_id = ?
			GROUP BY je.raw_description
			HAVING occurrences >= ?
			ORDER BY ${orderClause}
			LIMIT ?
		`;
		params = [chartAccountId, minOccurrences, limit];
	} else {
		sql = `
			SELECT
				je.raw_description,
				COUNT(DISTINCT je.id) as occurrences,
				SUM(p.amount_minor) as total_amount,
				GROUP_CONCAT(DISTINCT p.account_id) as chart_account_ids,
				MIN(je.posted_at) as first_seen,
				MAX(je.posted_at) as last_seen
			FROM journal_entries je
			JOIN postings p ON p.journal_entry_id = je.id
			WHERE je.raw_description IS NOT NULL
			GROUP BY je.raw_description
			HAVING occurrences >= ?
			ORDER BY ${orderClause}
			LIMIT ?
		`;
		params = [minOccurrences, limit];
	}

	const stmt = db.prepare(sql);
	const rows = stmt.all(...params) as DescriptionRow[];

	return rows.map((row) => ({
		rawDescription: row.raw_description,
		occurrences: row.occurrences,
		totalAmountMinor: row.total_amount,
		chartAccountIds: row.chart_account_ids ? row.chart_account_ids.split(',') : [],
		firstSeen: row.first_seen,
		lastSeen: row.last_seen,
	}));
}

/**
 * Get descriptions that don't match any current rules.
 */
export function discoverUnmappedDescriptions(db: Database, config: NameMappingConfig, options: DiscoveryOptions = {}): DescriptionSummary[] {
	const all = discoverDescriptions(db, options);

	return all.filter((summary) => {
		const result = sanitizeDescription(summary.rawDescription, config);
		return result.matchedRule === null;
	});
}
