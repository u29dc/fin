/**
 * categories - Spending by category.
 */

import { getGroupCategoryBreakdown, getGroupCategoryMonthlyMedian } from 'core';

import { getOption, getOptionAsNumberOrDefault, parseArgs, requireOption, validateGroupId } from '../args';
import { getReadonlyDb } from '../db';
import { formatAmount } from '../format';
import { type Column, parseFormat, renderOutput } from '../output';

export function runCategories(args: string[]): void {
	const parsed = parseArgs(args);
	const subcommand = parsed.positional[0];

	if (subcommand && !['breakdown', 'median'].includes(subcommand)) {
		console.error('Usage: fin categories <breakdown|median> --group=GROUP [options]');
		process.exit(1);
	}

	if (!subcommand || subcommand === 'breakdown') {
		runBreakdown(subcommand ? args.slice(1) : args);
		return;
	}

	runMedian(args.slice(1));
}

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

function runBreakdown(args: string[]): void {
	const parsed = parseArgs(args);
	const format = parseFormat(getOption(parsed, 'format'));
	const groupId = requireOption(parsed, 'group', 'categories breakdown');
	const months = getOptionAsNumberOrDefault(parsed, 'months', 3);
	const limit = getOptionAsNumberOrDefault(parsed, 'limit', 10);

	validateGroupId(groupId, 'categories breakdown');

	const db = getReadonlyDb(parsed);
	const data = getGroupCategoryBreakdown(db, groupId, { months, limit });

	const rows: BreakdownRow[] = data.map((p) => ({
		category: p.category,
		total: p.totalMinor,
		count: p.transactionCount,
	}));

	const grandTotal = rows.reduce((sum, r) => sum + r.total, 0);
	const summaryText = `${rows.length} categories | Total: ${formatAmount(grandTotal)} (${months} months)`;

	renderOutput(rows, BREAKDOWN_COLUMNS, format, summaryText);
}

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

function runMedian(args: string[]): void {
	const parsed = parseArgs(args);
	const format = parseFormat(getOption(parsed, 'format'));
	const groupId = requireOption(parsed, 'group', 'categories median');
	const months = getOptionAsNumberOrDefault(parsed, 'months', 6);
	const limit = getOptionAsNumberOrDefault(parsed, 'limit', 10);

	validateGroupId(groupId, 'categories median');

	const db = getReadonlyDb(parsed);
	const data = getGroupCategoryMonthlyMedian(db, groupId, { months, limit });

	const rows: MedianRow[] = data.map((p) => ({
		category: p.category,
		median: p.monthlyMedianMinor,
		months: p.monthCount,
	}));

	const totalMedian = rows.reduce((sum, r) => sum + r.median, 0);
	const summaryText = `${rows.length} categories | Est. monthly: ${formatAmount(totalMedian)}`;

	renderOutput(rows, MEDIAN_COLUMNS, format, summaryText);
}
