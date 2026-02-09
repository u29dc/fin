/**
 * `report reserves` -- Reserve breakdown (balance, tax reserve, expense reserve, available).
 *
 * Requires --group to specify which account group to analyze.
 */

import type { GroupId } from '@fin/core';
import { getGroupDailyReserveBreakdownSeries, isGroupId } from '@fin/core';
import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount, formatCount, formatDate } from '../../format';
import { type Column, parseFormat, renderOutput } from '../../output';
import { defineToolCommand } from '../../tool';

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

function buildReservesData(db: ReturnType<typeof getReadonlyDb>, groupId: GroupId, from: string | undefined, to: string | undefined): { rows: ReservesRow[]; latest: ReservesRow | null } {
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

	const latest = rows.length > 0 ? (rows[rows.length - 1] ?? null) : null;
	return { rows, latest };
}

export const reportReservesCommand = defineToolCommand(
	{
		name: 'report.reserves',
		command: 'fin report reserves',
		category: 'report',
		outputSchema: {
			series: { type: 'array', items: 'ReservesRow', description: 'Daily reserve breakdown data points' },
			latest: { type: 'object', description: 'Most recent reserve breakdown (or null)' },
		},
		idempotent: true,
		rateLimit: null,
		example: 'fin report reserves --group=business --json',
	},
	{
		meta: {
			name: 'reserves',
			description: 'Reserve breakdown (tax + expense reserves)',
		},
		args: {
			group: { type: 'string' as const, description: 'Group ID (personal, business, joint)', required: true as const },
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
				if (!args.group || !isGroupId(args.group)) {
					if (jsonMode) {
						fail('report.reserves', 'INVALID_GROUP', `Invalid or missing group: ${args.group ?? '(none)'}`, 'Use --group=personal, --group=business, or --group=joint', start);
					}
					process.stderr.write(`Error: Invalid or missing group: ${args.group ?? '(none)'}. Use: personal, business, joint\n`);
					process.exit(1);
				}

				const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const { rows, latest } = buildReservesData(db, args.group, args.from, args.to);

				if (jsonMode) {
					ok('report.reserves', { series: rows, latest }, start, { count: rows.length });
				}

				const format = parseFormat(args.format);
				const summaryText = latest ? `${formatCount(rows.length, 'day')} | Available: ${formatAmount(latest.available)}` : `${formatCount(rows.length, 'day')}`;
				renderOutput(rows, RESERVES_COLUMNS, format, summaryText);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('report.reserves', 'DB_ERROR', `Failed to query reserves: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
