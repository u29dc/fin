/**
 * `report health` -- Financial health metrics (balance minus reserves) over time.
 *
 * Requires --group to specify which account group to analyze.
 * Note: "health" here refers to financial health, not system health (see `fin health`).
 */

import type { GroupId } from '@fin/core';
import { getGroupDailyHealthSeries, isGroupId } from '@fin/core';
import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount, formatCount, formatDate } from '../../format';
import { type Column, parseFormat, renderOutput } from '../../output';
import { defineToolCommand } from '../../tool';

type HealthRow = {
	date: string;
	health: number;
};

const HEALTH_COLUMNS: Column<HealthRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'health', label: 'Health', align: 'right', minWidth: 14, format: (v) => formatAmount(v as number) },
];

function buildHealthData(db: ReturnType<typeof getReadonlyDb>, groupId: GroupId, from: string | undefined, to: string | undefined): { rows: HealthRow[]; latest: HealthRow | null } {
	const options: { from?: string; to?: string } = {};
	if (from) options.from = from;
	if (to) options.to = to;
	const series = getGroupDailyHealthSeries(db, groupId, options);

	const rows: HealthRow[] = series.map((p) => ({
		date: p.date,
		health: p.healthMinor,
	}));

	const latest = rows.length > 0 ? (rows[rows.length - 1] ?? null) : null;
	return { rows, latest };
}

export const reportHealthCommand = defineToolCommand(
	{
		name: 'report.health',
		command: 'fin report health',
		category: 'report',
		outputFields: ['series', 'latest'],
		idempotent: true,
		rateLimit: null,
		example: 'fin report health --group=personal --json',
	},
	{
		meta: {
			name: 'health',
			description: 'Financial health metrics (balance - reserves)',
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
						fail('report.health', 'INVALID_GROUP', `Invalid or missing group: ${args.group ?? '(none)'}`, 'Use --group=personal, --group=business, or --group=joint', start);
					}
					process.stderr.write(`Error: Invalid or missing group: ${args.group ?? '(none)'}. Use: personal, business, joint\n`);
					process.exit(1);
				}

				const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const { rows, latest } = buildHealthData(db, args.group, args.from, args.to);

				if (jsonMode) {
					ok('report.health', { series: rows, latest }, start, { count: rows.length });
				}

				const format = parseFormat(args.format);
				const summaryText = latest ? `${formatCount(rows.length, 'day')} | Latest: ${formatAmount(latest.health)}` : `${formatCount(rows.length, 'day')}`;
				renderOutput(rows, HEALTH_COLUMNS, format, summaryText);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('report.health', 'DB_ERROR', `Failed to query health: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
