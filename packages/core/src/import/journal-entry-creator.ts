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

function detectTransferPairsInBatch(transactions: CanonicalTransaction[]): {
	transfers: TransferPair[];
	nonTransfers: CanonicalTransaction[];
} {
	const transfers: TransferPair[] = [];
	const matched = new Set<string>();

	// Sort by date for efficient matching
	const sorted = [...transactions].sort((a, b) => a.postedAt.localeCompare(b.postedAt));

	for (let i = 0; i < sorted.length; i++) {
		const txn = sorted[i];
		if (!txn || matched.has(txn.id)) continue;

		// Only consider significant amounts (>= 1 GBP = 100 pence)
		// Lowered from 500 to catch more round-ups and small transfers
		if (Math.abs(txn.amountMinor) < 100) continue;

		// Look for matching opposite transaction within 2 days
		for (let j = i + 1; j < sorted.length; j++) {
			const candidate = sorted[j];
			if (!candidate || matched.has(candidate.id)) continue;

			// Must be opposite amounts
			if (txn.amountMinor + candidate.amountMinor !== 0) continue;

			// Must be different accounts
			if (txn.chartAccountId === candidate.chartAccountId) continue;

			// Must be within 2 days
			const txnDate = new Date(txn.postedAt);
			const candidateDate = new Date(candidate.postedAt);
			const daysDiff = Math.abs(txnDate.getTime() - candidateDate.getTime()) / (1000 * 60 * 60 * 24);
			if (daysDiff > 2) continue;

			// Match found
			transfers.push({
				from: txn.amountMinor < 0 ? txn : candidate,
				to: txn.amountMinor > 0 ? txn : candidate,
			});
			matched.add(txn.id);
			matched.add(candidate.id);
			break;
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

export function createJournalEntriesFromTransactions(db: Database, transactions: CanonicalTransaction[]): JournalEntryResult {
	const result: JournalEntryResult = {
		journalEntriesCreated: 0,
		transferPairsCreated: 0,
		errors: [],
	};

	// Check if the ledger tables exist
	if (!tableExists(db, 'journal_entries') || !tableExists(db, 'postings')) {
		// Ledger tables don't exist yet, skip journal entry creation
		return result;
	}

	// Filter out transactions that already have journal entries (by provider_txn_id)
	const newTransactions = transactions.filter((txn) => {
		if (txn.providerTxnId && providerTxnIdExists(db, txn.providerTxnId)) {
			return false;
		}
		return true;
	});

	if (newTransactions.length === 0) {
		return result;
	}

	// Prepare statements
	const insertJournalStmt = db.prepare(`
		INSERT INTO journal_entries (id, posted_at, description, raw_description, clean_description, counterparty, source_file)
		VALUES (?, ?, ?, ?, ?, ?, ?)
	`);

	const insertPostingStmt = db.prepare(`
		INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo, provider_txn_id, provider_balance_minor)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?)
	`);

	// Detect transfers and create entries
	const { transfers, nonTransfers } = detectTransferPairsInBatch(newTransactions);

	db.transaction(() => {
		// Create journal entries for transfers
		for (const pair of transfers) {
			const journalId = generateId('je');
			const postedAt = pair.from.postedAt < pair.to.postedAt ? pair.from.postedAt : pair.to.postedAt;

			try {
				insertJournalStmt.run(journalId, postedAt, 'Transfer', 'Transfer', 'Transfer', null, pair.from.sourceFile);

				insertPostingStmt.run(generateId('p'), journalId, pair.from.chartAccountId, pair.from.amountMinor, pair.from.currency, null, pair.from.providerTxnId, pair.from.balanceMinor);

				insertPostingStmt.run(generateId('p'), journalId, pair.to.chartAccountId, pair.to.amountMinor, pair.to.currency, null, pair.to.providerTxnId, pair.to.balanceMinor);

				result.journalEntriesCreated++;
				result.transferPairsCreated++;
			} catch (error) {
				result.errors.push(`Transfer ${pair.from.id} <-> ${pair.to.id}: ${error instanceof Error ? error.message : 'Unknown error'}`);
			}
		}

		// Create journal entries for non-transfers
		for (const txn of nonTransfers) {
			const journalId = generateId('je');

			try {
				const isInflow = txn.amountMinor > 0;
				const counterAccountId = mapCategoryToAccount(txn.category, txn.cleanDescription || txn.rawDescription, isInflow);

				insertJournalStmt.run(journalId, txn.postedAt, txn.cleanDescription || txn.rawDescription, txn.rawDescription, txn.cleanDescription, txn.counterparty, txn.sourceFile);

				// Asset posting
				insertPostingStmt.run(generateId('p'), journalId, txn.chartAccountId, txn.amountMinor, txn.currency, null, txn.providerTxnId, txn.balanceMinor);

				// Counter posting (opposite sign to balance)
				insertPostingStmt.run(generateId('p'), journalId, counterAccountId, -txn.amountMinor, txn.currency, null, null, null);

				result.journalEntriesCreated++;
			} catch (error) {
				result.errors.push(`Transaction ${txn.id}: ${error instanceof Error ? error.message : 'Unknown error'}`);
			}
		}
	})();

	return result;
}
