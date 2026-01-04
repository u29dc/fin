/**
 * runway - Months of cash remaining.
 */

import { getGroupDailyRunwaySeries } from 'core';

import { getOption, parseArgs, requireOption, validateGroupId } from '../args';
import { getReadonlyDb } from '../db';
import { formatDate, formatMonths } from '../format';
import { type Column, parseFormat, renderOutput } from '../output';

type RunwayRow = {
	date: string;
	runway: number;
};

const COLUMNS: Column<RunwayRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'runway', label: 'Runway', align: 'right', minWidth: 10, format: (v) => formatMonths(v as number) },
];

export function runRunway(args: string[]): void {
	const parsed = parseArgs(args);
	const format = parseFormat(getOption(parsed, 'format'));
	const groupId = requireOption(parsed, 'group', 'runway');
	const from = getOption(parsed, 'from');
	const to = getOption(parsed, 'to');

	validateGroupId(groupId, 'runway');

	const db = getReadonlyDb(parsed);
	const options: { from?: string; to?: string } = {};
	if (from) options.from = from;
	if (to) options.to = to;
	const series = getGroupDailyRunwaySeries(db, groupId, options);

	const rows: RunwayRow[] = series.map((p) => ({
		date: p.date,
		runway: p.runwayMonths,
	}));

	// Get latest runway value
	const latest = rows.length > 0 ? rows[rows.length - 1] : null;
	const summaryText = latest ? `${rows.length} days | Latest: ${formatMonths(latest.runway)}` : `${rows.length} days`;

	renderOutput(rows, COLUMNS, format, summaryText);
}
