/**
 * `edit transaction` -- Edit a journal entry's description and/or expense category.
 *
 * Supports atomic updates with auto-creation of chart accounts when the target
 * account doesn't exist yet. Follows the same envelope contract as void.ts.
 */

import type { Database } from 'bun:sqlite';
import { getReadonlyDb, getWritableDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { defineToolCommand } from '../../tool';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type JournalEntryRow = {
	id: string;
	posted_at: string;
	posted_date: string;
	is_transfer: number;
	description: string;
	raw_description: string | null;
	clean_description: string | null;
	counterparty: string | null;
	source_file: string | null;
};

type PostingRow = {
	id: string;
	journal_entry_id: string;
	account_id: string;
	amount_minor: number;
	currency: string;
	memo: string | null;
	provider_txn_id: string | null;
	provider_balance_minor: number | null;
};

type ChangeSet = Record<string, { from: string; to: string }>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function lookupEntry(db: Database, id: string): { entry: JournalEntryRow; postings: PostingRow[] } | null {
	const entry = db.query<JournalEntryRow, [string]>('SELECT * FROM journal_entries WHERE id = ?').get(id);
	if (!entry) return null;
	const postings = db.query<PostingRow, [string]>('SELECT * FROM postings WHERE journal_entry_id = ?').all(id);
	return { entry, postings };
}

function validateArgs(id: string | undefined, description: string | undefined, account: string | undefined, jsonMode: boolean, start: number): asserts id is string {
	if (!id) {
		if (jsonMode) {
			fail('edit.transaction', 'INVALID_INPUT', 'Missing required journal entry ID', 'Usage: fin edit transaction <id> [--description <text>] [--account <account_id>]', start);
		}
		process.stderr.write('Error: Missing required journal entry ID\n');
		process.exit(1);
	}
	if (!description && !account) {
		if (jsonMode) {
			fail(
				'edit.transaction',
				'INVALID_INPUT',
				'At least one of --description or --account must be provided',
				'Usage: fin edit transaction <id> --description "New name" --account Expenses:Personal:Immigration',
				start,
			);
		}
		process.stderr.write('Error: At least one of --description or --account must be provided\n');
		process.exit(1);
	}
}

function resolveEntry(db: Database, id: string, jsonMode: boolean, start: number): { entry: JournalEntryRow; postings: PostingRow[] } {
	const result = lookupEntry(db, id);
	if (result) return result;
	if (jsonMode) {
		fail('edit.transaction', 'NOT_FOUND', `Journal entry not found: ${id}`, 'Check the ID with: fin view ledger --json', start);
	}
	process.stderr.write(`Error: Journal entry not found: ${id}\n`);
	process.exit(1);
}

function findTargetPosting(postings: PostingRow[], jsonMode: boolean, start: number): PostingRow {
	const nonAsset = postings.filter((p) => !p.account_id.startsWith('Assets:'));
	if (nonAsset.length !== 1) {
		if (jsonMode) {
			fail(
				'edit.transaction',
				'AMBIGUOUS_POSTING',
				`Expected exactly one non-asset posting, found ${nonAsset.length}`,
				'Use fin view ledger to inspect the entry and edit postings manually',
				start,
			);
		}
		process.stderr.write(`Error: Expected exactly one non-asset posting, found ${nonAsset.length}\n`);
		process.stderr.write('Hint: Use fin view ledger to inspect the entry\n');
		process.exit(1);
	}
	return nonAsset[0] as PostingRow;
}

function ensureAccount(db: Database, accountId: string, jsonMode: boolean, start: number): boolean {
	const exists = db.query<{ id: string }, [string]>('SELECT id FROM chart_of_accounts WHERE id = ?').get(accountId);
	if (exists) return false;

	const parts = accountId.split(':');
	if (parts.length < 2) {
		if (jsonMode) {
			fail('edit.transaction', 'INVALID_ACCOUNT', `Invalid account format: ${accountId}`, 'Account IDs must be hierarchical (e.g., Expenses:Personal:Immigration)', start);
		}
		process.stderr.write(`Error: Invalid account format: ${accountId}\n`);
		process.exit(1);
	}

	const parentId = parts.slice(0, -1).join(':');
	const parentExists = db.query<{ id: string }, [string]>('SELECT id FROM chart_of_accounts WHERE id = ?').get(parentId);
	if (!parentExists) {
		if (jsonMode) {
			fail('edit.transaction', 'INVALID_ACCOUNT', `Parent account not found: ${parentId}`, 'Create parent accounts first or check the account hierarchy', start);
		}
		process.stderr.write(`Error: Parent account not found: ${parentId}\n`);
		process.exit(1);
	}

	const rootType = parts[0]?.toLowerCase() ?? 'expense';
	const typeMap: Record<string, string> = { expenses: 'expense', income: 'income', assets: 'asset', liabilities: 'liability', equity: 'equity' };
	const accountType = typeMap[rootType] ?? 'expense';
	const accountName = parts[parts.length - 1] ?? accountId;

	db.query('INSERT INTO chart_of_accounts (id, name, account_type, parent_id, is_placeholder) VALUES (?, ?, ?, ?, 0)').run(accountId, accountName, accountType, parentId);
	return true;
}

function accountExists(db: Database, accountId: string): boolean {
	return !!db.query<{ id: string }, [string]>('SELECT id FROM chart_of_accounts WHERE id = ?').get(accountId);
}

function previewChanges(db: Database, entry: JournalEntryRow, postings: PostingRow[], newDescription: string | undefined, newAccount: string | undefined, jsonMode: boolean, start: number): void {
	const changes: ChangeSet = {};
	let willCreateAccount = false;

	if (newDescription) {
		changes['description'] = { from: entry.description, to: newDescription };
	}
	if (newAccount) {
		const target = findTargetPosting(postings, jsonMode, start);
		changes['account'] = { from: target.account_id, to: newAccount };
		willCreateAccount = !accountExists(db, newAccount);
	}

	if (jsonMode) {
		ok('edit.transaction', { entry, changes, accountCreated: willCreateAccount, dryRun: true }, start);
	}

	process.stderr.write(`[dry-run] Would edit journal entry: ${entry.id}\n`);
	for (const [field, change] of Object.entries(changes)) {
		process.stderr.write(`  ${field}: "${change.from}" -> "${change.to}"\n`);
	}
	if (willCreateAccount) {
		process.stderr.write(`  Would create account: ${newAccount}\n`);
	}
}

function applyChanges(db: Database, entry: JournalEntryRow, postings: PostingRow[], newDescription: string | undefined, newAccount: string | undefined, jsonMode: boolean, start: number): void {
	const changes: ChangeSet = {};
	let created = false;

	db.run('BEGIN IMMEDIATE');
	try {
		if (newAccount) {
			const target = findTargetPosting(postings, jsonMode, start);
			created = ensureAccount(db, newAccount, jsonMode, start);
			db.query('UPDATE postings SET account_id = ? WHERE id = ?').run(newAccount, target.id);
			changes['account'] = { from: target.account_id, to: newAccount };
		}
		if (newDescription) {
			db.query('UPDATE journal_entries SET description = ?, clean_description = ? WHERE id = ?').run(newDescription, newDescription, entry.id);
			changes['description'] = { from: entry.description, to: newDescription };
		}
		db.run('COMMIT');
	} catch (e) {
		db.run('ROLLBACK');
		throw e;
	}

	const updated = lookupEntry(db, entry.id);
	if (jsonMode) {
		ok('edit.transaction', { entry: updated?.entry ?? entry, changes, accountCreated: created }, start);
	}

	process.stderr.write(`Edited journal entry: ${entry.id}\n`);
	for (const [field, change] of Object.entries(changes)) {
		process.stderr.write(`  ${field}: "${change.from}" -> "${change.to}"\n`);
	}
	if (created) {
		process.stderr.write(`  Created account: ${newAccount}\n`);
	}
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

export const editTransactionCommand = defineToolCommand(
	{
		name: 'edit.transaction',
		command: 'fin edit transaction',
		category: 'edit',
		outputSchema: {
			entry: { type: 'object', description: 'Updated journal entry with postings' },
			changes: { type: 'object', description: 'Applied changes (description, account)' },
			accountCreated: { type: 'boolean', description: 'Whether a new chart account was auto-created' },
		},
		idempotent: false,
		rateLimit: null,
		example: 'fin edit transaction je_abc123 --description "ILR Application" --account Expenses:Personal:Immigration --json',
	},
	{
		meta: {
			name: 'transaction',
			description: 'Edit a journal entry description and/or expense category',
		},
		args: {
			id: { type: 'positional' as const, description: 'Journal entry ID to edit', required: true },
			description: { type: 'string' as const, description: 'New description for the journal entry' },
			account: { type: 'string' as const, description: 'New account ID for the expense-side posting' },
			'dry-run': { type: 'boolean' as const, description: 'Preview changes without writing', default: false },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();
			const dryRun = args['dry-run'] ?? false;

			try {
				validateArgs(args.id, args.description, args.account, jsonMode, start);

				const dbArgs = args.db ? { options: new Map([['db', args.db]]) } : undefined;
				const db = dryRun ? getReadonlyDb(dbArgs) : getWritableDb(dbArgs);
				const { entry, postings } = resolveEntry(db, args.id, jsonMode, start);

				if (dryRun) {
					previewChanges(db, entry, postings, args.description, args.account, jsonMode, start);
					return;
				}

				applyChanges(db, entry, postings, args.description, args.account, jsonMode, start);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('edit.transaction', 'DB_ERROR', `Failed to edit entry: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
