import type { Database } from 'bun:sqlite';

import { mapCategoryToAccount } from '../db/category-mapping';

import type { CanonicalTransaction } from './transactions';

export type JournalEntryResult = {
	journalEntriesCreated: number;
	transferPairsCreated: number;
	errors: string[];
};

type TransferPair = {
	from: CanonicalTransaction;
	to: CanonicalTransaction;
};

function generateId(prefix: string): string {
	return `${prefix}_${crypto.randomUUID().replace(/-/g, '').slice(0, 16)}`;
}

function isValidTransferCandidate(txn: CanonicalTransaction, matched: Set<string>): boolean {
	if (matched.has(txn.id)) return false;
	// Only consider significant amounts (>= 1 GBP = 100 pence)
	if (Math.abs(txn.amountMinor) < 100) return false;
	return true;
}

function isMatchingTransfer(txn: CanonicalTransaction, candidate: CanonicalTransaction, matched: Set<string>): boolean {
	if (matched.has(candidate.id)) return false;
	if (txn.amountMinor + candidate.amountMinor !== 0) return false;
	if (txn.chartAccountId === candidate.chartAccountId) return false;

	const txnDate = new Date(txn.postedAt);
	const candidateDate = new Date(candidate.postedAt);
	const daysDiff = Math.abs(txnDate.getTime() - candidateDate.getTime()) / (1000 * 60 * 60 * 24);
	return daysDiff <= 2;
}

function findMatchingTransfer(txn: CanonicalTransaction, candidates: CanonicalTransaction[], startIndex: number, matched: Set<string>): CanonicalTransaction | null {
	for (let j = startIndex; j < candidates.length; j++) {
		const candidate = candidates[j];
		if (candidate && isMatchingTransfer(txn, candidate, matched)) {
			return candidate;
		}
	}
	return null;
}

function detectTransferPairsInBatch(transactions: CanonicalTransaction[]): {
	transfers: TransferPair[];
	nonTransfers: CanonicalTransaction[];
} {
	const transfers: TransferPair[] = [];
	const matched = new Set<string>();
	const sorted = [...transactions].sort((a, b) => a.postedAt.localeCompare(b.postedAt));

	for (let i = 0; i < sorted.length; i++) {
		const txn = sorted[i];
		if (!txn || !isValidTransferCandidate(txn, matched)) continue;

		const candidate = findMatchingTransfer(txn, sorted, i + 1, matched);
		if (candidate) {
			transfers.push({
				from: txn.amountMinor < 0 ? txn : candidate,
				to: txn.amountMinor > 0 ? txn : candidate,
			});
			matched.add(txn.id);
			matched.add(candidate.id);
		}
	}

	const nonTransfers = sorted.filter((t) => !matched.has(t.id));
	return { transfers, nonTransfers };
}

function tableExists(db: Database, tableName: string): boolean {
	const result = db.query<{ name: string }, [string]>(`SELECT name FROM sqlite_master WHERE type='table' AND name=?`).get(tableName);
	return result !== null;
}

function providerTxnIdExists(db: Database, providerTxnId: string): boolean {
	const result = db.query<{ count: number }, [string]>(`SELECT COUNT(*) as count FROM postings WHERE provider_txn_id = ?`).get(providerTxnId);
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

export function createJournalEntriesFromTransactions(db: Database, transactions: CanonicalTransaction[]): JournalEntryResult {
	const result: JournalEntryResult = {
		journalEntriesCreated: 0,
		transferPairsCreated: 0,
		errors: [],
	};

	if (!tableExists(db, 'journal_entries') || !tableExists(db, 'postings')) {
		return result;
	}

	const newTransactions = transactions.filter((txn) => {
		if (txn.providerTxnId && providerTxnIdExists(db, txn.providerTxnId)) {
			return false;
		}
		return true;
	});

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

	const { transfers, nonTransfers } = detectTransferPairsInBatch(newTransactions);

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
