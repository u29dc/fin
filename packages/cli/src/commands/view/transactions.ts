/**
 * `view transactions` -- Query transactions with filters.
 *
 * Returns transactions with support for --account, --group, --from, --to, --limit filters.
 */

import type { GroupId } from '@fin/core';
import { getGroupChartAccountIds, getTransactions, isGroupId } from '@fin/core';
import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount, formatCount, formatDate } from '../../format';
import { type Column, parseFormat, renderOutput } from '../../output';
import { defineToolCommand } from '../../tool';

type TransactionRow = {
	date: string;
	account: string;
	amount: number;
	description: string;
};

const TRANSACTION_COLUMNS: Column<TransactionRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'account', label: 'Account', minWidth: 24 },
	{ key: 'amount', label: 'Amount', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'description', label: 'Description', minWidth: 30, maxWidth: 40 },
];

function resolveChartAccountIds(account: string | undefined, group: string | undefined): string[] | undefined {
	if (group && isGroupId(group)) {
		return getGroupChartAccountIds(group as GroupId);
	}
	if (account) {
		return [account];
	}
	return undefined;
}

export const viewTransactionsCommand = defineToolCommand(
	{
		name: 'view.transactions',
		command: 'fin view transactions',
		category: 'view',
		outputSchema: {
			transactions: { type: 'array', items: 'TransactionRow', description: 'Transaction list' },
			count: { type: 'number', description: 'Number of transactions returned' },
		},
		idempotent: true,
		rateLimit: null,
		example: 'fin view transactions --group=personal --limit=20 --json',
	},
	{
		meta: {
			name: 'transactions',
			description: 'Query transactions with filters',
		},
		args: {
			account: { type: 'string' as const, description: 'Filter by chart account ID' },
			group: { type: 'string' as const, description: 'Filter by group (personal, business, joint)' },
			from: { type: 'string' as const, description: 'Start date (YYYY-MM-DD)' },
			to: { type: 'string' as const, description: 'End date (YYYY-MM-DD)' },
			limit: { type: 'string' as const, description: 'Max results', default: '50' },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
			format: { type: 'string' as const, description: 'Output format: table, json, tsv', default: 'table' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const limit = Number.parseInt(args.limit ?? '50', 10);
				const chartAccountIds = resolveChartAccountIds(args.account, args.group);

				const options: { chartAccountIds?: string[]; from?: string; to?: string; limit: number } = { limit };
				if (chartAccountIds) options.chartAccountIds = chartAccountIds;
				if (args.from) options.from = args.from;
				if (args.to) options.to = args.to;

				const txns = getTransactions(db, options);

				const rows: TransactionRow[] = txns.map((t) => ({
					date: t.postedAt,
					account: t.chartAccountId,
					amount: t.amountMinor,
					description: t.cleanDescription || t.rawDescription,
				}));

				if (jsonMode) {
					ok('view.transactions', { transactions: rows, count: rows.length }, start, { count: rows.length });
				}

				const format = parseFormat(args.format);
				const summaryText = formatCount(rows.length, 'transaction');
				renderOutput(rows, TRANSACTION_COLUMNS, format, summaryText);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('view.transactions', 'DB_ERROR', `Failed to query transactions: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
