/**
 * balance-sheet - Display double-entry balance sheet.
 */

import { getBalanceSheet } from 'core';

import { getOption, parseArgs } from '../args';
import { getReadonlyDb } from '../db';
import { formatAmount } from '../format';
import { type Column, parseFormat, renderOutput } from '../output';

type BalanceRow = {
	category: string;
	amount: number;
};

const COLUMNS: Column<BalanceRow>[] = [
	{ key: 'category', label: 'Category', minWidth: 20 },
	{ key: 'amount', label: 'Amount', align: 'right', minWidth: 15, format: (v) => formatAmount(v as number) },
];

export function runBalanceSheet(args: string[]): void {
	const parsed = parseArgs(args);
	const format = parseFormat(getOption(parsed, 'format'));
	const asOf = getOption(parsed, 'as-of');

	const db = getReadonlyDb(parsed);
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

	renderOutput(outputRows, COLUMNS, format);
}
