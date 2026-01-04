/**
 * reserves - Reserve breakdown (tax + expense reserves).
 */

import { getGroupDailyReserveBreakdownSeries } from 'core';

import { getOption, parseArgs, requireOption, validateGroupId } from '../args';
import { getReadonlyDb } from '../db';
import { formatAmount, formatDate } from '../format';
import { type Column, parseFormat, renderOutput } from '../output';

type ReservesRow = {
	date: string;
	balance: number;
	taxReserve: number;
	expenseReserve: number;
	available: number;
};

const COLUMNS: Column<ReservesRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'balance', label: 'Balance', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'taxReserve', label: 'Tax Rsv', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'expenseReserve', label: 'Exp Rsv', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'available', label: 'Available', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
];

export function runReserves(args: string[]): void {
	const parsed = parseArgs(args);
	const format = parseFormat(getOption(parsed, 'format'));
	const groupId = requireOption(parsed, 'group', 'reserves');
	const from = getOption(parsed, 'from');
	const to = getOption(parsed, 'to');

	validateGroupId(groupId, 'reserves');

	const db = getReadonlyDb(parsed);
	const options: { from?: string; to?: string } = {};
	if (from) options.from = from;
	if (to) options.to = to;
	const series = getGroupDailyReserveBreakdownSeries(db, groupId, options);

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

	renderOutput(rows, COLUMNS, format, summaryText);
}
