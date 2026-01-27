import type { Database } from 'bun:sqlite';

import { mapCategoryToAccount } from '../db/category-mapping';

import type { CanonicalTransaction } from './transactions';

export type JournalEntryResult = {
	totalTransactions: number;
	uniqueTransactions: number;
	duplicateTransactions: number;
	entriesAttempted: number;
	journalEntriesCreated: number;
	transferPairsCreated: number;
	errors: string[];
};

export type TransferDetectionOptions = {
	maxTransferDaysDifference?: number;
	enableCategoryFallback?: boolean;
};

const DEFAULT_TRANSFER_OPTIONS: Required<TransferDetectionOptions> = {
	maxTransferDaysDifference: 5,
	enableCategoryFallback: true,
};

type TransferPair = {
	from: CanonicalTransaction;
	to: CanonicalTransaction;
};

function generateId(prefix: string): string {
	return `${prefix}_${crypto.randomUUID().replace(/-/g, '').slice(0, 16)}`;
}

const MS_PER_DAY = 1000 * 60 * 60 * 24;

function daysBetween(date1: string, date2: string): number {
	return Math.abs(new Date(date1).getTime() - new Date(date2).getTime()) / MS_PER_DAY;
}

function createTransferPair(txn: CanonicalTransaction, candidate: CanonicalTransaction): TransferPair {
	return {
		from: txn.amountMinor < 0 ? txn : candidate,
		to: txn.amountMinor > 0 ? txn : candidate,
	};
}

/**
 * Build an index of transactions by amount for O(1) lookup of opposite amounts.
 */
function buildAmountIndex(transactions: CanonicalTransaction[]): Map<number, CanonicalTransaction[]> {
	const index = new Map<number, CanonicalTransaction[]>();
	for (const txn of transactions) {
		if (Math.abs(txn.amountMinor) < 100) continue;
		const bucket = index.get(txn.amountMinor);
		if (bucket) {
			bucket.push(txn);
		} else {
			index.set(txn.amountMinor, [txn]);
		}
	}
	return index;
}

/**
 * Check if candidate is a valid transfer match for txn.
 */
function isValidTransferMatch(txn: CanonicalTransaction, candidate: CanonicalTransaction, matched: Set<string>, maxDays: number | null): boolean {
	if (matched.has(candidate.id)) return false;
	if (txn.chartAccountId === candidate.chartAccountId) return false;
	if (maxDays !== null) {
		const days = daysBetween(txn.postedAt, candidate.postedAt);
		if (!Number.isFinite(days) || days > maxDays) return false;
	}
	return true;
}

/**
 * Find and record a matching transfer from candidates.
 */
function findAndRecordMatch(txn: CanonicalTransaction, candidates: CanonicalTransaction[], matched: Set<string>, transfers: TransferPair[], maxDays: number | null): boolean {
	for (const candidate of candidates) {
		if (isValidTransferMatch(txn, candidate, matched, maxDays)) {
			transfers.push(createTransferPair(txn, candidate));
			matched.add(txn.id);
			matched.add(candidate.id);
			return true;
		}
	}
	return false;
}

/**
 * Match transfers using amount index to reduce scanning for typical distributions.
 * Average case improves from O(n^2) to O(n*k) where k = bucket size.
 */
function matchTransfersByTimeWindowWithIndex(sorted: CanonicalTransaction[], matched: Set<string>, transfers: TransferPair[], maxDays: number, amountIndex: Map<number, CanonicalTransaction[]>): void {
	for (const txn of sorted) {
		if (matched.has(txn.id) || Math.abs(txn.amountMinor) < 100) continue;
		const candidates = amountIndex.get(-txn.amountMinor);
		if (candidates) {
			findAndRecordMatch(txn, candidates, matched, transfers, maxDays);
		}
	}
}

/**
 * Fallback matching for transactions explicitly categorized as transfers.
 */
function matchTransfersByCategoryFallback(sorted: CanonicalTransaction[], matched: Set<string>, transfers: TransferPair[], amountIndex: Map<number, CanonicalTransaction[]>): void {
	for (const txn of sorted) {
		if (matched.has(txn.id)) continue;
		if (Math.abs(txn.amountMinor) < 100) continue;
		if (txn.category?.toLowerCase() !== 'transfer') continue;

		const candidates = amountIndex.get(-txn.amountMinor);
		if (candidates) {
			findAndRecordMatch(txn, candidates, matched, transfers, null);
		}
	}
}

function detectTransferPairsInBatch(
	transactions: CanonicalTransaction[],
	options: Required<TransferDetectionOptions>,
): {
	transfers: TransferPair[];
	nonTransfers: CanonicalTransaction[];
} {
	const transfers: TransferPair[] = [];
	const matched = new Set<string>();
	const sorted = [...transactions].sort((a, b) => a.postedAt.localeCompare(b.postedAt));
	const amountIndex = buildAmountIndex(sorted);

	matchTransfersByTimeWindowWithIndex(sorted, matched, transfers, options.maxTransferDaysDifference, amountIndex);
	if (options.enableCategoryFallback) {
		matchTransfersByCategoryFallback(sorted, matched, transfers, amountIndex);
	}

	const nonTransfers = sorted.filter((t) => !matched.has(t.id));
	return { transfers, nonTransfers };
}

function tableExists(db: Database, tableName: string): boolean {
	const result = db.query<{ name: string }, [string]>(`SELECT name FROM sqlite_master WHERE type='table' AND name=?`).get(tableName);
	return result !== null;
}

function providerTxnIdExists(db: Database, providerTxnId: string, accountId: string): boolean {
	const result = db.query<{ count: number }, [string, string]>(`SELECT COUNT(*) as count FROM postings WHERE provider_txn_id = ? AND account_id = ?`).get(providerTxnId, accountId);
	return (result?.count ?? 0) > 0;
}

type PreparedStatements = {
	insertJournal: ReturnType<Database['prepare']>;
	insertPosting: ReturnType<Database['prepare']>;
};

function createTransferEntry(pair: TransferPair, stmts: PreparedStatements, result: JournalEntryResult): void {
	const journalId = generateId('je');
	const postedAt = pair.from.postedAt < pair.to.postedAt ? pair.from.postedAt : pair.to.postedAt;
	const description = pair.from.cleanDescription || pair.from.rawDescription || 'Transfer';

	try {
		stmts.insertJournal.run(journalId, postedAt, description, pair.from.rawDescription, pair.from.cleanDescription, pair.from.counterparty, pair.from.sourceFile);
		stmts.insertPosting.run(generateId('p'), journalId, pair.from.chartAccountId, pair.from.amountMinor, pair.from.currency, null, pair.from.providerTxnId, pair.from.balanceMinor);
		stmts.insertPosting.run(generateId('p'), journalId, pair.to.chartAccountId, pair.to.amountMinor, pair.to.currency, null, pair.to.providerTxnId, pair.to.balanceMinor);
		result.journalEntriesCreated++;
		result.transferPairsCreated++;
	} catch (error) {
		result.errors.push(`Transfer ${pair.from.id} <-> ${pair.to.id}: ${error instanceof Error ? error.message : 'Unknown error'}`);
	}
}

function createNonTransferEntry(txn: CanonicalTransaction, stmts: PreparedStatements, result: JournalEntryResult): void {
	const journalId = generateId('je');

	try {
		const isInflow = txn.amountMinor > 0;
		const counterAccountId = mapCategoryToAccount(txn.category, txn.cleanDescription || txn.rawDescription, isInflow);

		stmts.insertJournal.run(journalId, txn.postedAt, txn.cleanDescription || txn.rawDescription, txn.rawDescription, txn.cleanDescription, txn.counterparty, txn.sourceFile);
		stmts.insertPosting.run(generateId('p'), journalId, txn.chartAccountId, txn.amountMinor, txn.currency, null, txn.providerTxnId, txn.balanceMinor);
		stmts.insertPosting.run(generateId('p'), journalId, counterAccountId, -txn.amountMinor, txn.currency, null, null, null);
		result.journalEntriesCreated++;
	} catch (error) {
		result.errors.push(`Transaction ${txn.id}: ${error instanceof Error ? error.message : 'Unknown error'}`);
	}
}

export function createJournalEntriesFromTransactions(db: Database, transactions: CanonicalTransaction[], options?: TransferDetectionOptions): JournalEntryResult {
	const opts: Required<TransferDetectionOptions> = { ...DEFAULT_TRANSFER_OPTIONS, ...options };
	const totalTransactions = transactions.length;
	const result: JournalEntryResult = {
		totalTransactions,
		uniqueTransactions: 0,
		duplicateTransactions: 0,
		entriesAttempted: 0,
		journalEntriesCreated: 0,
		transferPairsCreated: 0,
		errors: [],
	};

	if (!tableExists(db, 'journal_entries') || !tableExists(db, 'postings')) {
		return result;
	}

	const newTransactions = transactions.filter((txn) => {
		if (txn.providerTxnId && providerTxnIdExists(db, txn.providerTxnId, txn.chartAccountId)) {
			return false;
		}
		return true;
	});
	result.uniqueTransactions = newTransactions.length;
	result.duplicateTransactions = totalTransactions - newTransactions.length;

	if (newTransactions.length === 0) {
		return result;
	}

	const stmts: PreparedStatements = {
		insertJournal: db.prepare(`
			INSERT INTO journal_entries (id, posted_at, description, raw_description, clean_description, counterparty, source_file)
			VALUES (?, ?, ?, ?, ?, ?, ?)
		`),
		insertPosting: db.prepare(`
			INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo, provider_txn_id, provider_balance_minor)
			VALUES (?, ?, ?, ?, ?, ?, ?, ?)
		`),
	};

	const { transfers, nonTransfers } = detectTransferPairsInBatch(newTransactions, opts);
	result.entriesAttempted = transfers.length + nonTransfers.length;

	db.transaction(() => {
		for (const pair of transfers) {
			createTransferEntry(pair, stmts, result);
		}
		for (const txn of nonTransfers) {
			createNonTransferEntry(txn, stmts, result);
		}
	})();

	return result;
}
