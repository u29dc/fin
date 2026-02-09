/**
 * `report cashflow` -- Monthly cashflow summary with income, expenses, net, and savings rate.
 *
 * Supports --expenses flag to switch to pure expenses only (excludes transfers, dividends, round-ups).
 * Requires --group to specify which account group to analyze.
 */

import type { GroupId } from '@fin/core';
import { getGroupMonthlyCashflowSeries, getGroupPureMonthlyCashflowSeries, isGroupId } from '@fin/core';
import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount, formatCount, formatMonth, formatPercentRaw } from '../../format';
import { type Column, parseFormat, renderOutput } from '../../output';
import { defineToolCommand } from '../../tool';

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

function buildCashflowData(
	db: ReturnType<typeof getReadonlyDb>,
	groupId: GroupId,
	months: number,
	from: string | undefined,
	pureExpenses: boolean,
): { rows: CashflowRow[]; totals: { income: number; expenses: number; net: number } } {
	const options: { from?: string; limit: number } = { limit: months };
	if (from) options.from = from;

	const series = pureExpenses ? getGroupPureMonthlyCashflowSeries(db, groupId, options) : getGroupMonthlyCashflowSeries(db, groupId, options);

	const rows: CashflowRow[] = series.map((p) => ({
		month: p.month,
		income: p.incomeMinor,
		expenses: p.expenseMinor,
		net: p.netMinor,
		savingsRate: p.savingsRatePct,
	}));

	const totalIncome = rows.reduce((sum, r) => sum + r.income, 0);
	const totalExpenses = rows.reduce((sum, r) => sum + r.expenses, 0);
	const totalNet = totalIncome - totalExpenses;

	return { rows, totals: { income: totalIncome, expenses: totalExpenses, net: totalNet } };
}

export const reportCashflowCommand = defineToolCommand(
	{
		name: 'report.cashflow',
		command: 'fin report cashflow',
		category: 'report',
		outputFields: ['series', 'totals'],
		idempotent: true,
		rateLimit: null,
		example: 'fin report cashflow --group=personal --months=12 --json',
	},
	{
		meta: {
			name: 'cashflow',
			description: 'Monthly cashflow summary with income, expenses, net, and savings rate',
		},
		args: {
			group: { type: 'string' as const, description: 'Group ID (personal, business, joint)', required: true as const },
			months: { type: 'string' as const, description: 'Number of months', default: '12' },
			from: { type: 'string' as const, description: 'Start month (YYYY-MM)' },
			expenses: { type: 'boolean' as const, description: 'Show true expenses only (exclude transfers, dividends, round-ups)', default: false },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
			format: { type: 'string' as const, description: 'Output format: table, json, tsv', default: 'table' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				if (!args.group || !isGroupId(args.group)) {
					if (jsonMode) {
						fail('report.cashflow', 'INVALID_GROUP', `Invalid or missing group: ${args.group ?? '(none)'}`, 'Use --group=personal, --group=business, or --group=joint', start);
					}
					process.stderr.write(`Error: Invalid or missing group: ${args.group ?? '(none)'}. Use: personal, business, joint\n`);
					process.exit(1);
				}

				const months = Number.parseInt(args.months ?? '12', 10);
				const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const { rows, totals } = buildCashflowData(db, args.group, months, args.from, args.expenses);

				if (jsonMode) {
					ok('report.cashflow', { series: rows, totals }, start, { count: rows.length });
				}

				const format = parseFormat(args.format);
				const modeLabel = args.expenses ? ' (pure expenses)' : '';
				const summaryText = `${formatCount(rows.length, 'month')}${modeLabel} | Net: ${formatAmount(totals.net)}`;
				renderOutput(rows, CASHFLOW_COLUMNS, format, summaryText);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('report.cashflow', 'DB_ERROR', `Failed to query cashflow: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
