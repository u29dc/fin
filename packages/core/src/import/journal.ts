import type { Database } from 'bun:sqlite';

import { mapCategoryToAccount } from '../db/categories';

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

function toPostedDate(postedAt: string): string {
	return postedAt.slice(0, 10);
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

type ProviderTxnPair = {
	provider_txn_id: string;
	account_id: string;
};

const PROVIDER_TXN_CHUNK_SIZE = 800;

function loadExistingProviderTxnPairs(db: Database, pairs: ProviderTxnPair[]): Set<string> {
	if (pairs.length === 0) {
		return new Set();
	}

	const providerTxnIds = Array.from(new Set(pairs.map((p) => p.provider_txn_id)));
	const accountIds = Array.from(new Set(pairs.map((p) => p.account_id)));

	if (providerTxnIds.length === 0 || accountIds.length === 0) {
		return new Set();
	}

	const existing = new Set<string>();
	const accountPlaceholders = accountIds.map(() => '?').join(', ');
	const accountParams = accountIds;

	for (let i = 0; i < providerTxnIds.length; i += PROVIDER_TXN_CHUNK_SIZE) {
		const chunk = providerTxnIds.slice(i, i + PROVIDER_TXN_CHUNK_SIZE);
		const placeholders = chunk.map(() => '?').join(', ');
		const rows = db
			.query<ProviderTxnPair, string[]>(
				`SELECT provider_txn_id, account_id
				FROM postings
				WHERE provider_txn_id IN (${placeholders})
					AND account_id IN (${accountPlaceholders})`,
			)
			.all(...chunk, ...accountParams);

		for (const row of rows) {
			existing.add(`${row.provider_txn_id}::${row.account_id}`);
		}
	}

	return existing;
}

type PreparedStatements = {
	insertJournal: ReturnType<Database['prepare']>;
	insertPosting: ReturnType<Database['prepare']>;
};

type InsertResult = {
	created: number;
	transferPairsCreated: number;
	errors: string[];
};

function createSavepointRunner(db: Database): (fn: () => void) => void {
	let savepointIndex = 0;
	return (fn: () => void) => {
		const name = `sp_${savepointIndex++}`;
		db.exec(`SAVEPOINT ${name}`);
		try {
			fn();
			db.exec(`RELEASE ${name}`);
		} catch (error) {
			db.exec(`ROLLBACK TO ${name}`);
			db.exec(`RELEASE ${name}`);
			throw error;
		}
	};
}

function createTransferEntry(pair: TransferPair, stmts: PreparedStatements): void {
	const journalId = generateId('je');
	const postedAt = pair.from.postedAt < pair.to.postedAt ? pair.from.postedAt : pair.to.postedAt;
	const postedDate = toPostedDate(postedAt);
	const description = pair.from.cleanDescription || pair.from.rawDescription || 'Transfer';

	stmts.insertJournal.run(journalId, postedAt, postedDate, 1, description, pair.from.rawDescription, pair.from.cleanDescription, pair.from.counterparty, pair.from.sourceFile);
	stmts.insertPosting.run(generateId('p'), journalId, pair.from.chartAccountId, pair.from.amountMinor, pair.from.currency, null, pair.from.providerTxnId, pair.from.balanceMinor);
	stmts.insertPosting.run(generateId('p'), journalId, pair.to.chartAccountId, pair.to.amountMinor, pair.to.currency, null, pair.to.providerTxnId, pair.to.balanceMinor);
}

function insertTransfers(transfers: TransferPair[], stmts: PreparedStatements, withSavepoint: (fn: () => void) => void): InsertResult {
	const result: InsertResult = { created: 0, transferPairsCreated: 0, errors: [] };

	for (const pair of transfers) {
		try {
			withSavepoint(() => createTransferEntry(pair, stmts));
			result.created++;
			result.transferPairsCreated++;
		} catch (error) {
			result.errors.push(`Transfer ${pair.from.id} <-> ${pair.to.id}: ${error instanceof Error ? error.message : 'Unknown error'}`);
		}
	}

	return result;
}

function createNonTransferEntry(txn: CanonicalTransaction, stmts: PreparedStatements): void {
	const journalId = generateId('je');
	const postedDate = toPostedDate(txn.postedAt);

	const isInflow = txn.amountMinor > 0;
	const counterAccountId = mapCategoryToAccount(txn.category, txn.cleanDescription || txn.rawDescription, isInflow);

	stmts.insertJournal.run(journalId, txn.postedAt, postedDate, 0, txn.cleanDescription || txn.rawDescription, txn.rawDescription, txn.cleanDescription, txn.counterparty, txn.sourceFile);
	stmts.insertPosting.run(generateId('p'), journalId, txn.chartAccountId, txn.amountMinor, txn.currency, null, txn.providerTxnId, txn.balanceMinor);
	stmts.insertPosting.run(generateId('p'), journalId, counterAccountId, -txn.amountMinor, txn.currency, null, null, null);
}

function insertNonTransfers(nonTransfers: CanonicalTransaction[], stmts: PreparedStatements, withSavepoint: (fn: () => void) => void): InsertResult {
	const result: InsertResult = { created: 0, transferPairsCreated: 0, errors: [] };

	for (const txn of nonTransfers) {
		try {
			withSavepoint(() => createNonTransferEntry(txn, stmts));
			result.created++;
		} catch (error) {
			result.errors.push(`Transaction ${txn.id}: ${error instanceof Error ? error.message : 'Unknown error'}`);
		}
	}

	return result;
}

function filterNewTransactions(db: Database, transactions: CanonicalTransaction[]): { newTransactions: CanonicalTransaction[]; duplicateCount: number } {
	const seenBatchTxnIds = new Set<string>();
	const newTransactions: CanonicalTransaction[] = [];
	const candidates: CanonicalTransaction[] = [];
	let duplicateCount = 0;

	for (const txn of transactions) {
		if (txn.providerTxnId) {
			const key = `${txn.providerTxnId}::${txn.chartAccountId}`;
			if (seenBatchTxnIds.has(key)) {
				duplicateCount++;
				continue;
			}
			seenBatchTxnIds.add(key);
			candidates.push(txn);
			continue;
		}

		newTransactions.push(txn);
	}

	const candidatePairs: ProviderTxnPair[] = candidates.map((txn) => ({
		provider_txn_id: txn.providerTxnId ?? '',
		account_id: txn.chartAccountId,
	}));
	const existingPairs = loadExistingProviderTxnPairs(db, candidatePairs);

	for (const txn of candidates) {
		const key = `${txn.providerTxnId ?? ''}::${txn.chartAccountId}`;
		if (existingPairs.has(key)) {
			duplicateCount++;
			continue;
		}
		newTransactions.push(txn);
	}

	return { newTransactions, duplicateCount };
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

	const { newTransactions, duplicateCount } = filterNewTransactions(db, transactions);
	result.uniqueTransactions = newTransactions.length;
	result.duplicateTransactions = duplicateCount;

	if (newTransactions.length === 0) {
		return result;
	}

	const stmts: PreparedStatements = {
		insertJournal: db.prepare(`
			INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, raw_description, clean_description, counterparty, source_file)
			VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
		`),
		insertPosting: db.prepare(`
			INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo, provider_txn_id, provider_balance_minor)
			VALUES (?, ?, ?, ?, ?, ?, ?, ?)
		`),
	};

	const { transfers, nonTransfers } = detectTransferPairsInBatch(newTransactions, opts);
	result.entriesAttempted = transfers.length + nonTransfers.length;

	db.transaction(() => {
		const withSavepoint = createSavepointRunner(db);
		const transferResult = insertTransfers(transfers, stmts, withSavepoint);
		const nonTransferResult = insertNonTransfers(nonTransfers, stmts, withSavepoint);

		result.journalEntriesCreated += transferResult.created + nonTransferResult.created;
		result.transferPairsCreated += transferResult.transferPairsCreated;
		result.errors.push(...transferResult.errors, ...nonTransferResult.errors);
	})();

	return result;
}
