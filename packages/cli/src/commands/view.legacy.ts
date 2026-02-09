/**
 * Legacy view commands -- ledger and balance.
 *
 * These will be migrated to defineToolCommand() in ENG-007.
 * Until then, they are imported by view/index.ts.
 */

import { getBalanceSheet, getJournalEntries, getJournalEntryCount } from '@fin/core';
import { defineCommand } from 'citty';

import { getReadonlyDb } from '../db';
import { formatAmount, formatCount, formatDate } from '../format';
import { json } from '../logger';
import { type Column, parseFormat, renderOutput } from '../output';

const formatArg = {
	type: 'string' as const,
	description: 'Output format: table, json, tsv',
	default: 'table',
};

const dbArg = {
	type: 'string' as const,
	description: 'Database path',
};

// ============================================================================
// ledger
// ============================================================================

type JournalEntry = Awaited<ReturnType<typeof getJournalEntries>>[number];

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

export const ledger = defineCommand({
	meta: { name: 'ledger', description: 'Query journal entries with postings' },
	args: {
		account: { type: 'string', description: 'Filter by account ID' },
		from: { type: 'string', description: 'Start date (YYYY-MM-DD)' },
		to: { type: 'string', description: 'End date (YYYY-MM-DD)' },
		limit: { type: 'string', description: 'Max entries', default: '50' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		const format = parseFormat(args.format);
		const accountId = args.account;
		const fromDate = args.from;
		const toDate = args.to;
		const limit = Number.parseInt(args.limit ?? '50', 10);
		const dbPath = args.db;

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);

		type JournalOptions = Parameters<typeof getJournalEntries>[1];
		const options: JournalOptions = { limit };
		if (accountId) options.accountId = accountId;
		if (fromDate) options.startDate = fromDate;
		if (toDate) options.endDate = toDate;

		const entries = getJournalEntries(db, options);

		if (format === 'json') {
			json(entries);
			return;
		}

		const rows = entriesToRows(entries);
		const total = getJournalEntryCount(db, accountId);
		const summaryText = `Showing ${formatCount(rows.length, 'entry', 'entries')} of ${total}`;
		renderOutput(rows, LEDGER_COLUMNS, format, summaryText);
	},
});

// ============================================================================
// balance
// ============================================================================

type BalanceRow = {
	category: string;
	amount: number;
};

const BALANCE_COLUMNS: Column<BalanceRow>[] = [
	{ key: 'category', label: 'Category', minWidth: 20 },
	{ key: 'amount', label: 'Amount', align: 'right', minWidth: 15, format: (v) => formatAmount(v as number) },
];

export const balance = defineCommand({
	meta: { name: 'balance', description: 'Display balance sheet' },
	args: {
		'as-of': { type: 'string', description: 'Balance as of date (YYYY-MM-DD)' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		const format = parseFormat(args.format);
		const asOf = args['as-of'];
		const dbPath = args.db;

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const bs = getBalanceSheet(db, asOf);

		const rows: BalanceRow[] = [
			{ category: 'Assets', amount: bs.assets },
			{ category: 'Liabilities', amount: bs.liabilities },
			{ category: 'Net Worth', amount: bs.netWorth },
			{ category: '', amount: 0 },
			{ category: 'Income', amount: bs.income },
			{ category: 'Expenses', amount: bs.expenses },
			{ category: 'Net Income', amount: bs.netIncome },
		];

		const outputRows = format === 'json' ? rows.filter((r) => r.category !== '') : rows;

		renderOutput(outputRows, BALANCE_COLUMNS, format);
	},
});
