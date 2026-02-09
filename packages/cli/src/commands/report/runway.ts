/**
 * `report runway` -- Months of cash remaining.
 *
 * Supports single group mode (--group) and consolidated multi-group mode (--consolidated --include=...).
 */

import type { GroupId } from '@fin/core';
import { getConsolidatedDailyRunwaySeries, getGroupDailyRunwaySeries, isGroupId } from '@fin/core';
import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount, formatCount, formatDate, formatMonths } from '../../format';
import { type Column, parseFormat, renderOutput } from '../../output';
import { defineToolCommand } from '../../tool';

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

function parseIncludeGroups(include: string | undefined, jsonMode: boolean, start: number): GroupId[] {
	if (!include) {
		if (jsonMode) {
			fail('report.runway', 'INVALID_INPUT', 'Missing required option: --include', 'Usage: fin report runway --consolidated --include=personal,business', start);
		}
		process.stderr.write('Error: Missing required option: --include\nUsage: fin report runway --consolidated --include=personal,business\n');
		process.exit(1);
	}
	const groups = include.split(',').map((g) => g.trim());
	for (const g of groups) {
		if (!isGroupId(g)) {
			if (jsonMode) {
				fail('report.runway', 'INVALID_GROUP', `Invalid group in --include: ${g}`, 'Use: personal, business, joint', start);
			}
			process.stderr.write(`Error: Invalid group in --include: ${g}. Use: personal, business, joint\n`);
			process.exit(1);
		}
	}
	return groups as GroupId[];
}

function buildSingleRunwayData(db: ReturnType<typeof getReadonlyDb>, groupId: GroupId, from: string | undefined, to: string | undefined): { rows: RunwayRow[]; latest: RunwayRow | null } {
	const options: { from?: string; to?: string } = {};
	if (from) options.from = from;
	if (to) options.to = to;
	const series = getGroupDailyRunwaySeries(db, groupId, options);
	const rows: RunwayRow[] = series.map((p) => ({ date: p.date, runway: p.runwayMonths }));
	const latest = rows.length > 0 ? (rows[rows.length - 1] ?? null) : null;
	return { rows, latest };
}

function buildConsolidatedRunwayData(
	db: ReturnType<typeof getReadonlyDb>,
	includeGroups: GroupId[],
	from: string | undefined,
	to: string | undefined,
): { rows: ConsolidatedRunwayRow[]; latest: ConsolidatedRunwayRow | null } {
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
	const latest = rows.length > 0 ? (rows[rows.length - 1] ?? null) : null;
	return { rows, latest };
}

function handleConsolidatedMode(args: { include?: string; from?: string; to?: string; db?: string; format?: string }, jsonMode: boolean, start: number): void {
	const includeGroups = parseIncludeGroups(args.include, jsonMode, start);
	const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
	const { rows, latest } = buildConsolidatedRunwayData(db, includeGroups, args.from, args.to);

	if (jsonMode) {
		ok('report.runway', { series: rows, latest, groups: includeGroups }, start, { count: rows.length });
	}

	const format = parseFormat(args.format);
	const groupsLabel = includeGroups.join('+');
	const summaryText = latest
		? `${formatCount(rows.length, 'day')} | ${groupsLabel} | Latest: ${formatMonths(latest.runway)} (${formatAmount(latest.balance)} / ${formatAmount(latest.burnRate)})`
		: `${formatCount(rows.length, 'day')} | ${groupsLabel}`;
	renderOutput(rows, CONSOLIDATED_RUNWAY_COLUMNS, format, summaryText);
}

function handleSingleMode(args: { group?: string; from?: string; to?: string; db?: string; format?: string }, jsonMode: boolean, start: number): void {
	if (!args.group || !isGroupId(args.group)) {
		if (jsonMode) {
			fail('report.runway', 'INVALID_GROUP', `Invalid or missing group: ${args.group ?? '(none)'}`, 'Use --group=personal, --group=business, or --group=joint', start);
		}
		process.stderr.write(`Error: Invalid or missing group: ${args.group ?? '(none)'}. Use: personal, business, joint\n`);
		process.exit(1);
	}

	const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
	const { rows, latest } = buildSingleRunwayData(db, args.group, args.from, args.to);

	if (jsonMode) {
		ok('report.runway', { series: rows, latest }, start, { count: rows.length });
	}

	const format = parseFormat(args.format);
	const summaryText = latest ? `${formatCount(rows.length, 'day')} | Latest: ${formatMonths(latest.runway)}` : `${formatCount(rows.length, 'day')}`;
	renderOutput(rows, RUNWAY_COLUMNS, format, summaryText);
}

export const reportRunwayCommand = defineToolCommand(
	{
		name: 'report.runway',
		command: 'fin report runway',
		category: 'report',
		outputSchema: {
			series: { type: 'array', items: 'RunwayRow', description: 'Daily runway data points' },
			latest: { type: 'object', description: 'Most recent runway data point (or null)' },
			groups: { type: 'array', items: 'string', description: 'Included group IDs (consolidated mode only)' },
		},
		idempotent: true,
		rateLimit: null,
		example: 'fin report runway --group=personal --json',
	},
	{
		meta: {
			name: 'runway',
			description: 'Months of cash remaining',
		},
		args: {
			group: { type: 'string' as const, description: 'Group ID (personal, business, joint)' },
			consolidated: { type: 'boolean' as const, description: 'Consolidated runway across multiple groups', default: false },
			include: { type: 'string' as const, description: 'Groups to include (comma-separated, requires --consolidated)' },
			from: { type: 'string' as const, description: 'Start date (YYYY-MM-DD)' },
			to: { type: 'string' as const, description: 'End date (YYYY-MM-DD)' },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
			format: { type: 'string' as const, description: 'Output format: table, json, tsv', default: 'table' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				if (args.consolidated) {
					handleConsolidatedMode(args, jsonMode, start);
					return;
				}
				handleSingleMode(args, jsonMode, start);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('report.runway', 'DB_ERROR', `Failed to query runway: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
