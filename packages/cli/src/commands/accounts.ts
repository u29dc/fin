/**
 * accounts - List accounts with current balances.
 */

import { type AssetAccountId, getAssetAccounts, getGroupChartAccountIds, getLatestBalances, isGroupId } from 'core';

import { getOption, parseArgs, validateGroupId } from '../args';
import { getReadonlyDb } from '../db';
import { formatAmount, formatCount, formatDate } from '../format';
import { type Column, parseFormat, renderOutput } from '../output';

type AccountRow = {
	id: string;
	name: string;
	type: string;
	balance: number | null;
	updated: string | null;
};

const COLUMNS: Column<AccountRow>[] = [
	{ key: 'id', label: 'Account', minWidth: 24 },
	{ key: 'name', label: 'Name', minWidth: 12 },
	{ key: 'type', label: 'Type', minWidth: 8 },
	{ key: 'balance', label: 'Balance', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number | null) },
	{ key: 'updated', label: 'Updated', minWidth: 10, format: (v) => formatDate(v as string | null) },
];

export function runAccounts(args: string[]): void {
	const parsed = parseArgs(args);
	const format = parseFormat(getOption(parsed, 'format'));
	const groupFilter = getOption(parsed, 'group');

	validateGroupId(groupFilter, 'accounts');

	const db = getReadonlyDb(parsed);
	const accounts = getAssetAccounts(db);
	const chartAccountIds = accounts.map((a) => a.id as AssetAccountId);
	const balances = getLatestBalances(db, chartAccountIds);

	// Filter by group if specified
	let filtered = accounts;
	if (groupFilter && isGroupId(groupFilter)) {
		const groupIds = new Set(getGroupChartAccountIds(groupFilter) as string[]);
		filtered = accounts.filter((a) => groupIds.has(a.id));
	}

	// Build output rows
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

	// Calculate total
	const total = rows.reduce((sum, r) => sum + (r.balance ?? 0), 0);
	const summaryText = `${formatCount(rows.length, 'account')} | Total: ${formatAmount(total)}`;

	renderOutput(rows, COLUMNS, format, summaryText);
}
