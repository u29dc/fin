/**
 * `report audit` -- Expense account payee breakdown.
 *
 * Shows payees for a specific expense account prefix, aggregated over a time period.
 * Optionally scoped to a single group's asset accounts.
 */

import type { GroupId } from '@fin/core';
import { getExpenseAccountPayeeBreakdown, isGroupId } from '@fin/core';
import { getAccountIdsByGroup } from '@fin/core/config';
import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount, formatDate } from '../../format';
import { type Column, parseFormat, renderOutput } from '../../output';
import { defineToolCommand } from '../../tool';

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

function validateAccountArg(account: string | undefined, jsonMode: boolean, start: number): asserts account is string {
	if (!account) {
		if (jsonMode) {
			fail('report.audit', 'INVALID_INPUT', 'Missing required option: --account', 'Provide an expense account prefix like Expenses:Business:Uncategorized', start);
		}
		process.stderr.write('Error: Missing required option: --account\nUsage: fin report audit --account=Expenses:Business:Uncategorized\n');
		process.exit(1);
	}
}

function validateGroupArg(group: string, jsonMode: boolean, start: number): asserts group is GroupId {
	if (!isGroupId(group)) {
		if (jsonMode) {
			fail('report.audit', 'INVALID_GROUP', `Invalid group: ${group}`, 'Use --group=personal, --group=business, or --group=joint', start);
		}
		process.stderr.write(`Error: Invalid group: ${group}. Use: personal, business, joint\n`);
		process.exit(1);
	}
}

function buildAuditData(
	db: ReturnType<typeof getReadonlyDb>,
	account: string,
	months: number,
	group: string | undefined,
	jsonMode: boolean,
	start: number,
): { payees: AuditRow[]; grandTotal: number } {
	const options: { months: number; chartAccountIds?: string[] } = { months };
	if (group) {
		validateGroupArg(group, jsonMode, start);
		options.chartAccountIds = getAccountIdsByGroup(group);
	}

	const data = getExpenseAccountPayeeBreakdown(db, account, options);

	const payees: AuditRow[] = data.map((p) => ({
		payee: p.payee,
		total: p.totalMinor,
		monthlyAvg: p.monthlyAvgMinor,
		count: p.transactionCount,
		account: p.sampleAccount,
		lastDate: p.lastDate,
	}));

	const grandTotal = payees.reduce((sum, r) => sum + r.total, 0);
	return { payees, grandTotal };
}

export const reportAuditCommand = defineToolCommand(
	{
		name: 'report.audit',
		command: 'fin report audit',
		category: 'report',
		outputFields: ['payees', 'total'],
		idempotent: true,
		rateLimit: null,
		example: 'fin report audit --account=Expenses:Business:Uncategorized --months=12 --json',
	},
	{
		meta: {
			name: 'audit',
			description: 'Expense account payee breakdown',
		},
		args: {
			account: { type: 'string' as const, description: 'Expense account prefix (e.g., Expenses:Business:Uncategorized)', required: true as const },
			months: { type: 'string' as const, description: 'Number of months to analyze', default: '12' },
			group: { type: 'string' as const, description: 'Filter by group (personal, business, joint)' },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
			format: { type: 'string' as const, description: 'Output format: table, json, tsv', default: 'table' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				validateAccountArg(args.account, jsonMode, start);

				const months = Number.parseInt(args.months ?? '12', 10);
				const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const { payees, grandTotal } = buildAuditData(db, args.account, months, args.group, jsonMode, start);

				if (jsonMode) {
					ok('report.audit', { payees, total: grandTotal }, start, { count: payees.length });
				}

				const format = parseFormat(args.format);
				const summaryText = `${payees.length} payees | Total: ${formatAmount(grandTotal)} (${months} months) | ${args.account}`;
				renderOutput(payees, AUDIT_COLUMNS, format, summaryText);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('report.audit', 'DB_ERROR', `Failed to query audit data: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
