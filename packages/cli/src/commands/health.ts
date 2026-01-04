/**
 * health - Financial health metrics (balance - reserves).
 */

import { getGroupDailyHealthSeries } from 'core';

import { getOption, parseArgs, requireOption, validateGroupId } from '../args';
import { getReadonlyDb } from '../db';
import { formatAmount, formatDate } from '../format';
import { type Column, parseFormat, renderOutput } from '../output';

type HealthRow = {
	date: string;
	health: number;
};

const COLUMNS: Column<HealthRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'health', label: 'Health', align: 'right', minWidth: 14, format: (v) => formatAmount(v as number) },
];

export function runHealth(args: string[]): void {
	const parsed = parseArgs(args);
	const format = parseFormat(getOption(parsed, 'format'));
	const groupId = requireOption(parsed, 'group', 'health');
	const from = getOption(parsed, 'from');
	const to = getOption(parsed, 'to');

	validateGroupId(groupId, 'health');

	const db = getReadonlyDb(parsed);
	const options: { from?: string; to?: string } = {};
	if (from) options.from = from;
	if (to) options.to = to;
	const series = getGroupDailyHealthSeries(db, groupId, options);

	const rows: HealthRow[] = series.map((p) => ({
		date: p.date,
		health: p.healthMinor,
	}));

	// Get latest health value
	const latest = rows.length > 0 ? rows[rows.length - 1] : null;
	const summaryText = latest ? `${rows.length} days | Latest: ${formatAmount(latest.health)}` : `${rows.length} days`;

	renderOutput(rows, COLUMNS, format, summaryText);
}
