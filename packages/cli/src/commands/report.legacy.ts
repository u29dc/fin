/**
 * report.legacy -- Legacy report subcommands not yet migrated to defineToolCommand().
 *
 * These are imported by report/index.ts alongside migrated commands.
 * Each command will be moved to its own file in report/ as it is migrated.
 */

import { getExpenseAccountPayeeBreakdown, isGroupId } from '@fin/core';
import { getAccountIdsByGroup } from '@fin/core/config';
import { defineCommand } from 'citty';

import { getReadonlyDb } from '../db';
import { formatAmount, formatDate } from '../format';
import { error } from '../logger';
import { type Column, parseFormat, renderOutput } from '../output';
import { summary } from './summary';

// Shared args for all report commands
const formatArg = {
	type: 'string' as const,
	description: 'Output format: table, json, tsv',
	default: 'table',
};

const dbArg = {
	type: 'string' as const,
	description: 'Database path',
};

// ============================================================================
// audit
// ============================================================================

type AuditRow = {
	payee: string;
	total: number;
	monthlyAvg: number;
	count: number;
	account: string;
	lastDate: string;
};

const AUDIT_COLUMNS: Column<AuditRow>[] = [
	{ key: 'payee', label: 'Payee', minWidth: 24 },
	{ key: 'total', label: 'Total', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'monthlyAvg', label: 'Monthly Avg', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'count', label: 'Count', align: 'right', minWidth: 6, format: (v) => String(v) },
	{ key: 'account', label: 'Account', minWidth: 20 },
	{ key: 'lastDate', label: 'Last Date', minWidth: 10, format: (v) => formatDate(v as string) },
];

export const audit = defineCommand({
	meta: { name: 'audit', description: 'Expense account payee breakdown' },
	args: {
		account: { type: 'string', description: 'Expense account prefix (e.g. Expenses:Business:Uncategorized)', required: true },
		months: { type: 'string', description: 'Number of months', default: '12' },
		group: { type: 'string', description: 'Filter by group (personal, business, joint)' },
		format: formatArg,
		db: dbArg,
	},
	run({ args }) {
		if (!args.account) {
			error('Missing required option: --account\nUsage: fin report audit --account=Expenses:Business:Uncategorized');
			process.exit(1);
		}

		const format = parseFormat(args.format);
		const months = Number.parseInt(args.months ?? '12', 10);
		const dbPath = args.db;

		const db = getReadonlyDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);

		const options: { months: number; chartAccountIds?: string[] } = { months };
		if (args.group) {
			if (!isGroupId(args.group)) {
				error(`Invalid group: ${args.group}. Use: personal, business, joint`);
				process.exit(1);
			}
			options.chartAccountIds = getAccountIdsByGroup(args.group);
		}

		const data = getExpenseAccountPayeeBreakdown(db, args.account, options);

		const rows: AuditRow[] = data.map((p) => ({
			payee: p.payee,
			total: p.totalMinor,
			monthlyAvg: p.monthlyAvgMinor,
			count: p.transactionCount,
			account: p.sampleAccount,
			lastDate: p.lastDate,
		}));

		const grandTotal = rows.reduce((sum, r) => sum + r.total, 0);
		const summaryText = `${rows.length} payees | Total: ${formatAmount(grandTotal)} (${months} months) | ${args.account}`;

		renderOutput(rows, AUDIT_COLUMNS, format, summaryText);
	},
});

// Re-export summary for use by report/index.ts
export { summary };
