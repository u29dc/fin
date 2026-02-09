/**
 * `view accounts` -- List accounts with current balances.
 *
 * Returns asset accounts with their latest balance and update date.
 * Supports --group filter to narrow by group (personal, business, joint).
 */

import type { AssetAccountId, GroupId } from '@fin/core';
import { getAssetAccounts, getGroupChartAccountIds, getLatestBalances, isGroupId } from '@fin/core';
import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount, formatCount, formatDate } from '../../format';
import { type Column, parseFormat, renderOutput } from '../../output';
import { defineToolCommand } from '../../tool';

type AccountRow = {
	id: string;
	name: string;
	type: string;
	balance: number | null;
	updated: string | null;
};

const ACCOUNT_COLUMNS: Column<AccountRow>[] = [
	{ key: 'id', label: 'Account', minWidth: 24 },
	{ key: 'name', label: 'Name', minWidth: 12 },
	{ key: 'type', label: 'Type', minWidth: 8 },
	{ key: 'balance', label: 'Balance', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number | null) },
	{ key: 'updated', label: 'Updated', minWidth: 10, format: (v) => formatDate(v as string | null) },
];

function buildRows(db: ReturnType<typeof getReadonlyDb>, groupFilter: string | undefined): { rows: AccountRow[]; total: number } {
	const accountList = getAssetAccounts(db);
	const chartAccountIds = accountList.map((a) => a.id as AssetAccountId);
	const balances = getLatestBalances(db, chartAccountIds);

	let filtered = accountList;
	if (groupFilter && isGroupId(groupFilter)) {
		const groupIds = new Set(getGroupChartAccountIds(groupFilter as GroupId) as string[]);
		filtered = accountList.filter((a) => groupIds.has(a.id));
	}

	const balanceMap = new Map(balances.map((b) => [b.chartAccountId, b]));
	const rows: AccountRow[] = filtered.map((a) => {
		const bal = balanceMap.get(a.id as AssetAccountId);
		return {
			id: a.id,
			name: a.name,
			type: a.accountType,
			balance: bal?.balanceMinor ?? null,
			updated: bal?.date ?? null,
		};
	});

	const total = rows.reduce((sum, r) => sum + (r.balance ?? 0), 0);
	return { rows, total };
}

export const viewAccountsCommand = defineToolCommand(
	{
		name: 'view.accounts',
		command: 'fin view accounts',
		category: 'view',
		outputFields: ['accounts', 'total'],
		idempotent: true,
		rateLimit: null,
		example: 'fin view accounts --group=personal --json',
	},
	{
		meta: {
			name: 'accounts',
			description: 'List accounts with current balances',
		},
		args: {
			group: { type: 'string' as const, description: 'Filter by group (personal, business, joint)' },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
			format: { type: 'string' as const, description: 'Output format: table, json, tsv', default: 'table' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const { rows, total } = buildRows(db, args.group);

				if (jsonMode) {
					ok('view.accounts', { accounts: rows, total }, start, { count: rows.length });
				}

				const format = parseFormat(args.format);
				const summaryText = `${formatCount(rows.length, 'account')} | Total: ${formatAmount(total)}`;
				renderOutput(rows, ACCOUNT_COLUMNS, format, summaryText);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('view.accounts', 'DB_ERROR', `Failed to query accounts: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
