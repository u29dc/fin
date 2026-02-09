/**
 * `view ledger` -- Query journal entries with postings.
 *
 * Returns journal entries in double-entry format with their postings.
 * Supports --account, --from, --to, --limit filters.
 */

import { getJournalEntries, getJournalEntryCount } from '@fin/core';
import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount, formatCount, formatDate } from '../../format';
import { type Column, parseFormat, renderOutput } from '../../output';
import { defineToolCommand } from '../../tool';

type LedgerRow = {
	date: string;
	title: string;
	account1: string;
	amount1: number | null;
	account2: string;
	amount2: number | null;
};

const LEDGER_COLUMNS: Column<LedgerRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'title', label: 'Title', minWidth: 20, maxWidth: 28 },
	{ key: 'account1', label: 'Account 1', minWidth: 20 },
	{ key: 'amount1', label: 'Amount', align: 'right', minWidth: 10, format: (v) => formatAmount(v as number | null) },
	{ key: 'account2', label: 'Account 2', minWidth: 20 },
	{ key: 'amount2', label: 'Amount', align: 'right', minWidth: 10, format: (v) => formatAmount(v as number | null) },
];

type JournalEntry = Awaited<ReturnType<typeof getJournalEntries>>[number];

function entriesToRows(entries: JournalEntry[]): LedgerRow[] {
	return entries.map((entry) => {
		const [p1, p2] = entry.postings;
		return {
			date: entry.postedAt.slice(0, 10),
			title: entry.description,
			account1: p1?.accountId ?? '',
			amount1: p1?.amountMinor ?? null,
			account2: p2?.accountId ?? '',
			amount2: p2?.amountMinor ?? null,
		};
	});
}

export const viewLedgerCommand = defineToolCommand(
	{
		name: 'view.ledger',
		command: 'fin view ledger',
		category: 'view',
		outputFields: ['entries', 'count', 'total'],
		idempotent: true,
		rateLimit: null,
		example: 'fin view ledger --account=Assets:Personal:Monzo --limit=20 --json',
	},
	{
		meta: {
			name: 'ledger',
			description: 'Query journal entries with postings',
		},
		args: {
			account: { type: 'string' as const, description: 'Filter by account ID' },
			from: { type: 'string' as const, description: 'Start date (YYYY-MM-DD)' },
			to: { type: 'string' as const, description: 'End date (YYYY-MM-DD)' },
			limit: { type: 'string' as const, description: 'Max entries', default: '50' },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
			format: { type: 'string' as const, description: 'Output format: table, json, tsv', default: 'table' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const limit = Number.parseInt(args.limit ?? '50', 10);

				type JournalOptions = Parameters<typeof getJournalEntries>[1];
				const options: JournalOptions = { limit };
				if (args.account) options.accountId = args.account;
				if (args.from) options.startDate = args.from;
				if (args.to) options.endDate = args.to;

				const entries = getJournalEntries(db, options);
				const total = getJournalEntryCount(db, args.account);

				if (jsonMode) {
					ok('view.ledger', { entries, count: entries.length, total }, start, { count: entries.length, total });
				}

				const rows = entriesToRows(entries);
				const format = parseFormat(args.format);
				const summaryText = `Showing ${formatCount(rows.length, 'entry', 'entries')} of ${total}`;
				renderOutput(rows, LEDGER_COLUMNS, format, summaryText);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('view.ledger', 'DB_ERROR', `Failed to query ledger: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
