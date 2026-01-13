/**
 * report - Financial analytics and reports.
 */

import { defineCommand } from 'citty';
import {
	type GroupId,
	getConsolidatedDailyRunwaySeries,
	getGroupCategoryBreakdown,
	getGroupCategoryMonthlyMedian,
	getGroupDailyHealthSeries,
	getGroupDailyReserveBreakdownSeries,
	getGroupDailyRunwaySeries,
	getGroupMonthlyCashflowSeries,
	isGroupId,
} from 'core';

import { getReadonlyDb } from '../db';
import { formatAmount, formatCount, formatDate, formatMonth, formatMonths, formatPercentRaw } from '../format';
import { error } from '../logger';
import { type Column, parseFormat, renderOutput } from '../output';

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
// cashflow
// ============================================================================

type CashflowRow = {
	month: string;
	income: number;
	expenses: number;
	net: number;
	savingsRate: number | null;
};

const CASHFLOW_COLUMNS: Column<CashflowRow>[] = [
	{ key: 'month', label: 'Month', minWidth: 7, format: (v) => formatMonth(v as string) },
	{ key: 'income', label: 'Income', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'expenses', label: 'Expenses', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'net', label: 'Net', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'savingsRate', label: 'Savings%', align: 'right', minWidth: 8, format: (v) => formatPercentRaw(v as number | null) },
];

const cashflow = defineCommand({
	meta: { name: 'cashflow', description: 'Monthly cashflow summary' },
	args: {
		group: groupArg,
		months: { type: 'string', description: 'Number of months', default: '12' },
		from: { type: 'string', description: 'Start month (YYYY-MM)' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		validateGroup(args.group, 'cashflow');
		const format = parseFormat(args.format);
		const months = Number.parseInt(args.months ?? '12', 10);
		const from = args.from;
		const dbPath = args.db;

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const options: { from?: string; limit: number } = { limit: months };
		if (from) options.from = from;
		const series = getGroupMonthlyCashflowSeries(db, args.group, options);

		const rows: CashflowRow[] = series.map((p) => ({
			month: p.month,
			income: p.incomeMinor,
			expenses: p.expenseMinor,
			net: p.netMinor,
			savingsRate: p.savingsRatePct,
		}));

		// Calculate totals
		const totalIncome = rows.reduce((sum, r) => sum + r.income, 0);
		const totalExpenses = rows.reduce((sum, r) => sum + r.expenses, 0);
		const totalNet = totalIncome - totalExpenses;
		const summaryText = `${formatCount(rows.length, 'month')} | Net: ${formatAmount(totalNet)}`;

		renderOutput(rows, CASHFLOW_COLUMNS, format, summaryText);
	},
});

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

const health = defineCommand({
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

const runway = defineCommand({
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

const reserves = defineCommand({
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

const categories = defineCommand({
	meta: { name: 'categories', description: 'Spending by category' },
	subCommands: {
		breakdown: categoriesBreakdown,
		median: categoriesMedian,
	},
});

// ============================================================================
// report (parent command)
// ============================================================================

export const report = defineCommand({
	meta: { name: 'report', description: 'Financial analytics and reports' },
	subCommands: {
		cashflow,
		health,
		runway,
		reserves,
		categories,
	},
});
