/**
 * `view void` -- Create a reversing journal entry to zero out a bad import.
 *
 * Looks up the original journal entry + postings, then creates a new entry
 * with negated amounts. Supports --dry-run to preview without writing.
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function generateId(prefix: string): string {
	return `${prefix}_${crypto.randomUUID().replace(/-/g, '').slice(0, 16)}`;
}

function lookupEntry(db: Database, id: string): { entry: JournalEntryRow; postings: PostingRow[] } | null {
	const entry = db.query<JournalEntryRow, [string]>('SELECT * FROM journal_entries WHERE id = ?').get(id);
	if (!entry) return null;
	const postings = db.query<PostingRow, [string]>('SELECT * FROM postings WHERE journal_entry_id = ?').all(id);
	return { entry, postings };
}

function insertVoidEntry(db: Database, original: JournalEntryRow, postings: PostingRow[]): string {
	const voidEntryId = generateId('je');
	const now = new Date().toISOString();
	const postedDate = now.slice(0, 10);
	const description = `VOID: ${original.description}`;

	db.query(
		`INSERT INTO journal_entries (id, posted_at, posted_date, is_transfer, description, raw_description, clean_description, counterparty, source_file)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
	).run(voidEntryId, now, postedDate, original.is_transfer, description, null, description, null, null);

	const stmt = db.query(
		`INSERT INTO postings (id, journal_entry_id, account_id, amount_minor, currency, memo, provider_txn_id, provider_balance_minor)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
	);

	for (const p of postings) {
		stmt.run(generateId('p'), voidEntryId, p.account_id, -p.amount_minor, p.currency, `VOID: ${p.memo ?? ''}`, null, null);
	}

	return voidEntryId;
}

function validateId(id: string | undefined, jsonMode: boolean, start: number): asserts id is string {
	if (id) return;
	if (jsonMode) {
		fail('view.void', 'INVALID_INPUT', 'Missing required journal entry ID', 'Usage: fin view void <id>', start);
	}
	process.stderr.write('Error: Missing required journal entry ID\n');
	process.exit(1);
}

function resolveEntry(db: Database, id: string, jsonMode: boolean, start: number): { entry: JournalEntryRow; postings: PostingRow[] } {
	const result = lookupEntry(db, id);
	if (result) return result;
	if (jsonMode) {
		fail('view.void', 'INVALID_INPUT', `Journal entry not found: ${id}`, 'Check the ID with: fin view ledger --json', start);
	}
	process.stderr.write(`Error: Journal entry not found: ${id}\n`);
	process.exit(1);
}

function handleDryRun(entry: JournalEntryRow, postings: PostingRow[], jsonMode: boolean, start: number): void {
	const preview = {
		originalEntry: entry,
		voidEntry: { description: `VOID: ${entry.description}`, postingsToReverse: postings.length },
		postingsReversed: postings.length,
	};
	if (jsonMode) {
		ok('view.void', preview, start);
	}
	process.stderr.write(`[dry-run] Would void journal entry: ${entry.id}\n`);
	process.stderr.write(`  Description: ${entry.description}\n`);
	process.stderr.write(`  Postings to reverse: ${postings.length}\n`);
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

export const viewVoidCommand = defineToolCommand(
	{
		name: 'view.void',
		command: 'fin view void',
		category: 'view',
		outputSchema: {
			originalEntry: { type: 'object', description: 'The original journal entry that was voided' },
			voidEntry: { type: 'object', description: 'The new reversing journal entry' },
			postingsReversed: { type: 'number', description: 'Number of postings reversed' },
		},
		idempotent: false,
		rateLimit: null,
		example: 'fin view void je_abc123 --json',
	},
	{
		meta: {
			name: 'void',
			description: 'Create a reversing journal entry to zero out a bad import',
		},
		args: {
			id: { type: 'positional' as const, description: 'Journal entry ID to void', required: true },
			'dry-run': { type: 'boolean' as const, description: 'Preview void without writing', default: false },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();
			const dryRun = args['dry-run'] ?? false;

			try {
				validateId(args.id, jsonMode, start);

				const dbArgs = args.db ? { options: new Map([['db', args.db]]) } : undefined;
				const db = dryRun ? getReadonlyDb(dbArgs) : getWritableDb(dbArgs);
				const { entry, postings } = resolveEntry(db, args.id, jsonMode, start);

				if (dryRun) {
					handleDryRun(entry, postings, jsonMode, start);
					return;
				}

				const voidEntryId = insertVoidEntry(db, entry, postings);
				const voidResult = lookupEntry(db, voidEntryId);

				if (jsonMode) {
					ok('view.void', { originalEntry: entry, voidEntry: voidResult?.entry ?? { id: voidEntryId }, postingsReversed: postings.length }, start);
				}

				process.stderr.write(`Voided journal entry: ${entry.id}\n`);
				process.stderr.write(`  Original: ${entry.description}\n`);
				process.stderr.write(`  Void entry: ${voidEntryId}\n`);
				process.stderr.write(`  Postings reversed: ${postings.length}\n`);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('view.void', 'DB_ERROR', `Failed to void entry: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
