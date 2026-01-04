/**
 * cashflow - Monthly cashflow summary.
 */

import { getGroupMonthlyCashflowSeries } from 'core';

import { getOption, getOptionAsNumberOrDefault, parseArgs, requireOption, validateGroupId } from '../args';
import { getReadonlyDb } from '../db';
import { formatAmount, formatCount, formatMonth, formatPercentRaw } from '../format';
import { type Column, parseFormat, renderOutput } from '../output';

type CashflowRow = {
	month: string;
	income: number;
	expenses: number;
	net: number;
	savingsRate: number | null;
};

const COLUMNS: Column<CashflowRow>[] = [
	{ key: 'month', label: 'Month', minWidth: 7, format: (v) => formatMonth(v as string) },
	{ key: 'income', label: 'Income', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'expenses', label: 'Expenses', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'net', label: 'Net', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'savingsRate', label: 'Savings%', align: 'right', minWidth: 8, format: (v) => formatPercentRaw(v as number | null) },
];

export function runCashflow(args: string[]): void {
	const parsed = parseArgs(args);
	const format = parseFormat(getOption(parsed, 'format'));
	const groupId = requireOption(parsed, 'group', 'cashflow');
	const months = getOptionAsNumberOrDefault(parsed, 'months', 12);
	const from = getOption(parsed, 'from');

	validateGroupId(groupId, 'cashflow');

	const db = getReadonlyDb(parsed);
	const options: { from?: string; limit: number } = { limit: months };
	if (from) options.from = from;
	const series = getGroupMonthlyCashflowSeries(db, groupId, options);

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

	renderOutput(rows, COLUMNS, format, summaryText);
}
