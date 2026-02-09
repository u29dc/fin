/**
 * `report categories` -- Spending by category (breakdown or monthly median).
 *
 * Requires --group to specify which account group to analyze.
 * Uses --mode=breakdown (default) for total spending, or --mode=median for monthly median.
 * Replaces the old `categories breakdown` / `categories median` subcommand nesting.
 */

import type { GroupId } from '@fin/core';
import { getGroupCategoryBreakdown, getGroupCategoryMonthlyMedian, isGroupId } from '@fin/core';
import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount, formatCount } from '../../format';
import { type Column, parseFormat, renderOutput } from '../../output';
import { defineToolCommand } from '../../tool';

type BreakdownRow = {
	category: string;
	total: number;
	count: number;
};

type MedianRow = {
	category: string;
	median: number;
	months: number;
};

const BREAKDOWN_COLUMNS: Column<BreakdownRow>[] = [
	{ key: 'category', label: 'Category', minWidth: 20 },
	{ key: 'total', label: 'Total', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'count', label: 'Count', align: 'right', minWidth: 8, format: (v) => String(v) },
];

const MEDIAN_COLUMNS: Column<MedianRow>[] = [
	{ key: 'category', label: 'Category', minWidth: 20 },
	{ key: 'median', label: 'Monthly Median', align: 'right', minWidth: 14, format: (v) => formatAmount(v as number) },
	{ key: 'months', label: 'Months', align: 'right', minWidth: 8, format: (v) => String(v) },
];

function validateGroupArg(group: string | undefined, jsonMode: boolean, start: number): asserts group is GroupId {
	if (!group || !isGroupId(group)) {
		if (jsonMode) {
			fail('report.categories', 'INVALID_GROUP', `Invalid or missing group: ${group ?? '(none)'}`, 'Use --group=personal, --group=business, or --group=joint', start);
		}
		process.stderr.write(`Error: Invalid or missing group: ${group ?? '(none)'}. Use: personal, business, joint\n`);
		process.exit(1);
	}
}

function validateModeArg(mode: string, jsonMode: boolean, start: number): asserts mode is 'breakdown' | 'median' {
	if (mode !== 'breakdown' && mode !== 'median') {
		if (jsonMode) {
			fail('report.categories', 'INVALID_INPUT', `Invalid mode: ${mode}`, 'Use --mode=breakdown or --mode=median', start);
		}
		process.stderr.write(`Error: Invalid mode: ${mode}. Use: breakdown, median\n`);
		process.exit(1);
	}
}

function handleBreakdownMode(db: ReturnType<typeof getReadonlyDb>, groupId: GroupId, months: number, limit: number, jsonMode: boolean, format: string, start: number): void {
	const data = getGroupCategoryBreakdown(db, groupId, { months, limit });

	const rows: BreakdownRow[] = data.map((p) => ({
		category: p.category,
		total: p.totalMinor,
		count: p.transactionCount,
	}));

	const grandTotal = rows.reduce((sum, r) => sum + r.total, 0);

	if (jsonMode) {
		ok('report.categories', { categories: rows, total: grandTotal }, start, { count: rows.length });
	}

	const parsedFormat = parseFormat(format);
	const summaryText = `${formatCount(rows.length, 'category', 'categories')} | Total: ${formatAmount(grandTotal)} (${months} months)`;
	renderOutput(rows, BREAKDOWN_COLUMNS, parsedFormat, summaryText);
}

function handleMedianMode(db: ReturnType<typeof getReadonlyDb>, groupId: GroupId, months: number, limit: number, jsonMode: boolean, format: string, start: number): void {
	const data = getGroupCategoryMonthlyMedian(db, groupId, { months, limit });

	const rows: MedianRow[] = data.map((p) => ({
		category: p.category,
		median: p.monthlyMedianMinor,
		months: p.monthCount,
	}));

	const estimatedMonthly = rows.reduce((sum, r) => sum + r.median, 0);

	if (jsonMode) {
		ok('report.categories', { categories: rows, estimatedMonthly }, start, { count: rows.length });
	}

	const parsedFormat = parseFormat(format);
	const summaryText = `${formatCount(rows.length, 'category', 'categories')} | Est. monthly: ${formatAmount(estimatedMonthly)}`;
	renderOutput(rows, MEDIAN_COLUMNS, parsedFormat, summaryText);
}

export const reportCategoriesCommand = defineToolCommand(
	{
		name: 'report.categories',
		command: 'fin report categories',
		category: 'report',
		outputSchema: {
			categories: { type: 'array', items: 'CategoryRow', description: 'Category spending data' },
			total: { type: 'number', description: 'Grand total spending (breakdown mode, minor units)' },
			estimatedMonthly: { type: 'number', description: 'Estimated monthly spend (median mode, minor units)' },
		},
		idempotent: true,
		rateLimit: null,
		example: 'fin report categories --group=personal --mode=breakdown --months=6 --json',
	},
	{
		meta: {
			name: 'categories',
			description: 'Spending by category (breakdown or monthly median)',
		},
		args: {
			group: { type: 'string' as const, description: 'Group ID (personal, business, joint)', required: true as const },
			mode: { type: 'string' as const, description: 'Mode: breakdown (total) or median (monthly)', default: 'breakdown' },
			months: { type: 'string' as const, description: 'Number of months to analyze', default: '3' },
			limit: { type: 'string' as const, description: 'Max categories to return', default: '10' },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
			format: { type: 'string' as const, description: 'Output format: table, json, tsv', default: 'table' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				validateGroupArg(args.group, jsonMode, start);
				const mode = args.mode ?? 'breakdown';
				validateModeArg(mode, jsonMode, start);

				const months = Number.parseInt(args.months ?? '3', 10);
				const limit = Number.parseInt(args.limit ?? '10', 10);
				const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const format = args.format ?? 'table';

				if (mode === 'median') {
					handleMedianMode(db, args.group, months, limit, jsonMode, format, start);
				} else {
					handleBreakdownMode(db, args.group, months, limit, jsonMode, format, start);
				}
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('report.categories', 'DB_ERROR', `Failed to query categories: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
