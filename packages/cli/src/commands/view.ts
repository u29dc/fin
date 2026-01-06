/**
 * view - View accounts, transactions, ledger, and balance sheet.
 */

import { defineCommand } from 'citty';
import type { GroupId } from 'core';
import { type AssetAccountId, getAssetAccounts, getBalanceSheet, getGroupChartAccountIds, getJournalEntries, getJournalEntryCount, getLatestBalances, getTransactions, isGroupId } from 'core';

import { getReadonlyDb } from '../db';
import { formatAmount, formatCount, formatDate } from '../format';
import { json } from '../logger';
import { type Column, parseFormat, renderOutput } from '../output';

// Shared args for all view commands
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
// accounts
// ============================================================================

type AccountRow = {
	id: string;
	name: string;
	type: string;
	balance: number | null;
	updated: string | null;
};

const ACCOUNT_COLUMNS: Column<AccountRow>[] = [
	{ key: 'id', label: 'Account', minWidth: 24 },
	{ key: 'name', label: 'Name', minWidth: 12 },
	{ key: 'type', label: 'Type', minWidth: 8 },
	{ key: 'balance', label: 'Balance', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number | null) },
	{ key: 'updated', label: 'Updated', minWidth: 10, format: (v) => formatDate(v as string | null) },
];

const accounts = defineCommand({
	meta: { name: 'accounts', description: 'List accounts with balances' },
	args: {
		group: { type: 'string', description: 'Filter by group (personal, business, joint)' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		const format = parseFormat(args.format);
		const groupFilter = args.group;
		const dbPath = args.db;

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const accountList = getAssetAccounts(db);
		const chartAccountIds = accountList.map((a) => a.id as AssetAccountId);
		const balances = getLatestBalances(db, chartAccountIds);

		// Filter by group if specified
		let filtered = accountList;
		if (groupFilter && isGroupId(groupFilter)) {
			const groupIds = new Set(getGroupChartAccountIds(groupFilter as GroupId) as string[]);
			filtered = accountList.filter((a) => groupIds.has(a.id));
		}

		// Build output rows
		const balanceMap = new Map(balances.map((b) => [b.chartAccountId, b]));
		const rows: AccountRow[] = filtered.map((a) => {
			const bal = balanceMap.get(a.id as AssetAccountId);
			return {
				id: a.id,
				name: a.name,
				type: a.accountType,
				balance: bal?.balanceMinor ?? null,
				updated: bal?.date ?? null,
			};
		});

		// Calculate total
		const total = rows.reduce((sum, r) => sum + (r.balance ?? 0), 0);
		const summaryText = `${formatCount(rows.length, 'account')} | Total: ${formatAmount(total)}`;

		renderOutput(rows, ACCOUNT_COLUMNS, format, summaryText);
	},
});

// ============================================================================
// transactions
// ============================================================================

type TransactionRow = {
	date: string;
	account: string;
	amount: number;
	description: string;
};

const TRANSACTION_COLUMNS: Column<TransactionRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'account', label: 'Account', minWidth: 24 },
	{ key: 'amount', label: 'Amount', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'description', label: 'Description', minWidth: 30, maxWidth: 40 },
];

const transactions = defineCommand({
	meta: { name: 'transactions', description: 'Query transactions with filters' },
	args: {
		account: { type: 'string', description: 'Filter by chart account ID' },
		group: { type: 'string', description: 'Filter by group (personal, business, joint)' },
		from: { type: 'string', description: 'Start date (YYYY-MM-DD)' },
		to: { type: 'string', description: 'End date (YYYY-MM-DD)' },
		limit: { type: 'string', description: 'Max results', default: '50' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		const format = parseFormat(args.format);
		const chartAccountId = args.account;
		const groupId = args.group;
		const from = args.from;
		const to = args.to;
		const limit = Number.parseInt(args.limit ?? '50', 10);
		const dbPath = args.db;

		// Determine account filter
		let chartAccountIds: string[] | undefined;
		if (groupId && isGroupId(groupId)) {
			chartAccountIds = getGroupChartAccountIds(groupId as GroupId);
		} else if (chartAccountId) {
			chartAccountIds = [chartAccountId];
		}

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const options: { chartAccountIds?: string[]; from?: string; to?: string; limit: number } = { limit };
		if (chartAccountIds) options.chartAccountIds = chartAccountIds;
		if (from) options.from = from;
		if (to) options.to = to;
		const txns = getTransactions(db, options);

		const rows: TransactionRow[] = txns.map((t) => ({
			date: t.postedAt,
			account: t.chartAccountId,
			amount: t.amountMinor,
			description: t.cleanDescription || t.rawDescription,
		}));

		const summaryText = formatCount(rows.length, 'transaction');
		renderOutput(rows, TRANSACTION_COLUMNS, format, summaryText);
	},
});

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

const ledger = defineCommand({
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

const balance = defineCommand({
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

		// Filter out empty separator row for JSON output
		const outputRows = format === 'json' ? rows.filter((r) => r.category !== '') : rows;

		renderOutput(outputRows, BALANCE_COLUMNS, format);
	},
});

// ============================================================================
// view (parent command)
// ============================================================================

export const view = defineCommand({
	meta: { name: 'view', description: 'View accounts, transactions, ledger, balance sheet' },
	subCommands: {
		accounts,
		transactions,
		ledger,
		balance,
	},
});
