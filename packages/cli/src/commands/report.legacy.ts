/**
 * report.legacy -- Legacy report subcommands not yet migrated to defineToolCommand().
 *
 * These are imported by report/index.ts alongside migrated commands.
 * Each command will be moved to its own file in report/ as it is migrated.
 */

import {
	type GroupId,
	getConsolidatedDailyRunwaySeries,
	getExpenseAccountPayeeBreakdown,
	getGroupCategoryBreakdown,
	getGroupCategoryMonthlyMedian,
	getGroupDailyHealthSeries,
	getGroupDailyReserveBreakdownSeries,
	getGroupDailyRunwaySeries,
	isGroupId,
} from '@fin/core';
import { getAccountIdsByGroup } from '@fin/core/config';
import { defineCommand } from 'citty';

import { getReadonlyDb } from '../db';
import { formatAmount, formatDate, formatMonths } from '../format';
import { error } from '../logger';
import { type Column, parseFormat, renderOutput } from '../output';
import { summary } from './summary';

// Shared args for all report commands
const formatArg = {
	type: 'string' as const,
	description: 'Output format: table, json, tsv',
	default: 'table',
};

const dbArg = {
	type: 'string' as const,
	description: 'Database path',
};

const groupArg = {
	type: 'string' as const,
	description: 'Group ID (personal, business, joint)',
	required: true as const,
};

function validateGroup(group: string | undefined, cmd: string): asserts group is GroupId {
	if (!group) {
		error(`Missing required option: --group\nUsage: fin report ${cmd} --group=GROUP`);
		process.exit(1);
	}
	if (!isGroupId(group)) {
		error(`Invalid group: ${group}. Use: personal, business, joint`);
		process.exit(1);
	}
}

// ============================================================================
// health
// ============================================================================

type HealthRow = {
	date: string;
	health: number;
};

const HEALTH_COLUMNS: Column<HealthRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'health', label: 'Health', align: 'right', minWidth: 14, format: (v) => formatAmount(v as number) },
];

export const health = defineCommand({
	meta: { name: 'health', description: 'Financial health metrics (balance - reserves)' },
	args: {
		group: groupArg,
		from: { type: 'string', description: 'Start date (YYYY-MM-DD)' },
		to: { type: 'string', description: 'End date (YYYY-MM-DD)' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		validateGroup(args.group, 'health');
		const format = parseFormat(args.format);
		const from = args.from;
		const to = args.to;
		const dbPath = args.db;

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const options: { from?: string; to?: string } = {};
		if (from) options.from = from;
		if (to) options.to = to;
		const series = getGroupDailyHealthSeries(db, args.group, options);

		const rows: HealthRow[] = series.map((p) => ({
			date: p.date,
			health: p.healthMinor,
		}));

		// Get latest health value
		const latest = rows.length > 0 ? rows[rows.length - 1] : null;
		const summaryText = latest ? `${rows.length} days | Latest: ${formatAmount(latest.health)}` : `${rows.length} days`;

		renderOutput(rows, HEALTH_COLUMNS, format, summaryText);
	},
});

// ============================================================================
// runway
// ============================================================================

type RunwayRow = {
	date: string;
	runway: number;
};

type ConsolidatedRunwayRow = {
	date: string;
	balance: number;
	burnRate: number;
	runway: number;
};

const RUNWAY_COLUMNS: Column<RunwayRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'runway', label: 'Runway', align: 'right', minWidth: 10, format: (v) => formatMonths(v as number) },
];

const CONSOLIDATED_RUNWAY_COLUMNS: Column<ConsolidatedRunwayRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'balance', label: 'Balance', align: 'right', minWidth: 14, format: (v) => formatAmount(v as number) },
	{ key: 'burnRate', label: 'Burn Rate', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'runway', label: 'Runway', align: 'right', minWidth: 10, format: (v) => formatMonths(v as number) },
];

function parseIncludeGroups(include: string | undefined): GroupId[] {
	if (!include) {
		error('Missing required option: --include\nUsage: fin report runway --consolidated --include=personal,business');
		process.exit(1);
	}
	const groups = include.split(',').map((g) => g.trim());
	for (const g of groups) {
		if (!isGroupId(g)) {
			error(`Invalid group in --include: ${g}. Use: personal, business, joint`);
			process.exit(1);
		}
	}
	return groups as GroupId[];
}

function renderConsolidatedRunway(db: ReturnType<typeof getReadonlyDb>, includeGroups: GroupId[], from: string | undefined, to: string | undefined, format: ReturnType<typeof parseFormat>): void {
	const options: { includeGroups: GroupId[]; from?: string; to?: string } = { includeGroups };
	if (from) options.from = from;
	if (to) options.to = to;
	const series = getConsolidatedDailyRunwaySeries(db, options);
	const rows: ConsolidatedRunwayRow[] = series.map((p) => ({
		date: p.date,
		balance: p.balanceMinor,
		burnRate: p.burnRateMinor,
		runway: p.runwayMonths,
	}));
	const latest = rows.length > 0 ? rows[rows.length - 1] : null;
	const groupsLabel = includeGroups.join('+');
	const summaryText = latest
		? `${rows.length} days | ${groupsLabel} | Latest: ${formatMonths(latest.runway)} (${formatAmount(latest.balance)} / ${formatAmount(latest.burnRate)})`
		: `${rows.length} days | ${groupsLabel}`;
	renderOutput(rows, CONSOLIDATED_RUNWAY_COLUMNS, format, summaryText);
}

export const runway = defineCommand({
	meta: { name: 'runway', description: 'Months of cash remaining' },
	args: {
		group: { type: 'string', description: 'Group ID (personal, business, joint)' },
		consolidated: { type: 'boolean', description: 'Consolidated runway across multiple groups' },
		include: { type: 'string', description: 'Groups to include (comma-separated, requires --consolidated)' },
		from: { type: 'string', description: 'Start date (YYYY-MM-DD)' },
		to: { type: 'string', description: 'End date (YYYY-MM-DD)' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		const format = parseFormat(args.format);
		const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);

		if (args.consolidated) {
			const includeGroups = parseIncludeGroups(args.include);
			renderConsolidatedRunway(db, includeGroups, args.from, args.to, format);
			return;
		}

		validateGroup(args.group, 'runway');
		const options: { from?: string; to?: string } = {};
		if (args.from) options.from = args.from;
		if (args.to) options.to = args.to;
		const series = getGroupDailyRunwaySeries(db, args.group, options);
		const rows: RunwayRow[] = series.map((p) => ({ date: p.date, runway: p.runwayMonths }));
		const latest = rows.length > 0 ? rows[rows.length - 1] : null;
		const summaryText = latest ? `${rows.length} days | Latest: ${formatMonths(latest.runway)}` : `${rows.length} days`;
		renderOutput(rows, RUNWAY_COLUMNS, format, summaryText);
	},
});

// ============================================================================
// reserves
// ============================================================================

type ReservesRow = {
	date: string;
	balance: number;
	taxReserve: number;
	expenseReserve: number;
	available: number;
};

const RESERVES_COLUMNS: Column<ReservesRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'balance', label: 'Balance', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'taxReserve', label: 'Tax Rsv', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'expenseReserve', label: 'Exp Rsv', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'available', label: 'Available', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
];

export const reserves = defineCommand({
	meta: { name: 'reserves', description: 'Reserve breakdown (tax + expense reserves)' },
	args: {
		group: groupArg,
		from: { type: 'string', description: 'Start date (YYYY-MM-DD)' },
		to: { type: 'string', description: 'End date (YYYY-MM-DD)' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		validateGroup(args.group, 'reserves');
		const format = parseFormat(args.format);
		const from = args.from;
		const to = args.to;
		const dbPath = args.db;

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const options: { from?: string; to?: string } = {};
		if (from) options.from = from;
		if (to) options.to = to;
		const series = getGroupDailyReserveBreakdownSeries(db, args.group, options);

		const rows: ReservesRow[] = series.map((p) => ({
			date: p.date,
			balance: p.balanceMinor,
			taxReserve: p.taxReserveMinor,
			expenseReserve: p.expenseReserveMinor,
			available: p.availableMinor,
		}));

		// Get latest values
		const latest = rows.length > 0 ? rows[rows.length - 1] : null;
		const summaryText = latest ? `${rows.length} days | Available: ${formatAmount(latest.available)}` : `${rows.length} days`;

		renderOutput(rows, RESERVES_COLUMNS, format, summaryText);
	},
});

// ============================================================================
// categories
// ============================================================================

type BreakdownRow = {
	category: string;
	total: number;
	count: number;
};

const BREAKDOWN_COLUMNS: Column<BreakdownRow>[] = [
	{ key: 'category', label: 'Category', minWidth: 20 },
	{ key: 'total', label: 'Total', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'count', label: 'Count', align: 'right', minWidth: 8, format: (v) => String(v) },
];

type MedianRow = {
	category: string;
	median: number;
	months: number;
};

const MEDIAN_COLUMNS: Column<MedianRow>[] = [
	{ key: 'category', label: 'Category', minWidth: 20 },
	{ key: 'median', label: 'Monthly Median', align: 'right', minWidth: 14, format: (v) => formatAmount(v as number) },
	{ key: 'months', label: 'Months', align: 'right', minWidth: 8, format: (v) => String(v) },
];

const categoriesBreakdown = defineCommand({
	meta: { name: 'breakdown', description: 'Total spending by category' },
	args: {
		group: groupArg,
		months: { type: 'string', description: 'Number of months', default: '3' },
		limit: { type: 'string', description: 'Max categories', default: '10' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		validateGroup(args.group, 'categories breakdown');
		const format = parseFormat(args.format);
		const months = Number.parseInt(args.months ?? '3', 10);
		const limit = Number.parseInt(args.limit ?? '10', 10);
		const dbPath = args.db;

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const data = getGroupCategoryBreakdown(db, args.group, { months, limit });

		const rows: BreakdownRow[] = data.map((p) => ({
			category: p.category,
			total: p.totalMinor,
			count: p.transactionCount,
		}));

		const grandTotal = rows.reduce((sum, r) => sum + r.total, 0);
		const summaryText = `${rows.length} categories | Total: ${formatAmount(grandTotal)} (${months} months)`;

		renderOutput(rows, BREAKDOWN_COLUMNS, format, summaryText);
	},
});

const categoriesMedian = defineCommand({
	meta: { name: 'median', description: 'Monthly median spending by category' },
	args: {
		group: groupArg,
		months: { type: 'string', description: 'Number of months', default: '6' },
		limit: { type: 'string', description: 'Max categories', default: '10' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		validateGroup(args.group, 'categories median');
		const format = parseFormat(args.format);
		const months = Number.parseInt(args.months ?? '6', 10);
		const limit = Number.parseInt(args.limit ?? '10', 10);
		const dbPath = args.db;

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const data = getGroupCategoryMonthlyMedian(db, args.group, { months, limit });

		const rows: MedianRow[] = data.map((p) => ({
			category: p.category,
			median: p.monthlyMedianMinor,
			months: p.monthCount,
		}));

		const totalMedian = rows.reduce((sum, r) => sum + r.median, 0);
		const summaryText = `${rows.length} categories | Est. monthly: ${formatAmount(totalMedian)}`;

		renderOutput(rows, MEDIAN_COLUMNS, format, summaryText);
	},
});

export const categories = defineCommand({
	meta: { name: 'categories', description: 'Spending by category' },
	subCommands: {
		breakdown: categoriesBreakdown,
		median: categoriesMedian,
	},
});

// ============================================================================
// audit
// ============================================================================

type AuditRow = {
	payee: string;
	total: number;
	monthlyAvg: number;
	count: number;
	account: string;
	lastDate: string;
};

const AUDIT_COLUMNS: Column<AuditRow>[] = [
	{ key: 'payee', label: 'Payee', minWidth: 24 },
	{ key: 'total', label: 'Total', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'monthlyAvg', label: 'Monthly Avg', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'count', label: 'Count', align: 'right', minWidth: 6, format: (v) => String(v) },
	{ key: 'account', label: 'Account', minWidth: 20 },
	{ key: 'lastDate', label: 'Last Date', minWidth: 10, format: (v) => formatDate(v as string) },
];

export const audit = defineCommand({
	meta: { name: 'audit', description: 'Expense account payee breakdown' },
	args: {
		account: { type: 'string', description: 'Expense account prefix (e.g. Expenses:Business:Uncategorized)', required: true },
		months: { type: 'string', description: 'Number of months', default: '12' },
		group: { type: 'string', description: 'Filter by group (personal, business, joint)' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		if (!args.account) {
			error('Missing required option: --account\nUsage: fin report audit --account=Expenses:Business:Uncategorized');
			process.exit(1);
		}

		const format = parseFormat(args.format);
		const months = Number.parseInt(args.months ?? '12', 10);
		const dbPath = args.db;

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);

		const options: { months: number; chartAccountIds?: string[] } = { months };
		if (args.group) {
			if (!isGroupId(args.group)) {
				error(`Invalid group: ${args.group}. Use: personal, business, joint`);
				process.exit(1);
			}
			options.chartAccountIds = getAccountIdsByGroup(args.group);
		}

		const data = getExpenseAccountPayeeBreakdown(db, args.account, options);

		const rows: AuditRow[] = data.map((p) => ({
			payee: p.payee,
			total: p.totalMinor,
			monthlyAvg: p.monthlyAvgMinor,
			count: p.transactionCount,
			account: p.sampleAccount,
			lastDate: p.lastDate,
		}));

		const grandTotal = rows.reduce((sum, r) => sum + r.total, 0);
		const summaryText = `${rows.length} payees | Total: ${formatAmount(grandTotal)} (${months} months) | ${args.account}`;

		renderOutput(rows, AUDIT_COLUMNS, format, summaryText);
	},
});

// Re-export summary for use by report/index.ts
export { summary };
